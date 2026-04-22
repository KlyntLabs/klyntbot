use ai_core::{
    mirror::{MirrorSignalSource, MirrorSnapshotSpec, MirrorSubscriberRunner},
    AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer,
};
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct CountSource {
    count: Arc<AtomicU32>,
    flushes: Arc<AtomicU32>,
}

#[async_trait]
impl MirrorSignalSource for CountSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "count",
        subscribed_kinds: &["Ping"],
        flush_interval_secs: Some(60),
    };

    fn name(&self) -> &'static str {
        "count-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn dummy_signal(kind: &'static str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: kind,
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    }
}

#[tokio::test]
async fn runner_filters_by_subscribed_kinds() {
    let count = Arc::new(AtomicU32::new(0));
    let flushes = Arc::new(AtomicU32::new(0));
    let source = Arc::new(CountSource {
        count: count.clone(),
        flushes: flushes.clone(),
    });
    let runner = MirrorSubscriberRunner::new(source, CancellationToken::new());

    runner.consume(&dummy_signal("Ping")).await.unwrap();
    runner.consume(&dummy_signal("Pong")).await.unwrap();
    runner.consume(&dummy_signal("Ping")).await.unwrap();

    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert_eq!(flushes.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn runner_flushes_on_shutdown() {
    let count = Arc::new(AtomicU32::new(0));
    let flushes = Arc::new(AtomicU32::new(0));
    let source = Arc::new(CountSource {
        count: count.clone(),
        flushes: flushes.clone(),
    });
    let cancel = CancellationToken::new();
    let runner = MirrorSubscriberRunner::new(source, cancel.clone());

    // Manually drive the flush loop with a tiny interval for test speed.
    let handle = runner.clone().spawn_flush_loop(Duration::from_millis(50));
    tokio::time::sleep(Duration::from_millis(120)).await;
    cancel.cancel();
    handle.await.unwrap();

    // At least one interval flush + one shutdown flush.
    assert!(flushes.load(Ordering::Relaxed) >= 2);
}
