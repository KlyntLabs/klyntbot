//! Memory consolidation — Mem0-style ADD/UPDATE/DELETE/NOOP for semantic facts.
//!
//! When a new fact candidate is extracted, consolidation checks existing
//! facts for conflicts/duplicates and decides the correct operation.
//! The `ConsolidationHandler` trait delegates the LLM decision to the agent crate.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::embedder::SemanticFactEmbedder;
use crate::repos::SemanticFactRepo;
use crate::types::{MemoryOp, SemanticFact};

/// A candidate fact paired with its existing matches for batch consolidation.
#[derive(Debug, Clone)]
pub struct ConsolidationCandidate {
    pub candidate: SemanticFact,
    pub existing: Vec<SemanticFact>,
}

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

/// Trait for batch consolidation decisions.
///
/// Given candidate facts paired with their existing similar facts,
/// decide what to do with each. Defined here (L3), implemented in agent (L5).
#[async_trait]
pub trait ConsolidationHandler: Send + Sync {
    /// Decide ADD/UPDATE/DELETE/NOOP for each candidate in the batch.
    /// Returns one `MemoryOp` per candidate, in the same order.
    async fn decide_batch(
        &self,
        candidates: &[ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>>;
}

/// Facts with confidence below this threshold are routed to pending review.
const LOW_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Execute consolidation decisions against the repo and embedder.
///
/// Each `MemoryOp` is applied to the corresponding `ConsolidationCandidate`.
/// This replaces the old `consolidate_fact`/`consolidate_batch` functions —
/// the repo lookup and LLM decision now happen separately in the batch pipeline.
///
/// If `pending_repo` is provided, new facts with confidence below
/// `LOW_CONFIDENCE_THRESHOLD` are routed to the pending review queue instead
/// of being inserted directly. When `pending_repo` is `None`, all facts are
/// written as normal (backward-compatible behaviour).
pub async fn execute_memory_ops(
    ops: &[MemoryOp],
    candidates: &[ConsolidationCandidate],
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    pending_repo: Option<&crate::repos::PendingMemoryRepo>,
) {
    for (op, entry) in ops.iter().zip(candidates.iter()) {
        match op {
            MemoryOp::Add { .. } => {
                // Route low-confidence new facts to pending review
                if entry.candidate.confidence < LOW_CONFIDENCE_THRESHOLD {
                    if let Some(pending) = pending_repo {
                        if let Err(e) = pending.insert(&entry.candidate, "low_confidence").await {
                            warn!(
                                "Failed to insert pending memory '{}': {e}",
                                entry.candidate.id
                            );
                        } else {
                            debug!(
                                "Routed low-confidence fact '{}' to pending review (confidence={:.2})",
                                entry.candidate.id, entry.candidate.confidence
                            );
                        }
                        continue;
                    }
                }
                if let Err(e) = repo.upsert(&entry.candidate).await {
                    warn!("Failed to upsert fact '{}': {e}", entry.candidate.id);
                    continue;
                }
                try_embed(embedder, &entry.candidate).await;
                debug!(
                    "Consolidated: ADD fact '{}' ({}.{} = {})",
                    entry.candidate.id,
                    entry.candidate.subject,
                    entry.candidate.predicate,
                    entry.candidate.object
                );
            }
            MemoryOp::Update { id, old_id } => {
                if let Err(e) = repo.supersede(old_id, id).await {
                    warn!("Failed to supersede '{old_id}': {e}");
                    continue;
                }
                if let Err(e) = repo.upsert(&entry.candidate).await {
                    warn!("Failed to upsert updated fact '{id}': {e}");
                    continue;
                }
                try_remove_embedding(embedder, old_id).await;
                try_embed(embedder, &entry.candidate).await;
                debug!("Consolidated: UPDATE '{old_id}' → '{id}'");
            }
            MemoryOp::Delete { id, superseded_by } => {
                if let Err(e) = repo.supersede(id, superseded_by).await {
                    warn!("Failed to supersede '{id}': {e}");
                    continue;
                }
                try_remove_embedding(embedder, id).await;
                debug!("Consolidated: DELETE '{id}' (superseded by '{superseded_by}')");
            }
            MemoryOp::Noop => {
                debug!("Consolidated: NOOP for candidate '{}'", entry.candidate.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DEFAULT_MEMORY_TYPE;

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
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
        }
    }

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_execute_memory_ops_add() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let candidate = test_fact("f1", "peak_hours", "10am-12pm");
        let candidates = vec![ConsolidationCandidate {
            candidate: candidate.clone(),
            existing: vec![],
        }];
        let ops = vec![MemoryOp::Add { id: "f1".into() }];

        execute_memory_ops(&ops, &candidates, &repo, None, None).await;

        let stored = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(stored.object, "10am-12pm");
    }

    #[tokio::test]
    async fn test_execute_memory_ops_update() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let old = test_fact("f1", "peak_hours", "10am-12pm");
        repo.upsert(&old).await.unwrap();

        let new_fact = test_fact("f2", "peak_hours", "9am-11am");
        let candidates = vec![ConsolidationCandidate {
            candidate: new_fact,
            existing: vec![old],
        }];
        let ops = vec![MemoryOp::Update {
            id: "f2".into(),
            old_id: "f1".into(),
        }];

        execute_memory_ops(&ops, &candidates, &repo, None, None).await;

        let old_fact = repo.get("f1").await.unwrap().unwrap();
        assert!(old_fact.superseded_at.is_some());

        let new_stored = repo.get("f2").await.unwrap().unwrap();
        assert_eq!(new_stored.object, "9am-11am");
    }

    #[tokio::test]
    async fn test_execute_memory_ops_noop() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let candidate = test_fact("f1", "peak_hours", "10am-12pm");
        let candidates = vec![ConsolidationCandidate {
            candidate,
            existing: vec![],
        }];
        let ops = vec![MemoryOp::Noop];

        execute_memory_ops(&ops, &candidates, &repo, None, None).await;

        // Nothing stored
        let stored = repo.get("f1").await.unwrap();
        assert!(stored.is_none());
    }

    #[tokio::test]
    async fn test_low_confidence_routed_to_pending() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());
        let pending_repo = crate::repos::PendingMemoryRepo::new(pool.clone());
        pending_repo.migrate().await.unwrap();

