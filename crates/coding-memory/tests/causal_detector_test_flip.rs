//! When the same test transitions pass → fail across consecutive episodes
//! within the session, emit a `FlippedToFail` edge.

use coding_memory::causal::{CausalEdgeDetector, CausalEdgeRepo};
use coding_memory::scope::CausalEdgeKind;
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;

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
async fn pass_then_fail_emits_flipped_to_fail_edge() {
    let pool = fresh_pool().await;

    // Seed two `test_run` episodes for session "s1": first passes, second fails.
    let pass_id = uuid::Uuid::new_v4();
    let fail_id = uuid::Uuid::new_v4();
    let pass_body = serde_json::json!({"command":"cargo test","passed":1,"failed":0,"failedTests":[],"sessionId":"s1"});
    let fail_body = serde_json::json!({"command":"cargo test","passed":0,"failed":1,"failedTests":["t1"],"sessionId":"s1"});
    for (id, body, ts) in [
        (pass_id, &pass_body, Timestamp::from_second(1).unwrap()),
        (fail_id, &fail_body, Timestamp::from_second(2).unwrap()),
    ] {
        sqlx::query(
            "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id) \
             VALUES (?1, 'coding', ?2, 'test_run', ?3, ?3, 0.5, 'repo:test')",
        )
        .bind(id.to_string())
        .bind(body.to_string())
        .bind(ts.to_string())
        .execute(pool.inner())
        .await
        .unwrap();
    }

    let edge_repo = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let detector = CausalEdgeDetector::new(edge_repo.clone(), ep_repo);
    let count = detector.detect_for_session("s1").await.unwrap();
    assert_eq!(count, 1);

    let edges = edge_repo.by_to(fail_id).await.unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].edge_kind, CausalEdgeKind::FlippedToFail);
    assert_eq!(edges[0].from_id, pass_id);
}
