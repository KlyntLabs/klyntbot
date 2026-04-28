//! Inv 6 — scope isolation: repo-scoped retrieval never leaks cross-repo facts
//! (except global `scope_repo_id IS NULL` facts).

use coding_memory::recall::budget::HeuristicBudgeter;
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use proptest::prelude::*;
use std::sync::Arc;
use storage::StoragePool;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn repo_scoped_query_excludes_other_repos(
        repos_count in 2usize..=5,
        facts_per_repo in 1usize..=4,
        query_repo_idx in 0usize..5,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let query_repo_idx = query_repo_idx % repos_count;
            let pool = common::pool_with_migrations().await;
            let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
            let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));

            for r in 0..repos_count {
                for f in 0..facts_per_repo {
                    fact_repo.upsert(&cognitive::types::SemanticFact {
                        id: format!("fact_{r}_{f}"),
                        domain: "code".into(),
                        subject: format!("subject_{r}_{f}"),
                        predicate: "uses".into(),
                        object: "rust".into(),
                        confidence: 0.9,
                        source: "distiller".into(),
                        valid_from: jiff::Timestamp::now().to_string(),
                        valid_until: None,
                        recorded_at: jiff::Timestamp::now().to_string(),
                        superseded_at: None,
                        superseded_by: None,
                        stability: 1.0,
                        last_accessed: None,
                        access_count: 0,
                        convergence_score: 1.0,
                        project_id: None,
                        memory_type: "fact".into(),
                        scope_type: "user".into(),
                        scope_id: None,
                        scope_repo_id: Some(format!("repo_{r}")),
                        metadata: None,
                    }).await.unwrap();
                }
            }
            // Insert one global fact (scope_repo_id IS NULL).
            fact_repo.upsert(&cognitive::types::SemanticFact {
                id: "global_fact".into(),
                domain: "code".into(),
                subject: "global_subject".into(),
                predicate: "is_global".into(),
                object: "true".into(),
                confidence: 0.9,
                source: "distiller".into(),
                valid_from: jiff::Timestamp::now().to_string(),
                valid_until: None,
                recorded_at: jiff::Timestamp::now().to_string(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 1.0,
                project_id: None,
                memory_type: "fact".into(),
                scope_type: "user".into(),
                scope_id: None,
                scope_repo_id: None,
                metadata: None,
            }).await.unwrap();

            let telem = coding_memory::RecallInvocationRepo::new(pool.clone());
            let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
            let svc = CodingRecallService::new(
                CodingRecallServiceConfig::default(),
                ums, fact_repo, ep_repo, telem,
                Arc::new(HeuristicBudgeter),
            );
            let res = svc.recall_index(
                "uses rust",
                Some(&format!("repo_{query_repo_idx}")),
                None,
                None,
                64,
            ).await.unwrap();

            for entry in &res.results {
                let expected_scope = format!("repo:repo_{query_repo_idx}");
                prop_assert!(
                    entry.scope == "global" || entry.scope == expected_scope,
                    "leak: got scope {} when querying repo_{}", entry.scope, query_repo_idx
                );
            }
        });
    }
}
