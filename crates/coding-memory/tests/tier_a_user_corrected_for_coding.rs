//! Tier A activation: UserCorrectedAI with MemoryMiss fires when user
//! corrects a coding-context retrieval.

use bus::{DomainEvent, CorrectionKind};

#[tokio::test]
async fn memory_miss_correction_propagates_for_coding_repo() {
    let bus = bus::DomainEventBus::new();
    let mut sub = bus.subscribe();
    bus.publish(DomainEvent::UserCorrectedAI {
        kind: CorrectionKind::MemoryMiss,
        repo: Some("myrepo".into()),
        memory_id: Some("ep_1".into()),
    }).await;
    let evt = sub.recv().await.unwrap();
    assert!(matches!(evt, DomainEvent::UserCorrectedAI { kind: CorrectionKind::MemoryMiss, .. }));
}
