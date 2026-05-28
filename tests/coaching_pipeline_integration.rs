//! v1.5 integration: DomainEvent → SignalRouter → CoachingSignalConsumer.
//!
//! The full coaching pipeline (accumulator → trigger → reasoner → router) is
//! covered by unit tests in `feature-coaching`. This test exercises the v1.5
//! wiring contract: a `coaching_signal`-flagged AiSignal reaches the consumer's
//! mpsc channel, and a non-coaching signal does not.

use ai_core::{AiSignal, SignalConsumer, SignalRouter};
use bus::{CoachingEvent, DomainEvent, DomainEventBus, NoteEvent};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn coaching_feedback_reaches_coaching_consumer_via_ai_pipeline() {
    let bus = Arc::new(DomainEventBus::new(32));
    let (tx, mut rx) = mpsc::channel::<AiSignal>(32);

    let consumer: Arc<dyn SignalConsumer> =
        Arc::new(feature_coaching::CoachingSignalConsumer::new(tx));

    let _router = SignalRouter::start(
        Arc::clone(&bus),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(DomainEvent::Coaching(CoachingEvent::CoachingFeedback {
        intervention_id: "i1".into(),
        response: bus::FeedbackResponse::Helpful,
    }));

    let signal = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for CoachingFeedback to reach coaching consumer")
        .expect("coaching consumer channel closed");

    assert!(
        signal.coaching_signal,
        "consumer must only forward coaching_signal=true signals"
    );
    assert_eq!(signal.event_kind, "CoachingFeedback");
    assert_eq!(signal.metrics.category.as_deref(), Some("thumbs_up"));
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

    // Publish a non-coaching event.
    bus.publish(DomainEvent::Note(NoteEvent::NoteCreated {
        note_id: "n1".into(),
        title: "Test note".into(),
    }));

    let result = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
    assert!(
        result.is_err() || result.unwrap().is_none(),
        "non-coaching signals must be filtered out"
    );
}
