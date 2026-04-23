use ai_core::metric::{Aggregation, MetricRegistry, MetricSample, MetricSpec};

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
    let s = MetricSample {
        name: "coaching_acceptance_rate",
        value: 1.0,
    };
    assert_eq!(s.name, "coaching_acceptance_rate");
    assert_eq!(s.value, 1.0);
}

#[test]
fn metric_sample_is_copy() {
    let s = MetricSample {
        name: "x",
        value: 0.5,
    };
    let _s2 = s;
    let _s3 = s;
}

#[test]
fn aggregation_as_sql_expr() {
    assert_eq!(Aggregation::Avg.as_sql_expr(), "AVG(value)");
    assert_eq!(Aggregation::Sum.as_sql_expr(), "SUM(value)");
    assert_eq!(Aggregation::Count.as_sql_expr(), "CAST(COUNT(*) AS REAL)");
}

#[test]
fn registry_starts_empty() {
    let r = MetricRegistry::new();
    assert_eq!(r.all().len(), 0);
    assert!(r.get("anything").is_none());
}

#[test]
fn registry_collects_specs() {
    static SPEC_A: MetricSpec = MetricSpec {
        name: "a",
        window_secs: 60,
        min_samples: 1,
        aggregation: Aggregation::Avg,
    };
    static SPEC_B: MetricSpec = MetricSpec {
        name: "b",
        window_secs: 3600,
        min_samples: 2,
        aggregation: Aggregation::Sum,
    };

    let mut r = MetricRegistry::new();
    r.register(&SPEC_A);
    r.register_all(&[&SPEC_B]);

    assert_eq!(r.all().len(), 2);
    assert_eq!(r.get("a").unwrap().name, "a");
    assert_eq!(r.get("b").unwrap().window_secs, 3600);
}

#[test]
fn registry_skips_duplicate_names() {
    static SPEC_1: MetricSpec = MetricSpec {
        name: "dup",
        window_secs: 60,
        min_samples: 1,
        aggregation: Aggregation::Avg,
    };
    static SPEC_2: MetricSpec = MetricSpec {
        name: "dup",
        window_secs: 120,
        min_samples: 2,
        aggregation: Aggregation::Sum,
    };

    let mut r = MetricRegistry::new();
    r.register(&SPEC_1);
    r.register(&SPEC_2); // should silently skip
    assert_eq!(r.all().len(), 1);
    assert_eq!(r.all()[0].window_secs, 60); // first one wins
}
