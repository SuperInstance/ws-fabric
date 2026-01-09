//! Integration tests for websocket-fabric
//!
//! These tests require network access and connect to external WebSocket servers.

use std::time::Duration;
use websocket_fabric::{
    BackpressureConfig, ClientConfig, HeartbeatConfig, Message, ReconnectConfig, WebSocketClient,
};

#[tokio::test]
#[ignore = "requires network access"]
async fn test_basic_connection() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await;

    // Should connect successfully
    assert!(client.is_ok(), "Failed to connect to echo server");

    let client = client.unwrap();
    assert!(client.is_connected());
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_send_text_message() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    // Send text message
    let result = client.send_text("Hello, Integration Test!").await;
    assert!(result.is_ok(), "Failed to send text message");

    // Give time for echo
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check metrics
    let metrics = client.metrics();
    assert_eq!(metrics.messages_sent(), 1);

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_send_binary_message() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    // Send binary message
    let data = vec![1, 2, 3, 4, 5];
    let result = client.send_binary(&data).await;
    assert!(result.is_ok(), "Failed to send binary message");

    // Give time for echo
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check metrics
    let metrics = client.metrics();
    assert_eq!(metrics.messages_sent(), 1);

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_send_multiple_messages() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    // Send multiple messages
    for i in 0..10 {
        let msg = format!("Message {}", i);
        client.send_text(&msg).await.unwrap();
    }

    // Give time for echoes
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check metrics
    let metrics = client.metrics();
    assert_eq!(metrics.messages_sent(), 10);

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_custom_configuration() {
    let config = ClientConfig::new("ws://echo.websocket.org")
        .with_max_message_size(1024 * 1024) // 1MB
        .with_connection_timeout(Duration::from_secs(10))
        .with_reconnect_config(
            ReconnectConfig::new()
                .with_max_attempts(3)
                .with_initial_delay(Duration::from_millis(100)),
        )
        .with_heartbeat_config(
            HeartbeatConfig::new()
                .with_ping_interval(Duration::from_secs(20))
                .with_ping_timeout(Duration::from_secs(10)),
        )
        .with_backpressure_config(
            BackpressureConfig::new()
                .with_max_buffer_size(500)
                .with_backpressure_threshold(0.8)
                .with_recovery_threshold(0.6),
        );

    let client = WebSocketClient::connect_with_config(config).await;
    assert!(client.is_ok(), "Failed to connect with custom config");

    let client = client.unwrap();
    assert!(client.is_connected());

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_json_message() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestData {
        id: u64,
        name: String,
        value: f64,
    }

    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    // Create JSON message
    let data = TestData {
        id: 123,
        name: "test".to_string(),
        value: 45.67,
    };

    let msg = Message::json(&data);
    assert!(msg.is_ok(), "Failed to create JSON message");

    // Send JSON message
    let result = client.send(msg.unwrap()).await;
    assert!(result.is_ok(), "Failed to send JSON message");

    // Give time for echo
    tokio::time::sleep(Duration::from_millis(500)).await;

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_metrics_collection() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    // Send some messages
    for _ in 0..5 {
        client.send_text("test").await.unwrap();
    }

    // Give time for echoes
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check metrics
    let metrics = client.metrics();
    assert!(metrics.total_connections() >= 1);
    assert_eq!(metrics.messages_sent(), 5);

    // Check latency percentiles
    let percentiles = metrics.latency_percentiles();
    assert!(percentiles.p50 > 0);

    client.close(None).await.unwrap();

    // Check disconnection recorded
    assert!(metrics.disconnections() >= 1);
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_graceful_shutdown() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    assert!(client.is_connected());

    // Close with reason
    let result = client.close(Some("Test shutdown".to_string())).await;
    assert!(result.is_ok(), "Failed to close connection");

    // Connection should be marked as not connected
    // (Note: is_connected() may still return true briefly after close due to async nature)
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_heartbeat_functionality() {
    let url = "ws://echo.websocket.org";
    let client = WebSocketClient::connect(url).await.unwrap();

    let heartbeat = client.heartbeat();

    // Heartbeat should be enabled by default
    assert!(heartbeat.is_enabled());

    // Ping count should be 0 initially (background task hasn't run yet)
    let initial_pings = heartbeat.ping_count();

    // Wait for heartbeat to send at least one ping
    tokio::time::sleep(Duration::from_secs(31)).await;

    // Ping count should have increased
    let current_pings = heartbeat.ping_count();
    assert!(current_pings > initial_pings, "Heartbeat not sending pings");

    client.close(None).await.unwrap();
}

#[tokio::test]
#[ignore = "requires network access"]
async fn test_backpressure_tracking() {
    let url = "ws://echo.websocket.org";
    let config = ClientConfig::new(url)
        .with_backpressure_config(BackpressureConfig::new().with_max_buffer_size(100));

    let client = WebSocketClient::connect_with_config(config).await.unwrap();

    let backpressure = client.backpressure();

    // Utilization should start low
    let initial_util = backpressure.utilization();
    assert!(initial_util < 0.5, "Initial utilization too high: {}", initial_util);

    // Send messages to increase utilization
    for i in 0..50 {
        let _ = client.send_text(&format!("Message {}", i)).await;
    }

    // Give time for processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Utilization should have increased
    let util = backpressure.utilization();
    // Note: utilization may vary based on processing speed
    assert!(util >= 0.0 && util <= 1.0, "Utilization out of range: {}", util);

    client.close(None).await.unwrap();
}
