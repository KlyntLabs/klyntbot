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
        metric_samples: Vec::new(),
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
        domain: RecallDomain::Productivity,
        event_kind: "FocusSessionStarted",
        importance: 0.9,
        salience: SalienceVerdict::Extract,
        content: "alert".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: metrics.clone(),
        coaching_signal: true,
        coaching_rule: Some("Review spending when budget pressure rises".into()),
        metric_samples: Vec::new(),
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

#[test]
fn signal_carries_metric_samples() {
    use ai_core::{AiSignal, MetricSample, RecallDomain, SalienceVerdict};

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "EstimationRecorded",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: "est 30m actual 45m".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![MetricSample {
            name: "task_estimation_bias",
            value: 0.5,
        }],
    };
    assert_eq!(sig.metric_samples.len(), 1);
    assert_eq!(sig.metric_samples[0].name, "task_estimation_bias");
}

#[test]
fn signal_metric_samples_default_empty() {
    use ai_core::{AiSignal, RecallDomain, SalienceVerdict};

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Created",
        importance: 0.7,
        salience: SalienceVerdict::Accumulate,
        content: "Task created".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    };
    assert!(sig.metric_samples.is_empty());
}

#[test]
fn coaching_domain_roundtrips() {
    use ai_core::RecallDomain;
    assert_eq!(RecallDomain::Coaching.as_str(), "coaching");
    assert_eq!(
        RecallDomain::from_str_or_general("coaching"),
        RecallDomain::Coaching
    );
}
