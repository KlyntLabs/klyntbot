//! Input/output types for the Reforge cycle phases.

use std::collections::HashMap;
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

#[derive(Debug, Clone)]
pub struct ReforgeCollected {
    pub sessions: Vec<SessionContext>,
    pub episodic_memories: Vec<crate::types::EpisodicMemory>,
    pub user_model: crate::types::UserModel,
    pub rules: Vec<crate::types::ProceduralRule>,
    pub routing_summaries: Vec<RoutingSummary>,
    pub pending_meta_rules: Vec<String>,
    pub skill_files: std::collections::HashMap<String, Vec<super::skill_files::SkillFile>>,
    pub retrieval_precision: Option<f64>,
    pub is_bootstrap: bool,
    pub autotuner_ctx: Option<AutotunerContext>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillContent {
    pub skill_name: String,
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
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
}
