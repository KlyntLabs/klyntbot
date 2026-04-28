//! Inv 1 — provenance-always: every Distiller-emitted fact carries
//! `metadata.provenance.source_events` non-empty.

use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use jiff::Timestamp;
use proptest::prelude::*;
use storage::StoragePool;
use uuid::Uuid;

mod common;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn provenance_always_non_empty_for_facts(
        source_event_count in 1usize..=8,
        source_kind in prop::sample::select(vec![
            ProvenanceKind::DistillerExtractive,
            ProvenanceKind::DistillerLlm,
            ProvenanceKind::ReforgeSynthesis,
        ]),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
                .await
                .unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
                .await
                .unwrap();

            let writer = common::writer_for_pool(&pool);
            let event_ids: Vec<Uuid> = (0..source_event_count).map(|_| Uuid::new_v4()).collect();
            let prov = ProvenanceMetadata {
                source_events: event_ids.clone(),
                session_id: "s1".into(),
                turn_id: Some("t1".into()),
                distilled_at: Timestamp::now(),
                distiller_model: "test-model".into(),
                source_kind,
            };
            let prepared = common::prepared_fact_with_prov(prov);
            writer.write_fact(prepared).await.expect("write_fact");

            let meta: (Option<String>,) = sqlx::query_as("SELECT metadata FROM semantic_facts LIMIT 1")
                .fetch_one(pool.inner())
                .await
                .unwrap();
            let meta_json: serde_json::Value = serde_json::from_str(&meta.0.unwrap()).unwrap();
            let events = meta_json["provenance"]["sourceEvents"].as_array().unwrap();
            prop_assert_eq!(events.len(), source_event_count);
            Ok(())
        }).unwrap();
    }
}
