//! Echo server example - echoes messages back to clients

use std::time::Duration;
use tokio::time::sleep;
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

    // Create echo server
    let config = ServerConfig::new("127.0.0.1:8081")
        .with_max_connections(50)
        .with_max_message_size(1024 * 1024); // 1MB

    let server = WebSocketServer::new(config);

    println!("Echo Server Example");
    println!("==================");
    println!("Connect with: ws://127.0.0.1:8081");
    println!();

    // Spawn server
    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_secs(1)).await;

    println!("Echo server is running!");
    println!("Press Ctrl+C to stop");
    println!();

    // Monitor connections
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    let mut last_count = 0;

    loop {
        tick.tick().await;

        let count = server.connection_count();
        let metrics = server.metrics();

        if count != last_count {
            if count > last_count {
                println!("Client connected (total: {})", count);
            } else {
                println!("Client disconnected (total: {})", count);
            }
            last_count = count;
        }

        println!(
            "Status: {} clients | Sent: {} msgs | Recv: {} msgs",
            count,
            metrics.messages_sent(),
            metrics.messages_received()
        );
    }
}
