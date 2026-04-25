use coding_memory::recall::telemetry::{RecallInvocationRepo, RecallInvocationRow};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn insert_then_list() {
    let pool = fresh_pool().await;
    let repo = RecallInvocationRepo::new(pool.clone());
    let row = RecallInvocationRow {
        id: Uuid::new_v4(),
        occurred_at: Timestamp::now(),
        session_id: Some("sess1".into()),
        turn_id: Some("t1".into()),
        repo_id: Some("repo:foo".into()),
        layer: "index".into(),
        query: "null pointer parser".into(),
        coverage_score: Some(0.42),
        skill_used: None,
        latency_ms: 17,
        result_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        rendered_tokens: None,
        metadata: serde_json::json!({}),
    };
    repo.insert(&row).await.expect("insert");
    let page = repo
        .list_by_session("sess1", 50, 0)
        .await
        .expect("list");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].layer, "index");
    assert_eq!(page[0].result_ids.len(), 2);
}

#[tokio::test]
async fn list_paginates() {
    let pool = fresh_pool().await;
    let repo = RecallInvocationRepo::new(pool.clone());
    for i in 0..5 {
        let row = RecallInvocationRow {
            id: Uuid::new_v4(),
            occurred_at: Timestamp::now(),
            session_id: Some("s".into()),
            turn_id: Some(format!("t{i}")),
            repo_id: None,
            layer: "index".into(),
            query: format!("q{i}"),
            coverage_score: None,
            skill_used: None,
            latency_ms: 1,
            result_ids: vec![],
            rendered_tokens: None,
            metadata: serde_json::json!({}),
        };
        repo.insert(&row).await.unwrap();
    }
    let page1 = repo.list_by_session("s", 2, 0).await.unwrap();
    let page2 = repo.list_by_session("s", 2, 2).await.unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 2);
    assert_ne!(page1[0].id, page2[0].id);
}
