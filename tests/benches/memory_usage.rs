//! Memory usage benchmarks
//!
//! Benchmarks memory usage and profiling for websocket-fabric.
//!
//! Run with: cargo bench --bench memory_usage

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::runtime::Runtime;

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

fn bench_memory_per_connection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![100, 500, 1000];

    let mut group = c.benchmark_group("memory_per_connection");

    for count in connection_counts {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    let initial_memory = get_memory_usage();

                    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                    let addr = server.local_addr().unwrap();
                    server.spawn().await.unwrap();

                    let mut clients = vec![];

                    // Connect clients
                    for _ in 0..count {
                        let client = WebSocketClient::connect(&format!("ws://{}", addr))
                            .await
                            .unwrap();

                        clients.push(client);
                    }

                    // Let memory stabilize
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                    let final_memory = get_memory_usage();
                    let memory_per_connection = (final_memory - initial_memory) / count;

                    black_box(memory_per_connection);
                })
            })
        });
    }

    group.finish();
}

fn bench_memory_message_buffering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let buffer_sizes = vec![100, 1_000, 10_000];

    let mut group = c.benchmark_group("memory_message_buffering");

    for size in buffer_sizes {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let (mut server, mut client) = async {
                        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let client = WebSocketClient::connect(&format!("ws://{}", addr))
                            .await
                            .unwrap();

                        (server, client)
                    }
                    .await;

                    let initial_memory = get_memory_usage();

                    // Send messages (buffer them without receiving)
                    for i in 0..size {
                        client
                            .send_text(&format!("message {}", i))
                            .await
                            .unwrap();
                    }

                    // Let memory stabilize
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    let final_memory = get_memory_usage();
                    let memory_used = final_memory - initial_memory;

                    black_box(memory_used);

                    // Now receive messages
                    for _ in 0..size {
                        server.recv().await.unwrap().unwrap();
                    }
                })
            })
        });
    }

    group.finish();
}

fn bench_memory_large_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let message_sizes = vec![1024, 10_240, 102_400, 1_048_576]; // 1KB, 10KB, 100KB, 1MB

    let mut group = c.benchmark_group("memory_large_messages");

    for size in message_sizes {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let (mut server, mut client) = async {
                        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let client = WebSocketClient::connect(&format!("ws://{}", addr))
                            .await
                            .unwrap();

                        (server, client)
                    }
                    .await;

                    let initial_memory = get_memory_usage();

                    let data = vec![0xABu8; size];

                    // Send large message
                    client.send_binary(&data).await.unwrap();

                    // Receive message
                    let msg = server.recv().await.unwrap().unwrap();
                    black_box(msg);

                    let final_memory = get_memory_usage();
                    let memory_delta = final_memory - initial_memory;

                    black_box(memory_delta);
                })
            })
        });
    }

    group.finish();
}

fn bench_memory_fragmentation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_fragmentation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut connections = vec![];

                // Create and destroy many connections
                for cycle in 0..100 {
                    let (server, client) = async {
                        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let client = WebSocketClient::connect(&format!("ws://{}", addr))
                            .await
                            .unwrap();

                        (server, client)
                    }
                    .await;

                    connections.push((server, client));

                    // Close some connections
                    if cycle > 50 {
                        let (server, client) = connections.remove(0);
                        client
                            .close(websocket_fabric::CloseFrame::normal())
                            .await
                            .ok();
                        server.shutdown().await.ok();
                    }
                }

                // Cleanup
                for (server, client) in connections {
                    client
                        .close(websocket_fabric::CloseFrame::normal())
                        .await
                        .ok();
                    server.shutdown().await.ok();
                }
            })
        })
    });
}

fn bench_memory_reuse(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("memory_buffer_reuse", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (mut server, mut client) = async {
                    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                    let addr = server.local_addr().unwrap();
                    server.spawn().await.unwrap();

                    let client = WebSocketClient::connect(&format!("ws://{}", addr))
                        .await
                        .unwrap();

                    (server, client)
                }
                .await;

                // Send and receive many messages (testing buffer reuse)
                for i in 0..10_000 {
                    client
                        .send_text(&format!("message {}", i))
                        .await
                        .unwrap();

                    let msg = server.recv().await.unwrap().unwrap();
                    black_box(msg);
                }
            })
        })
    });
}

criterion_group!(
    benches,
    bench_memory_per_connection,
    bench_memory_message_buffering,
    bench_memory_large_messages,
    bench_memory_fragmentation,
    bench_memory_reuse
);
criterion_main!(benches);
