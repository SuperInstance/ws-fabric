//! Message operation benchmarks
//!
//! Run with: cargo bench --bench message_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use websocket_fabric::Message;

fn bench_message_text_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_text_creation");

    for size in [10, 100, 1000, 10000].iter() {
        let text = "a".repeat(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(Message::text(text.clone()));
            });
        });
    }

    group.finish();
}

fn bench_message_binary_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_binary_creation");

    for size in [10, 100, 1000, 10000].iter() {
        let data = vec![0u8; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(Message::binary(data.clone()));
            });
        });
    }

    group.finish();
}

fn bench_message_from_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_binary_from_slice");

    for size in [10, 100, 1000, 10000].iter() {
        let data = vec![0u8; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(Message::binary_from_slice(&data));
            });
        });
    }

    group.finish();
}

fn bench_message_json(c: &mut Criterion) {
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        id: u64,
        name: String,
        value: f64,
        timestamp: u64,
    }

    let data = TestData {
        id: 123,
        name: "test".to_string(),
        value: 45.67,
        timestamp: 1640000000,
    };

    c.bench_function("message_json", |b| {
        b.iter(|| {
            black_box(Message::json(&data).unwrap());
        });
    });
}

fn bench_message_inspection(c: &mut Criterion) {
    let text_msg = Message::text("Hello, World!");
    let bin_msg = Message::binary(vec![1, 2, 3, 4, 5]);
    let ping_msg = Message::ping(&b"ping"[..]);
    let pong_msg = Message::pong(&b"pong"[..]);
    let close_msg = Message::close(Some(1000), Some("Goodbye".to_string()));

    let mut group = c.benchmark_group("message_inspection");

    group.bench_function("is_text", |b| {
        b.iter(|| {
            black_box(text_msg.is_text());
        });
    });

    group.bench_function("is_binary", |b| {
        b.iter(|| {
            black_box(bin_msg.is_binary());
        });
    });

    group.bench_function("is_ping", |b| {
        b.iter(|| {
            black_box(ping_msg.is_ping());
        });
    });

    group.bench_function("is_pong", |b| {
        b.iter(|| {
            black_box(pong_msg.is_pong());
        });
    });

    group.bench_function("is_close", |b| {
        b.iter(|| {
            black_box(close_msg.is_close());
        });
    });

    group.finish();
}

fn bench_message_as_text(c: &mut Criterion) {
    let msg = Message::text("Hello, World!");

    c.bench_function("message_as_text", |b| {
        b.iter(|| {
            black_box(msg.as_text());
        });
    });
}

fn bench_message_payload(c: &mut Criterion) {
    let msg = Message::binary(vec![1, 2, 3, 4, 5]);

    let mut group = c.benchmark_group("message_payload");

    group.bench_function("as_bytes", |b| {
        b.iter(|| {
            black_box(msg.as_bytes());
        });
    });

    group.bench_function("payload", |b| {
        b.iter(|| {
            black_box(msg.payload());
        });
    });

    group.finish();
}

fn bench_message_clone(c: &mut Criterion) {
    let msg = Message::text("Hello, World!");

    c.bench_function("message_clone", |b| {
        b.iter(|| {
            black_box(msg.clone());
        });
    });
}

fn bench_message_size(c: &mut Criterion) {
    let msg = Message::binary(vec![1, 2, 3, 4, 5]);

    let mut group = c.benchmark_group("message_size");

    group.bench_function("len", |b| {
        b.iter(|| {
            black_box(msg.len());
        });
    });

    group.bench_function("is_empty", |b| {
        b.iter(|| {
            black_box(msg.is_empty());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_message_text_creation,
    bench_message_binary_creation,
    bench_message_from_slice,
    bench_message_json,
    bench_message_inspection,
    bench_message_as_text,
    bench_message_payload,
    bench_message_clone,
    bench_message_size
);
criterion_main!(benches);
