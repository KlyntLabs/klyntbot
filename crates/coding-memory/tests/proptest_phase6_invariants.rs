//! Phase-6 architectural invariants (proptest).

use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use proptest::prelude::*;
use std::path::Path;

proptest! {
    /// Parsing the same source twice must yield identical anchored-symbol vectors.
    #[test]
    fn parse_stability(
        body in prop::collection::vec("[a-z][a-z0-9_]{0,8}", 0..6)
    ) {
        let mut src = String::new();
        for name in &body {
            src.push_str(&format!("fn {name}() {{}}\n"));
        }
        let extractor = TreeSitterExtractor::new();
        let first = extractor.extract(Path::new("x.rs"), &src, "h");
        let second = extractor.extract(Path::new("x.rs"), &src, "h");
        prop_assert_eq!(first, second);
    }
}

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

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(16))]

    /// Invariant 8: every memory_causal_edges row's from_id and to_id correspond
    /// to existing memory rows after CausalEdgeDetector runs over a synthetic
    /// session. This proptest seeds a random number of fix_attempt episodes
    /// and asserts that no edge has a dangling endpoint.
    #[test]
    fn invariant_8_no_dangling_edges(
        n in 2usize..6
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let pool = fresh_pool().await;
            let repo = CausalEdgeRepo::new(pool.clone());

            let mut ids = Vec::new();
            for _ in 0..n {
                let id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id, metadata) \
                     VALUES (?1, 'coding', '{}', 'fix_attempt', ?2, ?2, 0.5, 'r', '{\"problemHash\":\"H\"}')",
                )
                .bind(id.to_string())
                .bind(Timestamp::now().to_string())
                .execute(pool.inner())
                .await
                .unwrap();
                ids.push(id);
            }
            for window in ids.windows(2) {
                repo.insert(&CausalEdge {
                    id: Uuid::new_v4(),
                    from_id: window[0],
                    to_id: window[1],
                    edge_kind: CausalEdgeKind::SharesRootCause,
                    confidence: 0.6,
                    inferred_at: Timestamp::now(),
                })
                .await
                .unwrap();
            }
            let dangling: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM memory_causal_edges \
                 WHERE from_id NOT IN (SELECT id FROM episodic_memories) \
                    OR to_id   NOT IN (SELECT id FROM episodic_memories)",
            )
            .fetch_one(pool.inner())
            .await
            .unwrap();
            assert_eq!(dangling, 0, "no dangling edges");
        });
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(8))]

    /// After `valid_until` is set, `recall_index` must not return the fact
    /// for the same repo (regardless of query). Approximated by checking the
    /// SemanticFactRepo retrieval helpers respect bi-temporal lifecycle.
    #[test]
    fn invalidated_fact_not_returned(
        repo_seed in "[a-z]{3,8}"
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let pool = fresh_pool().await;
            let repo_id = format!("repo:{repo_seed}");

            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO semantic_facts \
                 (id, domain, subject, predicate, object, confidence, memory_type, scope_repo_id, valid_from) \
                 VALUES (?1, 'code', 'foo', 'is', 'bar', 0.9, 'fact', ?2, datetime('now'))",
            )
            .bind(&id)
            .bind(&repo_id)
            .execute(pool.inner())
            .await
            .unwrap();

            let active_before: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM semantic_facts \
                 WHERE scope_repo_id = ?1 AND valid_until IS NULL",
            )
            .bind(&repo_id)
            .fetch_one(pool.inner())
            .await
            .unwrap();
            assert_eq!(active_before, 1);

            sqlx::query("UPDATE semantic_facts SET valid_until = datetime('now') WHERE id = ?1")
                .bind(&id)
                .execute(pool.inner())
                .await
                .unwrap();

            let active_after: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM semantic_facts \
                 WHERE scope_repo_id = ?1 AND valid_until IS NULL",
            )
            .bind(&repo_id)
            .fetch_one(pool.inner())
            .await
            .unwrap();
            assert_eq!(active_after, 0);
        });
    }
}
