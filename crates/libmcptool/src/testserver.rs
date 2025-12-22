use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};
use tmcp::{
    Error, Result, Server, ServerConn, ServerCtx,
    schema::{
        ClientCapabilities, ClientNotification, Cursor, Implementation, InitializeResult,
        ListPromptsResult, ListResourcesResult, ListToolsResult, LoggingLevel, ProgressToken,
        Prompt, PromptArgument, ReadResourceResult, Resource, ServerCapabilities,
        ServerNotification, Tool, ToolSchema,
    },
};

use crate::{ctx::Ctx, output::Output};

/// Sample user data structure for demonstrating JSON resource serving
#[derive(Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    email: String,
    role: String,
    last_login: String,
}

/// Response structure for the users resource
#[derive(Serialize, Deserialize)]
struct UsersResponse {
    users: Vec<User>,
    total_count: usize,
    generated_at: String,
}

/// Information about a connected client
#[derive(Clone, Debug)]
struct ClientInfo {
    remote_addr: String,
    client_name: String,
    client_version: String,
    connected_at: Instant,
}

/// Shared state for the test server that can be accessed by both connections and the REPL
#[derive(Clone)]
struct TestServerState {
    request_counter: Arc<AtomicU64>,
    output: Output,
    log_level: Arc<Mutex<LoggingLevel>>,
    connected_clients: Arc<Mutex<HashMap<String, ClientInfo>>>,
    active_contexts: Arc<Mutex<HashMap<String, ServerCtx>>>,
}

