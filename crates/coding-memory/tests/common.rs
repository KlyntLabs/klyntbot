//! Shared test helpers for coding-memory tests.
#![allow(dead_code)]

use coding_memory::distiller::writer::{DistillerWriter, PreparedFact};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

pub async fn pool_with_migrations() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

pub fn writer_for_pool(pool: &StoragePool) -> DistillerWriter {
    DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    )
}

pub fn uuid_vec(n: usize) -> Vec<Uuid> {
    (0..n).map(|_| Uuid::new_v4()).collect()
}

/// Build a minimal `PreparedFact` with the given provenance.
pub fn prepared_fact_with_prov(
    provenance: coding_memory::scope::ProvenanceMetadata,
) -> PreparedFact {
    PreparedFact {
        fact: cognitive::types::SemanticFact {
            id: Uuid::new_v4().to_string(),
            domain: "code".into(),
            subject: "subject".into(),
            predicate: "predicate".into(),
            object: "object".into(),
            confidence: 0.85,
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
        },
        metadata_json: None,
        scope_repo_id: None,
        provenance,
    }
}
