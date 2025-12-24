//! Integration tests for TCP connectivity functionality.
#![allow(clippy::tests_outside_test_module)]

use std::io::ErrorKind;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::{Duration, sleep},
};

#[tokio::test]
async fn test_tcp_port_connectivity() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to local address");

    let addr = listener.local_addr().expect("Failed to get local address");

    let server_handle = tokio::spawn(async move {
        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        let mut buf = [0; 1024];
        let n = stream
            .read(&mut buf)
            .await
            .expect("Failed to read from stream");

        let received = String::from_utf8_lossy(&buf[..n]);
        assert_eq!(received, "Hello, server!");

        stream
            .write_all(b"Hello, client!")
            .await
            .expect("Failed to write to stream");
    });

    sleep(Duration::from_millis(100)).await;

    let mut client = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to server");

    client
        .write_all(b"Hello, server!")
        .await
        .expect("Failed to write to server");

    let mut buf = vec![0; 1024];
    let n = client
        .read(&mut buf)
        .await
        .expect("Failed to read from server");

    let response = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(response, "Hello, client!");

    server_handle.await.expect("Server task failed");
}

#[tokio::test]
async fn test_connection_refused() {
    // Use a random high port that's unlikely to be in use
    let result = TcpStream::connect("127.0.0.1:49999").await;

    assert!(result.is_err());

    let error = result.unwrap_err();
    // The error could be ConnectionRefused or AddrNotAvailable depending on the OS
    assert!(
        error.kind() == ErrorKind::ConnectionRefused || error.kind() == ErrorKind::AddrNotAvailable,
        "Expected ConnectionRefused or AddrNotAvailable, got {:?}",
        error.kind()
    );
}

#[tokio::test]
async fn test_connect_to_open_port() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to local address");

    let addr = listener.local_addr().expect("Failed to get local address");

    let _server_handle = tokio::spawn(async move {
        drop(listener.accept().await);
    });

    sleep(Duration::from_millis(50)).await;

    let result = TcpStream::connect(addr).await;
    assert!(result.is_ok(), "Should successfully connect to open port");
}

#[tokio::test]
async fn test_multiple_connections() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to local address");

    let addr = listener.local_addr().expect("Failed to get local address");

    let server_handle = tokio::spawn(async move {
        for _ in 0..3 {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("Failed to accept connection");

            tokio::spawn(async move {
                let mut buf = [0; 1024];
                drop(stream.read(&mut buf).await);
                drop(stream.write_all(b"OK").await);
            });
        }
    });

    for i in 0..3 {
        let mut client = TcpStream::connect(addr)
            .await
            .unwrap_or_else(|_| panic!("Failed to connect on attempt {i}"));

        client.write_all(b"test").await.expect("Failed to write");

        let mut buf = vec![0; 2];
        let n = client.read(&mut buf).await.expect("Failed to read");
        assert_eq!(&buf[..n], b"OK");
    }

    server_handle.await.expect("Server task failed");
}

#[tokio::test]
async fn test_verify_port_is_actually_open() {
    // This test specifically addresses the "connection refused" issue
    // by ensuring we can distinguish between open and closed ports

    // First, create a listener on a random port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to local address");

    let open_port_addr = listener.local_addr().expect("Failed to get local address");

    // Spawn a server that accepts connections
    let server_handle = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            // Echo back a simple message
            drop(stream.write_all(b"PORT_IS_OPEN").await);
        }
    });

    // Test 1: Connect to the open port - should succeed
    let mut client = TcpStream::connect(open_port_addr)
        .await
        .expect("Should successfully connect to open port");

    let mut buf = vec![0; 12];
    let n = client
        .read(&mut buf)
        .await
        .expect("Should read from open port");
    assert_eq!(&buf[..n], b"PORT_IS_OPEN");

    // Test 2: Try a definitely closed port - should fail
    let closed_port = if open_port_addr.port() < 64535 {
        open_port_addr.port() + 1000
    } else {
        open_port_addr.port() - 1000
    };
    let closed_port_addr = format!("127.0.0.1:{closed_port}");
    let result = TcpStream::connect(&closed_port_addr).await;
    assert!(result.is_err(), "Should fail to connect to closed port");

    drop(client);
    server_handle.abort();
}
