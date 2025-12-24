//! Integration tests for MCP initialization functionality.
#![allow(clippy::tests_outside_test_module)]

use std::{collections::HashMap, time::Duration};

use libmcptool::{client, ctx::Ctx, mcp, output::Output, target::Target};
use tempfile::TempDir;
use tmcp::{
    Result as McpResult, Server, ServerCtx, ServerHandler,
    schema::{
        ClientCapabilities, Implementation, InitializeResult, LATEST_PROTOCOL_VERSION,
        PromptsCapability, ResourcesCapability, ServerCapabilities, ToolsCapability,
    },
};
use tokio::{net::TcpListener, time::sleep};

/// Create a test context with a temporary config directory
fn create_test_ctx() -> (Ctx, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().to_path_buf();
    let ctx =
        Ctx::new(config_path, None, false, false, false, 80).expect("Failed to create context");
    (ctx, temp_dir)
}

// Create a simple test server connection that mirrors the testserver behavior
#[derive(Clone)]
struct SimpleTestConn;

#[async_trait::async_trait]
impl ServerHandler for SimpleTestConn {
    async fn initialize(
        &self,
        _context: &ServerCtx,
        _protocol_version: String,
        _capabilities: ClientCapabilities,
        _client_info: Implementation,
    ) -> McpResult<InitializeResult> {
        Ok(InitializeResult::new("mcptool-testserver")
            .with_version("0.1.0")
            .with_tools(true)
            .with_prompts(true)
            .with_resources(true, true)
            .with_instructions("mcptool test server"))
    }
}

#[tokio::test]
async fn test_mcp_init_with_test_server() {
    // Start the test server on a random port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to local address");
    let port = listener
        .local_addr()
        .expect("Failed to get local address")
        .port();
    drop(listener); // Release the port so test server can bind to it

    let (_ctx, _temp_dir) = create_test_ctx();

    // Create and start the server directly
    let server = Server::default()
        .with_handler(|| SimpleTestConn)
        .with_capabilities(
            ServerCapabilities::default()
                .with_tools(Some(true))
                .with_prompts(None)
                .with_resources(None, None),
        );

    let addr = format!("127.0.0.1:{port}");
    let server_handle = tokio::spawn(async move { server.serve_tcp(&addr).await });

    // Give the server time to start
    sleep(Duration::from_millis(100)).await;

    // Test with JSON output
    {
        let output = Output::new(false, 80).with_json(true);
        let target =
            Target::parse(&format!("tcp://127.0.0.1:{port}")).expect("Failed to parse target");

        let (_client, init_result) = client::connect_to_server(&target, ())
            .await
            .expect("Failed to connect to server");

        let result = mcp::init(&init_result, &output);
        assert!(result.is_ok(), "init should succeed with JSON output");

        // Verify basic fields
        assert_eq!(init_result.server_info.name, "mcptool-testserver");
        assert_eq!(init_result.server_info.version, "0.1.0");
        assert_eq!(init_result.protocol_version, LATEST_PROTOCOL_VERSION);
    }

    // Test with text output
    {
        let output = Output::new(false, 80).with_json(false);
        let target =
            Target::parse(&format!("tcp://127.0.0.1:{port}")).expect("Failed to parse target");

        let (_client, init_result) = client::connect_to_server(&target, ())
            .await
            .expect("Failed to connect to server");

        let result = mcp::init(&init_result, &output);
        assert!(result.is_ok(), "init should succeed with text output");
    }

    // Abort the server
    server_handle.abort();
}

#[tokio::test]
async fn test_mcp_init_output_format() {
    // This test verifies the init function handles both output modes correctly
    // We use a mock InitializeResult to avoid needing a real server

    let init_result = InitializeResult {
        protocol_version: "2025-06-18".to_string(),
        capabilities: ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            resources: Some(ResourcesCapability {
                subscribe: Some(true),
                list_changed: Some(false),
            }),
            prompts: Some(PromptsCapability {
                list_changed: Some(false),
            }),
            logging: Some(serde_json::Value::Object(serde_json::Map::new())),
            completions: Some(serde_json::Value::Object(serde_json::Map::new())),
            experimental: Some({
                let mut map = HashMap::new();
                map.insert(
                    "custom_feature".to_string(),
                    serde_json::json!({
                        "enabled": true,
                        "version": "1.0"
                    }),
                );
                map
            }),
        },
        server_info: Implementation {
            name: "Test Server".to_string(),
            version: "1.2.3".to_string(),
            title: Some("Test MCP Server".to_string()),
        },
        instructions: Some("Test instructions\nWith multiple lines".to_string()),
        _meta: None,
    };

    // Test JSON output
    {
        let output = Output::new(false, 80).with_json(true);
        let result = mcp::init(&init_result, &output);
        assert!(result.is_ok(), "init should succeed with JSON output");
    }

    // Test text output
    {
        let output = Output::new(false, 80).with_json(false);
        let result = mcp::init(&init_result, &output);
        assert!(result.is_ok(), "init should succeed with text output");
    }

    // Test with minimal server (no optional fields)
    let minimal_init_result = InitializeResult {
        protocol_version: "2025-06-18".to_string(),
        capabilities: ServerCapabilities::default(),
        server_info: Implementation {
            name: "Minimal".to_string(),
            version: "0.1.0".to_string(),
            title: None,
        },
        instructions: None,
        _meta: None,
    };

    {
        let output = Output::new(false, 80).with_json(false);
        let result = mcp::init(&minimal_init_result, &output);
        assert!(result.is_ok(), "init should succeed with minimal server");
    }
}
