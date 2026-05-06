use coding_memory::distiller::writer::{DistillerWriter, PreparedEpisode, PreparedFact};
use coding_memory::distiller::DistillerError;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::{EpisodicMemory, SemanticFact};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

async fn prepared() -> (StoragePool, DistillerWriter) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let facts = SemanticFactRepo::new(pool.inner().clone());
    let episodes = EpisodicMemoryRepo::new(pool.inner().clone());
    (pool, DistillerWriter::new(facts, episodes))
}

fn valid_provenance() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    }
}

fn dummy_fact() -> SemanticFact {
    SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "work".into(),
        subject: "repo:x".into(),
        predicate: "framework".into(),
        object: "rust".into(),
        confidence: 0.9,
        source: "distiller".into(),
        valid_from: Timestamp::now().to_string(),
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
        speaker: None,
    }
}

fn dummy_episode() -> EpisodicMemory {
    EpisodicMemory {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        content: "turn trace".into(),
        summary: None,
        importance: 0.5,
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
        kind: None,
        actor_id: None,
        tier: "raw".into(),
        parent_id: None,
        child_count: 0,
        rolled_up_at: None,
    }
}

#[tokio::test]
async fn write_fact_rejects_empty_provenance() {
    let (_pool, writer) = prepared().await;
    let mut prov = valid_provenance();
    prov.source_events.clear();
    let r = writer
        .write_fact(PreparedFact {
            fact: dummy_fact(),
            metadata_json: None,
            scope_repo_id: None,
            provenance: prov,
        })
        .await;
    assert!(matches!(r, Err(DistillerError::ProvenanceMissing)));
}

#[tokio::test]
async fn write_fact_persists_metadata_and_scope_repo_id() {
    let (pool, writer) = prepared().await;
    writer
        .write_fact(PreparedFact {
            fact: dummy_fact(),
            metadata_json: None,
            scope_repo_id: Some("github.com/klynt/bot".into()),
            provenance: valid_provenance(),
        })
        .await
        .unwrap();

    let row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT scope_repo_id, metadata FROM semantic_facts LIMIT 1")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(row.0.as_deref(), Some("github.com/klynt/bot"));
    let meta: serde_json::Value = serde_json::from_str(&row.1.unwrap()).unwrap();
    assert!(meta["provenance"]["sourceEvents"].is_array());
    assert!(!meta["provenance"]["sourceEvents"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn write_episode_rejects_empty_provenance() {
    let (_pool, writer) = prepared().await;
    let mut prov = valid_provenance();
    prov.source_events.clear();
    let r = writer
        .write_episode(PreparedEpisode {
            episode: dummy_episode(),
            kind: "turn_trace".into(),
            metadata_json: None,
            scope_repo_id: None,
            provenance: prov,
        })
        .await;
    assert!(matches!(r, Err(DistillerError::ProvenanceMissing)));
}
