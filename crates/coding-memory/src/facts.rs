//! Coding fact taxonomy — the in-memory shape of every kind the Distiller
//! and Reforge write. Persistence uses existing `SemanticFact` /
//! `EpisodicMemory` / `ProceduralRule` rows; these structs are what the
//! Distiller constructs before handing off to the cognitive repos.
//!
//! See coding-memory design §7 for the full taxonomy table.

use crate::scope::{AnchoredSymbol, ProvenanceMetadata, Sensitivity};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// The 5-value `CodingKind` enum the LLM Distiller `record_observation` tool
/// accepts. Reforge-only kinds (ProblemSolutionPattern, ProjectUnderstanding,
/// UserHabit) are NOT in this enum — Distiller MUST NOT emit them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodingKind {
    /// `FixAttempt` episode.
    FixAttempt,
    /// `StylePreference` semantic fact.
    StylePreference,
    /// `WorkflowPattern` procedural rule.
    WorkflowPattern,
    /// `RepoContext` semantic fact.
    RepoContext,
    /// `FailurePattern` procedural rule.
    FailurePattern,
}

impl CodingKind {
    /// Canonical snake_case wire representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            CodingKind::FixAttempt => "fix_attempt",
            CodingKind::StylePreference => "style_preference",
            CodingKind::WorkflowPattern => "workflow_pattern",
            CodingKind::RepoContext => "repo_context",
            CodingKind::FailurePattern => "failure_pattern",
        }
    }
}

/// Outcome of a fix attempt.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FixOutcome {
    /// Fix worked — tests pass, behavior confirmed.
    Success,
    /// Fix partially worked; follow-up needed.
    Partial,
    /// Fix did not work — reverted or replaced.
    Failure,
    /// Abandoned without reaching a conclusion.
    Abandoned,
}

/// Structured JSON body of a `FixAttempt` episodic memory.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FixAttempt {
    /// Stable hash of (canonical problem statement).
    pub problem_hash: String,
    /// Human-readable problem statement.
    pub problem: String,
    /// Files touched.
    pub files: Vec<PathBuf>,
    /// One-sentence description of the approach.
    pub approach: String,
    /// How it ended.
    pub outcome: FixOutcome,
    /// What we learned.
    pub insight: Option<String>,
    /// Wall-clock duration.
    pub duration_ms: u32,
    /// Pre-fix test outcome summary.
    pub test_before: Option<String>,
    /// Post-fix test outcome summary.
    pub test_after: Option<String>,
    /// Symbols touched (Phase 6 populates; Phase 1 allows `vec![]`).
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Derived "tried X, didn't work" fact emitted alongside a `Failure`/`Abandoned`
/// `FixAttempt`. Stored as a `SemanticFact { memory_type: 'counterfactual' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndAttempt {
    /// Links back to the episode that caused this dead-end entry.
    pub source_attempt_id: Uuid,
    /// Canonical problem hash.
    pub problem_hash: String,
    /// What we tried.
    pub approach: String,
    /// Why it didn't work.
    pub reason: String,
    /// Confidence the dead-end warning is valid.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}

/// A style / preference statement. `SemanticFact { domain: 'preferences' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StylePreference {
    /// Predicate — `prefers | avoids | uses | dislikes`.
    pub predicate: String,
    /// Object of the preference.
    pub object: String,
    /// Scope — `"global"` or `"repo"`.
    pub scope_kind: StyleScope,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Where a style preference applies.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleScope {
    /// Applies everywhere.
    Global,
    /// Applies to one repo only.
    Repo,
}

/// A repo-level fact — framework, language, conventions, etc.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoContext {
    /// Predicate — one of `framework | language | package_manager |
    /// test_command | lint_command | deployment | convention |
    /// architecture_layer | depends_on | has_gotcha`.
    pub predicate: String,
    /// Object value.
    pub object: String,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// A recurring workflow pattern. `ProceduralRule { source: 'observed' }`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPattern {
    /// Short name.
    pub name: String,
    /// When-to-apply heuristic in plain language.
    pub when_to_use: String,
    /// Step-by-step procedure (one line per step).
    pub procedure: Vec<String>,
    /// Starting effectiveness (Distiller default 0.5).
    pub effectiveness: f32,
    /// Confidence at emission time.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// A recurring failure pattern with a remediation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FailurePattern {
    /// Name of the pattern (stable identifier).
    pub name: String,
    /// Signature / symptom description.
    pub symptom: String,
    /// Remediation (one line per step).
    pub remediation: Vec<String>,
    /// Confidence at emission time.
    pub confidence: f32,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
    /// Sensitivity tier.
    pub sensitivity: Sensitivity,
}

/// Episodic memory emitted by Phase-A extractive when a refactor pattern
/// is detected (file-edit clustering). No LLM.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RefactorEpisode {
    /// Files touched.
    pub files: Vec<PathBuf>,
    /// Symbols touched.
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Summary of the change.
    pub summary: String,
    /// When it happened.
    pub occurred_at: Timestamp,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}

/// Episodic memory emitted by Phase-A when a test runner ran.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TestRunEpisode {
    /// Command as executed.
    pub command: String,
    /// Detected framework.
    pub framework: Option<String>,
    /// Passed count.
    pub passed: u32,
    /// Failed count.
    pub failed: u32,
    /// When it ran.
    pub occurred_at: Timestamp,
    /// Provenance.
    pub provenance: ProvenanceMetadata,
}
