use coding_memory::causal::{CausalEdgeDetector, CausalEdgeRepo};
use coding_memory::scope::CausalEdgeKind;
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn two_fix_attempts_with_shared_problem_hash_emit_shares_root_cause() {
    let pool = fresh_pool().await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    for (id, ts) in [(a, 1_i64), (b, 2_i64)] {
        sqlx::query(
            "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id, metadata) \
             VALUES (?1, 'coding', ?2, 'fix_attempt', ?3, ?3, 0.5, 'repo:test', ?4)",
        )
        .bind(id.to_string())
        .bind(serde_json::json!({"sessionId":"s1"}).to_string())
        .bind(Timestamp::from_second(ts).unwrap().to_string())
        .bind(r#"{"problemHash":"H1"}"#)
        .execute(pool.inner())
        .await
        .unwrap();
    }
    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let eps = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let detector = CausalEdgeDetector::new(edges.clone(), eps);
    detector.detect_for_session("s1").await.unwrap();

    let by_from = edges.by_from(a).await.unwrap();
    assert!(by_from
        .iter()
        .any(|e| e.edge_kind == CausalEdgeKind::SharesRootCause && e.to_id == b));
}
