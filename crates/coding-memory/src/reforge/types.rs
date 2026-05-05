//! Public types used across Reforge phases.

use crate::reforge_phase::RuleArtifact;
use crate::scope::AnchoredSymbol;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Bundle of dependencies the cognitive `run_reforge` cycle needs to invoke
/// the coding-specific Phase 2.5 / 3.5 / 6 / 6.5 hooks.
pub struct CodingPhaseHandlers<'a> {
    /// Phase 2.5 LLM seam.
    pub synthesis: Option<&'a dyn super::CodingSynthesisHandler>,
    /// Phase 3.5 LLM seam.
    pub rule_artifacts: Option<&'a dyn super::RuleArtifactsHandler>,
    /// Repo handles needed by every coding phase.
    pub fact_repo: &'a cognitive::SemanticFactRepo,
    /// Episodic repo.
    pub episodic_repo: &'a cognitive::EpisodicMemoryRepo,
    /// Procedural rule repo.
    pub rule_repo: &'a cognitive::ProceduralRuleRepo,
    /// Co-activation repo.
    pub co_activation_repo: &'a cognitive::CoActivationRepo,
    /// Memory utilization repo (cited-vs-ignored).
    pub utilization_repo: &'a crate::recall::telemetry::RecallInvocationRepo,
    /// Session summary repo.
    pub session_summary_repo: &'a super::SessionSummaryRepo,
    /// Selective-delete log writer.
    pub selective_delete_log: &'a super::selective_delete::SelectiveDeleteLogRepo,
    /// Pattern-effectiveness log writer.
    pub pattern_effectiveness_log:
        &'a crate::mirror::pattern_effectiveness::PatternEffectivenessLogRepo,
    /// Optional bus for emitting `PatternOutcome` etc. during the cycle.
    pub bus: Option<Arc<bus::DomainEventBus>>,
    /// Causal edge repo (Phase 6).
    pub causal_repo: Option<&'a crate::causal::CausalEdgeRepo>,
    /// Optional symbol extractor (Phase 6).
    pub symbol_extractor: Option<&'a dyn crate::symbols::SymbolExtractor>,
    /// Map of repo_id → filesystem root (Phase 6 symbol validation).
    pub repo_roots: &'a std::collections::HashMap<String, std::path::PathBuf>,
}

/// Input bundle for `CodingSynthesisHandler::synthesize_coding`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSynthesisInput {
    /// Session window scoped to this run.
    pub since: Timestamp,
    /// Per-repo fact + episode + workflow-pattern bundles.
    pub repo_bundles: Vec<RepoSynthesisBundle>,
    /// Recent counterfactual facts (for ProblemSolutionPattern promotion).
    pub recent_counterfactuals: Vec<SerializableSemanticFact>,
}

/// One repo's slice of synthesis input.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSynthesisBundle {
    /// Canonical repo id.
    pub repo_id: String,
    /// `FixAttempt` episodes new since `since`.
    pub fix_attempts: Vec<SerializableEpisodicMemory>,
    /// Active `WorkflowPattern` rules.
    pub workflow_patterns: Vec<SerializableProceduralRule>,
    /// `RepoContext` facts.
    pub repo_context_facts: Vec<SerializableSemanticFact>,
    /// Recent causal-edge groups (problem_hash → chains).
    pub causal_chains: Vec<CausalChainGroup>,
}

/// Causal chain group keyed by problem hash.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalChainGroup {
    /// Problem hash anchoring the chains.
    pub problem_hash: String,
    /// Member edges.
    pub edge_ids: Vec<Uuid>,
}

impl CausalChainGroup {
    /// Edge count, derived from `edge_ids`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.edge_ids.len()
    }
}

