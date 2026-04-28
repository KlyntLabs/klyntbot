//! Tier A activation: UserCorrectedAI with MemoryMiss fires when user
//! corrects a coding-context retrieval.

use bus::{CorrectionKind, DomainEvent};

#[tokio::test]
async fn memory_miss_correction_propagates_for_coding_repo() {
    let bus = bus::DomainEventBus::new(1024);
    let mut sub = bus.subscribe();
    bus.publish(DomainEvent::UserCorrectedAI {
        original: "old".into(),
        correction: "new".into(),
        kind: CorrectionKind::MemoryMiss,
        strength: 1.0,
        session_key: "coding:myrepo".into(),
        active_skill: None,
    });
    let evt = sub.recv().await.unwrap();
    assert!(matches!(
        evt,
        DomainEvent::UserCorrectedAI {
            kind: CorrectionKind::MemoryMiss,
            ..
        }
    ));
}
