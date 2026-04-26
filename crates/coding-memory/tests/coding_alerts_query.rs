use coding_memory::mirror::coding_alerts_query::{CodingAlertFilter, CodingAlertsQuery};
use storage::StoragePool;

async fn seed(pool: &StoragePool, kind: &str, severity: &str) {
    sqlx::query(
        "INSERT INTO mirror_snippets \
         (id, created_at, alert_type, headline, body, coding_alert_kind, coding_alert_severity) \
         VALUES (?1, ?2, 'Coding', ?3, ?4, ?5, ?6)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(jiff::Timestamp::now().to_string())
    .bind(format!("{kind}-headline"))
    .bind("{}")
    .bind(kind)
    .bind(severity)
    .execute(pool.inner())
    .await
    .unwrap();
}

#[tokio::test]
async fn filters_by_kind_and_severity() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    seed(&pool, "project_skill_obsolete", "high").await;
    seed(&pool, "stale_fact_detected", "medium").await;
    seed(&pool, "project_skill_obsolete", "low").await;

    let q = CodingAlertsQuery::new(pool.clone());
    let rows = q
        .query(&CodingAlertFilter {
            kind: Some("project_skill_obsolete".into()),
            severity: Some("high".into()),
            repo: None,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}
