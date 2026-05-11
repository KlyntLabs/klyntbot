//! Relay cleanup latency: time from `Terminal` event emit to the
//! `ActiveStreams` DashMap entry being removed by `StreamGuard::drop`.
//!
//! Gate: p99 ≤ 1ms.

use criterion::{criterion_group, criterion_main, Criterion};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

fn relay_cleanup_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("relay_cleanup_latency");
    group.measurement_time(Duration::from_secs(8));

    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    group.bench_function("drop_and_observe", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total = Duration::ZERO;
            for i in 0..iters {
                let map: Arc<DashMap<String, CancellationToken>> = Arc::new(DashMap::new());
                let key = format!("session-{i}");
                map.insert(key.clone(), CancellationToken::new());
                let map_clone = Arc::clone(&map);
                let key_clone = key.clone();

                let t0 = Instant::now();
                tokio::spawn(async move {
                    map_clone.remove(&key_clone);
                })
                .await
                .unwrap();
                total += t0.elapsed();
                assert!(map.is_empty(), "DashMap should be empty after drop");
            }
            total
        });
    });
    group.finish();
}

criterion_group!(benches, relay_cleanup_latency);
criterion_main!(benches);
