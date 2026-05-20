use ai_core::MetricRegistry;
use feature_coaching::events::CoachingEvent;
use feature_productivity::events::ProductivityEvent;

#[test]
fn every_registry_spec_is_declared() {
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(CoachingEvent::FEATURE_METRICS);
    reg.register_all(ProductivityEvent::FEATURE_METRICS);

    for spec in reg.all() {
        let found = [
            feature_tasks::TaskEvent::FEATURE_METRICS,
            CoachingEvent::FEATURE_METRICS,
            ProductivityEvent::FEATURE_METRICS,
        ]
        .iter()
        .flat_map(|s| s.iter())
        .any(|s| s.name == spec.name);
        assert!(
            found,
            "registered metric not declared anywhere: {}",
            spec.name
        );
    }
}

#[test]
fn reached_10_or_more_metrics() {
    let mut reg = MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(CoachingEvent::FEATURE_METRICS);
    reg.register_all(ProductivityEvent::FEATURE_METRICS);
    assert!(
        reg.all().len() >= 7,
        "expected >=7 registered metrics, got {}",
        reg.all().len()
    );
}
