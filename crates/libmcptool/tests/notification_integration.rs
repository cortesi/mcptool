use std::time::Duration;

use libmcptool::{client, ctx::Ctx, target::Target};
use tmcp::{
    ClientConn, ClientCtx, Result as McpResult, Server, ServerAPI, ServerConn, ServerCtx,
    schema::{ClientNotification, LoggingLevel, ServerCapabilities, ServerNotification},
};
use tokio::{sync::mpsc, time::timeout};

// Simple test server connection for integration tests
#[derive(Clone)]
struct SimpleTestServerConn {
    client_notification_sender: mpsc::UnboundedSender<ClientNotification>,
}

#[derive(Clone)]
struct SimpleTestClientConn {
    server_notification_sender: mpsc::UnboundedSender<ServerNotification>,
}

#[async_trait::async_trait]
impl ServerConn for SimpleTestServerConn {
    async fn initialize(
        &self,
        _context: &ServerCtx,
        _protocol_version: String,
        _capabilities: tmcp::schema::ClientCapabilities,
        _client_info: tmcp::schema::Implementation,
    ) -> McpResult<tmcp::schema::InitializeResult> {
        Ok(tmcp::schema::InitializeResult::new("test-server").with_version("1.0.0"))
    }

    async fn set_level(&self, context: &ServerCtx, level: LoggingLevel) -> McpResult<()> {
        // Send notification
        let notification = ServerNotification::LoggingMessage {
            level,
            logger: Some("test-notification".to_string()),
            data: serde_json::json!({ "message": "test-notification-message" }),
        };
        let _ = context.notify(notification);
        Ok(())
    }

    async fn notification(
        &self,
        _context: &ServerCtx,
        notification: ClientNotification,
    ) -> McpResult<()> {
        let _ = self.client_notification_sender.send(notification);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClientConn for SimpleTestClientConn {
    async fn notification(
        &self,
        _context: &ClientCtx,
        notification: ServerNotification,
    ) -> McpResult<()> {
        let _ = self.server_notification_sender.send(notification);
        Ok(())
    }
}

#[tokio::test]
async fn test_set_level_command_notifications_via_tcp() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for the test
    let temp_dir = tempfile::tempdir()?;
    let config_path = temp_dir.path().to_path_buf();

    // Create context
    let ctx = Ctx::new(config_path.clone(), None, false, true, false, 80)?;

    // Create channels for capturing notifications
    let (client_notification_sender, mut client_notification_receiver) = mpsc::unbounded_channel();
    let (server_notification_sender, mut server_notification_receiver) = mpsc::unbounded_channel();

    // Get a random available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener); // Release the port so server can bind to it

    // Start simple test server
    let server = Server::default()
        .with_connection(move || SimpleTestServerConn {
            client_notification_sender: client_notification_sender.clone(),
        })
        .with_capabilities(ServerCapabilities::default().with_tools(Some(true)));

    let addr = format!("127.0.0.1:{}", port);
    let server_handle = tokio::spawn(async move {
        server.serve_tcp(&addr).await.unwrap();
    });

    // Wait a bit for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect to the testserver via TCP
    let target = Target::parse(&format!("tcp://127.0.0.1:{}", port))?;
    let (mut client, _init_result) = client::get_client_with_connection(
        &ctx,
        &target,
        SimpleTestClientConn {
            server_notification_sender: server_notification_sender.clone(),
        },
    )
    .await?;

    // Client -> Server notifications
    let notification = timeout(
        Duration::from_millis(10),
        client_notification_receiver.recv(),
    )
    .await;
    assert!(
        matches!(notification, Ok(Some(ClientNotification::Initialized))),
        "Expected an initialized notification from the client"
    );

    // Server -> Client notifications

    // Test set_level command - this should work and the server should handle it
    // The client should not hang given that a notification message is sent on each call
    client.set_level(LoggingLevel::Debug).await?;
    client.set_level(LoggingLevel::Info).await?;
    client.set_level(LoggingLevel::Warning).await?;
    client.set_level(LoggingLevel::Error).await?;

    assert_eq!(
        server_notification_receiver.len(),
        4,
        "Expected 4 server sent notifications for set_level commands"
    );
    for _ in 0..4 {
        let notification = timeout(
            Duration::from_millis(10),
            server_notification_receiver.recv(),
        )
        .await;
        assert!(
            matches!(
                notification,
                Ok(Some(ServerNotification::LoggingMessage { .. }))
            ),
            "Expected a logging message notification from the server"
        );
    }

    // Clean up
    server_handle.abort();

    Ok(())
}
