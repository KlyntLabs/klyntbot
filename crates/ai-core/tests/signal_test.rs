use ai_core::{AiMetrics, AiSignal, EntityRef, RecallDomain, SalienceVerdict};
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
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
    };
    assert_eq!(sig.event_kind, "TaskCreated");
    assert_eq!(sig.importance, 0.7);
    assert!(matches!(sig.salience, SalienceVerdict::Accumulate));
    assert!(sig.entity.is_some());
}

#[test]
fn signal_carries_metrics_and_coaching_flags() {
    let metrics = AiMetrics {
        app: Some("reddit".into()),
        amount: Some(42.0),
        category: Some("food".into()),
    };
    let sig = AiSignal {
        domain: RecallDomain::Finance,
        event_kind: "BudgetAlert",
        importance: 0.9,
        salience: SalienceVerdict::Extract,
        content: "alert".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: metrics.clone(),
        coaching_signal: true,
        coaching_rule: Some("Review spending when budget pressure rises".into()),
    };
    assert_eq!(sig.metrics.app.as_deref(), Some("reddit"));
    assert_eq!(sig.metrics.amount, Some(42.0));
    assert!(sig.coaching_signal);
    assert!(sig.coaching_rule.is_some());
}

#[test]
fn metrics_default_all_none() {
    let m = AiMetrics::default();
    assert!(m.app.is_none() && m.amount.is_none() && m.category.is_none());
}

#[test]
fn salience_verdict_variants() {
    let _ = SalienceVerdict::Extract;
    let _ = SalienceVerdict::Accumulate;
    let _ = SalienceVerdict::Discard;
}

#[test]
fn mirror_domain_roundtrips() {
    use ai_core::RecallDomain;
    assert_eq!(RecallDomain::Mirror.as_str(), "mirror");
    assert_eq!(
        RecallDomain::from_str_or_general("mirror"),
        RecallDomain::Mirror
    );
}
