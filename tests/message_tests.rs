//! WebSocket message handling tests
//!
//! Tests all aspects of WebSocket message handling:
//! - Text and binary messages
//! - Fragmented messages
//! - Ping/pong frames
//! - Message ordering
//! - Message size limits

use websocket_fabric::{WebSocketClient, WebSocketServer, CloseFrame};
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
async fn test_text_message() {
    // Test sending and receiving text messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send text message from client
    client
        .send_text("Hello, WebSocket!")
        .await
        .expect("Send should succeed");

    // Server receives
    let msg = timeout(Duration::from_secs(1), server.recv())
        .await
        .expect("Should receive message within timeout")
        .expect("Should receive message");

    assert_eq!(msg.as_text().unwrap(), "Hello, WebSocket!");

    // Server responds
    server
        .send_text("Hello back!")
        .await
        .expect("Server send should succeed");

    // Client receives response
    let response = timeout(Duration::from_secs(1), client.recv())
        .await
        .expect("Should receive response within timeout")
        .expect("Should receive response");

    assert_eq!(response.as_text().unwrap(), "Hello back!");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_binary_message() {
    // Test sending and receiving binary messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send binary message
    let data = vec![0u8; 1024];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }

    client
        .send_binary(&data)
        .await
        .expect("Send should succeed");

    // Server receives
    let msg = timeout(Duration::from_secs(1), server.recv())
        .await
        .expect("Should receive message")
        .expect("Should receive message");

    assert_eq!(msg.as_binary().unwrap(), &data);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_large_binary_message() {
    // Test handling of large binary messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send 1MB binary message
    let data = vec![0xABu8; 1024 * 1024];

    client
        .send_binary(&data)
        .await
        .expect("Send should succeed");

    // Server receives
    let msg = timeout(Duration::from_secs(5), server.recv())
        .await
        .expect("Should receive large message")
        .expect("Should receive message");

    assert_eq!(msg.as_binary().unwrap().len(), 1024 * 1024);
    assert_eq!(msg.as_binary().unwrap(), &data);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_fragmented_message() {
    // Test sending fragmented messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send fragmented message
    client
        .send_fragment_start("Hello, ")
        .await
        .expect("Fragment start should succeed");

    client
        .send_fragment_continue("Fragmented ")
        .await
        .expect("Fragment continue should succeed");

    client
        .send_fragment_end("WebSocket!")
        .await
        .expect("Fragment end should succeed");

    // Server receives reassembled message
    let msg = timeout(Duration::from_secs(1), server.recv())
        .await
        .expect("Should receive fragmented message")
        .expect("Should receive message");

    assert_eq!(msg.as_text().unwrap(), "Hello, Fragmented WebSocket!");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_ping_pong() {
    // Test ping/pong frames
    let (server, mut client) = setup_connection().await.unwrap();

    // Send ping
    client.ping().await.expect("Ping should succeed");

    // Receive pong
    let pong = timeout(Duration::from_secs(1), client.recv_pong())
        .await
        .expect("Should receive pong within timeout")
        .expect("Should receive pong");

    assert!(pong.is_some(), "Pong should be received");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_ping_with_payload() {
    // Test ping with custom payload
    let (server, mut client) = setup_connection().await.unwrap();

    let payload = b"custom ping data";

    client
        .ping_with_payload(payload)
        .await
        .expect("Ping should succeed");

    // Receive pong with same payload
    let pong = timeout(Duration::from_secs(1), client.recv_pong())
        .await
        .expect("Should receive pong")
        .expect("Should receive pong");

    assert_eq!(pong.unwrap().as_ref(), payload);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_ordering() {
    // Test that messages are received in order
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send multiple messages
    for i in 0..100 {
        client
            .send_text(&format!("message {}", i))
            .await
            .expect("Send should succeed");
    }

    // Receive in order
    for i in 0..100 {
        let msg = timeout(Duration::from_secs(1), server.recv())
            .await
            .expect("Should receive message")
            .expect("Should receive message");

        assert_eq!(msg.as_text().unwrap(), &format!("message {}", i));
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_send_receive() {
    // Test concurrent send and receive operations
    let (mut server, mut client) = setup_connection().await.unwrap();

    let server_task = tokio::spawn(async move {
        for i in 0..100 {
            server
                .send_text(&format!("server {}", i))
                .await
                .expect("Server send should succeed");

            let msg = timeout(Duration::from_secs(1), server.recv())
                .await
                .expect("Should receive client message")
                .expect("Should receive message");

            assert_eq!(msg.as_text().unwrap(), &format!("client {}", i));
        }
    });

    let client_task = tokio::spawn(async move {
        for i in 0..100 {
            client
                .send_text(&format!("client {}", i))
                .await
                .expect("Client send should succeed");

            let msg = timeout(Duration::from_secs(1), client.recv())
                .await
                .expect("Should receive server message")
                .expect("Should receive message");

            assert_eq!(msg.as_text().unwrap(), &format!("server {}", i));
        }
    });

    // Both should complete successfully
    tokio::try_join!(server_task, client_task).unwrap();

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_size_limit() {
    // Test message size limit enforcement
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    server.set_max_message_size(1024); // 1KB limit

    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    // Send message within limit
    client
        .send_text(&"a".repeat(512))
        .await
        .expect("Should send message within limit");

    // Send message exceeding limit
    let result = client.send_text(&"a".repeat(2048)).await;

    assert!(result.is_err(), "Should reject message exceeding size limit");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_utf8_validation() {
    // Test UTF-8 validation for text messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send valid UTF-8
    client
        .send_text("Valid UTF-8: 你好世界")
        .await
        .expect("Should send valid UTF-8");

    let msg = server.recv().await.unwrap().unwrap();
    assert_eq!(msg.as_text().unwrap(), "Valid UTF-8: 你好世界");

    // Send invalid UTF-8 (should fail or be handled as binary)
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let result = client.send_text_bytes(&invalid_utf8).await;

    assert!(result.is_err(), "Should reject invalid UTF-8");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_empty_message() {
    // Test handling of empty messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send empty text message
    client.send_text("").await.expect("Should send empty message");

    let msg = server.recv().await.unwrap().unwrap();
    assert_eq!(msg.as_text().unwrap(), "");

    // Send empty binary message
    client
        .send_binary(&[])
        .await
        .expect("Should send empty binary");

    let msg = server.recv().await.unwrap().unwrap();
    assert_eq!(msg.as_binary().unwrap(), &[]);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_compression() {
    // Test message compression (permessage-deflate)
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    server.enable_compression(true);

    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .compression(true)
        .build()
        .await
        .unwrap();

    // Send compressible data
    let data = "Hello, World! ".repeat(1000);

    client
        .send_text(&data)
        .await
        .expect("Should send compressed message");

    let msg = server.recv().await.unwrap().unwrap();
    assert_eq!(msg.as_text().unwrap(), &data);

    server.shutdown().await.unwrap();
}
