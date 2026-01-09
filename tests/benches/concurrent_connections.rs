//! Concurrent connection benchmarks
//!
//! Benchmarks performance with multiple concurrent connections.
//!
//! Run with: cargo bench --bench concurrent_connections

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::runtime::Runtime;

fn bench_concurrent_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![10, 50, 100, 500];

    let mut group = c.benchmark_group("concurrent_connections");

    for count in connection_counts {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                rt.block_on(async {
                    let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                    let addr = server.local_addr().unwrap();
                    server.spawn().await.unwrap();

                    let mut handles = vec![];

                    // Connect clients
                    for i in 0..count {
                        let url = format!("ws://{}", addr);

                        let handle = tokio::spawn(async move {
                            let mut client = WebSocketClient::connect(&url)
                                .await
                                .unwrap();

                            // Send a message
                            client
                                .send_text(&format!("client {}", i))
                                .await
                                .unwrap();

                            // Receive response
                            let msg = client.recv().await.unwrap().unwrap();
                            black_box(msg);

                            // Close
                            client
                                .close(websocket_fabric::CloseFrame::normal())
                                .await
                                .unwrap();
                        });

                        handles.push(handle);
                    }

                    // Wait for all clients
                    for handle in handles {
                        handle.await.unwrap();
                    }
                })
            })
        });
    }

    group.finish();
}

fn bench_concurrent_message_exchange(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![10, 50, 100];
    let messages_per_connection = 100;

    let mut group = c.benchmark_group("concurrent_message_exchange");

    for count in connection_counts {
        group.bench_with_input(
            BenchmarkId::new("connections", count),
            &count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let server_handle = tokio::spawn(async move {
                            // Echo server
                            for _ in 0..(count * messages_per_connection) {
                                let msg = server.recv().await.unwrap().unwrap();
                                server.send_text(msg.as_text().unwrap()).await.unwrap();
                            }
                        });

                        let mut client_handles = vec![];

                        // Connect clients
                        for client_id in 0..count {
                            let url = format!("ws://{}", addr);

                            let handle = tokio::spawn(async move {
                                let mut client = WebSocketClient::connect(&url)
                                    .await
                                    .unwrap();

                                // Send messages
                                for i in 0..messages_per_connection {
                                    client
                                        .send_text(&format!("client {} msg {}", client_id, i))
                                        .await
                                        .unwrap();

                                    // Receive echo
                                    let msg = client.recv().await.unwrap().unwrap();
                                    black_box(msg);
                                }
                            });

                            client_handles.push(handle);
                        }

                        // Wait for all clients
                        for handle in client_handles {
                            handle.await.unwrap();
                        }

                        server_handle.await.unwrap();
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_connection_establishment_rate(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![10, 50, 100, 500];

    let mut group = c.benchmark_group("connection_establishment_rate");

    for count in connection_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let mut handles = vec![];

                        // Establish connections
                        for _ in 0..count {
                            let url = format!("ws://{}", addr);

                            let handle = tokio::spawn(async move {
                                let client = WebSocketClient::connect(&url).await.unwrap();
                                black_box(client);
                            });

                            handles.push(handle);
                        }

                        // Wait for all connections
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_broadcast_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![10, 50, 100];
    let messages_count = 1000;

    let mut group = c.benchmark_group("broadcast_throughput");

    for count in connection_counts {
        group.bench_with_input(
            BenchmarkId::new("connections", count),
            &count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let mut clients = vec![];

                        // Connect clients
                        for _ in 0..count {
                            let mut client = WebSocketClient::connect(&format!("ws://{}", addr))
                                .await
                                .unwrap();

                            // Subscribe to broadcasts
                            client.send_text("subscribe").await.unwrap();

                            clients.push(client);
                        }

                        // Broadcast messages
                        for i in 0..messages_count {
                            server
                                .broadcast_text(&format!("broadcast {}", i))
                                .await
                                .unwrap();
                        }

                        // Receive broadcasts
                        for client in &mut clients {
                            for _ in 0..messages_count {
                                let msg = client.recv().await.unwrap().unwrap();
                                black_box(msg);
                            }
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_connections,
    bench_concurrent_message_exchange,
    bench_connection_establishment_rate,
    bench_broadcast_throughput
);
criterion_main!(benches);
