//! AutoTuner transparency handlers — status, history, revert, pause/resume.

use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};
use tracing::info;

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

            // Read the paused flag from the learning state repo.
            let paused = match orch
                .learning_state_repo()
                .get_value("autotuner_paused")
                .await
            {
                Ok(Some(val)) => val.as_bool().unwrap_or(false),
                _ => false,
            };

            // Get the most recent active experiment as the current experiment.
            let active_experiment = match orch.experiment_history(1).await {
                Ok(exps) => exps.into_iter().next(),
                Err(_) => None,
            };

            Ok(AutoTunerStatus {
                enabled: orch.is_active(),
                champion,
                active_experiment,
                paused,
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
        limit: u32,
    ) -> Result<Vec<autotuner::ExperimentSummary>, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            orch.experiment_history(limit).await.map_err(|e| {
                ApiError::new(
                    "INTERNAL",
                    format!("failed to fetch autotuner history: {e}"),
                )
            })
        } else {
            Ok(vec![])
        }
    }

    pub async fn autotuner_revert(&self) -> Result<autotuner::ChampionSummary, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            let learning_state = orch.learning_state_repo();

            // Load previous champion from LearningStateRepo.
            let prev_json = learning_state
                .get_value("autotuner_previous_champion")
                .await
                .map_err(|e| {
                    ApiError::new("INTERNAL", format!("failed to load previous champion: {e}"))
                })?
                .ok_or_else(|| {
                    ApiError::new("NOT_FOUND", "No previous champion available for revert")
                })?;

            let prev_champion: autotuner::Champion =
                serde_json::from_value(prev_json).map_err(|e| {
                    ApiError::new(
                        "INTERNAL",
                        format!("failed to deserialize previous champion: {e}"),
                    )
                })?;

            // Mark the current champion's trial as reverted if it has one.
            let current = orch.champion().await;
            if let Some(trial_id) = current.trial_id {
                let _ = orch
                    .trial_repo()
                    .update_trial_status(
                        &trial_id.to_string(),
                        autotuner::TrialStatus::Reverted.as_str(),
                    )
                    .await;
            }

            info!("manual revert: restoring previous champion");
            orch.update_champion(prev_champion).await;
            Ok(orch.champion_summary().await)
        } else {
            Err(ApiError::new(
                "FEATURE_DISABLED",
                "AutoTuner is not enabled",
            ))
        }
    }

    pub async fn autotuner_pause(&self) -> Result<(), ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            orch.learning_state_repo()
                .set("autotuner_paused", &serde_json::Value::Bool(true))
                .await
                .map_err(|e| {
                    ApiError::new("INTERNAL", format!("failed to set paused state: {e}"))
                })?;
            info!("autotuner paused");
            Ok(())
        } else {
            Err(ApiError::new(
                "FEATURE_DISABLED",
                "AutoTuner is not enabled",
            ))
        }
    }

    pub async fn autotuner_resume(&self) -> Result<(), ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            orch.learning_state_repo()
                .delete("autotuner_paused")
                .await
                .map_err(|e| {
                    ApiError::new("INTERNAL", format!("failed to clear paused state: {e}"))
                })?;
            info!("autotuner resumed");
            Ok(())
        } else {
            Err(ApiError::new(
                "FEATURE_DISABLED",
                "AutoTuner is not enabled",
            ))
        }
    }
}
