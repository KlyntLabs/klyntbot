use ai_core_macros::AiEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Tasks")]
pub enum TaskMetricDemo {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "est {estimated_mins}m actual {actual_mins}m",
        metric(
            name = "task_estimation_bias",
            value_from = *deviation_pct,
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    EstimationRecorded {
        task_id: String,
        estimated_mins: u32,
        actual_mins: u32,
        deviation_pct: f64,
    },

    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Completed {title}",
        metric(
            name = "task_completion_rate",
            value_from = 1.0_f64,
            window = "1d",
            min_samples = 5,
            aggregation = "avg",
        ),
    )]
    Completed { task_id: String, title: String },
}

fn main() {
    use ai_core::AiEventMeta;
    let e = TaskMetricDemo::EstimationRecorded {
        task_id: "abc".into(),
        estimated_mins: 30,
        actual_mins: 45,
        deviation_pct: 0.5,
    };
    let sig = e.to_signal();
    assert_eq!(sig.metric_samples.len(), 1);
    assert_eq!(sig.metric_samples[0].name, "task_estimation_bias");
    assert!((sig.metric_samples[0].value - 0.5).abs() < 1e-9);

    assert_eq!(TaskMetricDemo::FEATURE_METRICS.len(), 2);
    let bias = TaskMetricDemo::FEATURE_METRICS
        .iter()
        .find(|s| s.name == "task_estimation_bias")
        .unwrap();
    assert_eq!(bias.window_secs, 7 * 86_400);
    assert_eq!(bias.min_samples, 3);
    assert!(matches!(bias.aggregation, ai_core::Aggregation::Avg));
}
