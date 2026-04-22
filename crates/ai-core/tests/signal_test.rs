use ai_core::{AiSignal, EntityRef, RecallDomain, SalienceVerdict};
use jiff::Timestamp;

#[test]
fn signal_construction_sets_all_fields() {
    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "TaskCreated",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "Created task: Ship v1".into(),
        entity: Some(EntityRef {
            entity_type: "task",
            id: "abc123".into(),
            name: "Ship v1".into(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
    };
    assert_eq!(sig.event_kind, "TaskCreated");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert!(sig.entity.is_some());
}

#[test]
fn salience_verdict_variants() {
    let _ = SalienceVerdict::Extract;
    let _ = SalienceVerdict::Accumulate;
    let _ = SalienceVerdict::Discard;
}
