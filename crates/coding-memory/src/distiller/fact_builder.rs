//! `Observation` → `PreparedFact` / `PreparedEpisode`.
//!
//! Routes LLM-emitted observations to the right cognitive-layer row shape.
//! The mapping matches design §7 exactly:
//!
//! | `CodingKind`      | Destination                   | `domain`      | `memory_type`    |
//! |-------------------|-------------------------------|---------------|------------------|
//! | `FixAttempt`      | `EpisodicMemory`              | `coding`      | (n/a)            |
//! | `StylePreference` | `SemanticFact`                | `preferences` | `fact`           |
//! | `WorkflowPattern` | `SemanticFact`                | `procedural`  | `pattern`        |
//! | `RepoContext`     | `SemanticFact`                | `work`        | `fact`           |
//! | `FailurePattern`  | `SemanticFact`                | `procedural`  | `failure_pattern`|
//!
//! `FailurePattern` is a fact rather than an episode: it's a reusable rule,
//! not a point-in-time event. The Reforge `ProblemSolutionPattern` kind
//! (Phase 5) subsumes both FailurePattern + causal chains — we do not
//! conflate here.

use super::error::DistillerError;
use super::record_observation::{Observation, ObservationScope};
use super::writer::{PreparedEpisode, PreparedFact};
use crate::facts::CodingKind;
use crate::scope::ProvenanceMetadata;
use cognitive::types::{EpisodicMemory, SemanticFact};
use jiff::Timestamp;
use uuid::Uuid;

/// One of the two row shapes, ready for `DistillerWriter`.
pub enum Prepared {
    /// A semantic fact.
    Fact(PreparedFact),
    /// An episodic memory.
    Episode(PreparedEpisode),
}

/// Convert one `Observation` into the appropriate `PreparedFact`/`PreparedEpisode`.
pub fn build_prepared(
    obs: &Observation,
    scope_repo_id: Option<&str>,
    provenance: &ProvenanceMetadata,
) -> Result<Prepared, DistillerError> {
    let effective_scope_repo = match obs.scope {
        ObservationScope::Global => None,
        ObservationScope::Repo => scope_repo_id.map(str::to_string),
    };
    let now = Timestamp::now().to_string();

    match obs.kind {
        CodingKind::FixAttempt => {
            let id = Uuid::new_v4().to_string();
            let content = serde_json::json!({
                "subject": obs.subject,
                "predicate": obs.predicate,
                "object": obs.object,
                "reasoning": obs.reasoning,
            })
            .to_string();
            Ok(Prepared::Episode(PreparedEpisode {
                episode: EpisodicMemory {
                    id,
                    domain: "coding".into(),
                    content,
                    summary: Some(obs.object.clone()),
                    importance: importance_from_confidence(obs.confidence),
                    occurred_at: now.clone(),
                    recorded_at: now,
                    stability: 1.0,
                    last_accessed: None,
                    access_count: 0,
                    project_id: None,
                    scope_type: scope_type_for(&effective_scope_repo),
                    scope_id: effective_scope_repo.clone(),
                },
                kind: "fix_attempt".into(),
                metadata_json: Some(serde_json::json!({ "reasoning": obs.reasoning })),
                scope_repo_id: effective_scope_repo,
                provenance: provenance.clone(),
            }))
        }
        CodingKind::StylePreference => Ok(Prepared::Fact(build_fact(
            obs,
            "preferences",
            "fact",
            effective_scope_repo,
            provenance,
        ))),
        CodingKind::WorkflowPattern => Ok(Prepared::Fact(build_fact(
            obs,
            "procedural",
            "pattern",
            effective_scope_repo,
            provenance,
        ))),
        CodingKind::RepoContext => Ok(Prepared::Fact(build_fact(
            obs,
            "work",
            "fact",
            effective_scope_repo,
            provenance,
        ))),
        CodingKind::FailurePattern => Ok(Prepared::Fact(build_fact(
            obs,
            "procedural",
            "failure_pattern",
            effective_scope_repo,
            provenance,
        ))),
    }
}

fn build_fact(
    obs: &Observation,
    domain: &str,
    memory_type: &str,
    effective_scope_repo: Option<String>,
    provenance: &ProvenanceMetadata,
) -> PreparedFact {
    let now = Timestamp::now().to_string();
    PreparedFact {
        fact: SemanticFact {
            id: Uuid::new_v4().to_string(),
            domain: domain.into(),
            subject: obs.subject.clone(),
            predicate: obs.predicate.clone(),
            object: obs.object.clone(),
            confidence: obs.confidence as f64,
            source: "distiller".into(),
            valid_from: now.clone(),
            valid_until: None,
            recorded_at: now.clone(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 1.0,
            project_id: None,
            memory_type: memory_type.into(),
            scope_type: scope_type_for(&effective_scope_repo),
            scope_id: effective_scope_repo.clone(),
        },
        metadata_json: Some(serde_json::json!({ "reasoning": obs.reasoning })),
        scope_repo_id: effective_scope_repo,
        provenance: provenance.clone(),
    }
}

fn scope_type_for(scope_repo: &Option<String>) -> String {
    if scope_repo.is_some() {
        "project".into()
    } else {
        "user".into()
    }
}

fn importance_from_confidence(c: f32) -> f64 {
    (0.3 + c as f64 * 0.6).clamp(0.0, 1.0)
}
