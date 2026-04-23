use ai_core::{MetricRegistry, SignalConsumer};
use cognitive::consumers::MetricHarvestConsumer;
use cognitive::repos::MetricRepo;
use cognitive::services::reforge::feedback::load_behavioral_metrics;
use storage::StoragePool;

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();
    storage::StoragePool::run_feature_migrations(&inner, &cognitive::repos::cognitive_migrations())
        .await
        .unwrap();
    inner
}

#[tokio::test]
async fn task_estimation_recorded_flows_end_to_end() {
    let pool = setup_pool().await;
    let metric_repo = MetricRepo::new(pool.clone());
    let consumer = MetricHarvestConsumer::new(metric_repo.clone());

    let mut registry = MetricRegistry::new();
    registry.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);

    for dp in [0.1_f64, 0.3, 0.5] {
        let e = feature_tasks::TaskEvent::EstimationRecorded {
            task_id: "t".into(),
            estimated_minutes: Some(30),
            actual_minutes: Some(45),
            deviation_pct: dp,
        };
        let sig = <feature_tasks::TaskEvent as ai_core::AiEventMeta>::to_signal(&e);
        consumer.consume(&sig).await.unwrap();
    }

    let bm = load_behavioral_metrics(&metric_repo, &registry).await;
    let bias = bm
        .get("task_estimation_bias")
        .expect("task_estimation_bias present");
    assert!((bias - 0.3).abs() < 1e-9, "expected 0.3, got {}", bias);
}

#[tokio::test]
async fn dead_metric_not_present_in_behavioral_metrics() {
    let pool = setup_pool().await;
    let metric_repo = MetricRepo::new(pool.clone());
    let mut registry = MetricRegistry::new();
    registry.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    let bm = load_behavioral_metrics(&metric_repo.clone(), &registry).await;
    assert!(bm.get("suggestion_dismiss_rate").is_none());
    assert!(bm.get("forecast_accuracy").is_none());
}
