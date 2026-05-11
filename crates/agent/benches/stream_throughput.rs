//! Stream throughput criterion bench.
//!
//! Measures: how fast the merged `AgentEvent` mpsc can be drained while
//! a producer floods it with `ContentChunk` events of 16 bytes each.
//! No bridge, no Tauri — pure channel throughput.
//!
//! Gate: ≥ 5,000 events/sec sustained, p95 receive latency ≤ 200µs.

use agent::events::AgentEvent;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::mpsc;

fn stream_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_throughput");
    group.measurement_time(Duration::from_secs(10));

    for batch_size in [100usize, 1_000, 10_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &n| {
                let rt = Builder::new_current_thread().enable_all().build().unwrap();
                b.to_async(&rt).iter(|| async move {
                    let (tx, mut rx) = mpsc::channel::<AgentEvent>(64);
                    let producer = tokio::spawn(async move {
                        for _ in 0..n {
                            tx.send(AgentEvent::ContentChunk { data: "x".repeat(16) })
                                .await
                                .unwrap();
                        }
                    });
                    let mut count = 0usize;
                    while let Some(ev) = rx.recv().await {
                        criterion::black_box(&ev);
                        count += 1;
                        if count >= n {
                            break;
                        }
                    }
                    producer.await.unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, stream_throughput);
criterion_main!(benches);
