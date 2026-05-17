use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::{Arc, Mutex};

struct RecordingConsumer {
    seen: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl SignalConsumer for RecordingConsumer {
    fn name(&self) -> &'static str {
        "recording"
    }
    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        self.seen.lock().unwrap().push(signal.event_kind);
        Ok(())
    }
}

#[tokio::test]
async fn consumer_receives_signal() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let consumer: Arc<dyn SignalConsumer> = Arc::new(RecordingConsumer { seen: log.clone() });
    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "TaskCreated",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "x".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    };
    consumer.consume(&sig).await.unwrap();
    assert_eq!(log.lock().unwrap().as_slice(), &["TaskCreated"]);
}

use ai_core::{RecallProvider, RecallProviderRegistry, RecallQuery};

struct FakeProvider(RecallDomain, f64);
impl RecallProvider for FakeProvider {
    fn domain(&self) -> RecallDomain {
        self.0
    }
    fn score_query(&self, _q: &RecallQuery) -> f64 {
        self.1
    }
}

#[test]
fn registry_iterates_providers() {
    let reg = RecallProviderRegistry::new()
        .with(FakeProvider(RecallDomain::Tasks, 0.9))
        .with(FakeProvider(RecallDomain::Finance, 0.4));
    let q = RecallQuery {
        message: "deadline".into(),
    };
    let ranked = reg.rank(&q);
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].0, RecallDomain::Tasks);
    assert!((ranked[0].1 - 0.9).abs() < 1e-9);
}

#[test]
fn registry_filters_zero_scores() {
    let reg = RecallProviderRegistry::new().with(FakeProvider(RecallDomain::Tasks, 0.0));
    let q = RecallQuery {
        message: "x".into(),
    };
    assert!(reg.rank(&q).is_empty());
}