impl TestServerState {
    fn new(output: Output, request_counter: Arc<AtomicU64>) -> Self {
        Self {
            request_counter,
            output,
            log_level: Arc::new(Mutex::new(LoggingLevel::Error)),
            connected_clients: Arc::new(Mutex::new(HashMap::new())),
            active_contexts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn add_client(
        &self,
        context: &ServerCtx,
        remote_addr: String,
        client_info: Implementation,
    ) {
        let client_id = format!(
            "{}_{}",
            remote_addr,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let info = ClientInfo {
            remote_addr: remote_addr.clone(),
            client_name: client_info.name,
            client_version: client_info.version,
            connected_at: Instant::now(),
        };

        self.connected_clients
            .lock()
            .unwrap()
            .insert(client_id.clone(), info);
        self.active_contexts
            .lock()
            .unwrap()
            .insert(client_id, context.clone());

        let _ = self
            .output
            .trace_success(format!("client connected from {}", remote_addr));
    }

    async fn broadcast_notification(&self, notification: ServerNotification) -> Result<()> {
        let contexts = self.active_contexts.lock().unwrap();

        for context in contexts.values() {
            context.notify(notification.clone())?;
        }

        Ok(())
    }

    fn get_client_count(&self) -> usize {
        self.connected_clients.lock().unwrap().len()
    }

    fn get_client_list(&self) -> Vec<ClientInfo> {
        self.connected_clients
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }
}

/// A test server connection that logs all interactions verbosely
#[derive(Clone)]
struct TestServerConn {
    state: TestServerState,
}

impl TestServerConn {
    fn new(state: TestServerState) -> Self {
        Self { state }
    }

    async fn send_notification(
        &self,
        context: &ServerCtx,
        notification: ServerNotification,
    ) -> Result<()> {
        let _ = self.state.output.h1("sending notification");
        let _ = self.state.output.text(format!(
            "content: {}",
            serde_json::to_string_pretty(&notification).unwrap()
        ));

        context.notify(notification)
    }

    async fn send_log_message(
        &self,
        context: &ServerCtx,
        level: LoggingLevel,
        message: String,
    ) -> Result<()> {
        let current_level = *self.state.log_level.lock().unwrap();

        // Check if message should be sent based on current log level
        if self.should_log_message(&level, &current_level) {
            let notification = ServerNotification::LoggingMessage {
                level,
                logger: Some("testserver".to_string()),
                data: serde_json::json!({ "message": message }),
            };
            self.send_notification(context, notification).await
        } else {
            Ok(())
        }
    }

    fn should_log_message(
        &self,
        message_level: &LoggingLevel,
        current_level: &LoggingLevel,
    ) -> bool {
        message_level >= current_level
    }
}

#[async_trait::async_trait]
impl ServerConn for TestServerConn {
    async fn on_connect(&self, _context: &ServerCtx, remote_addr: &str) -> Result<()> {
        // Note: We'll add the client in the initialize method when we have more info
        let _ = self
            .state
            .output
            .trace_success(format!("client connecting from {remote_addr}"));
        Ok(())
    }

    async fn on_shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn initialize(
        &self,
        context: &ServerCtx,
        protocol_version: String,
        capabilities: ClientCapabilities,
        client_info: Implementation,
    ) -> Result<InitializeResult> {
        self.state.request_counter.fetch_add(1, Ordering::Relaxed);
        let _ = self.state.output.h1("initialize");
        let params = serde_json::json!({
            "protocol_version": protocol_version,
            "capabilities": capabilities,
            "client_info": client_info,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        // Add client to shared state with context info
        // Note: We don't have direct access to remote_addr from context, so we'll use a placeholder
        self.state
            .add_client(context, "client_connection".to_string(), client_info);

        let result = InitializeResult::new("mcptool-testserver")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_tools(true)
            .with_prompts(true)
            .with_resources(true, true)
            .with_instructions("mcptool test server");

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn pong(&self, _context: &ServerCtx) -> Result<()> {
        let _ = self.state.output.h1("pong");
        let _ = self.state.output.text("parameters: {}");
        let _ = self.state.output.text("result: pong");
        Ok(())
    }

    async fn list_tools(
        &self,
        _context: &ServerCtx,
        cursor: Option<Cursor>,
    ) -> Result<ListToolsResult> {
        let _ = self.state.output.h1("list_tools");
        let params = serde_json::json!({
            "cursor": cursor,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        let echo_tool = Tool::new(
            "echo",
            ToolSchema::default()
                .with_property(
                    "message",
                    serde_json::json!({
                        "type": "string",
                        "description": "The message to echo back"
                    }),
                )
                .with_required("message"),
        )
        .with_description("Echoes back the provided message");

        let result = ListToolsResult::default().with_tool(echo_tool);

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn call_tool(
        &self,
        context: &ServerCtx,
        name: String,
        arguments: Option<tmcp::Arguments>,
    ) -> Result<tmcp::schema::CallToolResult> {
        let _ = self.state.output.h1("call_tool");
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        // Send notification about tool call
        self.send_log_message(
            context,
            LoggingLevel::Debug,
            format!("Tool '{}' called with arguments: {:?}", name, arguments),
        )
        .await?;

        if name != "echo" {
            self.send_log_message(
                context,
                LoggingLevel::Error,
                format!("Unknown tool requested: {}", name),
            )
            .await?;
            return Err(Error::ToolNotFound(format!("Unknown tool: {name}")));
        }

        let message = arguments
            .as_ref()
            .and_then(|args| args.get_string("message"))
            .unwrap_or_else(|| "No message provided".to_string());

        let result =
            tmcp::schema::CallToolResult::new().with_text_content(format!("Echo: {message}"));

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        // Send notification about successful tool execution
        self.send_log_message(
            context,
            LoggingLevel::Info,
            format!("Successfully executed tool '{}'", name),
        )
        .await?;

        Ok(result)
    }

    async fn notification(
        &self,
        context: &ServerCtx,
        notification: ClientNotification,
    ) -> Result<()> {
        let _ = self.state.output.h1("notification");
        let _ = self.state.output.text(format!(
            "content: {}",
            serde_json::to_string_pretty(&notification).unwrap()
        ));

        // Send a demo notification back to the client
        self.send_log_message(
            context,
            LoggingLevel::Info,
            format!("Received client notification: {:?}", notification),
        )
        .await?;

        Ok(())
    }

    async fn set_level(&self, context: &ServerCtx, level: LoggingLevel) -> Result<()> {
        let _ = self.state.output.h1("set_level");
        let _ = self.state.output.text(format!(
            "level: {}",
            serde_json::to_string_pretty(&level).unwrap()
        ));

        // Update the log level
        *self.state.log_level.lock().unwrap() = level;

        // Acknowledge the level change
        self.send_log_message(
            context,
            LoggingLevel::Info,
            format!("Log level changed to: {:?}", level),
        )
        .await?;

        // Send some demo messages at different levels to demonstrate filtering
        self.send_log_message(
            context,
            LoggingLevel::Debug,
            "This is a debug message".to_string(),
        )
        .await?;

        self.send_log_message(
            context,
            LoggingLevel::Info,
            "This is an info message".to_string(),
        )
        .await?;

        self.send_log_message(
            context,
            LoggingLevel::Warning,
            "This is a warning message".to_string(),
        )
        .await?;

        self.send_log_message(
            context,
            LoggingLevel::Error,
            "This is an error message".to_string(),
        )
        .await?;

        Ok(())
    }

    async fn list_prompts(
        &self,
        _context: &ServerCtx,
        cursor: Option<Cursor>,
    ) -> Result<ListPromptsResult> {
        let _ = self.state.output.h1("list_prompts");
        let params = serde_json::json!({
            "cursor": cursor,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        let greeting_prompt = Prompt {
            name: "greeting".to_string(),
            title: None,
            description: Some("Generate a greeting message".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "name".to_string(),
                    title: None,
                    description: Some("The name to greet".to_string()),
                    required: Some(true),
                },
                PromptArgument {
                    name: "style".to_string(),
                    title: None,
                    description: Some("The greeting style (formal/casual)".to_string()),
                    required: Some(false),
                },
            ]),
            _meta: None,
        };

        let code_review_prompt = Prompt {
            name: "code_review".to_string(),
            title: None,
            description: Some("Review code and provide feedback".to_string()),
            arguments: Some(vec![
                PromptArgument {
                    name: "language".to_string(),
                    title: None,
                    description: Some("Programming language of the code".to_string()),
                    required: Some(true),
                },
                PromptArgument {
                    name: "code".to_string(),
                    title: None,
                    description: Some("The code to review".to_string()),
                    required: Some(true),
                },
            ]),
            _meta: None,
        };

        let result = ListPromptsResult::default()
            .with_prompt(greeting_prompt)
            .with_prompt(code_review_prompt);

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn get_prompt(
        &self,
        _context: &ServerCtx,
        name: String,
        arguments: Option<tmcp::Arguments>,
    ) -> Result<tmcp::schema::GetPromptResult> {
        let _ = self.state.output.h1("get_prompt");
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        let result = match name.as_str() {
            "greeting" => {
                let name = arguments
                    .as_ref()
                    .and_then(|args| args.get_string("name"))
                    .unwrap_or_else(|| "World".to_string());
                let style = arguments
                    .as_ref()
                    .and_then(|args| args.get_string("style"))
                    .unwrap_or_else(|| "casual".to_string());

                let message = match style.as_str() {
                    "formal" => format!("Good day, {name}. How may I assist you today?"),
                    _ => format!("Hey {name}! What's up?"),
                };

                tmcp::schema::GetPromptResult::new()
                    .with_description("A personalized greeting")
                    .with_message(tmcp::schema::PromptMessage::user_text(message))
            }
            "code_review" => {
                let language = arguments
                    .as_ref()
                    .and_then(|args| args.get_string("language"))
                    .unwrap_or_else(|| "unknown".to_string());
                let code = arguments
                    .as_ref()
                    .and_then(|args| args.get_string("code"))
                    .unwrap_or_default();

                let review = format!(
                    "Please review the following {language} code:\n\n```{language}\n{code}\n```\n\nProvide feedback on code quality, potential bugs, and improvements."
                );

                tmcp::schema::GetPromptResult::new()
                    .with_description("Code review request")
                    .with_message(tmcp::schema::PromptMessage::user_text(review))
            }
            _ => return Err(Error::MethodNotFound(format!("Unknown prompt: {name}"))),
        };

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn list_resources(
        &self,
        _context: &ServerCtx,
        cursor: Option<Cursor>,
    ) -> Result<ListResourcesResult> {
        let _ = self.state.output.h1("list_resources");
        let params = serde_json::json!({
            "cursor": cursor,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        let log_resource = Resource::new("server-log", "log://testserver/current")
            .with_description("Current test server log")
            .with_mime_type("text/plain");

        let sample_data_resource = Resource::new("sample-data", "data://testserver/users.json")
            .with_description("Sample user data for testing")
            .with_mime_type("application/json")
            .with_size(512);

        let metrics_resource = Resource::new("server-metrics", "metrics://testserver/stats")
            .with_description("Server performance metrics")
            .with_mime_type("text/plain");

        let result = ListResourcesResult::default()
            .with_resource(log_resource)
            .with_resource(sample_data_resource)
            .with_resource(metrics_resource);

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn read_resource(&self, context: &ServerCtx, uri: String) -> Result<ReadResourceResult> {
        let _ = self.state.output.h1("read_resource");
        let params = serde_json::json!({
            "uri": uri,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        // Send notification about resource access
        self.send_log_message(
            context,
            LoggingLevel::Debug,
            format!("Resource accessed: {}", uri),
        )
        .await?;

        let result = match uri.as_str() {
            "log://testserver/current" => {
                let log_content = format!(
                    "Test Server Log\n===============\n\n{} - Server started\n{} - Listening for connections\n{} - Processing requests...",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                );
                ReadResourceResult::new().with_text(uri, log_content)
            }
            "data://testserver/users.json" => {
                // Sample user data that demonstrates JSON resource serving
                let users = vec![
                    User {
                        id: 1,
                        name: "Alice Johnson".to_string(),
                        email: "alice@example.com".to_string(),
                        role: "admin".to_string(),
                        last_login: "2024-01-15T10:30:00Z".to_string(),
                    },
                    User {
                        id: 2,
                        name: "Bob Smith".to_string(),
                        email: "bob@example.com".to_string(),
                        role: "user".to_string(),
                        last_login: "2024-01-14T15:45:00Z".to_string(),
                    },
                    User {
                        id: 3,
                        name: "Charlie Davis".to_string(),
                        email: "charlie@example.com".to_string(),
                        role: "moderator".to_string(),
                        last_login: "2024-01-13T09:00:00Z".to_string(),
                    },
                ];

                let response = UsersResponse {
                    total_count: users.len(),
                    users,
                    generated_at: chrono::Local::now().to_rfc3339(),
                };

                ReadResourceResult::new().with_json(uri, &response).unwrap()
            }
            "metrics://testserver/stats" => {
                let metrics = format!(
                    "Server Metrics\n==============\n\nTotal requests processed: {}\nUptime: N/A\nMemory usage: N/A\nActive connections: 1",
                    self.state.request_counter.load(Ordering::Relaxed)
                );
                ReadResourceResult::new().with_text(uri, metrics)
            }
            _ => return Err(Error::ResourceNotFound { uri }),
        };

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        _context: &ServerCtx,
        cursor: Option<Cursor>,
    ) -> Result<tmcp::schema::ListResourceTemplatesResult> {
        let _ = self.state.output.h1("list_resource_templates");
        let params = serde_json::json!({
            "cursor": cursor,
        });
        let _ = self.state.output.text(format!(
            "parameters: {}",
            serde_json::to_string_pretty(&params).unwrap()
        ));

        // Create sample resource templates
        let user_template = tmcp::schema::ResourceTemplate::new(
            "user-profile",
            "user://testserver/{user_id}/profile",
        )
        .with_title("User Profile")
        .with_description("Access user profile information by ID")
        .with_mime_type("application/json");

        let log_template =
            tmcp::schema::ResourceTemplate::new("dated-log", "log://testserver/{date}/entries")
                .with_title("Daily Log Entries")
                .with_description("Server log entries for a specific date (YYYY-MM-DD)")
                .with_mime_type("text/plain")
                .with_annotations(
                    tmcp::schema::Annotations::new()
                        .with_priority(0.8)
                        .with_audience(vec![tmcp::schema::Role::Assistant]),
                );

        let config_template = tmcp::schema::ResourceTemplate::new(
            "config-section",
            "config://testserver/{section}/{key}",
        )
        .with_title("Configuration Values")
        .with_description("Access configuration values by section and key")
        .with_mime_type("text/plain");

        let metrics_template = tmcp::schema::ResourceTemplate::new(
            "metric-history",
            "metrics://testserver/{metric_name}/history?period={period}",
        )
        .with_title("Metric History")
        .with_description("Historical data for a specific metric (period: 1h, 1d, 1w)")
        .with_mime_type("application/json")
        .with_annotations(
            tmcp::schema::Annotations::new()
                .with_priority(0.5)
                .with_last_modified(chrono::Local::now().to_rfc3339()),
        );

        let result = tmcp::schema::ListResourceTemplatesResult::default()
            .with_resource_template(user_template)
            .with_resource_template(log_template)
            .with_resource_template(config_template)
            .with_resource_template(metrics_template);

        let _ = self.state.output.text(format!(
            "result: {}",
            serde_json::to_string_pretty(&result).unwrap()
        ));

        Ok(result)
    }
}

/// Run the interactive REPL for server management
async fn run_interactive_repl(
    ctx: &Ctx,
    server_address: String,
    server_state: TestServerState,
) -> crate::Result<()> {
    // Run the entire REPL in a blocking task to avoid blocking the tokio executor
    // TODO Use mpsc to pass stuff around
    let ctx_clone = ctx.clone();
    tokio::task::spawn_blocking(move || {
        run_interactive_repl_blocking(&ctx_clone, server_address, server_state)
    })
    .await
    .map_err(|e| crate::Error::Internal(e.to_string()))??;

    Ok(())
}

/// Blocking version of the interactive REPL
fn run_interactive_repl_blocking(
    ctx: &Ctx,
    server_address: String,
    server_state: TestServerState,
) -> crate::Result<()> {
    let mut rl = DefaultEditor::new()?;
    let rt_handle = tokio::runtime::Handle::current();

    ctx.output.text("Interactive testserver console started")?;
    ctx.output
        .text("Type 'help' for available commands, 'quit' to exit")?;
    ctx.output
        .text("Use server-side commands to manage connected clients")?;
    ctx.output.text("")?;

    loop {
        let client_count = server_state.get_client_count();
        let prompt = format!("testserver[{}]> ", client_count);

        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "quit" | "exit" => {
                        ctx.output.text("Goodbye!")?;
                        break;
                    }
                    "help" => {
                        ctx.output.h1("Available commands")?;
                        ctx.output.text("Server Management:")?;
                        ctx.output.text("  status                     - Show server status and connected clients")?;
                        ctx.output
                            .text("  clients                    - List all connected clients")?;
                        ctx.output.text("")?;
                        ctx.output.text("Notifications:")?;
                        ctx.output.text(
                            "  notify <level> <message>   - Send log notification to all clients",
                        )?;
                        ctx.output.text(
                            "                               Levels: debug, info, warning, error",
                        )?;
                        ctx.output.text(
                            "  progress <id> <progress>   - Send progress notification (0.0-1.0)",
                        )?;
                        ctx.output.text(
                            "  resource <uri>             - Send resource update notification",
                        )?;
                        ctx.output.text("")?;
                        ctx.output.text("Server Configuration:")?;
                        ctx.output
                            .text("  setlevel <level>           - Set global logging level")?;
                        ctx.output.text(
                            "                               Levels: debug, info, warning, error",
                        )?;
                        ctx.output.text("")?;
                        ctx.output.text("General:")?;
                        ctx.output
                            .text("  help                       - Show this help message")?;
                        ctx.output
                            .text("  quit/exit                  - Exit the interactive console")?;
                    }
                    "status" => {
                        ctx.output.h1("Server Status")?;
                        ctx.output
                            .text(format!("Server Address: {}", server_address))?;
                        ctx.output.text(format!(
                            "Total Requests: {}",
                            server_state.request_counter.load(Ordering::Relaxed)
                        ))?;
                        ctx.output
                            .text(format!("Connected Clients: {}", client_count))?;
                        let current_level = *server_state.log_level.lock().unwrap();
                        ctx.output
                            .text(format!("Current Log Level: {:?}", current_level))?;
                    }
                    "clients" => {
                        ctx.output.h1("Connected Clients")?;
                        let clients = server_state.get_client_list();
                        if clients.is_empty() {
                            ctx.output.text("No clients connected")?;
                        } else {
                            for (i, client) in clients.iter().enumerate() {
                                ctx.output.text(format!(
                                    "{}. {} v{} from {} (connected {}s ago)",
                                    i + 1,
                                    client.client_name,
                                    client.client_version,
                                    client.remote_addr,
                                    client.connected_at.elapsed().as_secs()
                                ))?;
                            }
                        }
                    }
                    "notify" => {
                        if parts.len() < 3 {
                            ctx.output.trace_error("Usage: notify <level> <message>")?;
                            continue;
                        }

                        let level = match parts[1] {
                            "debug" => LoggingLevel::Debug,
                            "info" => LoggingLevel::Info,
                            "warning" => LoggingLevel::Warning,
                            "error" => LoggingLevel::Error,
                            _ => {
                                ctx.output.trace_error(
                                    "Invalid level. Use: debug, info, warning, error",
                                )?;
                                continue;
                            }
                        };

                        let message = parts[2..].join(" ");
                        let notification = ServerNotification::LoggingMessage {
                            level,
                            logger: Some("testserver-repl".to_string()),
                            data: serde_json::json!({ "message": message }),
                        };

                        match rt_handle.block_on(server_state.broadcast_notification(notification))
                        {
                            Ok(_) => ctx
                                .output
                                .trace_success("Notification sent to all clients")?,
                            Err(e) => ctx
                                .output
                                .trace_error(format!("Failed to send notification: {}", e))?,
                        }
                    }
                    "progress" => {
                        if parts.len() < 3 {
                            ctx.output
                                .trace_error("Usage: progress <operation_id> <progress>")?;
                            continue;
                        }

                        let progress: f64 = match parts[2].parse() {
                            Ok(p) if (0.0..=1.0).contains(&p) => p,
                            _ => {
                                ctx.output
                                    .trace_error("Progress must be a number between 0.0 and 1.0")?;
                                continue;
                            }
                        };

                        let notification = ServerNotification::Progress {
                            progress_token: ProgressToken::String(parts[1].to_string()),
                            progress,
                            total: Some(1.0),
                            message: Some(format!(
                                "Operation {} progress: {:.1}%",
                                parts[1],
                                progress * 100.0
                            )),
                        };

                        match rt_handle.block_on(server_state.broadcast_notification(notification))
                        {
                            Ok(_) => ctx
                                .output
                                .trace_success("Progress notification sent to all clients")?,
                            Err(e) => ctx
                                .output
                                .trace_error(format!("Failed to send progress: {}", e))?,
                        }
                    }
                    "resource" => {
                        if parts.len() < 2 {
                            ctx.output.trace_error("Usage: resource <uri>")?;
                            continue;
                        }

                        let notification = ServerNotification::ResourceUpdated {
                            uri: parts[1].to_string(),
                        };

                        match rt_handle.block_on(server_state.broadcast_notification(notification))
                        {
                            Ok(_) => ctx.output.trace_success(
                                "Resource update notification sent to all clients",
                            )?,
                            Err(e) => ctx
                                .output
                                .trace_error(format!("Failed to send resource update: {}", e))?,
                        }
                    }
                    "setlevel" => {
                        if parts.len() < 2 {
                            ctx.output.trace_error("Usage: setlevel <level>")?;
                            continue;
                        }

                        let level = match parts[1] {
                            "debug" => LoggingLevel::Debug,
                            "info" => LoggingLevel::Info,
                            "warning" => LoggingLevel::Warning,
                            "error" => LoggingLevel::Error,
                            _ => {
                                ctx.output.trace_error(
                                    "Invalid level. Use: debug, info, warning, error",
                                )?;
                                continue;
                            }
                        };

                        *server_state.log_level.lock().unwrap() = level;
                        ctx.output
                            .trace_success(format!("Log level set to: {:?}", level))?;

                        // Notify all clients about the level change
                        let notification = ServerNotification::LoggingMessage {
                            level: LoggingLevel::Info,
                            logger: Some("testserver-repl".to_string()),
                            data: serde_json::json!({ "message": format!("Server log level changed to: {:?}", level) }),
                        };

                        let _ =
                            rt_handle.block_on(server_state.broadcast_notification(notification));
                    }
                    _ => {
                        ctx.output
                            .trace_error(format!("Unknown command: {}", parts[0]))?;
                        ctx.output.text("Type 'help' for available commands.")?;
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                ctx.output.text("CTRL-C")?;
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                ctx.output.text("CTRL-D")?;
                break;
            }
            Err(err) => {
                ctx.output.trace_error(format!("Error: {:?}", err))?;
                break;
            }
        }
    }

    Ok(())
}

/// Create a configured server instance with test connection handler
fn create_test_server(
    output: Output,
    request_counter: Arc<AtomicU64>,
) -> (
    Server<impl Fn() -> Box<dyn ServerConn> + Clone + Send + Sync + 'static>,
    TestServerState,
) {
    let state = TestServerState::new(output, request_counter);
    let state_for_conn = state.clone();

    let server = Server::default()
        .with_connection(move || TestServerConn::new(state_for_conn.clone()))
        .with_capabilities(
            ServerCapabilities::default()
                .with_tools(Some(true))
                .with_prompts(None)
                .with_resources(None, None),
        );

    (server, state)
}

/// Handle interactive mode for both TCP and HTTP servers
async fn handle_interactive_mode<F, Fut>(
    ctx: &Ctx,
    server_address: String,
    server_state: TestServerState,
    output: &Output,
    server_starter: F,
) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let _ = output.trace_success(format!("Listening on: {}", server_address));
    let _ = output.text("Starting interactive mode...");

    let ctx_clone = ctx.clone();
    let server_state_clone = server_state.clone();

    // Start server and REPL concurrently
    tokio::select! {
        result = server_starter() => {
            let _ = output.text("Closing tcp server...");
            result?;
        }
        // TODO Fix this
        // _ = tokio::signal::ctrl_c() => {
        //     let _ = output.trace_warn("Shutting down server...");
        //     result.stop().await?;
        // }
        resultb = run_interactive_repl(&ctx_clone, server_address, server_state_clone) => {
            resultb.map_err(|e| Error::InternalError(e.to_string()))?;
        }
    }

    Ok(())
}

/// Handle non-interactive mode for TCP server
async fn handle_tcp_non_interactive(
    server: Server<impl Fn() -> Box<dyn ServerConn> + Clone + Send + Sync + 'static>,
    addr: &str,
    output: &Output,
) -> Result<()> {
    let _ = output.text("Transport: TCP");
    let _ = output.trace_success(format!("Listening on: tcp://{}", addr));
    let _ = output.text("Press Ctrl+C to stop the server");
    server.serve_tcp(addr).await
}

pub async fn run_test_server(
    ctx: &Ctx,
    stdio: bool,
    tcp: bool,
    port: u16,
    interactive: bool,
) -> Result<()> {
    // Validate that only one transport is specified
    let transport_count = [stdio, tcp].iter().filter(|&&x| x).count();
    if transport_count > 1 {
        return Err(Error::InvalidConfiguration(
            "Only one transport can be specified: --stdio, --tcp, or HTTP (default)".to_string(),
        ));
    }

    // Interactive mode is incompatible with stdio transport
    if interactive && stdio {
        return Err(Error::InvalidConfiguration(
            "Interactive mode is not compatible with stdio transport".to_string(),
        ));
    }

    let output = if stdio {
        // In stdio mode, silence all output
        ctx.output.clone().with_quiet(true)
    } else {
        ctx.output.clone()
    };

    let _ = output.h1("mcptool testserver");
    let _ = output.text(format!("Version: {}", env!("CARGO_PKG_VERSION")));
    let _ = output.text(format!(
        "Protocol: {}",
        tmcp::schema::LATEST_PROTOCOL_VERSION
    ));

    // Create shared request counter for interactive mode
    let request_counter = Arc::new(AtomicU64::new(0));
    let (server, server_state) = create_test_server(output.clone(), request_counter.clone());

    if stdio {
        server.serve_stdio().await?;
    } else {
        let addr = format!("127.0.0.1:{port}");
        if interactive {
            handle_interactive_mode(
                ctx,
                format!("tcp://{addr}"),
                server_state,
                &output.clone(),
                || async move { server.serve_tcp(&addr).await },
            )
            .await?;
        } else {
            handle_tcp_non_interactive(server, &addr, &output).await?;
        }
    }

    Ok(())
}
