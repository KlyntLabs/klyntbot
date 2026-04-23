use ai_core::AiEventMeta;

#[test]
fn strategy_applied_emits_acceptance_sample() {
    let e = feature_coaching::events::CoachingEvent::StrategyApplied {
        strategy_id: "s".into(),
        rule_text: "review spending".into(),
        accepted: true,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "coaching_acceptance_rate")
        .unwrap();
    assert!((s.value - 1.0).abs() < 1e-9);
}

#[test]
fn strategy_rejected_emits_zero_sample() {
    let e = feature_coaching::events::CoachingEvent::StrategyApplied {
        strategy_id: "s".into(),
        rule_text: "review spending".into(),
        accepted: false,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "coaching_acceptance_rate")
        .unwrap();
    assert!(s.value.abs() < 1e-9);
}
