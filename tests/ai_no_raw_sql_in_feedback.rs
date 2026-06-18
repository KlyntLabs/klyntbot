use std::fs;
use std::path::Path;

#[test]
fn feedback_rs_has_no_behavioral_metric_raw_sql() {
    let path = Path::new("crates/cognitive-reforge/src/feedback.rs");
    let src = fs::read_to_string(path).expect("read feedback.rs");

    // The 5 behavioral metrics that were migrated to registry-driven approach
    // should no longer appear as raw SQL queries
    let dead_metrics = [
        "task_estimation_bias",
        "suggestion_dismiss_rate",
        "forecast_accuracy",
        "coaching_acceptance_rate",
        "focus_quality_trend",
    ];

    for metric in &dead_metrics {
        assert!(
            !src.contains(metric),
            "feedback.rs still references migrated metric '{}' in raw SQL",
            metric
        );
    }
}
