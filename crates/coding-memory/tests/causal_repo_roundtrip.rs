use coding_memory::causal::CausalEdgeRepo;
use coding_memory::scope::{CausalEdge, CausalEdgeKind};
use jiff::Timestamp;
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

fn edge(from: Uuid, to: Uuid, kind: CausalEdgeKind) -> CausalEdge {
    CausalEdge {
        id: Uuid::new_v4(),
        from_id: from,
        to_id: to,
        edge_kind: kind,
        confidence: 0.7,
        inferred_at: Timestamp::now(),
    }
}

#[tokio::test]
async fn insert_then_by_from_returns_row() {
    let pool = fresh_pool().await;
    let repo = CausalEdgeRepo::new(pool.clone());
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let e = edge(a, b, CausalEdgeKind::FlippedToFail);
    repo.insert(&e).await.unwrap();
    let got = repo.by_from(a).await.unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].to_id, b);
    assert_eq!(got[0].edge_kind, CausalEdgeKind::FlippedToFail);
}

#[tokio::test]
async fn by_to_returns_inbound_rows() {
    let pool = fresh_pool().await;
    let repo = CausalEdgeRepo::new(pool.clone());
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    repo.insert(&edge(a, c, CausalEdgeKind::FixedBy)).await.unwrap();
    repo.insert(&edge(b, c, CausalEdgeKind::Broke)).await.unwrap();
    let got = repo.by_to(c).await.unwrap();
    assert_eq!(got.len(), 2);
}

#[tokio::test]
async fn groups_by_problem_hash_filters_min_count() {
    let pool = fresh_pool().await;
    let repo = CausalEdgeRepo::new(pool.clone());

    // Seed 3 episodic_memories with shared problemHash via raw SQL.
    let problem_hash = "abc123";
    let mut ep_ids = Vec::new();
    for _ in 0..3 {
        let ep_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id, metadata) \
             VALUES (?1, 'coding', ?2, 'fix_attempt', ?3, ?3, 0.5, 'repo:test', ?4)",
        )
        .bind(ep_id.to_string())
        .bind("body")
        .bind(Timestamp::now().to_string())
        .bind(format!(r#"{{"problemHash":"{problem_hash}"}}"#))
        .execute(pool.inner())
        .await
        .unwrap();
        ep_ids.push(ep_id);
    }

    // 3 edges among them.
    repo.insert(&edge(ep_ids[0], ep_ids[1], CausalEdgeKind::SharesRootCause))
        .await
        .unwrap();
    repo.insert(&edge(ep_ids[1], ep_ids[2], CausalEdgeKind::SharesRootCause))
        .await
        .unwrap();
    repo.insert(&edge(ep_ids[0], ep_ids[2], CausalEdgeKind::SharesRootCause))
        .await
        .unwrap();

    let since = Timestamp::from_second(0).unwrap();
    let groups = repo
        .groups_by_problem_hash(Some("repo:test"), since, 3)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].problem_hash, problem_hash);
    assert_eq!(groups[0].edge_ids.len(), 3);

    // min_count = 4 → no groups.
    let none = repo
        .groups_by_problem_hash(Some("repo:test"), since, 4)
        .await
        .unwrap();
    assert!(none.is_empty());
}
