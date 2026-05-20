use ai_core::{AiSignal, SignalConsumer};
use feature_coaching::CoachingSignalConsumer;
use tokio::sync::mpsc;

#[tokio::test]
async fn non_coaching_signals_are_dropped() {
    let (tx, mut rx) = mpsc::channel(8);
    let consumer = CoachingSignalConsumer::new(tx);
    let mut sig = dummy_signal();
    sig.coaching_signal = false;
    consumer.consume(&sig).await.unwrap();
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn coaching_signals_forwarded() {
    let (tx, mut rx) = mpsc::channel(8);
    let consumer = CoachingSignalConsumer::new(tx);
    let mut sig = dummy_signal();
    sig.coaching_signal = true;
    consumer.consume(&sig).await.unwrap();
    let fwd = rx.recv().await.unwrap();
    assert_eq!(fwd.event_kind, sig.event_kind);
}

fn dummy_signal() -> AiSignal {
    AiSignal {
        domain: ai_core::RecallDomain::Productivity,
        event_kind: "FocusAlert",
        importance: 0.9,
        salience: ai_core::SalienceVerdict::Extract,
        content: "alert".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: true,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}
