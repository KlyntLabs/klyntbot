//! Counterfactual memory derivation (Tier B1).
//!
//! When a `FixAttempt` episode lands with `outcome: Failure | Abandoned`, we
//! emit a derived `DeadEndAttempt` as a `SemanticFact { memory_type: "counterfactual" }`.
//! This enables the Reforge phase (Phase 5) to pattern-match failed approaches
//! and avoid them in future sessions.

use crate::facts::{FixAttempt, FixOutcome};
use crate::problem_hash::ProblemHash;
use crate::scope::ProvenanceMetadata;
use cognitive::types::SemanticFact;
use jiff::Timestamp;
use uuid::Uuid;

/// Derive a counterfactual fact from a failed/abandoned fix attempt.
/// Returns `None` for successful attempts — nothing to warn against.
pub fn derive_dead_end(
    attempt: &FixAttempt,
    _provenance: &ProvenanceMetadata,
) -> Option<SemanticFact> {
    let should_emit = matches!(attempt.outcome, FixOutcome::Failure | FixOutcome::Abandoned);
    if !should_emit {
        return None;
    }

    let problem_hash = ProblemHash::of(&attempt.problem);
    let now = Timestamp::now().to_string();
    Some(SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        subject: format!("problem:{}", problem_hash.as_str()),
        predicate: "dead_end".into(),
        object: attempt.approach.clone(),
        confidence: 0.7,
        source: "distiller_counterfactual".into(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now,
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 1.0,
        project_id: None,
        memory_type: "counterfactual".into(),
        scope_type: "user".into(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
        speaker: None,
    })
}
