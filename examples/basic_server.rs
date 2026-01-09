//! Basic WebSocket server example

use std::time::Duration;
use tokio::time::{sleep, interval};
use websocket_fabric::{Message, ServerConfig, WebSocketServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Create server configuration
    let config = ServerConfig::new("127.0.0.1:8080")
        .with_max_connections(100)
        .with_max_message_size(1024 * 1024) // 1MB
        .with_buffer_size(1000);

    // Create server
    let server = WebSocketServer::new(config);

    println!("WebSocket Server Example");
    println!("=======================");
    println!();

    // Spawn server in background
    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_secs(1)).await;

    println!("Server started on ws://127.0.0.1:8080");
    println!();

    // Broadcast a message every 2 seconds
    let mut counter = 0;
    let mut broadcast_interval = interval(Duration::from_secs(2));

    loop {
        broadcast_interval.tick().await;

        counter += 1;
        let message = format!("Broadcast message #{}", counter);

        match server.broadcast_text(&message).await {
            Ok(count) => {
                if count > 0 {
                    println!("Broadcast to {} clients: {}", count, message);
                } else {
                    println!("No clients connected (message: {})", message);
                }
            }
            Err(e) => {
                eprintln!("Broadcast error: {}", e);
            }
        }

        // Print server statistics
        let connection_count = server.connection_count();
        let metrics = server.metrics();

        println!(
            "Connections: {} | Sent: {} | Recv: {}",
            connection_count,
            metrics.messages_sent(),
            metrics.messages_received()
        );
        println!();

        // Stop after 10 broadcasts for demo
        if counter >= 10 {
            println!("Demo complete, shutting down...");
            server.shutdown().await;
            break;
        }
    }

    Ok(())
}
