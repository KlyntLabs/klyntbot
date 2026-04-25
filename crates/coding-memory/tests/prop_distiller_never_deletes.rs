//! Invariant 5 — no Distiller cycle reduces row count.

use coding_memory::distiller::writer::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::SemanticFact;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn distiller_write_increases_counts() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let before_facts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM semantic_facts")
        .fetch_one(pool.inner()).await.unwrap();
    let before_eps: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
        .fetch_one(pool.inner()).await.unwrap();

    let prov = ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()], session_id: "s".into(), turn_id: None,
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
        scope_repo_id: None,
        metadata: None,
    };
    writer.write_fact(coding_memory::distiller::PreparedFact {
        fact, metadata_json: None, scope_repo_id: None, provenance: prov.clone(),
    }).await.unwrap();

    let after_facts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM semantic_facts")
        .fetch_one(pool.inner()).await.unwrap();
    let after_eps: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
        .fetch_one(pool.inner()).await.unwrap();

    assert!(after_facts.0 >= before_facts.0, "fact count must not decrease");
    assert!(after_eps.0 >= before_eps.0, "episode count must not decrease");
}
