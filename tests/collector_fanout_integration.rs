//! v1.5 integration: DomainEvent → SignalRouter → cognitive collectors.
//!
//! Confirms the cognitive collectors are now `SignalConsumer` impls that read
//! `AiSignal`s from the router (not `DomainEvent` broadcast subscriptions),
//! and that each collector filters on the right `event_kind` before pushing
//! a `CognitiveSignal` onto the convergence queue.

use ai_core::{SignalConsumer, SignalRouter};
use bus::{DomainEvent, DomainEventBus};
use cognitive::pipeline::{
    signal_queue, AtomCollector, ChatTurnCollector, RecallCollector, SignalSource,
};
use std::sync::Arc;

#[tokio::test]
async fn chat_turn_event_reaches_chat_turn_collector_only() {
    let bus = Arc::new(DomainEventBus::new(32));
    let (tx, mut rx) = signal_queue(32);

    let chat_turn: Arc<dyn SignalConsumer> = Arc::new(ChatTurnCollector::new(tx.clone()));
    let recall: Arc<dyn SignalConsumer> = Arc::new(RecallCollector::new(tx.clone()));
    let atom: Arc<dyn SignalConsumer> = Arc::new(AtomCollector::new(tx.clone()));

    let _router = SignalRouter::start(
        Arc::clone(&bus),
        vec![chat_turn, recall, atom],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        // long enough to clear the collector's min-length filter.
        user_message: Some("a sufficiently long message about my preferences and tasks".into()),
    });

    let sig = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timed out waiting for ChatTurnCollector to emit a CognitiveSignal")
        .expect("convergence queue closed");

    assert_eq!(sig.source, SignalSource::ChatTurn);

    // RecallCollector buffers silently; AtomCollector ignores ChatTurnCompleted.
    // No further CognitiveSignals should arrive in the next 200 ms.
    let extra = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
    assert!(
        extra.is_err(),
        "only ChatTurnCollector should emit; got extra: {extra:?}"
    );
}
