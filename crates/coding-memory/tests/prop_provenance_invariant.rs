//! Invariant 1 — every Distiller-authored row has non-empty `source_events`.

use coding_memory::distiller::writer::{DistillerWriter, PreparedFact};
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use cognitive::types::SemanticFact;
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn provenance_always_non_empty_for_facts() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let prov = ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s".into(), turn_id: None,
        distilled_at: Timestamp::now(), distiller_model: "m".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    };
    let fact = SemanticFact {
        id: Uuid::new_v4().to_string(), domain: "work".into(),
        subject: "x".into(), predicate: "y".into(), object: "z".into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(), valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    };
    writer.write_fact(PreparedFact { fact, metadata_json: None, scope_repo_id: None, provenance: prov }).await.unwrap();

    let meta: (Option<String>,) = sqlx::query_as("SELECT metadata FROM semantic_facts LIMIT 1")
        .fetch_one(pool.inner()).await.unwrap();
    let meta_json: serde_json::Value = serde_json::from_str(&meta.0.unwrap()).unwrap();
    let events = meta_json["provenance"]["sourceEvents"].as_array().unwrap();
    assert!(!events.is_empty());
}
