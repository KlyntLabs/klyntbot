//! Types for the micro-Reforge timer (KCA Track 4).
//!
//! Micro-Reforge fires every N turns or every M minutes (whichever first) and
//! synthesizes recent episodics + observations into candidate procedural rules.
//! Conservative: only ADD operations, no UPDATE or DELETE. Nightly Reforge
//! handles refinement and pruning.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroReforgeInput {
    pub recent_episodics: Vec<EpisodicRef>,
    pub recent_observations: Vec<ObservationRef>,
    pub existing_rules_summary: Vec<RuleSummary>,
    pub session_count: u32,
    pub turn_count_since_last_run: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicRef {
    pub id: String,
    pub domain: String,
    pub summary: String,
    pub importance: f64,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRef {
    pub content_truncated: String,
    pub domain: String,
    pub importance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MicroReforgeOutput {
    pub proposed_rules: Vec<ProposedRule>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedRule {
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub signal_count: u32,
    pub evidence_episodic_ids: Vec<String>,
}
