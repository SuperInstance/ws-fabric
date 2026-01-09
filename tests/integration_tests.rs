//! Integration tests for websocket-fabric with equilibrium-tokens
//!
//! Tests the integration between websocket-fabric and equilibrium-tokens,
//! ensuring real-time communication works correctly for the equilibrium system.

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
async fn test_equilibrium_integration() {
    // Test integration with equilibrium-tokens orchestrator

    // This would integrate with the actual equilibrium-tokens library
    // For now, we'll simulate the integration

    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    // Setup message handler for equilibrium-tokens messages
    let mut handler = EquilibriumMessageHandler::new();

    server
        .on_message(move |msg| {
            let handler = handler.clone();
            async move {
                // Process equilibrium-tokens message
                handler.handle(msg).await
            }
        })
        .await
        .unwrap();

    server.spawn().await.unwrap();

    // Connect client
    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    // Send equilibrium-tokens message
    let equilibrium_message = r#"{
        "type": "token_request",
        "token_id": "eq_123",
        "action": "mint"
    }"#;

    client
        .send_text(equilibrium_message)
        .await
        .expect("Send should succeed");

    // Receive response
    let response = timeout(Duration::from_secs(1), client.recv())
        .await
        .expect("Should receive response")
        .expect("Should receive response");

    assert!(response.is_text());

    let response_text = response.as_text().unwrap();
    assert!(response_text.contains("token_id"));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_backpressure_handling() {
    // Test backpressure when client sends faster than server can process
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Slow down server processing
    server.set_message_processor(async move |msg| {
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    });

    // Send messages faster than server can process
    let send_count = 0;
    for i in 0..1000 {
        match client.send_text(&format!("message {}", i)).await {
            Ok(_) => send_count += 1,
            Err(_) if send_count > 100 => break, // Backpressure applied
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    // Server should apply backpressure without crashing
    assert!(server.is_running());

    // Client should handle backpressure gracefully
    assert!(client.is_connected() || client.is_applying_backpressure());

    // Drain messages
    tokio::time::sleep(Duration::from_secs(2)).await;

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_batch_message_processing() {
    // Test batch processing of equilibrium-tokens messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Send batch of token operations
    let operations = vec![
        r#"{"type": "mint", "token_id": "eq_1", "amount": 100}"#,
        r#"{"type": "transfer", "token_id": "eq_1", "to": "user_2", "amount": 50}"#,
        r#"{"type": "burn", "token_id": "eq_1", "amount": 25}"#,
    ];

    for op in &operations {
        client.send_text(op).await.unwrap();
    }

    // Process batch
    let mut responses = vec![];
    for _ in 0..operations.len() {
        let msg = timeout(Duration::from_secs(1), server.recv())
            .await
            .expect("Should receive message")
            .expect("Should receive message");

        responses.push(msg.as_text().unwrap().to_string());
    }

    assert_eq!(responses.len(), operations.len());

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_real_time_token_updates() {
    // Test real-time token state updates via WebSocket
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    // Subscribe to token updates
    server
        .on_message(|msg| async move {
            if msg.as_text().unwrap().contains("subscribe") {
                Ok(Some(websocket_fabric::Message::text(
                    r#"{"type": "subscribed", "channel": "token_updates"}"#,
                )))
            } else {
                Ok(None)
            }
        })
        .await
        .unwrap();

    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    // Subscribe to updates
    client
        .send_text(r#"{"type": "subscribe", "channel": "token_updates"}"#)
        .await
        .unwrap();

    // Receive subscription confirmation
    let response = client.recv().await.unwrap().unwrap();
    assert!(response.as_text().unwrap().contains("subscribed"));

    // Simulate server broadcasting token update
    server
        .broadcast_text(
            r#"{"type": "token_update", "token_id": "eq_123", "price": 1.23}"#,
        )
        .await
        .unwrap();

    // Client should receive update
    let update = timeout(Duration::from_secs(1), client.recv())
        .await
        .expect("Should receive update")
        .expect("Should receive update");

    assert!(update.as_text().unwrap().contains("token_update"));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_multi_client_synchronization() {
    // Test multiple clients receiving synchronized updates
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    // Connect multiple clients
    let mut clients = vec![];
    for i in 0..10 {
        let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
            .await
            .unwrap();

        // Subscribe to updates
        client
            .send_text(&format!(
                r#"{{"type": "subscribe", "client": {}}}"#,
                i
            ))
            .await
            .unwrap();

        clients.push(client);
    }

    // Broadcast update to all clients
    server
        .broadcast_text(r#"{"type": "sync_update", "value": 42}"#)
        .await
        .unwrap();

    // All clients should receive the update
    for (i, client) in clients.iter_mut().enumerate() {
        let msg = timeout(Duration::from_secs(1), client.recv())
            .await
            .expect(&format!("Client {} should receive update", i))
            .expect("Should receive message");

        assert!(msg.as_text().unwrap().contains("sync_update"));
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_error_propagation() {
    // Test error propagation from equilibrium-tokens to client
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    server
        .on_message(|msg| async move {
            let text = msg.as_text().unwrap();
            if text.contains("invalid") {
                // Simulate equilibrium-tokens error
                Ok(Some(websocket_fabric::Message::text(
                    r#"{"type": "error", "code": 400, "message": "Invalid token operation"}"#,
                )))
            } else {
                Ok(None)
            }
        })
        .await
        .unwrap();

    server.spawn().await.unwrap();

    let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
        .await
        .unwrap();

    // Send invalid operation
    client
        .send_text(r#"{"type": "invalid_operation"}"#)
        .await
        .unwrap();

    // Should receive error response
    let error = timeout(Duration::from_secs(1), client.recv())
        .await
        .expect("Should receive error")
        .expect("Should receive error");

    assert!(error.as_text().unwrap().contains("error"));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_authentication_flow() {
    // Test WebSocket authentication for equilibrium-tokens
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    // Enable authentication
    server.set_authenticator(|token| async move {
        if token == "valid_token_123" {
            Ok(true)
        } else {
            Ok(false)
        }
    });

    server.spawn().await.unwrap();

    // Connect with invalid token
    let result = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .auth_token("invalid_token")
        .connect()
        .await;

    assert!(result.is_err(), "Should reject invalid token");

    // Connect with valid token
    let client = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .auth_token("valid_token_123")
        .connect()
        .await
        .unwrap();

    assert!(client.is_connected());

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_acknowledgment() {
    // Test message acknowledgment pattern for critical operations
    let (mut server, mut client) = setup_connection().await.unwrap();

    server
        .on_message(|msg| async move {
            let text = msg.as_text().unwrap();
            if text.contains("critical") {
                // Send acknowledgment
                Ok(Some(websocket_fabric::Message::text(
                    r#"{"type": "ack", "message_id": "123", "status": "confirmed"}"#,
                )))
            } else {
                Ok(None)
            }
        })
        .await
        .unwrap();

    // Send critical message requiring acknowledgment
    client
        .send_text(r#"{"type": "critical", "message_id": "123", "data": "..."}"#)
        .await
        .unwrap();

    // Should receive acknowledgment
    let ack = timeout(Duration::from_secs(1), client.recv())
        .await
        .expect("Should receive acknowledgment")
        .expect("Should receive acknowledgment");

    assert!(ack.as_text().unwrap().contains("ack"));
    assert!(ack.as_text().unwrap().contains("confirmed"));

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_reconnection_with_state_recovery() {
    // Test reconnection with state recovery
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut client = WebSocketClient::builder()
        .url(&format!("ws://{}", addr))
        .auto_reconnect(true)
        .state_recovery(true)
        .build()
        .await
        .unwrap();

    // Send some state
    client
        .send_text(r#"{"type": "set_state", "value": "persistent"}"#)
        .await
        .unwrap();

    // Disconnect and reconnect
    client.disconnect().await;
    server.shutdown().await.unwrap();

    // Restart server
    let mut server = WebSocketServer::new(addr).await.unwrap();
    server.spawn().await.unwrap();

    // Wait for reconnection
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(client.is_connected());

    // State should be recovered
    client
        .send_text(r#"{"type": "get_state"}"#)
        .await
        .unwrap();

    let response = client.recv().await.unwrap().unwrap();
    assert!(response.as_text().unwrap().contains("persistent"));

    server.shutdown().await.unwrap();
}

// Helper struct for equilibrium-tokens integration
#[derive(Clone)]
struct EquilibriumMessageHandler {
    // In real implementation, this would contain reference to EquilibriumOrchestrator
}

impl EquilibriumMessageHandler {
    fn new() -> Self {
        Self {}
    }

    async fn handle(&self, msg: websocket_fabric::Message) -> anyhow::Result<Option<websocket_fabric::Message>> {
        let text = msg.as_text()?;
        let json: serde_json::Value = serde_json::from_str(text)?;

        // Process equilibrium-tokens message
        let response = match json["type"].as_str() {
            Some("token_request") => Some(r#"{
                "type": "token_response",
                "status": "success",
                "token_id": "eq_123"
            }"#),
            _ => None,
        };

        Ok(response.map(|r| websocket_fabric::Message::text(r)))
    }
}
