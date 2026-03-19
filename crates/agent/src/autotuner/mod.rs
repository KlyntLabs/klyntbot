//! AutoTuner orchestrator — thin L5 glue wiring the autotuner crate to the
//! agent runtime.  Holds champion state, coordinates shadow classification,
//! and metric collection.

pub mod hooks;
pub mod metric_collector;
pub mod shadow_classifier;

use std::sync::Arc;

use autotuner::{Champion, ChampionSummary, MetricSource, NightlyCycle};
use common::TrialParams;
use config::AutoTunerConfig;
use scheduling::{CronSchedule, CronService, JobCallback};
use storage::TrialRepo;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Cron job name for the nightly autotuner evaluation cycle.
pub const JOB_AUTOTUNER_NIGHTLY: &str = "__klyntbot_autotuner_nightly";

/// Thin glue that holds the current champion state and exposes it to the
/// agent runtime.  The nightly cycle updates the champion via
/// [`update_champion`].
pub struct AutoTunerOrchestrator {
    champion: RwLock<Champion>,
    active: bool,
}

impl AutoTunerOrchestrator {
    pub fn new(champion: Champion, enabled: bool) -> Self {
        Self {
            champion: RwLock::new(champion),
            active: enabled,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Return the current champion's trial params if the autotuner is active
    /// and a non-default champion has been promoted.
    pub async fn current_champion_params(&self) -> Option<TrialParams> {
        if !self.active {
            return None;
        }
        let champion = self.champion.read().await;
        if champion.trial_id.is_some() {
            Some(champion.params.clone())
        } else {
            None
        }
    }

    /// Replace the champion after a successful promotion.
    pub async fn update_champion(&self, new_champion: Champion) {
        *self.champion.write().await = new_champion;
    }

    /// Build a summary for the transparency panel / events.
    pub async fn champion_summary(&self) -> ChampionSummary {
        let c = self.champion.read().await;
        let days = (chrono::Utc::now() - c.promoted_at).num_days().max(0) as u32;
        ChampionSummary {
            trial_id: c.trial_id,
            description: c.reason_for_promotion.clone(),
            impact: c.impact_summary.clone(),
            promoted_at: c.promoted_at,
            days_active: days,
        }
    }

    /// Clone the full champion (for nightly cycle evaluation).
    pub async fn champion(&self) -> Champion {
        self.champion.read().await.clone()
    }

    /// Register the nightly evaluation cycle with the CronService.
    ///
    /// Creates a [`NightlyCycle`] and registers a named handler that:
    /// 1. Reads the current champion from the orchestrator.
    /// 2. Runs evaluation and promotion via [`NightlyCycle::run_evaluation_and_promotion`].
    /// 3. Updates the champion if a trial was promoted.
    /// 4. Increments the regression counter when regression is detected.
    /// 5. Logs the results.
    ///
    /// Must be called before wrapping the `CronService` in `Arc` (takes `&mut`).
    pub fn register_nightly_cycle(
        orchestrator: Arc<Self>,
        cron_service: &mut CronService,
        autotuner_config: AutoTunerConfig,
        trial_repo: TrialRepo,
        metric_source: Arc<dyn MetricSource>,
    ) {
        let rt = tokio::runtime::Handle::current();
        let cycle = Arc::new(NightlyCycle::new(
            autotuner_config.clone(),
            trial_repo,
            metric_source,
        ));
        let rollback_threshold = autotuner_config.rollback_after_days;

        let callback: JobCallback = Arc::new(move |_job: &scheduling::CronJob| {
            let orch = Arc::clone(&orchestrator);
            let cycle = Arc::clone(&cycle);
            let rollback_threshold = rollback_threshold;
            tokio::task::block_in_place(|| {
                rt.block_on(async move {
                    let champion = orch.champion().await;
                    let result = match cycle.run_evaluation_and_promotion(&champion).await {
                        Ok(r) => r,
                        Err(e) => {
                            error!("autotuner nightly cycle failed: {e}");
                            return Ok(Some(format!("autotuner nightly cycle error: {e}")));
                        }
                    };

                    let was_promoted = result.promotion.is_some();
                    info!(
                        completed = result.completed_count,
                        promoted = was_promoted,
                        regression = result.regression,
                        failed_constraints = result.failed_constraints.len(),
                        "autotuner nightly cycle completed"
                    );

                    // Handle promotion: update champion with new trial params + metrics.
                    if let Some((trial_id, trial_result, params)) = result.promotion {
                        info!(
                            trial_id = %trial_id,
                            correction_rate = trial_result.correction_rate,
                            "promoting trial to champion"
                        );
                        let new_champion = Champion {
                            trial_id: Some(trial_id),
                            params,
                            promoted_at: chrono::Utc::now(),
                            baseline_metrics: trial_result,
                            reason_for_promotion: format!(
                                "Promoted by nightly cycle (trial {trial_id})"
                            ),
                            impact_summary: format!(
                                "Evaluated {} trials, {} passed constraints",
                                result.completed_count,
                                result.completed_count - result.failed_constraints.len(),
                            ),
                            consecutive_regression_days: 0,
                        };
                        orch.update_champion(new_champion).await;
                    }

                    // Handle regression: increment counter, warn if approaching rollback.
                    if result.regression {
                        let mut champ = orch.champion.write().await;
                        champ.consecutive_regression_days =
                            champ.consecutive_regression_days.saturating_add(1);
                        let days = champ.consecutive_regression_days;
                        warn!(
                            consecutive_days = days,
                            rollback_threshold, "champion regression detected"
                        );
                        if days >= rollback_threshold {
                            warn!(
                                "regression threshold reached ({days}/{rollback_threshold}) — \
                                 rollback should be triggered by the next experiment cycle"
                            );
                        }
                    } else {
                        // Reset regression counter on a healthy day.
                        let mut champ = orch.champion.write().await;
                        if champ.consecutive_regression_days > 0 {
                            info!("regression counter reset (healthy day)");
                            champ.consecutive_regression_days = 0;
                        }
                    }

                    Ok(Some(format!(
                        "autotuner: evaluated {} trials, promoted={}",
                        result.completed_count, was_promoted
                    )))
                })
            })
        });

        cron_service.register_handler(JOB_AUTOTUNER_NIGHTLY, callback);
    }

    /// Ensure the nightly autotuner cron job exists in the CronService.
    ///
    /// Idempotent — skips if a job with the same name already exists.
    pub async fn ensure_nightly_job(
        cron_service: &Arc<CronService>,
        schedule_expr: &str,
    ) -> common::Result<()> {
        let existing: std::collections::HashSet<String> = cron_service
            .list_jobs(true)
            .await
            .into_iter()
            .map(|j| j.name)
            .collect();

        if !existing.contains(JOB_AUTOTUNER_NIGHTLY) {
            cron_service
                .add_job(
                    JOB_AUTOTUNER_NIGHTLY,
                    CronSchedule::Cron {
                        expr: schedule_expr.to_string(),
                        tz: None,
                    },
                    "Nightly autotuner evaluation and promotion cycle",
                    false,
                    None,
                    None,
                    false,
                    scheduling::CronOrigin::System,
                )
                .await?;
            info!("registered autotuner nightly cron job (schedule: {schedule_expr})");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inactive_returns_no_params() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), false);
        assert!(!orch.is_active());
        assert!(orch.current_champion_params().await.is_none());
    }

    #[tokio::test]
    async fn active_default_champion_returns_none() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), true);
        assert!(orch.is_active());
        // Default champion has trial_id = None → returns None.
        assert!(orch.current_champion_params().await.is_none());
    }

    #[tokio::test]
    async fn active_promoted_champion_returns_params() {
        let champion = Champion {
            trial_id: Some(uuid::Uuid::new_v4()),
            params: TrialParams {
                heuristic_confidence_threshold: Some(0.75),
                ..Default::default()
            },
            ..Champion::default()
        };
        let orch = AutoTunerOrchestrator::new(champion, true);
        let params = orch.current_champion_params().await;
        assert!(params.is_some());
        assert_eq!(params.unwrap().heuristic_confidence_threshold, Some(0.75));
    }

    #[tokio::test]
    async fn update_champion_replaces_state() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), true);
        assert!(orch.champion().await.trial_id.is_none());

        let new = Champion {
            trial_id: Some(uuid::Uuid::new_v4()),
            reason_for_promotion: "improved accuracy".into(),
            ..Champion::default()
        };
        orch.update_champion(new.clone()).await;

        let current = orch.champion().await;
        assert!(current.trial_id.is_some());
        assert_eq!(current.reason_for_promotion, "improved accuracy");
    }

    #[tokio::test]
    async fn champion_summary_populates_days_active() {
        let champion = Champion {
            promoted_at: chrono::Utc::now() - chrono::Duration::days(3),
            ..Champion::default()
        };
        let orch = AutoTunerOrchestrator::new(champion, true);
        let summary = orch.champion_summary().await;
        assert!(summary.days_active >= 3);
    }
}
