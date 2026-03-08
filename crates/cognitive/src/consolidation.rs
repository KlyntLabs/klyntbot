//! Memory consolidation — Mem0-style ADD/UPDATE/DELETE/NOOP for semantic facts.
//!
//! When a new fact candidate is extracted, consolidation checks existing
//! facts for conflicts/duplicates and decides the correct operation.
//! The `ConsolidationHandler` trait delegates the LLM decision to the agent crate.

use async_trait::async_trait;
use storage::StorageError;
use tracing::{debug, warn};

use crate::embedder::SemanticFactEmbedder;
use crate::repos::SemanticFactRepo;
use crate::types::{MemoryOp, SemanticFact};

/// Embed a fact into the vector store, logging on failure.
async fn try_embed(embedder: Option<&dyn SemanticFactEmbedder>, fact: &SemanticFact) {
    if let Some(emb) = embedder {
        if let Err(e) = emb.embed_and_store_fact(fact).await {
            warn!("Failed to embed fact '{}': {e}", fact.id);
        }
    }
}

/// Remove a fact's embedding from the vector store, logging on failure.
async fn try_remove_embedding(embedder: Option<&dyn SemanticFactEmbedder>, fact_id: &str) {
    if let Some(emb) = embedder {
        if let Err(e) = emb.remove_embedding(fact_id).await {
            warn!("Failed to remove embedding '{fact_id}': {e}");
        }
    }
}

/// Trait for LLM-backed consolidation decisions.
///
/// Given a candidate fact and existing similar facts, decide what to do.
/// Defined here (L3), implemented in agent (L5).
#[async_trait]
pub trait ConsolidationHandler: Send + Sync {
    /// Decide whether to ADD, UPDATE, DELETE, or NOOP given a candidate and existing facts.
    async fn decide(
        &self,
        candidate: &SemanticFact,
        existing: &[SemanticFact],
    ) -> common::Result<MemoryOp>;
}

/// Execute consolidation for a single candidate fact.
///
/// 1. Find similar existing facts (same subject + predicate)
/// 2. If none exist → ADD directly
/// 3. If similar exist → ask handler to decide (UPDATE/DELETE/NOOP)
/// 4. Execute the decided operation on the repo
pub async fn consolidate_fact(
    candidate: &SemanticFact,
    repo: &SemanticFactRepo,
    handler: &dyn ConsolidationHandler,
    embedder: Option<&dyn SemanticFactEmbedder>,
) -> common::Result<MemoryOp> {
    let existing = repo
        .find_similar(&candidate.subject, &candidate.predicate)
        .await
        .map_err(StorageError::from)?;

    if existing.is_empty() {
        // No similar facts — direct ADD
        repo.upsert(candidate).await.map_err(StorageError::from)?;
        try_embed(embedder, candidate).await;
        debug!(
            "Consolidated: ADD fact '{}' ({}.{} = {})",
            candidate.id, candidate.subject, candidate.predicate, candidate.object
        );
        return Ok(MemoryOp::Add {
            id: candidate.id.clone(),
        });
    }

    // Ask handler to decide
    let op = handler.decide(candidate, &existing).await?;

    match &op {
        MemoryOp::Add { .. } => {
            repo.upsert(candidate).await.map_err(StorageError::from)?;
            try_embed(embedder, candidate).await;
            debug!("Consolidated: ADD (handler confirmed) '{}'", candidate.id);
        }
        MemoryOp::Update { id, old_id } => {
            repo.supersede(old_id, id)
                .await
                .map_err(StorageError::from)?;
            repo.upsert(candidate).await.map_err(StorageError::from)?;
            try_remove_embedding(embedder, old_id).await;
            try_embed(embedder, candidate).await;
            debug!("Consolidated: UPDATE '{old_id}' → '{id}'");
        }
        MemoryOp::Delete { id, superseded_by } => {
            repo.supersede(id, superseded_by)
                .await
                .map_err(StorageError::from)?;
            try_remove_embedding(embedder, id).await;
            debug!("Consolidated: DELETE '{id}' (superseded by '{superseded_by}')");
        }
        MemoryOp::Noop => {
            debug!("Consolidated: NOOP for candidate '{}'", candidate.id);
        }
    }

    Ok(op)
}