/// Output bundle from the LLM — six action variants.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PromoteAction {
    /// Extract a new `WorkflowPattern`.
    ExtractPattern {
        /// Repo scope.
        repo_id: Option<String>,
        /// Rule text.
        rule: String,
        /// Confidence (0.0 – 1.0).
        confidence: f32,
        /// Supporting episode ids.
        supporting: Vec<Uuid>,
    },
    /// Extract a new `FailurePattern`.
    ExtractFailurePattern {
        /// Repo scope.
        repo_id: Option<String>,
        /// Pattern text.
        rule: String,
        /// Remediation text.
        remediation: String,
        /// Confidence.
        confidence: f32,
        /// Supporting episode ids.
        supporting: Vec<Uuid>,
    },
    /// Promote ≥3 failed FixAttempts sharing a `problem_hash` to a problem-class refactor signal.
    PromoteToProblemClass {
        /// Hash key.
        problem_hash: String,
        /// Suggested refactor description.
        suggestion: String,
    },
    /// Synthesize a `ProjectUnderstanding` semantic fact.
    PromoteToProjectUnderstanding {
        /// Repo scope.
        repo_id: String,
        /// Subject.
        subject: String,
        /// Predicate.
        predicate: String,
        /// Object.
        object: String,
        /// Convergence score (≥0.7 to land).
        convergence: f32,
    },
    /// Promote a recurring `WorkflowPattern` observed across ≥3 repos to a global `UserHabit`.
    PromoteToUserHabit {
        /// Habit text.
        rule: String,
        /// Confidence.
        confidence: f32,
        /// Witness repos.
        witness_repos: Vec<String>,
    },
    /// Promote ≥3 causal chains sharing a `problem_hash` to a `ProblemSolutionPattern`.
    PromoteToProblemSolutionPattern {
        /// Anchoring problem hash.
        problem_hash: String,
        /// Solution text.
        solution: String,
        /// Supporting causal-edge ids.
        supporting_edges: Vec<Uuid>,
    },
}

/// Synthesis call result.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSynthesisOutput {
    /// Ordered actions to apply.
    pub actions: Vec<PromoteAction>,
    /// Free-form narrative for telemetry.
    pub narrative: String,
}

/// One repo's plan for rule-artifact externalization.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoArtifactPlan {
    /// Canonical repo id.
    pub repo_id: String,
    /// Filesystem root for the repo (resolved by `app-core`).
    pub root: PathBuf,
    /// Which artifacts are enabled per `config.codingMemory.reforge.ruleArtifacts`.
    pub enabled: Vec<RuleArtifact>,
    /// Filtered facts safe to externalize (sensitivity != high/excluded).
    pub facts: Vec<SerializableSemanticFact>,
    /// Filtered procedural rules.
    pub rules: Vec<SerializableProceduralRule>,
}

/// Input to `RuleArtifactsHandler::synthesize_artifact`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactInput {
    /// Repo plan.
    pub plan: RepoArtifactPlan,
    /// Which artifact kind to produce.
    pub artifact: RuleArtifact,
}

/// Output: one managed-block body per artifact kind.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactOutput {
    /// Markdown body (without managed-block markers).
    pub body: String,
    /// Optional ordered section labels for diff display.
    pub section_labels: Vec<String>,
}

/// One managed-block section, identified by header.
#[derive(Debug, Clone)]
pub struct ManagedBlockSection {
    /// Section heading (e.g. "Architecture").
    pub heading: String,
    /// Body lines.
    pub body: String,
}

/// Spec emitted by the project-skill evolver — used to produce `SKILL.md`.
#[derive(Debug, Clone)]
pub struct ProjectSkillSpec {
    /// Skill id (slug-style, sanitized from rule text).
    pub skill_id: String,
    /// Repo scope.
    pub repo_id: String,
    /// `name`.
    pub name: String,
    /// `description`.
    pub description: String,
    /// `whenToUse` list.
    pub when_to_use: Vec<String>,
    /// Procedure body (markdown).
    pub procedure: String,
    /// Anchoring fact / episode ids.
    pub references: Vec<Uuid>,
    /// Anchored symbol refs (Phase 6 will populate; Phase 5 may be empty).
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Starting effectiveness.
    pub effectiveness: f32,
}

// ─────────────────────────────────────────────────────────────────────
// Serializable mirrors of cognitive's row types (so handler trait signatures
// stay clean and don't leak `cognitive::types::*` to the agent crate).
// ─────────────────────────────────────────────────────────────────────

/// Fact slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableSemanticFact {
    /// Id.
    pub id: String,
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Object.
    pub object: String,
    /// Confidence.
    pub confidence: f32,
    /// Memory type (e.g. `fact`, `counterfactual`).
    pub memory_type: String,
    /// Sensitivity (`normal`/`high`/`excluded`).
    pub sensitivity: crate::scope::Sensitivity,
    /// Repo scope id.
    pub scope_repo_id: Option<String>,
    /// `valid_from`.
    pub valid_from: String,
}

/// Episode slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableEpisodicMemory {
    /// Id.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Content (raw markdown / JSON-stringified content).
    pub content: String,
    /// Importance.
    pub importance: f32,
    /// Recorded at.
    pub recorded_at: String,
    /// Repo scope id.
    pub scope_repo_id: Option<String>,
}

/// Rule slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableProceduralRule {
    /// Id.
    pub id: String,
    /// Rule text.
    pub rule: String,
    /// Source.
    pub source: String,
    /// Confidence.
    pub confidence: f32,
    /// Effectiveness.
    pub effectiveness: f32,
    /// Repo scope.
    pub scope_repo_id: Option<String>,
}
