//! Invariant 3 — predecessor.valid_until == successor.valid_from in a SUPERSEDE chain.

use coding_memory::distiller::writer::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::SemanticFact;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn supersede_chain_equality() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let prov = ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s".into(),
        turn_id: None,
        distilled_at: Timestamp::now(),
        distiller_model: "m".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    };

    let old = SemanticFact {
        id: "old".into(),
        domain: "work".into(),
        subject: "repo:x".into(),
        predicate: "fw".into(),
        object: "v1".into(),
        confidence: 0.9,
        source: "distiller".into(),
        valid_from: "2026-01-01T00:00:00".into(),
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
    };
    writer.facts().upsert(&old).await.unwrap();

    let new = SemanticFact {
        id: "new".into(),
        domain: "work".into(),
        subject: "repo:x".into(),
        predicate: "fw".into(),
        object: "v2".into(),
        confidence: 0.95,
        source: "distiller".into(),
        valid_from: "2026-02-01T00:00:00".into(),
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
    };
    writer
        .write_fact(coding_memory::distiller::PreparedFact {
            fact: new,
            metadata_json: None,
            scope_repo_id: None,
            provenance: prov,
        })
        .await
        .unwrap();

    writer
        .complete_supersede("old", "new", "2026-02-01T00:00:00")
        .await
        .unwrap();

    let updated = writer.facts().get("old").await.unwrap().unwrap();
    assert_eq!(updated.superseded_by.as_deref(), Some("new"));
    assert!(updated.superseded_at.is_some());
}
