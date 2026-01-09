//! Message latency benchmarks
//!
//! Benchmarks WebSocket message latency using Criterion.
//!
//! Run with: cargo bench --bench message_latency

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use websocket_fabric::{WebSocketClient, WebSocketServer};
use tokio::runtime::Runtime;

fn setup_connection() -> (WebSocketServer, WebSocketClient) {
    let rt = Runtime::new().unwrap();

    let (server, client) = rt
        .block_on(async {
            let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();
            server.spawn().await.unwrap();

            let client = WebSocketClient::connect(&format!("ws://{}", addr))
                .await
                .unwrap();

            Ok::<_, anyhow::Error>((server, client))
        })
        .unwrap();

    (server, client)
}

fn bench_message_latency_single(c: &mut Criterion) {
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

    c.bench_function("message_latency_single", |b| {
        b.iter(|| {
            rt.block_on(async {
                client
                    .send_text(black_box("ping"))
                    .await
                    .unwrap();
                let msg = server.recv().await.unwrap().unwrap();
                black_box(msg);
            })
        })
    });
}

fn bench_message_latency_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let sizes = vec![16, 64, 256, 1024, 4096];

    let mut group = c.benchmark_group("message_latency_by_size");

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

        let data = "A".repeat(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    client.send_text(black_box(&data)).await.unwrap();
                    let msg = server.recv().await.unwrap().unwrap();
                    black_box(msg);
                })
            })
        });
    }

    group.finish();
}

fn bench_ping_pong_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (server, mut client) = rt.block_on(async {
        let server = WebSocketServer::new("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        server.spawn().await.unwrap();

        let client = WebSocketClient::connect(&format!("ws://{}", addr))
            .await
            .unwrap();

        Ok::<_, anyhow::Error>((server, client))
    });

    c.bench_function("ping_pong_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                client.ping().await.unwrap();
                let pong = client.recv_pong().await.unwrap().unwrap();
                black_box(pong);
            })
        })
    });
}

fn bench_round_trip_latency(c: &mut Criterion) {
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

    c.bench_function("round_trip_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Client sends
                client.send_text(black_box("request")).await.unwrap();

                // Server receives and responds
                let msg = server.recv().await.unwrap().unwrap();
                server.send_text("response").await.unwrap();

                // Client receives response
                let response = client.recv().await.unwrap().unwrap();

                black_box((msg, response));
            })
        })
    });
}

fn bench_latency_concurrent(c: &mut Criterion) {
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

    c.bench_function("latency_concurrent_10", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut handles = vec![];

                // Send 10 concurrent messages
                for i in 0..10 {
                    let mut client = client.clone();
                    let handle = tokio::spawn(async move {
                        client
                            .send_text(&format!("concurrent {}", i))
                            .await
                            .unwrap();
                    });
                    handles.push(handle);
                }

                // Wait for all sends
                for handle in handles {
                    handle.await.unwrap();
                }

                // Receive all messages
                for _ in 0..10 {
                    let msg = server.recv().await.unwrap().unwrap();
                    black_box(msg);
                }
            })
        })
    });
}

criterion_group!(
    benches,
    bench_message_latency_single,
    bench_message_latency_sizes,
    bench_ping_pong_latency,
    bench_round_trip_latency,
    bench_latency_concurrent
);
criterion_main!(benches);
