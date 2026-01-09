//! Throughput benchmarks
//!
//! Benchmarks WebSocket throughput (messages per second).
//!
//! Run with: cargo bench --bench throughput

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::runtime::Runtime;

fn bench_throughput_unidirectional(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let message_counts = vec![1_000, 10_000, 100_000];

    let mut group = c.benchmark_group("throughput_unidirectional");

    for count in message_counts {
        let (mut server, mut client) = rt.block_on(async {
            let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();
            server.spawn().await.unwrap();

            let client = WebSocketClient::connect(&format!("ws://{}", addr))
                .await
                .unwrap();

            Ok::<_, anyhow::Error>((server, client))
        });

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                rt.block_on(async {
                    for i in 0..count {
                        client
                            .send_text(black_box(&format!("message {}", i)))
                            .await
                            .unwrap();
                    }

                    for _ in 0..count {
                        let msg = server.recv().await.unwrap().unwrap();
                        black_box(msg);
                    }
                })
            })
        });
    }

    group.finish();
}

fn bench_throughput_bidirectional(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let message_counts = vec![1_000, 10_000, 100_000];

    let mut group = c.benchmark_group("throughput_bidirectional");

    for count in message_counts {
        let (mut server, mut client) = rt.block_on(async {
            let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();
            server.spawn().await.unwrap();

            let client = WebSocketClient::connect(&format!("ws://{}", addr))
                .await
                .unwrap();

            Ok::<_, anyhow::Error>((server, client))
        });

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                rt.block_on(async {
                    // Spawn concurrent send/receive tasks
                    let server_task = tokio::spawn(async move {
                        for i in 0..count {
                            server
                                .send_text(&format!("server {}", i))
                                .await
                                .unwrap();
                            let msg = server.recv().await.unwrap().unwrap();
                            black_box(msg);
                        }
                    });

                    let client_task = tokio::spawn(async move {
                        for i in 0..count {
                            client
                                .send_text(&format!("client {}", i))
                                .await
                                .unwrap();
                            let msg = client.recv().await.unwrap().unwrap();
                            black_box(msg);
                        }
                    });

                    // Wait for both
                    tokio::try_join!(server_task, client_task).unwrap();
                })
            })
        });
    }

    group.finish();
}

fn bench_throughput_binary(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let sizes = vec![1024, 4096, 16384]; // 1KB, 4KB, 16KB

    let mut group = c.benchmark_group("throughput_binary");

    for size in sizes {
        let (mut server, mut client) = rt.block_on(async {
            let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();
            server.spawn().await.unwrap();

            let client = WebSocketClient::connect(&format!("ws://{}", addr))
                .await
                .unwrap();

            Ok::<_, anyhow::Error>((server, client))
        });

        let count = 10_000;
        let data = vec![0xABu8; size];

        group.throughput(Throughput::Bytes((size * count) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    for _ in 0..count {
                        client.send_binary(black_box(&data)).await.unwrap();
                    }

                    for _ in 0..count {
                        let msg = server.recv().await.unwrap().unwrap();
                        black_box(msg);
                    }
                })
            })
        });
    }

    group.finish();
}

fn bench_throughput_multiple_connections(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let connection_counts = vec![1, 10, 50];

    let mut group = c.benchmark_group("throughput_multiple_connections");

    for conn_count in connection_counts {
        let messages_per_conn = 1_000;

        group.throughput(Throughput::Elements((conn_count * messages_per_conn) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(conn_count),
            &conn_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
                        let addr = server.local_addr().unwrap();
                        server.spawn().await.unwrap();

                        let mut handles = vec![];

                        // Connect multiple clients
                        for client_id in 0..conn_count {
                            let url = format!("ws://{}", addr);

                            let handle = tokio::spawn(async move {
                                let mut client = WebSocketClient::connect(&url)
                                    .await
                                    .unwrap();

                                for i in 0..messages_per_conn {
                                    client
                                        .send_text(&format!("client {} msg {}", client_id, i))
                                        .await
                                        .unwrap();
                                }

                                // Receive responses
                                for _ in 0..messages_per_conn {
                                    let msg = client.recv().await.unwrap().unwrap();
                                    black_box(msg);
                                }
                            });

                            handles.push(handle);
                        }

                        // Wait for all clients
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

fn bench_throughput_small_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (mut server, mut client) = rt.block_on(async {
        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        server.spawn().await.unwrap();

        let client = WebSocketClient::connect(&format!("ws://{}", addr))
            .await
            .unwrap();

        Ok::<_, anyhow::Error>((server, client))
    });

    let count = 100_000;

    c.bench_function("throughput_small_messages", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Send very small messages
                for i in 0..count {
                    client.send_text(black_box(&i.to_string())).await.unwrap();
                }

                for _ in 0..count {
                    let msg = server.recv().await.unwrap().unwrap();
                    black_box(msg);
                }
            })
        })
    });
}

criterion_group!(
    benches,
    bench_throughput_unidirectional,
    bench_throughput_bidirectional,
    bench_throughput_binary,
    bench_throughput_multiple_connections,
    bench_throughput_small_messages
);
criterion_main!(benches);
