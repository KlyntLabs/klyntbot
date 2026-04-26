use cognitive::mirror::{MirrorAlert, MirrorAlertSeverity, MirrorRepo};
use storage::StoragePool;

#[tokio::test]
async fn coding_alert_persists_with_kind_and_severity() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::repos::cognitive_migrations())
        .await
        .unwrap();

    let repo = MirrorRepo::new(pool.clone());
    let alert = MirrorAlert::Coding {
        kind: "project_skill_obsolete".into(),
        severity: MirrorAlertSeverity::High,
        payload: serde_json::json!({"skill": "foo", "drop_percent": 60.0}),
    };
    let snippet = cognitive::mirror::snippet_from_alert(&alert);
    repo.insert_snippet(&snippet).await.unwrap();

    let (kind, severity): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT coding_alert_kind, coding_alert_severity FROM mirror_snippets LIMIT 1",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(kind.as_deref(), Some("project_skill_obsolete"));
    assert_eq!(severity.as_deref(), Some("high"));
}
