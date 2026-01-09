//! Basic WebSocket client example
//!
//! This example demonstrates:
//! - Connecting to a WebSocket server
//! - Sending text messages
//! - Sending binary messages
//! - Handling errors
//! - Graceful shutdown

use std::time::Duration;
use websocket_fabric::{ClientConfig, WebSocketClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("WebSocket Basic Client Example");
    println!("==============================\n");

    // Connect to echo server
    let url = "ws://echo.websocket.org";
    println!("Connecting to {}...", url);

    let config = ClientConfig::new(url)
        .with_connection_timeout(Duration::from_secs(5))
        .with_auto_reconnect(true);

    let client = WebSocketClient::connect_with_config(config).await?;

    println!("Connected successfully!\n");

    // Send text message
    println!("Sending text message...");
    client.send_text("Hello, WebSocket!").await?;
    println!("Text message sent!\n");

    // Wait for echo response
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Send binary message
    println!("Sending binary message...");
    client.send_binary(&[1, 2, 3, 4, 5]).await?;
    println!("Binary message sent!\n");

    // Wait for echo response
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Check metrics
    let metrics = client.metrics();
    println!("Connection Metrics:");
    println!("  Messages sent: {}", metrics.messages_sent());
    println!("  Messages received: {}", metrics.messages_received());
    println!("  Bytes sent: {}", metrics.bytes_sent());
    println!("  Bytes received: {}", metrics.bytes_received());

    let percentiles = metrics.latency_percentiles();
    println!("  Latency P50: {}µs", percentiles.p50);
    println!("  Latency P95: {}µs", percentiles.p95);

    // Close connection
    println!("\nClosing connection...");
    client.close(Some("Example completed".to_string())).await?;
    println!("Connection closed.");

    Ok(())
}
