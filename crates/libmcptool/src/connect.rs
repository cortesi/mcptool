use clap::Parser;
use rustyline::DefaultEditor;
use tmcp::{ClientConn, ClientCtx, Result as McpResult, schema::ServerNotification};
use tokio::sync::mpsc;

use crate::{
    Result, client,
    command::{ReplCommandWrapper, execute_mcp_command_with_client, generate_repl_help},
    ctx::Ctx,
    output::initresult,
    target::Target,
};

#[derive(Clone)]
struct NotificationClientConn {
    notification_sender: mpsc::UnboundedSender<ServerNotification>,
}

#[async_trait::async_trait]
impl ClientConn for NotificationClientConn {
    async fn notification(
        &self,
        _context: &ClientCtx,
        notification: ServerNotification,
    ) -> McpResult<()> {
        let _ = self.notification_sender.send(notification);
        Ok(())
    }
}

pub async fn connect_command(ctx: &Ctx, target: String) -> Result<()> {
    let target = Target::parse(&target)?;

    ctx.output.text(format!("Connecting to {target}..."))?;

    // Create notification channel
    let (notification_sender, mut notification_receiver) = mpsc::unbounded_channel();

    // Create client connection with notification handling
    let conn = NotificationClientConn {
        notification_sender,
    };
    let (mut client, init_result) = client::get_client_with_connection(ctx, &target, conn).await?;

    ctx.output.trace_success(format!(
        "Connected to: {} v{}",
        init_result.server_info.name, init_result.server_info.version
    ))?;
    ctx.output
        .text("Type 'help' for available commands, 'quit' to exit\n")?;

    let mut rl = DefaultEditor::new()?;

    loop {
        tokio::select! {
            // Handle incoming notifications
            notification = notification_receiver.recv() => {
                if let Some(notification) = notification {
                    display_notification(&ctx.output, &notification)?;
                }
            }
            // Handle user input (in a non-blocking way)
            readline_result = tokio::task::spawn_blocking(|| {
                let mut rl = DefaultEditor::new().expect("Failed to create readline editor");
                rl.readline("mcp> ")
            }) => {
                match readline_result {
                    Ok(readline) => match readline {
                        Ok(line) => {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            rl.add_history_entry(line)?;

                            match line {
                                "quit" | "exit" => {
                                    ctx.output.text("Goodbye!")?;
                                    break;
                                }
                                "help" => {
                                    ctx.output.h1("Available commands")?;
                                    ctx.output.text(generate_repl_help())?;
                                }
                                "init" => {
                                    ctx.output.note("Showing initialization result from initial connection (not re-initializing)")?;
                                    initresult::init_result(&ctx.output, &init_result)?;
                                }
                                _ => {
                                    // Try to parse as an MCP command using clap
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    match ReplCommandWrapper::try_parse_from(parts) {
                                        Ok(wrapper) => {
                                            match execute_mcp_command_with_client(
                                                wrapper.command,
                                                &mut client,
                                                &init_result,
                                                ctx,
                                            )
                                            .await
                                            {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    ctx.output.trace_error(format!("Command failed: {e}"))?
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            ctx.output.trace_error(format!("Invalid command: {e}"))?;
                                            ctx.output.text("Type 'help' for available commands.")?;
                                        }
                                    }
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
                            ctx.output.trace_error(format!("Error: {err:?}"))?;
                            break;
                        }
                    }
                    Err(e) => {
                        ctx.output.trace_error(format!("Failed to read input: {e:?}"))?;
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn display_notification(
    output: &crate::output::Output,
    notification: &ServerNotification,
) -> Result<()> {
    match notification {
        ServerNotification::LoggingMessage {
            level,
            logger,
            data,
        } => {
            let logger_str = logger.as_deref().unwrap_or("server");
            output.text(format!(
                "[NOTIFICATION] {:?} [{}]: {}",
                level, logger_str, data
            ))?;
        }
        ServerNotification::ResourceUpdated { uri } => {
            output.text(format!("[NOTIFICATION] Resource updated: {}", uri))?;
        }
        ServerNotification::ResourceListChanged => {
            output.text("[NOTIFICATION] Resource list changed")?;
        }
        ServerNotification::ToolListChanged => {
            output.text("[NOTIFICATION] Tool list changed")?;
        }
        ServerNotification::PromptListChanged => {
            output.text("[NOTIFICATION] Prompt list changed")?;
        }
        ServerNotification::Cancelled { request_id, reason } => {
            let reason_str = reason.as_deref().unwrap_or("no reason given");
            output.text(format!(
                "[NOTIFICATION] Request cancelled: {:?} ({})",
                request_id, reason_str
            ))?;
        }
        ServerNotification::Progress {
            progress_token,
            progress,
            total,
            message,
        } => {
            let total_str = total.map(|t| format!("/{}", t)).unwrap_or_default();
            let message_str = message.as_deref().unwrap_or("");
            output.text(format!(
                "[NOTIFICATION] Progress {:?}: {}{} - {}",
                progress_token, progress, total_str, message_str
            ))?;
        }
    }
    Ok(())
}
