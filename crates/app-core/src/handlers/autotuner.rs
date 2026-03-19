//! AutoTuner transparency handlers — status, history, revert, pause/resume.

use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};

use crate::state::AppCore;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerStatus {
    pub enabled: bool,
    pub champion: autotuner::ChampionSummary,
    pub active_experiment: Option<autotuner::ExperimentSummary>,
    pub paused: bool,
}

impl AppCore {
    pub async fn autotuner_status(&self) -> Result<AutoTunerStatus, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            let champion = orch.champion_summary().await;
            Ok(AutoTunerStatus {
                enabled: orch.is_active(),
                champion,
                active_experiment: None, // TODO: wire to TrialRepo
                paused: false,
            })
        } else {
            Ok(AutoTunerStatus {
                enabled: false,
                champion: autotuner::ChampionSummary {
                    trial_id: None,
                    description: "AutoTuner not enabled".into(),
                    impact: String::new(),
                    promoted_at: chrono::Utc::now(),
                    days_active: 0,
                },
                active_experiment: None,
                paused: false,
            })
        }
    }

    pub async fn autotuner_history(
        &self,
        _limit: u32,
    ) -> Result<Vec<autotuner::ExperimentSummary>, ApiError> {
        // TODO: query TrialRepo
        Ok(vec![])
    }

    pub async fn autotuner_revert(&self) -> Result<autotuner::ChampionSummary, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            // TODO: load previous champion from LearningStateRepo, update orchestrator
            Ok(orch.champion_summary().await)
        } else {
            Err(ApiError::new(
                "FEATURE_DISABLED",
                "AutoTuner is not enabled",
            ))
        }
    }

    pub async fn autotuner_pause(&self) -> Result<(), ApiError> {
        // TODO: set paused state in LearningStateRepo
        Ok(())
    }

    pub async fn autotuner_resume(&self) -> Result<(), ApiError> {
        // TODO: clear paused state
        Ok(())
    }
}
