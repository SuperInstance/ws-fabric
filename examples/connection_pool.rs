//! Connection pool example
//!
//! This example demonstrates how to use the ConnectionPool to manage
//! multiple WebSocket connections efficiently.

use websocket_fabric::{ConnectionPool, PoolConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create a connection pool with custom configuration
    let config = PoolConfig::new()
        .with_min_connections(1)
        .with_max_connections(5)
        .with_idle_timeout(Duration::from_secs(300))
        .with_health_check_interval(Duration::from_secs(30))
        .with_acquire_timeout(Duration::from_secs(5));

    let pool = ConnectionPool::new(config)?;

    // Start the health check task
    let _health_check_handle = pool.start_health_check_task();

    println!("WebSocket Connection Pool Example");
    println!("==================================\n");

    // Example 1: Get a connection from the pool
    println!("Example 1: Acquiring connection from pool...");
    match pool.get("ws://echo.websocket.org").await {
        Ok(client) => {
            println!("✓ Connection acquired: {}", client.connection_id);

            // Send a message
            if let Err(e) = client.send_text("Hello from pool!").await {
                println!("✗ Failed to send: {}", e);
            } else {
                println!("✓ Message sent successfully");
            }

            // Connection is automatically returned to pool when dropped
            drop(client);
            println!("✓ Connection returned to pool");
        }
        Err(e) => {
            println!("✗ Failed to acquire connection: {}", e);
        }
    }

    // Example 2: Get pool statistics
    println!("\nExample 2: Pool statistics...");
    let stats = pool.stats();
    println!("Server pools: {}", stats.server_count);
    println!("Total connections: {}", stats.total_connections);
    println!("Available connections: {}", stats.available_connections);
    println!("In-use connections: {}", stats.in_use_connections);

    // Example 3: Get server-specific statistics
    if let Some(server_stats) = pool.server_stats("ws://echo.websocket.org") {
        println!("\nExample 3: Server-specific statistics");
        println!("URL: {}", server_stats.url);
        println!("Total created: {}", server_stats.total_created);
        println!("Total acquired: {}", server_stats.total_acquired);
        println!("Total released: {}", server_stats.total_released);
    }

    // Example 4: Manual cleanup
    println!("\nExample 4: Running cleanup...");
    let cleanup_stats = pool.cleanup();
    println!("Connections removed: {}", cleanup_stats.connections_removed);
    println!("Servers cleaned: {}", cleanup_stats.servers_cleaned);

    // Example 5: Close all connections
    println!("\nExample 5: Closing all connections...");
    pool.close_all().await;
    println!("✓ All connections closed");

    println!("\n✓ Example completed successfully");

    Ok(())
}
