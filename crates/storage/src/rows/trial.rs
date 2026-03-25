//! Row structs for `autotuner_experiments`, `autotuner_trials`, and
//! `autotuner_shadow_log` tables.

use serde::{Deserialize, Serialize};

/// Row struct for the `autotuner_experiments` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExperimentRow {
    pub id: String,
    pub hypothesis: String,
    pub trend_analysis: String,
    pub recommendation_for_next: String,
    pub created_at: String,
}

/// Row struct for the `autotuner_trials` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrialRow {
    pub id: String,
    pub experiment_id: String,
    /// JSON-serialized `TrialParams`.
    pub params: String,
    pub generation_reasoning: String,
    /// One of: `pending`, `active`, `completed`, `promoted`, `reverted`.
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    /// JSON-serialized `TrialResult`.
    pub result: Option<String>,
}

/// Row struct for the `autotuner_shadow_log` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ShadowLogRow {
    pub id: i64,
    pub trial_id: String,
    pub message_timestamp: String,
    pub chat_id: String,
    pub predicted_orchestrator: String,
    pub predicted_mode: String,
    pub confidence: f64,
    pub predicted_iteration_budget: i64,
    pub control_orchestrator: String,
    pub control_mode: String,
    pub user_corrected: bool,
    pub created_at: String,
}
