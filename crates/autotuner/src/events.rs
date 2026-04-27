use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoTunerEvent {
    Report(AutoTunerReport),
    Promotion(AutoTunerPromotion),
    Rollback(AutoTunerRollback),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerReport {
    pub champion: ChampionSummary,
    pub active_experiment: Option<ExperimentSummary>,
    pub completed_trials: Vec<TrialSummary>,
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerPromotion {
    pub trial_id: Uuid,
    pub reason: String,
    pub impact: String,
    pub params_changed: Vec<ParamChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerRollback {
    pub reverted_trial_id: Uuid,
    pub reason: String,
    pub reverted_to: ChampionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ChampionSummary {
    pub trial_id: Option<Uuid>,
    pub description: String,
    pub impact: String,
    #[specta(type = String)]
    pub promoted_at: Timestamp,
    pub days_active: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ExperimentSummary {
    pub id: Uuid,
    pub variant_count: u8,
    pub messages_scored: u32,
    pub hypothesis: String,
    #[specta(type = String)]
    pub started_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialSummary {
    pub id: Uuid,
    pub status: String,
    pub reasoning: String,
    pub impact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamChange {
    pub name: String,
    pub old_value: f64,
    pub new_value: f64,
}
