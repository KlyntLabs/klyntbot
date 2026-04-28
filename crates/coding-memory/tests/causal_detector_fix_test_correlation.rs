//! When a FixAttempt episode and a TestRun episode share the same turn,
//! emit a `FixedBy` edge if the test passes after the fix, or `Broke` if
//! the test starts failing after the fix.

use coding_memory::causal::{CausalEdgeDetector, CausalEdgeRepo};
use coding_memory::scope::CausalEdgeKind;
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

async fn seeded_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

async fn insert_episode(
    pool: &StoragePool,
    id: Uuid,
    kind: &str,
    content: serde_json::Value,
    ts: Timestamp,
) {
    sqlx::query(
        "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id) \
         VALUES (?1, 'coding', ?2, ?3, ?4, ?4, 0.5, 'repo:test')",
    )
    .bind(id.to_string())
    .bind(content.to_string())
    .bind(kind)
    .bind(ts.to_string())
    .execute(pool.inner())
    .await
    .unwrap();
}

#[tokio::test]
async fn fix_then_passing_test_emits_fixed_by() {
    let pool = seeded_pool().await;
    let fix = Uuid::new_v4();
    let test = Uuid::new_v4();
    insert_episode(
        &pool,
        fix,
        "fix_attempt",
        serde_json::json!({"sessionId":"s1","turnId":"t1","outcome":"success"}),
        Timestamp::from_second(1).unwrap(),
    )
    .await;
    insert_episode(
        &pool,
        test,
        "test_run",
        serde_json::json!({"sessionId":"s1","turnId":"t1","passed":3,"failed":0}),
        Timestamp::from_second(2).unwrap(),
    )
    .await;

    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let eps = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let detector = CausalEdgeDetector::new(edges.clone(), eps);
    let count = detector.detect_for_session("s1").await.unwrap();
    assert!(count >= 1);

    let by_to = edges.by_to(test).await.unwrap();
    assert!(
        by_to
            .iter()
            .any(|e| e.edge_kind == CausalEdgeKind::FixedBy && e.from_id == fix),
        "expected FixedBy edge from {fix} to {test}; got {:?}",
        by_to
    );
}

#[tokio::test]
async fn fix_then_failing_test_emits_broke() {
    let pool = seeded_pool().await;
    let fix = Uuid::new_v4();
    let test = Uuid::new_v4();
    insert_episode(
        &pool,
        fix,
        "fix_attempt",
        serde_json::json!({"sessionId":"s2","turnId":"t1","outcome":"success"}),
        Timestamp::from_second(1).unwrap(),
    )
    .await;
    insert_episode(
        &pool,
        test,
        "test_run",
        serde_json::json!({"sessionId":"s2","turnId":"t1","passed":0,"failed":2}),
        Timestamp::from_second(2).unwrap(),
    )
    .await;

    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let eps = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let detector = CausalEdgeDetector::new(edges.clone(), eps);
    detector.detect_for_session("s2").await.unwrap();
    let by_from = edges.by_from(fix).await.unwrap();
    assert!(by_from.iter().any(|e| e.edge_kind == CausalEdgeKind::Broke));
}
