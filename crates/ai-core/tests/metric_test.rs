use ai_core::metric::{Aggregation, MetricSample, MetricSpec};

#[test]
fn metric_spec_fields_constructable_at_const() {
    const SPEC: MetricSpec = MetricSpec {
        name: "task_estimation_bias",
        window_secs: 7 * 86_400,
        min_samples: 3,
        aggregation: Aggregation::Avg,
    };
    assert_eq!(SPEC.name, "task_estimation_bias");
    assert_eq!(SPEC.window_secs, 604_800);
    assert_eq!(SPEC.min_samples, 3);
    assert!(matches!(SPEC.aggregation, Aggregation::Avg));
}

#[test]
fn aggregation_variants() {
    let _ = Aggregation::Avg;
    let _ = Aggregation::Sum;
    let _ = Aggregation::Count;
}

#[test]
fn metric_sample_carries_name_and_value() {
    let s = MetricSample { name: "coaching_acceptance_rate", value: 1.0 };
    assert_eq!(s.name, "coaching_acceptance_rate");
    assert_eq!(s.value, 1.0);
}

#[test]
fn metric_sample_is_copy() {
    let s = MetricSample { name: "x", value: 0.5 };
    let _s2 = s;
    let _s3 = s;
}

#[test]
fn aggregation_as_sql_expr() {
    assert_eq!(Aggregation::Avg.as_sql_expr(), "AVG(value)");
    assert_eq!(Aggregation::Sum.as_sql_expr(), "SUM(value)");
    assert_eq!(Aggregation::Count.as_sql_expr(), "CAST(COUNT(*) AS REAL)");
}
