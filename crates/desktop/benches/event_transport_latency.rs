use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Instant;
use tokio::sync::broadcast;

fn bench_broadcast_channel(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("tokio_broadcast_event_p50", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (tx, mut rx) = broadcast::channel::<u64>(16);
                let started = Instant::now();
                tx.send(42u64).unwrap();
                let _ = rx.recv().await.unwrap();
                started.elapsed()
            })
        });
    });
}

criterion_group!(benches, bench_broadcast_channel);
criterion_main!(benches);