/// Run consolidation for a batch of candidate facts.
pub async fn consolidate_batch(
    candidates: &[SemanticFact],
    repo: &SemanticFactRepo,
    handler: &dyn ConsolidationHandler,
    embedder: Option<&dyn SemanticFactEmbedder>,
) -> Vec<MemoryOp> {
    let mut ops = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        match consolidate_fact(candidate, repo, handler, embedder).await {
            Ok(op) => ops.push(op),
            Err(e) => {
                warn!("Consolidation failed for candidate '{}': {e}", candidate.id);
                ops.push(MemoryOp::Noop);
            }
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConsolidationHandler {
        decision: MemoryOp,
    }

    #[async_trait]
    impl ConsolidationHandler for MockConsolidationHandler {
        async fn decide(
            &self,
            _candidate: &SemanticFact,
            _existing: &[SemanticFact],
        ) -> common::Result<MemoryOp> {
            Ok(self.decision.clone())
        }
    }

    fn test_fact(id: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
        }
    }

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_consolidate_adds_when_no_existing() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Handler won't be called since there are no existing facts
        let handler = MockConsolidationHandler {
            decision: MemoryOp::Noop,
        };

        let candidate = test_fact("f1", "peak_hours", "10am-12pm");
        let op = consolidate_fact(&candidate, &repo, &handler, None)
            .await
            .unwrap();

        assert!(matches!(op, MemoryOp::Add { ref id } if id == "f1"));

        let stored = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(stored.object, "10am-12pm");
    }

    #[tokio::test]
    async fn test_consolidate_updates_existing() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert existing fact
        let old = test_fact("f1", "peak_hours", "10am-12pm");
        repo.upsert(&old).await.unwrap();

        // Candidate with updated value
        let candidate = test_fact("f2", "peak_hours", "9am-11am");
        let handler = MockConsolidationHandler {
            decision: MemoryOp::Update {
                id: "f2".into(),
                old_id: "f1".into(),
            },
        };

        let op = consolidate_fact(&candidate, &repo, &handler, None)
            .await
            .unwrap();
        assert!(matches!(op, MemoryOp::Update { .. }));

        // Old fact should be superseded
        let old_fact = repo.get("f1").await.unwrap().unwrap();
        assert!(old_fact.superseded_at.is_some());
        assert_eq!(old_fact.superseded_by, Some("f2".into()));

        // New fact should exist
        let new_fact = repo.get("f2").await.unwrap().unwrap();
        assert_eq!(new_fact.object, "9am-11am");
    }

    #[tokio::test]
    async fn test_consolidate_noop_on_duplicate() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let old = test_fact("f1", "peak_hours", "10am-12pm");
        repo.upsert(&old).await.unwrap();

        let candidate = test_fact("f2", "peak_hours", "10am-12pm");
        let handler = MockConsolidationHandler {
            decision: MemoryOp::Noop,
        };

        let op = consolidate_fact(&candidate, &repo, &handler, None)
            .await
            .unwrap();
        assert_eq!(op, MemoryOp::Noop);

        // Original fact unchanged
        let stored = repo.get("f1").await.unwrap().unwrap();
        assert!(stored.superseded_at.is_none());
    }

    #[tokio::test]
    async fn test_consolidate_batch() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let handler = MockConsolidationHandler {
            decision: MemoryOp::Noop, // Won't be reached — all are new
        };

        let candidates = vec![
            test_fact("f1", "peak_hours", "10am-12pm"),
            test_fact("f2", "break_pattern", "every 90min"),
        ];

        let ops = consolidate_batch(&candidates, &repo, &handler, None).await;
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], MemoryOp::Add { ref id } if id == "f1"));
        assert!(matches!(ops[1], MemoryOp::Add { ref id } if id == "f2"));
    }
}
