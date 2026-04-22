//! v1.5 integration: DomainEvent → SignalRouter → CoachingSignalConsumer.
//!
//! The full coaching pipeline (accumulator → trigger → reasoner → router) is
//! covered by unit tests in `feature-coaching`. This test exercises the v1.5
//! wiring contract: a `coaching_signal`-flagged AiSignal reaches the consumer's
//! mpsc channel, and a non-coaching signal does not.

use ai_core::{AiSignal, SignalConsumer, SignalRouter};
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn budget_alert_reaches_coaching_consumer_via_ai_pipeline() {
    let bus = Arc::new(DomainEventBus::new(32));
    let (tx, mut rx) = mpsc::channel::<AiSignal>(32);

    let consumer: Arc<dyn SignalConsumer> =
        Arc::new(feature_coaching::CoachingSignalConsumer::new(tx));

    let _router = SignalRouter::start(
        Arc::clone(&bus),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(DomainEvent::BudgetAlert {
        category: "food".into(),
        spent: 450.0,
        limit: 500.0,
    });

    let signal = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for BudgetAlert to reach coaching consumer")
        .expect("coaching consumer channel closed");

    assert!(
        signal.coaching_signal,
        "consumer must only forward coaching_signal=true signals"
    );
    assert_eq!(signal.event_kind, "BudgetAlert");
    assert_eq!(signal.metrics.category.as_deref(), Some("food"));
}

#[tokio::test]
async fn non_coaching_signals_are_filtered_out() {
    let bus = Arc::new(DomainEventBus::new(32));
    let (tx, mut rx) = mpsc::channel::<AiSignal>(32);

    let consumer: Arc<dyn SignalConsumer> =
        Arc::new(feature_coaching::CoachingSignalConsumer::new(tx));

    let _router = SignalRouter::start(
        Arc::clone(&bus),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    // ChatTurnCompleted has coaching_signal=false in its translation.
    bus.publish(DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        user_message: Some("just chatting".into()),
    });

    let result = tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await;
    assert!(
        result.is_err(),
        "non-coaching signal must NOT reach the coaching consumer; got {result:?}"
    );
}
