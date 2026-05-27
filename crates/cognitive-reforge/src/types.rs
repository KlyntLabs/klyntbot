//! Input/output types for the Reforge cycle phases.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Session context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SessionContext {
    pub session_key: String,
    pub scratchpad: String,
    pub updated_at: String,
    pub turn_count: i64,
}

// ---------------------------------------------------------------------------
// Routing summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RoutingSummary {
    pub skill_name: String,
    pub message_count: u32,
    pub avg_confidence: f64,
    pub fallback_rate: f64,
}

// ---------------------------------------------------------------------------
// Phase 1: Collect
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ReforgeCollected {
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<cognitive_memory::types::EpisodicMemory>,
    pub user_model: cognitive_memory::types::UserModel,
    pub rules: Vec<cognitive_memory::types::ProceduralRule>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub pending_meta_rules: Vec<String>,
    pub skill_files: std::collections::HashMap<String, Vec<super::skill_files::SkillFile>>,
    pub retrieval_precision: Option<f64>,
    pub is_bootstrap: bool,
    pub autotuner_ctx: Option<AutotunerContext>,
    pub tool_failures: Vec<ToolFailureSummary>,
    pub correction_summaries: Vec<CorrectionSummary>,
    pub retrieval_precision_by_domain: Vec<(ai_core::RecallDomain, f64)>,
    pub behavioral_metrics: BehavioralMetrics,
    pub graph_health: GraphHealthMetrics,
    pub previous_suggestions: Vec<ReforgeSuggestion>,
    pub extraction_yield_by_domain: Vec<(String, f64)>,
    // Phase B2: Enrichment context
    pub pending_enrichment_turns: u32,
    pub graph_consolidation_needed: bool,
    // Phase C: Deep signals
    pub runtime_signal_summary: Option<RuntimeSignalSummary>,
    pub validation_warning_counts: Vec<(String, i64)>,
    pub near_miss_patterns: u32,
    pub coaching_behavioral: Option<CoachingBehavioralSummary>,
}

