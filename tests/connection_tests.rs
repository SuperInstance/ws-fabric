//! WebSocket connection lifecycle tests
//!
//! Tests the complete WebSocket connection lifecycle including:
//! - Handshake and connection establishment
//! - Connection lifecycle management
//! - Automatic reconnection
//! - Graceful shutdown
//! - Connection state transitions

use websocket_fabric::{WebSocketClient, WebSocketServer, CloseCode, CloseFrame};
use tokio::time::{timeout, Duration};

/// Helper to setup a test server and client connection
async fn setup_connection() -> anyhow::Result<(WebSocketServer, WebSocketClient)> {
    let server = WebSocketServer::new("127.0.0.1:0").await?;
    let addr = server.local_addr()?;
    server.spawn().await?;

    let client = WebSocketClient::connect(&format!("ws://{}", addr)).await?;

    Ok((server, client))
}

#[tokio::test]
async fn test_websocket_handshake() {
    // Test successful WebSocket handshake
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .expect("Connection should succeed");

    assert!(client.is_connected(), "Client should be connected after handshake");
}

#[tokio::test]
async fn test_websocket_handshake_failure() {
    // Test connection failure to non-existent server
    let result = WebSocketClient::connect("ws://127.0.0.1:55555")
        .await;

    assert!(result.is_err(), "Should fail to connect to non-existent server");
}

#[tokio::test]
async fn test_connection_lifecycle() {
    // Test: connect → ping/pong → close
    let (server, mut client) = setup_connection().await.unwrap();

    // Send ping
    client.ping().await.expect("Ping should succeed");

    // Wait for pong
    let pong = timeout(Duration::from_secs(1), client.recv_pong())
        .await
        .expect("Pong should arrive within timeout")
        .expect("Should receive pong");

    assert!(pong.is_some(), "Pong should be received");

    // Close connection
    client
        .close(CloseFrame {
            code: CloseCode::Normal,
            reason: "Normal closure".into(),
        })
        .await
        .expect("Close should succeed");

    assert!(!client.is_connected(), "Client should be disconnected");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_reconnection() {
    // Test automatic reconnection on connection loss
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .auto_reconnect(true)
        .max_reconnect_attempts(5)
        .build()
        .await
        .unwrap();

    assert!(client.is_connected(), "Should be connected initially");

    // Simulate server restart
    server.shutdown().await.unwrap();
    drop(server);

    // Server should restart on same port
    let server = WebSocketServer::new(addr).await.unwrap();
    server.spawn().await.unwrap();

    // Client should reconnect automatically
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        client.is_connected(),
        "Client should reconnect automatically"
    );

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_graceful_shutdown() {
    // Test graceful shutdown with close frame propagation
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    // Spawn task to receive close frame
    let client_task = tokio::spawn(async move {
        let close_frame = client.recv_close().await.unwrap();
        close_frame
    });

    // Gracefully shutdown server
    server.shutdown_gracefully().await.unwrap();

    // Client should receive close frame
    let close_frame = timeout(Duration::from_secs(1), client_task)
        .await
        .expect("Close frame should arrive")
        .unwrap();

    assert_eq!(close_frame.code, CloseCode::Normal);
    assert_eq!(close_frame.reason, "Server shutdown");
}

#[tokio::test]
async fn test_connection_timeout() {
    // Test connection timeout
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    // Don't start server, connection should timeout
    let result = timeout(
        Duration::from_millis(100),
        WebSocketClient::connect(&format!("ws://{}", addr)),
    )
    .await;

    assert!(result.is_err(), "Connection should timeout");
}

#[tokio::test]
async fn test_concurrent_connections() {
    // Test multiple concurrent connections to same server
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut handles = vec![];

    // Connect 100 clients concurrently
    for i in 0..100 {
        let url = format!("ws://{}", addr);
        let handle = tokio::spawn(async move {
            let client = WebSocketClient::connect(&url).await.unwrap();
            client.send_text(&format!("client {}", i)).await.unwrap();
            client.close(CloseFrame::normal()).await.unwrap();
        });
        handles.push(handle);
    }

    // All connections should succeed
    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(server.active_connections(), 0, "All clients should disconnect");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_connection_state_tracking() {
    // Test connection state transitions
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    assert_eq!(client.state(), websocket_fabric::ConnectionState::Connected);

    client
        .close(CloseFrame::normal())
        .await
        .unwrap();

    assert_eq!(client.state(), websocket_fabric::ConnectionState::Closed);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_ping_interval() {
    // Test periodic ping/pong for connection health
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .ping_interval(Duration::from_millis(100))
        .build()
        .await
        .unwrap();

    // Wait for several pings
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Connection should still be healthy
    assert!(client.is_connected());

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_close_with_reason() {
    // Test close frame with custom reason
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    let close_frame = CloseFrame {
        code: CloseCode::GoingAway,
        reason: "Server maintenance".into(),
    };

    client.close(close_frame.clone()).await.unwrap();

    server.shutdown().await.unwrap();
}
