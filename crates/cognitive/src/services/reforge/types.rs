//! Input/output types for the Reforge cycle phases.

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

#[derive(Debug, Clone, Deserialize)]
pub struct FactUpdate {
    pub action: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub domain: String,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleUpdate {
    pub action: String,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillEdit {
    pub skill_name: String,
    pub file_path: String,
    pub edit_type: String,
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
}
