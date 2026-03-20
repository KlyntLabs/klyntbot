//! AutoTuner transparency handlers — status, history, revert, pause/resume.

use agent::autotuner::{PAUSED_KEY, PREVIOUS_CHAMPION_KEY};
use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::state::AppCore;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainGrowth {
    pub corrections_captured_7d: i64,
    pub trials_evaluated_7d: i64,
    pub promoted_this_week: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsHealth {
    pub correction_rate_available: bool,
    pub token_rate_available: bool,
    pub stability_available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerStatus {
    pub enabled: bool,
    pub champion: autotuner::ChampionSummary,
    pub active_experiment: Option<autotuner::ExperimentSummary>,
    pub paused: bool,
    pub brain_growth: Option<BrainGrowth>,
    pub metrics_health: Option<MetricsHealth>,
}

impl AppCore {
    pub async fn autotuner_status(&self) -> Result<AutoTunerStatus, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            let champion = orch.champion_summary().await;

            // Read the paused flag from the learning state repo.
            let paused = match orch.learning_state_repo().get_value(PAUSED_KEY).await {
                Ok(Some(val)) => val.as_bool().unwrap_or(false),
                _ => false,
            };

            // Get the most recent active experiment as the current experiment.
            let active_experiment = match orch.experiment_history(1).await {
                Ok(exps) => exps.into_iter().next(),
                Err(_) => None,
            };

            // ── Brain growth: 7-day feedback loop stats ──────────────
            let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
            let trial_repo = orch.trial_repo();

            let (corrections_7d, trials_7d, promoted_7d, total_messages) = tokio::join!(
                async {
                    match &self.event_log_repo {
                        Some(repo) => repo
                            .count_by_event_type("UserCorrectedAI", seven_days_ago)
                            .await
                            .unwrap_or(0),
                        None => 0,
                    }
                },
                trial_repo.count_trials_since(seven_days_ago),
                trial_repo.count_promoted_since(seven_days_ago),
                self.repos.strategies.count_all(),
            );
            let trials_7d = trials_7d.unwrap_or(0);
            let promoted_7d = promoted_7d.unwrap_or(0);
            let total_messages = total_messages.unwrap_or(0);

            let growth_status = if corrections_7d == 0 || total_messages < 50 {
                "needs_feedback".into()
            } else if promoted_7d == 0 {
                "adapting".into()
            } else {
                "growing".into()
            };

            let brain_growth = Some(BrainGrowth {
                corrections_captured_7d: corrections_7d,
                trials_evaluated_7d: trials_7d,
                promoted_this_week: promoted_7d,
                status: growth_status,
            });

            let metrics_health = Some(MetricsHealth {
                correction_rate_available: corrections_7d > 0,
                token_rate_available: true,
                stability_available: true,
            });

            Ok(AutoTunerStatus {
                enabled: orch.is_active(),
                champion,
                active_experiment,
                paused,
                brain_growth,
                metrics_health,
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
                brain_growth: None,
                metrics_health: None,
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
                .get_value(PREVIOUS_CHAMPION_KEY)
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
                .set(PAUSED_KEY, &serde_json::Value::Bool(true))
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
                .delete(PAUSED_KEY)
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
