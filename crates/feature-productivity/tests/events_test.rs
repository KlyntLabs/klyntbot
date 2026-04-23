use ai_core::AiEventMeta;

#[test]
fn session_ended_emits_focus_quality_sample() {
    let e = feature_productivity::events::ProductivityEvent::SessionEnded {
        session_id: "s".into(),
        quality: 0.82,
        duration_mins: 45,
    };
    let sig = e.to_signal();
    let s = sig
        .metric_samples
        .iter()
        .find(|s| s.name == "focus_quality_trend")
        .unwrap();
    assert!((s.value - 0.82).abs() < 1e-9);
}
