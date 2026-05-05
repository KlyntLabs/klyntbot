//! Inv 2 — Distiller never deletes: row count of semantic_facts and
//! episodic_memories is monotone non-decreasing across writes.

use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use jiff::Timestamp;
use proptest::prelude::*;
use uuid::Uuid;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn row_count_monotone_non_decreasing(
        fact_count in 0usize..=5,
        episode_count in 0usize..=3,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = common::pool_with_migrations().await;
            let writer = common::writer_for_pool(&pool);

            let mut prev_facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
                .fetch_one(pool.inner()).await.unwrap();
            let mut prev_eps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner()).await.unwrap();

            for i in 0..fact_count {
                let prov = ProvenanceMetadata {
                    source_events: vec![Uuid::new_v4()],
                    session_id: format!("s{i}"),
                    turn_id: None,
                    distilled_at: Timestamp::now(),
                    distiller_model: "test".into(),
                    source_kind: ProvenanceKind::DistillerLlm,
                };
                let mut prepared = common::prepared_fact_with_prov(prov);
                prepared.fact.id = Uuid::new_v4().to_string();
                prepared.fact.subject = format!("subject_{i}");
                writer.write_fact(prepared).await.unwrap();
            }

            let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
                .fetch_one(pool.inner()).await.unwrap();
            let eps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner()).await.unwrap();

            prop_assert!(facts >= prev_facts, "facts decreased {prev_facts} -> {facts}");
            prop_assert!(eps >= prev_eps, "episodes decreased {prev_eps} -> {eps}");
            prev_facts = facts;
            prev_eps = eps;

            for i in 0..episode_count {
                let prov = ProvenanceMetadata {
                    source_events: vec![Uuid::new_v4()],
                    session_id: format!("s{i}"),
                    turn_id: None,
                    distilled_at: Timestamp::now(),
                    distiller_model: "test".into(),
                    source_kind: ProvenanceKind::DistillerLlm,
                };
                let episode = coding_memory::distiller::PreparedEpisode {
                    episode: cognitive::types::EpisodicMemory {
                        id: Uuid::new_v4().to_string(),
                        domain: "code".into(),
                        content: format!("episode {i}"),
                        summary: None,
                        importance: 0.8,
                        occurred_at: Timestamp::now().to_string(),
                        recorded_at: Timestamp::now().to_string(),
                        stability: 1.0,
                        last_accessed: None,
                        access_count: 0,
                        project_id: None,
                        scope_type: "user".into(),
                        scope_id: None,
                        scope_repo_id: None,
                        metadata: None,
                        kind: Some("turn_trace".into()),
                        actor_id: None,
                        tier: "raw".into(),
                        parent_id: None,
                        child_count: 0,
                        rolled_up_at: None,
                    },
                    kind: "turn_trace".into(),
                    metadata_json: None,
                    scope_repo_id: None,
                    provenance: prov,
                };
                writer.write_episode(episode).await.unwrap();
            }

            let facts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_facts")
                .fetch_one(pool.inner()).await.unwrap();
            let eps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner()).await.unwrap();

            prop_assert!(facts >= prev_facts, "facts decreased {prev_facts} -> {facts}");
            prop_assert!(eps >= prev_eps, "episodes decreased {prev_eps} -> {eps}");
            Ok(())
        }).unwrap();
    }
}
