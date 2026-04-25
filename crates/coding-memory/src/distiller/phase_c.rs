//! Phase C — reconciliation (Mem0-style, DELETE-free).
//!
//! For each candidate fact we look up the top-k vector-similar existing facts
//! (scoped by `scope_repo_id`). Policy:
//!
//! | Condition                                                  | Decision   |
//! |------------------------------------------------------------|------------|
//! | top.similarity > 0.9 AND (subject, predicate) exact match  | NOOP       |
//! | top.similarity > 0.75                                      | SUPERSEDE  |
//! | otherwise                                                  | ADD        |
//!
//! NOOP bumps `access_count` on the predecessor (delegated to caller).
//! SUPERSEDE writes the new row with a pending link to the predecessor;
//! Task 16 completes the chain by setting predecessor's `valid_until` +
//! `superseded_by` atomically.

use cognitive::types::SemanticFact;

const NOOP_THRESHOLD: f32 = 0.9;
const SUPERSEDE_THRESHOLD: f32 = 0.75;

/// A candidate-adjacent existing row with its similarity score.
#[derive(Debug, Clone)]
pub struct SimilarFact {
    /// The existing row.
    pub fact: SemanticFact,
    /// Cosine similarity (0.0–1.0).
    pub similarity: f32,
}

/// Reconciliation decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileDecision {
    /// Drop the candidate; bump `access_count` on `predecessor_id`.
    Noop {
        /// Existing row id whose access_count should bump.
        predecessor_id: String,
    },
    /// Write the candidate as a new row; later update predecessor to mark it superseded.
    Supersede {
        /// Existing row id being superseded.
        predecessor_id: String,
    },
    /// Write the candidate as a fresh row; no predecessor interaction.
    Add,
}

/// Decide how to reconcile `candidate` against pre-fetched `similar` rows.
#[must_use]
pub fn reconcile(candidate: &SemanticFact, similar: &[SimilarFact]) -> ReconcileDecision {
    let Some(top) = similar.iter().max_by(|a, b| a.similarity.partial_cmp(&b.similarity).unwrap_or(std::cmp::Ordering::Equal)) else {
        return ReconcileDecision::Add;
    };
    if top.similarity > NOOP_THRESHOLD
        && top.fact.subject == candidate.subject
        && top.fact.predicate == candidate.predicate
    {
        return ReconcileDecision::Noop { predecessor_id: top.fact.id.clone() };
    }
    if top.similarity > SUPERSEDE_THRESHOLD {
        return ReconcileDecision::Supersede { predecessor_id: top.fact.id.clone() };
    }
    ReconcileDecision::Add
}
