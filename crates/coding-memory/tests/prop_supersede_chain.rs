//! Inv 5 — SUPERSEDE chain: predecessor.valid_until == successor.valid_from.

use coding_memory::distiller::writer::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::SemanticFact;
use jiff::{Timestamp, ToSpan};
use proptest::prelude::*;
use storage::StoragePool;
use uuid::Uuid;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn supersede_chain_links_are_consistent(chain_len in 2usize..=6) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = common::pool_with_migrations().await;
            let writer = common::writer_for_pool(&pool);

            let base_time = Timestamp::now();
            let mut prev_id: Option<String> = None;
            let mut prev_valid_from = base_time;

            for i in 0..chain_len {
                let id = format!("fact_{i}");
                let valid_from = base_time.checked_add((i as i64 * 3600).seconds()).unwrap();
                let fact = SemanticFact {
                    id: id.clone(),
                    domain: "code".into(),
                    subject: "repo:x".into(),
                    predicate: "fw".into(),
                    object: format!("v{i}"),
                    confidence: 0.9,
                    source: "distiller".into(),
                    valid_from: valid_from.to_string(),
                    valid_until: None,
                    recorded_at: Timestamp::now().to_string(),
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
                
                    speaker: None,};
                writer.facts().upsert(&fact).await.unwrap();

                if let Some(prev) = prev_id {
                    writer.complete_supersede(&prev, &id, &valid_from.to_string()).await.unwrap();
                }
                prev_id = Some(id);
                prev_valid_from = valid_from;
            }

            let chain = writer.facts().list_supersede_chain("repo:x", "fw").await.unwrap();
            prop_assert_eq!(chain.len() as usize, chain_len);
            for pair in chain.windows(2) {
                let pred = &pair[0];
                let succ = &pair[1];
                prop_assert_eq!(pred.valid_until.as_deref(), Some(succ.valid_from.as_str()));
            }
            Ok(())
        }).unwrap();
    }
}
