//! TTFT end-to-end criterion bench.
//!
//! Measures: from `AgentRuntime::process` call to the first `ContentChunk`
//! draining off the `event_rx` mpsc, using `MockProvider` that streams
//! a fixed 8-token response with 1ms inter-token spacing.
//!
//! Gate: p95 ≤ 15ms on M2 Pro.

use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use tokio::runtime::Builder;

fn ttft_e2e(c: &mut Criterion) {
    let mut group = c.benchmark_group("ttft_e2e");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let rt = Builder::new_current_thread().enable_all().build().unwrap();

    group.bench_function("mock_8_tokens", |b| {
        b.to_async(&rt).iter(|| async move {
            // The actual harness function lives in `tests/common/chat_harness.rs`.
            // For now this benchmark is a SKELETON. Task 6 wires in the real
            // ChatTestHarness::send_and_await_first_chunk().
            tokio::time::sleep(Duration::from_micros(1)).await;
        });
    });

    group.finish();
}

criterion_group!(benches, ttft_e2e);
criterion_main!(benches);
