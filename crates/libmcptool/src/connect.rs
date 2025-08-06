use clap::Parser;
use rustyline::{DefaultEditor, error::ReadlineError};
use tenx_mcp::{ClientConn, ClientCtx, Result as McpResult, schema::ServerNotification};
use tokio::sync::mpsc;

use std::sync::mpsc as std_mpsc;

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

    // Channel for user input from blocking thread
    let (input_tx, mut input_rx) =
        mpsc::unbounded_channel::<std::result::Result<String, ReadlineError>>();
    // Channel to signal when the prompt should be shown again
    let (prompt_tx, prompt_rx) = std_mpsc::channel::<()>();

    // Spawn blocking thread to handle readline with history support
    std::thread::spawn({
        let input_tx = input_tx.clone();
        let prompt_rx = prompt_rx;
        move || {
            let mut rl = DefaultEditor::new().expect("Failed to create readline editor");
            loop {
                match rl.readline("mcp> ") {
                    Ok(line) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        rl.add_history_entry(line.clone()).ok();
                        if input_tx.send(Ok(line)).is_err() {
                            break;
                        }
                        // wait for main thread to signal before showing the prompt again
                        if prompt_rx.recv().is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = input_tx.send(Err(err));
                        break;
                    }
                }
            }
        }
    });

    // Drop the extra sender so channel closes when thread exits
    drop(input_tx);

    loop {
        tokio::select! {
            // Handle incoming notifications
            notification = notification_receiver.recv() => {
                if let Some(notification) = notification {
                    display_notification(&ctx.output, &notification)?;
                }
            }
            // Handle user input from blocking thread
            user_input = input_rx.recv() => {
                match user_input {
                    Some(Ok(line)) => {
                        match line.as_str() {
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
                        // signal the input thread to show the prompt again
                        let _ = prompt_tx.send(());
                    }
                    Some(Err(ReadlineError::Interrupted)) => {
                        ctx.output.text("CTRL-C")?;
                        break;
                    }
                    Some(Err(ReadlineError::Eof)) => {
                        ctx.output.text("CTRL-D")?;
                        break;
                    }
                    Some(Err(err)) => {
                        ctx.output.trace_error(format!("Error: {err:?}"))?;
                        break;
                    }
                    None => {
                        // Channel closed
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
