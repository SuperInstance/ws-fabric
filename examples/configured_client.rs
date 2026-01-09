//! Configured WebSocket client example
//!
//! This example demonstrates:
//! - Custom reconnection configuration
//! - Custom heartbeat configuration
//! - Custom backpressure configuration
//! - JSON message handling
//! - Metrics monitoring

use serde::{Deserialize, Serialize};
use std::time::Duration;
use websocket_fabric::{
    BackpressureConfig, ClientConfig, HeartbeatConfig, Message, ReconnectConfig, WebSocketClient,
};

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    username: String,
    content: String,
    timestamp: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("WebSocket Configured Client Example");
    println!("==================================\n");

    // Create custom configuration
    let config = ClientConfig::new("ws://echo.websocket.org")
        // Message size: 16MB
        .with_max_message_size(16 * 1024 * 1024)
        // Reconnection: up to 10 attempts with exponential backoff
        .with_reconnect_config(
            ReconnectConfig::new()
                .with_max_attempts(10)
                .with_initial_delay(Duration::from_millis(100))
                .with_max_delay(Duration::from_secs(30))
                .with_backoff_multiplier(2.0),
        )
        // Heartbeat: ping every 20 seconds, 10 second timeout
        .with_heartbeat_config(
            HeartbeatConfig::new()
                .with_ping_interval(Duration::from_secs(20))
                .with_ping_timeout(Duration::from_secs(10)),
        )
        // Backpressure: 5000 message buffer, activate at 75%, recover at 50%
        .with_backpressure_config(
            BackpressureConfig::new()
                .with_max_buffer_size(5000)
                .with_backpressure_threshold(0.75)
                .with_recovery_threshold(0.50),
        )
        // Connection timeout: 10 seconds
        .with_connection_timeout(Duration::from_secs(10));

    println!("Connecting with custom configuration...");
    let client = WebSocketClient::connect_with_config(config).await?;

    println!("Connected!\n");

    // Send JSON message
    println!("Sending JSON message...");
    let chat_msg = ChatMessage {
        username: "alice".to_string(),
        content: "Hello from websocket-fabric!".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };

    let ws_msg = Message::json(&chat_msg)?;
    client.send(ws_msg).await?;
    println!("JSON message sent!\n");

    // Wait for echo
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Monitor backpressure
    let backpressure = client.backpressure();
    println!("Backpressure Status:");
    println!("  Utilization: {:.1}%", backpressure.utilization() * 100.0);
    println!("  Active: {}", backpressure.is_backpressure_active());
    println!("  Can send: {}", backpressure.can_send());

    // Monitor heartbeat
    let heartbeat = client.heartbeat();
    println!("\nHeartbeat Status:");
    println!("  Enabled: {}", heartbeat.is_enabled());
    println!("  Pings sent: {}", heartbeat.ping_count());
    println!("  Pongs received: {}", heartbeat.pong_count());
    println!("  Alive: {}", heartbeat.is_alive());

    // Print comprehensive metrics
    let metrics = client.metrics();
    println!("\nComprehensive Metrics:");
    println!("  Total connections: {}", metrics.total_connections());
    println!("  Active connections: {}", metrics.active_connections());
    println!("  Messages sent: {}", metrics.messages_sent());
    println!("  Messages received: {}", metrics.messages_received());
    println!("  Bytes sent: {}", metrics.bytes_sent());
    println!("  Bytes received: {}", metrics.bytes_received());
    println!("  Send errors: {}", metrics.send_errors());
    println!("  Receive errors: {}", metrics.receive_errors());

    let percentiles = metrics.latency_percentiles();
    println!("  Latency P50: {}µs", percentiles.p50);
    println!("  Latency P95: {}µs", percentiles.p95);
    println!("  Latency P99: {}µs", percentiles.p99);
    println!("  Latency P99.9: {}µs", percentiles.p999);

    // Close gracefully
    println!("\nClosing connection...");
    client.close(Some("Configured client example completed".to_string())).await?;

    println!("Done!");
    Ok(())
}
