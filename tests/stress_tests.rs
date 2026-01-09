//! Stress tests for websocket-fabric
//!
//! These tests are designed to be run in dedicated environments with
//! sufficient resources. They test the limits of the system under extreme load.
//!
//! Run with: cargo test --release --test stress_tests -- --ignored

use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::time::{timeout, Duration, Instant};

#[tokio::test]
#[ignore] // Only run in dedicated stress tests
async fn test_concurrent_connections_10k() {
    // Test 10,000 concurrent connections
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let num_connections = 10_000;
    let mut handles = vec![];

    println!("Connecting {} clients...", num_connections);
    let start = Instant::now();

    // Connect clients in batches to avoid overwhelming system
    let batch_size = 100;
    for batch in 0..(num_connections / batch_size) {
        for i in 0..batch_size {
            let client_id = batch * batch_size + i;
            let url = format!("ws://{}", addr);

            let handle = tokio::spawn(async move {
                let mut client = WebSocketClient::connect(&url).await.unwrap();

                // Send a message
                client
                    .send_text(&format!("client {}", client_id))
                    .await
                    .unwrap();

                // Keep connection alive
                tokio::time::sleep(Duration::from_secs(30)).await;

                client.close(websocket_fabric::CloseFrame::normal()).await.unwrap();
            });

            handles.push(handle);
        }

        // Small delay between batches
        tokio::time::sleep(Duration::from_millis(10)).await;
        println!("Connected {} clients", (batch + 1) * batch_size);
    }

    let elapsed = start.elapsed();
    println!(
        "Connected {} clients in {:?}",
        num_connections, elapsed
    );

    // Server should still be responsive
    assert!(server.is_running());
    assert_eq!(
        server.active_connections(),
        num_connections,
        "All connections should be active"
    );

    // Wait for some messages to be processed
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Cleanup
    println!("Disconnecting clients...");
    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(server.active_connections(), 0, "All clients should disconnect");

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_memory_leak_long_running() {
    // Run for extended period, check for memory leaks
    let (mut server, mut client) = setup_connection().await.unwrap();

    let iterations = 1_000_000;
    let start_memory = get_memory_usage();

    println!(
        "Starting memory leak test ({} iterations)...",
        iterations
    );
    let start = Instant::now();

    for i in 0..iterations {
        client.send_text(&format!("message {}", i)).await.unwrap();
        let _msg = server.recv().await.unwrap().unwrap();

        // Check memory every 100K iterations
        if i % 100_000 == 0 && i > 0 {
            let current_memory = get_memory_usage();
            let growth = current_memory as i64 - start_memory as i64;

            println!(
                "Iteration {}: Memory growth = {} bytes",
                i, growth
            );

            // Memory should not grow significantly
            assert!(growth < 50_000_000, "Memory growth exceeds 50MB");
        }
    }

    let elapsed = start.elapsed();
    println!("Completed {} iterations in {:?}", iterations, elapsed);

    let end_memory = get_memory_usage();
    let total_growth = end_memory as i64 - start_memory as i64;

    println!(
        "Total memory growth: {} bytes ({} MB)",
        total_growth,
        total_growth / 1_000_000
    );

    // Memory should not grow more than 10MB over the entire run
    assert!(total_growth < 10_000_000, "Memory growth exceeds 10MB limit");

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_message_burst() {
    // Test handling of message bursts
    let (mut server, mut client) = setup_connection().await.unwrap();

    let burst_size = 100_000;
    println!("Sending burst of {} messages...", burst_size);

    let start = Instant::now();

    // Send burst
    for i in 0..burst_size {
        client
            .send_text(&format!("burst message {}", i))
            .await
            .unwrap();
    }

    let send_elapsed = start.elapsed();
    println!(
        "Sent {} messages in {:?} ({:.2} msg/sec)",
        burst_size,
        send_elapsed,
        burst_size as f64 / send_elapsed.as_secs_f64()
    );

    // Receive all
    let receive_start = Instant::now();

    for _ in 0..burst_size {
        server.recv().await.unwrap().unwrap();
    }

    let receive_elapsed = receive_start.elapsed();
    println!(
        "Received {} messages in {:?} ({:.2} msg/sec)",
        burst_size,
        receive_elapsed,
        burst_size as f64 / receive_elapsed.as_secs_f64()
    );

    assert!(server.is_running(), "Server should still be running");

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rapid_connect_disconnect() {
    // Test rapid connection and disconnection cycles
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let cycles = 10_000;
    println!("Testing {} connect/disconnect cycles...", cycles);

    let start = Instant::now();

    for i in 0..cycles {
        let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
            .await
            .unwrap();

        client
            .send_text(&format!("cycle {}", i))
            .await
            .unwrap();

        client
            .close(websocket_fabric::CloseFrame::normal())
            .await
            .unwrap();

        if i % 1000 == 0 && i > 0 {
            println!("Completed {} cycles", i);
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Completed {} cycles in {:?} ({:.2} cycles/sec)",
        cycles,
        elapsed,
        cycles as f64 / elapsed.as_secs_f64()
    );

    assert!(server.is_running());

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_large_message_handling() {
    // Test handling of very large messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    // Test progressively larger messages
    let sizes = vec![
        1_000,      // 1KB
        10_000,     // 10KB
        100_000,    // 100KB
        1_000_000,  // 1MB
        10_000_000, // 10MB
    ];

    for size in sizes {
        println!("Testing message size: {} bytes", size);

        let data = "A".repeat(size);

        let start = Instant::now();

        client.send_text(&data).await.unwrap();

        let msg = timeout(Duration::from_secs(30), server.recv())
            .await
            .expect(&format!("Should receive {} byte message within timeout", size))
            .expect("Should receive message");

        let elapsed = start.elapsed();

        assert_eq!(msg.as_text().unwrap().len(), size);

        println!(
            "Transferred {} bytes in {:?} ({:.2} MB/sec)",
            size,
            elapsed,
            (size as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
        );
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_churn_connections() {
    // Test connection churn (simulating real-world scenario)
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let target_connections = 5_000;
    let churn_rate = 100; // Connections per second
    let duration = Duration::from_secs(60);

    println!(
        "Testing churn: {} target connections, {}/sec for {} seconds",
        target_connections, churn_rate, duration.as_secs()
    );

    let start = Instant::now();
    let mut handles = vec![];

    while start.elapsed() < duration {
        // Add new connections
        for _ in 0..churn_rate {
            if server.active_connections() >= target_connections {
                break;
            }

            let url = format!("ws://{}", addr);
            let handle = tokio::spawn(async move {
                if let Ok(mut client) = WebSocketClient::connect(&url).await {
                    // Keep connection alive for random time
                    let lifetime = Duration::from_millis(rand::random::<u64>() % 10000 + 1000);
                    tokio::time::sleep(lifetime).await;

                    client
                        .close(websocket_fabric::CloseFrame::normal())
                        .await
                        .ok();
                }
            });

            handles.push(handle);
        }

        // Wait a bit
        tokio::time::sleep(Duration::from_secs(1)).await;

        let elapsed = start.elapsed();
        println!(
            "[{:?}] Active connections: {}",
            elapsed,
            server.active_connections()
        );
    }

    // Cleanup
    for handle in handles {
        handle.await.ok();
    }

    assert!(server.is_running());

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_server_restart_under_load() {
    // Test server behavior when restarting under load
    let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    // Connect many clients
    let num_clients = 1000;
    let mut clients = vec![];

    println!("Connecting {} clients...", num_clients);

    for _ in 0..num_clients {
        let client = WebSocketClient::connect(&format!("ws://{}", addr)).await;
        if let Ok(c) = client {
            clients.push(c);
        }
    }

    println!("Connected {} clients", clients.len());

    // Shutdown server
    println!("Shutting down server...");
    server.shutdown().await.unwrap();

    // Wait a bit
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Restart server
    println!("Restarting server...");
    let mut server = WebSocketServer::new(addr).await.unwrap();
    server.spawn().await.unwrap();

    // Clients should reconnect
    println!("Waiting for reconnections...");

    let mut reconnected = 0;
    for client in &mut clients {
        if client.wait_for_reconnection(Duration::from_secs(10)).await.is_ok() {
            reconnected += 1;
        }
    }

    println!(
        "Reconnected {}/{} clients",
        reconnected, num_clients
    );

    // Most clients should reconnect
    assert!(reconnected > num_clients * 80 / 100);

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_24_hour_stability() {
    // Long-running stability test (24 hours)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let duration = Duration::from_secs(24 * 60 * 60); // 24 hours
    let check_interval = Duration::from_secs(60); // Check every minute

    println!("Starting 24-hour stability test...");

    let start = Instant::now();
    let mut checks = 0;

    while start.elapsed() < duration {
        // Send message
        client
            .send_text(&format!("stability check {}", checks))
            .await
            .unwrap();

        // Receive response
        timeout(Duration::from_secs(5), server.recv())
            .await
            .expect("Should receive message within 5 seconds")
            .expect("Should receive message");

        checks += 1;

        let elapsed = start.elapsed();
        println!(
            "[{:?}] Completed {} checks ({} hours remaining)",
            elapsed,
            checks,
            (duration - elapsed).as_secs() / 3600
        );

        tokio::time::sleep(check_interval).await;

        // Verify server is still healthy
        assert!(server.is_running());
        assert!(client.is_connected());
    }

    println!("24-hour stability test completed successfully");

    server.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_mixed_workload() {
    // Test with mixed workload (text, binary, ping, etc.)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let operations = 100_000;
    println!("Testing mixed workload of {} operations...", operations);

    let start = Instant::now();

    for i in 0..operations {
        match i % 5 {
            0 => {
                // Text message
                client.send_text(&format!("text {}", i)).await.unwrap();
            }
            1 => {
                // Binary message
                let data = vec![i as u8; 256];
                client.send_binary(&data).await.unwrap();
            }
            2 => {
                // Ping
                client.ping().await.unwrap();
            }
            3 => {
                // Fragmented message
                client.send_fragment_start("fragment ").await.unwrap();
                client.send_fragment_end(&format!("{}", i)).await.unwrap();
            }
            4 => {
                // Multiple messages
                for j in 0..10 {
                    client
                        .send_text(&format!("batch {}-{}", i, j))
                        .await
                        .unwrap();
                }
            }
            _ => unreachable!(),
        }

        // Receive response
        if let Ok(Some(msg)) = timeout(Duration::from_millis(100), server.recv()).await {
            // Process message
            drop(msg);
        }

        if i % 10000 == 0 && i > 0 {
            println!("Completed {} operations", i);
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Completed {} operations in {:?} ({:.2} ops/sec)",
        operations,
        elapsed,
        operations as f64 / elapsed.as_secs_f64()
    );

    assert!(server.is_running());

    server.shutdown().await.unwrap();
}

/// Helper to setup a test server and client connection
async fn setup_connection() -> anyhow::Result<(WebSocketServer, WebSocketClient)> {
    let server = WebSocketServer::new("127.0.0.1:0").await?;
    let addr = server.local_addr()?;
    server.spawn().await?;

    let client = WebSocketClient::connect(&format!("ws://{}", addr)).await?;

    Ok((server, client))
}

/// Get current process memory usage in bytes
fn get_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Ok(kb) = parts[1].parse::<usize>() {
                        return kb * 1024;
                    }
                }
            }
        }
    }

    0 // Placeholder for other platforms
}
