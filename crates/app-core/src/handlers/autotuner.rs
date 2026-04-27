//! AutoTuner transparency handlers — status, history, revert, pause/resume.

use agent::autotuner::{EXPERIMENT_PACE_KEY, PAUSED_KEY, PREVIOUS_CHAMPION_KEY, TOAST_COUNT_KEY};
use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::state::AppCore;

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BrainGrowth {
    pub corrections_captured_7d: i64,
    pub trials_evaluated_7d: i64,
    pub promoted_this_week: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetricsHealth {
    pub correction_rate_available: bool,
    pub token_rate_available: bool,
    pub stability_available: bool,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerStatus {
    pub champion: autotuner::ChampionSummary,
    pub active_experiment: Option<autotuner::ExperimentSummary>,
    pub paused: bool,
    pub brain_growth: Option<BrainGrowth>,
    pub metrics_health: Option<MetricsHealth>,
    pub experiment_pace: Option<String>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn autotuner_status(&self) -> Result<AutoTunerStatus, ApiError> {
        if let Some(orch) = self.autotuner_orchestrator() {
            let champion = orch.champion_summary().await;

            // Read the paused flag from the learning state repo.
            let paused = match orch.learning_state_repo().get_value(PAUSED_KEY).await {
                Ok(Some(val)) => val.as_bool().unwrap_or(false),
                _ => false,
            };

            // Read experiment pace preference, defaulting to "balanced".
            let experiment_pace = match orch
                .learning_state_repo()
                .get_value(EXPERIMENT_PACE_KEY)
                .await
            {
                Ok(Some(val)) => val
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "balanced".into()),
                _ => "balanced".into(),
            };

            // Get the most recent active experiment as the current experiment.
            let active_experiment = match orch.experiment_history(1).await {
                Ok(exps) => exps.into_iter().next(),
                Err(_) => None,
            };

            // ── Brain growth: 7-day feedback loop stats ──────────────
            let seven_days_ago =
                jiff::Timestamp::now() - jiff::SignedDuration::from_secs(7 * 86400);
            let trial_repo = orch.trial_repo();

            let (corrections_7d, trials_7d, promoted_7d, total_messages) = tokio::join!(
                async {
                    match &self.event_log_repo {
                        Some(repo) => repo
                            .count_by_event_type(
                                bus::DomainEvent::KIND_USER_CORRECTED_AI,
                                seven_days_ago,
                            )
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
                champion,
                active_experiment,
                paused,
                brain_growth,
                metrics_health,
                experiment_pace: Some(experiment_pace),
            })
        } else {
            Ok(AutoTunerStatus {
                champion: autotuner::ChampionSummary {
                    trial_id: None,
                    description: "AutoTuner not enabled".into(),
                    impact: String::new(),
                    promoted_at: jiff::Timestamp::now(),
                    days_active: 0,
                },
                active_experiment: None,
                paused: false,
                brain_growth: None,
                metrics_health: None,
                experiment_pace: None,
            })
        }
    }

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
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

    #[tracing::instrument(skip(self), err)]
    pub async fn autotuner_get_toast_count(&self) -> Result<i64, ApiError> {
        let orch = self
            .autotuner_orchestrator()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "AutoTuner is not enabled"))?;
        let count = match orch.learning_state_repo().get_value(TOAST_COUNT_KEY).await {
            Ok(Some(val)) => val.as_i64().unwrap_or(0),
            _ => 0,
        };
        Ok(count)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn autotuner_increment_toast_count(&self) -> Result<i64, ApiError> {
        let orch = self
            .autotuner_orchestrator()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "AutoTuner is not enabled"))?;
        let current = match orch.learning_state_repo().get_value(TOAST_COUNT_KEY).await {
            Ok(Some(val)) => val.as_i64().unwrap_or(0),
            _ => 0,
        };
        let new_count = current + 1;
        orch.learning_state_repo()
            .set(
                TOAST_COUNT_KEY,
                &serde_json::Value::Number(new_count.into()),
            )
            .await
            .map_err(|e| {
                ApiError::new(
                    "INTERNAL",
                    format!("failed to set promotion toast count: {e}"),
                )
            })?;
        Ok(new_count)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn autotuner_set_pace(&self, pace: &str) -> Result<(), ApiError> {
        let orch = self
            .autotuner_orchestrator()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "AutoTuner is not enabled"))?;
        match pace {
            "conservative" | "balanced" | "bold" => {}
            _ => {
                return Err(ApiError::new(
                    "VALIDATION",
                    "pace must be conservative, balanced, or bold",
                ))
            }
        }
        orch.learning_state_repo()
            .set(EXPERIMENT_PACE_KEY, &serde_json::Value::String(pace.into()))
            .await
            .map_err(|e| {
                ApiError::new("INTERNAL", format!("failed to set experiment pace: {e}"))
            })?;
        info!(pace, "autotuner experiment pace updated");
        Ok(())
    }
}
