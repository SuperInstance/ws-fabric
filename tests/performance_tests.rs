//! Performance benchmarks for websocket-fabric
//!
//! Tests message latency, throughput, and resource usage under various conditions.
//! These tests use criterion for accurate benchmarking.

use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::time::{Duration, Instant};

/// Helper to setup a test server and client connection
async fn setup_connection() -> anyhow::Result<(WebSocketServer, WebSocketClient)> {
    let server = WebSocketServer::new("127.0.0.1:0").await?;
    let addr = server.local_addr()?;
    server.spawn().await?;

    let client = WebSocketClient::connect(&format!("ws://{}", addr)).await?;

    Ok((server, client))
}

#[tokio::test]
async fn test_message_latency_p50() {
    // Test P50 message latency (target: <100µs)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let mut latencies = vec![];

    for _ in 0..1000 {
        let start = Instant::now();

        client.send_text("ping").await.unwrap();
        let _msg = server.recv().await.unwrap().unwrap();

        let latency = start.elapsed();
        latencies.push(latency);
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];

    assert!(p50.as_micros() < 100, "P50 latency should be <100µs, got {:?}", p50);

    println!("P50 latency: {:?}", p50);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_latency_p95() {
    // Test P95 message latency (target: <500µs)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let mut latencies = vec![];

    for _ in 0..1000 {
        let start = Instant::now();

        client.send_text("ping").await.unwrap();
        let _msg = server.recv().await.unwrap().unwrap();

        let latency = start.elapsed();
        latencies.push(latency);
    }

    latencies.sort();
    let p95 = latencies[(latencies.len() * 95) / 100];

    assert!(
        p95.as_micros() < 500,
        "P95 latency should be <500µs, got {:?}",
        p95
    );

    println!("P95 latency: {:?}", p95);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_message_latency_p99() {
    // Test P99 message latency (target: <1ms)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let mut latencies = vec![];

    for _ in 0..1000 {
        let start = Instant::now();

        client.send_text("ping").await.unwrap();
        let _msg = server.recv().await.unwrap().unwrap();

        let latency = start.elapsed();
        latencies.push(latency);
    }

    latencies.sort();
    let p99 = latencies[(latencies.len() * 99) / 100];

    assert!(
        p99.as_millis() < 1,
        "P99 latency should be <1ms, got {:?}",
        p99
    );

    println!("P99 latency: {:?}", p99);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_throughput_per_connection() {
    // Test throughput per connection (target: >100K messages/sec)
    let (mut server, mut client) = setup_connection().await.unwrap();

    let message_count = 100_000;
    let start = Instant::now();

    for i in 0..message_count {
        client.send_text(&format!("message {}", i)).await.unwrap();
    }

    // Receive all messages
    for _ in 0..message_count {
        server.recv().await.unwrap().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = message_count as f64 / elapsed.as_secs_f64();

    assert!(
        throughput > 100_000.0,
        "Throughput should be >100K msg/sec, got {:.2}",
        throughput
    );

    println!("Throughput: {:.2} messages/sec", throughput);

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_throughput_multiple_connections() {
    // Test server throughput with multiple connections
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let num_clients = 10;
    let messages_per_client = 10_000;

    let mut handles = vec![];

    for client_id in 0..num_clients {
        let url = format!("ws://{}", addr);
        let handle = tokio::spawn(async move {
            let mut client = WebSocketClient::connect(&url).await.unwrap();

            let start = Instant::now();

            for i in 0..messages_per_client {
                client
                    .send_text(&format!("client {} message {}", client_id, i))
                    .await
                    .unwrap();
            }

            let elapsed = start.elapsed();
            let throughput = messages_per_client as f64 / elapsed.as_secs_f64();

            (client_id, throughput)
        });

        handles.push(handle);
    }

    let mut total_throughput = 0.0;

    for handle in handles {
        let (_client_id, throughput) = handle.await.unwrap();
        total_throughput += throughput;
    }

    println!(
        "Total throughput across {} clients: {:.2} messages/sec",
        num_clients, total_throughput
    );

    assert!(
        total_throughput > 1_000_000.0,
        "Total throughput should be >1M msg/sec, got {:.2}",
        total_throughput
    );

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_memory_per_connection() {
    // Test memory usage per connection (target: <10KB)
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let initial_memory = get_memory_usage();

    // Connect 1000 clients
    let mut handles = vec![];

    for _ in 0..1000 {
        let url = format!("ws://{}", addr);
        let handle = tokio::spawn(async move {
            let client = WebSocketClient::connect(&url).await.unwrap();
            // Keep connection alive
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        handles.push(handle);
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    let final_memory = get_memory_usage();
    let memory_per_connection = (final_memory - initial_memory) / 1000;

    assert!(
        memory_per_connection < 10_000,
        "Memory per connection should be <10KB, got {} bytes",
        memory_per_connection
    );

    println!(
        "Memory per connection: {} bytes",
        memory_per_connection
    );

    // Cleanup
    for handle in handles {
        handle.abort();
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_binary_message_throughput() {
    // Test throughput for binary messages
    let (mut server, mut client) = setup_connection().await.unwrap();

    let message_count = 10_000;
    let message_size = 1024; // 1KB

    let data = vec![0xABu8; message_size];

    let start = Instant::now();

    for _ in 0..message_count {
        client.send_binary(&data).await.unwrap();
    }

    for _ in 0..message_count {
        server.recv().await.unwrap().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = message_count as f64 / elapsed.as_secs_f64();
    let mb_per_sec = (message_count * message_size) as f64 / (1024.0 * 1024.0 * elapsed.as_secs_f64());

    println!("Binary throughput: {:.2} messages/sec", throughput);
    println!("Binary throughput: {:.2} MB/sec", mb_per_sec);

    assert!(throughput > 50_000.0, "Binary throughput should be >50K msg/sec");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_ping_pong_latency() {
    // Test ping/pong latency
    let (server, mut client) = setup_connection().await.unwrap();

    let mut latencies = vec![];

    for _ in 0..1000 {
        let start = Instant::now();

        client.ping().await.unwrap();
        client.recv_pong().await.unwrap().unwrap();

        let latency = start.elapsed();
        latencies.push(latency);
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];

    println!("P50 ping/pong latency: {:?}", p50);

    assert!(p50.as_micros() < 100, "Ping/pong latency should be <100µs");

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_connection_establishment_time() {
    // Test time to establish connection
    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    server.spawn().await.unwrap();

    let mut times = vec![];

    for _ in 0..100 {
        let start = Instant::now();

        let client = WebSocketClient::connect(&format!("ws://{}", addr))
            .await
            .unwrap();

        let elapsed = start.elapsed();
        times.push(elapsed);

        client.close(websocket_fabric::CloseFrame::normal()).await.unwrap();
    }

    times.sort();
    let p50 = times[times.len() / 2];

    println!("P50 connection establishment time: {:?}", p50);

    assert!(
        p50.as_millis() < 10,
        "Connection establishment should be <10ms, got {:?}",
        p50
    );

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_send_performance() {
    // Test performance of concurrent sends
    let (mut server, mut client) = setup_connection().await.unwrap();

    let message_count = 10_000;
    let start = Instant::now();

    let mut handles = vec![];

    // Spawn 10 concurrent senders
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            for j in 0..(message_count / 10) {
                client
                    .send_text(&format!("sender {} message {}", i, j))
                    .await
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all sends
    for handle in handles {
        handle.await.unwrap();
    }

    // Receive all messages
    for _ in 0..message_count {
        server.recv().await.unwrap().unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = message_count as f64 / elapsed.as_secs_f64();

    println!("Concurrent send throughput: {:.2} messages/sec", throughput);

    server.shutdown().await.unwrap();
}

/// Get current process memory usage in bytes
fn get_memory_usage() -> usize {
    // Use /proc/self/status or similar platform-specific method
    // For now, return a placeholder
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status").unwrap();
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let kb = parts[1].parse::<usize>().unwrap();
                return kb * 1024;
            }
        }
    }

    0 // Placeholder for other platforms
}