// ---------------------------------------------------------------------------
// Phase 2: Synthesize
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SynthesizeInput {
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<EpisodicSummary>,
    pub user_model_summary: String,
    pub rules_summary: String,
    pub retrieval_precision: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodicSummary {
    pub domain: String,
    pub summary: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SynthesizeOutput {
    #[serde(default)]
    pub fact_updates: Vec<FactUpdate>,
    #[serde(default)]
    pub rule_updates: Vec<RuleUpdate>,
    #[serde(default)]
    pub stale_facts: Vec<StaleFact>,
    #[serde(default)]
    pub cross_session_patterns: Vec<CrossSessionPattern>,
    pub extraction_quality_flag: Option<String>,
}

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

pub const SOURCE_REFORGE: &str = "reforge";

// ---------------------------------------------------------------------------
// Enums for stringly-typed action/type fields
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactAction {
    Add,
    Update,
    Remove,
}

impl fmt::Display for FactAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "add"),
            Self::Update => write!(f, "update"),
            Self::Remove => write!(f, "remove"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Add,
    Update,
    Reinforce,
}

impl fmt::Display for RuleAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "add"),
            Self::Update => write!(f, "update"),
            Self::Reinforce => write!(f, "reinforce"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillEditType {
    Frontmatter,
    BodyReplace,
    BodyInsert,
    BodyRemove,
}

impl fmt::Display for SkillEditType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frontmatter => write!(f, "frontmatter"),
            Self::BodyReplace => write!(f, "body_replace"),
            Self::BodyInsert => write!(f, "body_insert"),
            Self::BodyRemove => write!(f, "body_remove"),
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2 detail types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct FactUpdate {
    pub action: FactAction,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub domain: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleUpdate {
    pub action: RuleAction,
    pub rule_text: String,
    pub domain: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaleFact {
    pub fact_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrossSessionPattern {
    pub pattern: String,
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// Phase 3: Review
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ReviewInput {
    pub pending_meta_rules: Vec<String>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub skill_contents: Vec<SkillContent>,
    pub new_facts_summary: String,
    pub retrieval_precision: Option<f64>,
    pub autotuner_context: Option<String>,
    pub tool_failure_summary: Option<String>,
    pub correction_summary: Option<String>,
    pub previous_suggestions_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillContent {
    pub skill_name: String,
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReviewOutput {
    #[serde(default)]
    pub skill_edits: Vec<SkillEdit>,
    #[serde(default)]
    pub routing_insights: Vec<String>,
    #[serde(default)]
    pub context_priority_suggestions: Vec<ContextPrioritySuggestion>,
    #[serde(default)]
    pub trial_suggestions: Vec<TrialSuggestion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillEdit {
    pub skill_name: String,
    pub file_path: String,
    pub edit_type: SkillEditType,
    pub field: Option<String>,
    pub new_value: Option<String>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub section: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextPrioritySuggestion {
    pub source: String,
    pub suggestion: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Phase 4: Narrate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct NarrateInput {
    pub synthesize_summary: String,
    pub review_summary: String,
    pub routing_summary: String,
}

// ---------------------------------------------------------------------------
// Autotuner types (Phase 6)
// ---------------------------------------------------------------------------

/// A trial suggestion from the Review LLM call.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrialSuggestion {
    pub hypothesis: String,
    #[serde(default = "default_pace")]
    pub pace: String,
    #[serde(default)]
    pub param_overrides: HashMap<String, f64>,
}

fn default_pace() -> String {
    "balanced".to_string()
}

/// Summary of a past trial outcome for experiment history context.
#[derive(Debug, Clone, Serialize)]
pub struct TrialOutcome {
    pub params_summary: String,
    pub result: String,
    pub constraint_failures: Vec<String>,
    pub improvement: Option<f64>,
}

/// Summary of a past experiment for LLM context.
#[derive(Debug, Clone, Serialize)]
pub struct TrialHistoryEntry {
    pub experiment_id: String,
    pub days_ago: u32,
    pub trials: Vec<TrialOutcome>,
}

/// Snapshot of key performance metrics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct MetricsSnapshot {
    pub correction_rate: f64,
    pub retrieval_precision: f64,
    pub avg_response_time_ms: f64,
    pub avg_tokens_per_message: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
}

/// Autotuner context collected in Phase 1 for Phase 3 + Phase 6.
#[derive(Debug, Clone, Default)]
pub struct AutotunerContext {
    pub champion_summary: String,
    pub trial_history: Vec<TrialHistoryEntry>,
    pub metrics_24h: MetricsSnapshot,
    pub metrics_7d: MetricsSnapshot,
    pub active_trial_count: u32,
}

// ---------------------------------------------------------------------------
// Feedback types
// ---------------------------------------------------------------------------

/// Aggregated tool failure stats for a single tool since last Reforge run.
#[derive(Debug, Clone, Serialize)]
pub struct ToolFailureSummary {
    pub tool_name: String,
    pub total_calls: u32,
    pub failure_count: u32,
    pub failure_rate: f64,
    pub error_types: Vec<String>,
}

/// Aggregated corrections attributed to a specific skill.
#[derive(Debug, Clone, Serialize)]
pub struct CorrectionSummary {
    pub skill_name: String,
    pub correction_count: u32,
    pub sample_corrections: Vec<String>,
}

/// Behavioral metrics collected from feature crates.
/// Backed by a `BTreeMap` so metrics are registry-driven, not hard-coded.
#[derive(Debug, Clone, Serialize, Default)]
pub struct BehavioralMetrics {
    #[serde(flatten)]
    pub values: BTreeMap<String, f64>,
}

impl BehavioralMetrics {
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: f64) {
        self.values.insert(name.into(), value);
    }
}

/// Knowledge graph health snapshot.
#[derive(Debug, Clone, Serialize, Default)]
pub struct GraphHealthMetrics {
    pub active_facts: u32,
    pub active_rules: u32,
    pub co_activation_pairs: u32,
    pub facts_per_domain: Vec<(String, u32)>,
    pub avg_fact_stability: f64,
}

/// A persisted suggestion from a previous Reforge cycle.
#[derive(Debug, Clone, Serialize)]
pub struct ReforgeSuggestion {
    pub suggestion_type: String,
    pub content: String,
    pub reason: String,
    pub confidence: f64,
    pub cycle_run_at: String,
}

// ---------------------------------------------------------------------------
// Phase C: Deep signal types
// ---------------------------------------------------------------------------

/// Summary of agent runtime signals since last Reforge run.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RuntimeSignalSummary {
    pub budget_exhaustions: u32,
    pub avg_turns: f64,
    pub loop_detections: u32,
    pub avg_context_fill_pct: f64,
}

/// Summary of coaching behavioral outcomes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CoachingBehavioralSummary {
    pub total_positive: u32,
    pub total_negative: u32,
    pub acceptance_rate: f64,
}

// ---------------------------------------------------------------------------
// Enhancement pipeline signal
// ---------------------------------------------------------------------------

/// Aggregated enhancement pipeline metrics grouped by depth_mode for Reforge analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementSignal {
    pub depth_mode: String,
    pub total_runs: u32,
    pub avg_latency_ms: f64,
    pub avg_llm_calls: f64,
    pub avg_confidence: f64,
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ReforgeResult {
    pub facts_added: u32,
    pub facts_updated: u32,
    pub facts_stale_flagged: u32,
    pub rules_added: u32,
    pub rules_reinforced: u32,
    pub skills_edited: u32,
    pub narrative: String,
    pub skipped_skill_edits: Vec<String>,
    pub phase_errors: Vec<String>,
    pub trials_created: u32,
    pub champion_promoted: bool,
    pub regression_detected: bool,
    pub suggestions_persisted: u32,
    pub patterns_persisted: u32,
    // Phase B2: Graph consolidation
    pub entities_merged: u32,
    pub relationships_discovered: u32,
    pub snapshot_recorded: bool,
    // Community intelligence
    pub communities_renamed: u32,
    pub communities_merged: u32,
    pub communities_split: u32,
    // KCA Track 10 + 12
    pub cross_cli_promoted: u32,
    pub skills_proposed: u32,
}