        let mut candidate = test_fact("f-low", "peak_hours", "maybe 10am");
        candidate.confidence = 0.35;

        let candidates = vec![ConsolidationCandidate {
            candidate: candidate.clone(),
            existing: vec![],
        }];
        let ops = vec![MemoryOp::Add { id: "f-low".into() }];

        execute_memory_ops(&ops, &candidates, &repo, None, Some(&pending_repo)).await;

        // Should NOT be in semantic_facts
        let stored = repo.get("f-low").await.unwrap();
        assert!(
            stored.is_none(),
            "low-confidence fact should not be in semantic_facts"
        );

        // Should be in pending_memories
        let pending = pending_repo.list_pending(10).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "f-low");
        assert!(pending[0].reason.contains("low_confidence"));
    }

    #[tokio::test]
    async fn test_high_confidence_bypasses_pending() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());
        let pending_repo = crate::repos::PendingMemoryRepo::new(pool.clone());
        pending_repo.migrate().await.unwrap();

        let candidate = test_fact("f-high", "peak_hours", "10am-12pm"); // confidence 0.8 from test_fact

        let candidates = vec![ConsolidationCandidate {
            candidate: candidate.clone(),
            existing: vec![],
        }];
        let ops = vec![MemoryOp::Add {
            id: "f-high".into(),
        }];

        execute_memory_ops(&ops, &candidates, &repo, None, Some(&pending_repo)).await;

        // Should be in semantic_facts (confidence 0.8 >= 0.5)
        let stored = repo.get("f-high").await.unwrap();
        assert!(
            stored.is_some(),
            "high-confidence fact should be stored directly"
        );

        // Should NOT be in pending_memories
        let pending = pending_repo.list_pending(10).await;
        assert!(
            pending.is_empty(),
            "high-confidence fact should not be pending"
        );
    }
}
