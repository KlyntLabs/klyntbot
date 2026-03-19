//! AutoTuner orchestrator — thin L5 glue wiring the autotuner crate to the
//! agent runtime.  Holds champion state, coordinates shadow classification,
//! and metric collection.

pub mod hooks;
pub mod metric_collector;
pub mod shadow_classifier;

use std::sync::Arc;

use autotuner::{
    build_generation_prompt, Champion, ChampionSummary, ExperimentSummary, GenerationContext,
    GenerationResponse, MetricSource, NightlyCycle, TrialSummaryForPrompt,
};
use common::TrialParams;
use config::AutoTunerConfig;
use providers::{ChatParams, DynProvider, Message};
use scheduling::{CronSchedule, CronService, JobCallback};
use storage::rows::trial::{ExperimentRow, TrialRow};
use storage::{LearningStateRepo, TrialRepo};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Cron job name for the nightly autotuner evaluation cycle.
pub const JOB_AUTOTUNER_NIGHTLY: &str = "__klyntbot_autotuner_nightly";

/// Key used to persist the current champion in the learning_state table.
const CHAMPION_KEY: &str = "autotuner_champion";
/// Key used to persist the previous champion (for rollback).
const PREVIOUS_CHAMPION_KEY: &str = "autotuner_previous_champion";
/// Key used to persist the paused flag.
const PAUSED_KEY: &str = "autotuner_paused";
/// Key used to persist the experiment pace preference.
const EXPERIMENT_PACE_KEY: &str = "autotuner_experiment_pace";
/// Key used to persist the previous cycle recommendation.
const RECOMMENDATION_KEY: &str = "autotuner_recommendation";

/// Thin glue that holds the current champion state and exposes it to the
/// agent runtime.  The nightly cycle updates the champion via
/// [`update_champion`].
pub struct AutoTunerOrchestrator {
    champion: RwLock<Champion>,
    active: bool,
    learning_state: LearningStateRepo,
    trial_repo: TrialRepo,
    provider: DynProvider,
    model: String,
}

impl AutoTunerOrchestrator {
    pub fn new(
        champion: Champion,
        enabled: bool,
        learning_state: LearningStateRepo,
        trial_repo: TrialRepo,
        provider: DynProvider,
        model: String,
    ) -> Self {
        Self {
            champion: RwLock::new(champion),
            active: enabled,
            learning_state,
            trial_repo,
            provider,
            model,
        }
    }

    /// Load the persisted champion from the learning_state table, falling
    /// back to `Champion::default()` when no entry exists.
    pub async fn load_champion(repo: &LearningStateRepo) -> Champion {
        match repo.get_value(CHAMPION_KEY).await {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            _ => Champion::default(),
        }
    }

    /// Return a reference to the learning state repo (used by handlers).
    pub fn learning_state_repo(&self) -> &LearningStateRepo {
        &self.learning_state
    }

    /// Return a reference to the trial repo (used by handlers).
    pub fn trial_repo(&self) -> &TrialRepo {
        &self.trial_repo
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

    /// Non-blocking try-read of champion params.  Returns `None` if the lock
    /// is currently held (e.g. during nightly promotion) or the autotuner is
    /// inactive / no champion has been promoted.
    pub fn try_current_champion_params(&self) -> Option<TrialParams> {
        if !self.active {
            return None;
        }
        let champion = self.champion.try_read().ok()?;
        if champion.trial_id.is_some() {
            Some(champion.params.clone())
        } else {
            None
        }
    }

    /// Replace the champion after a successful promotion, persisting both
    /// the old (for rollback) and new champion to the learning_state table.
    pub async fn update_champion(&self, new_champion: Champion) {
        // Save the old champion as previous (for rollback).
        let old_champion = self.champion.read().await.clone();
        if let Ok(old_json) = serde_json::to_value(&old_champion) {
            if let Err(e) = self
                .learning_state
                .set(PREVIOUS_CHAMPION_KEY, &old_json)
                .await
            {
                error!("failed to persist previous champion: {e}");
            }
        }
        // Save the new champion.
        if let Ok(new_json) = serde_json::to_value(&new_champion) {
            if let Err(e) = self.learning_state.set(CHAMPION_KEY, &new_json).await {
                error!("failed to persist new champion: {e}");
            }
        }
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
    /// 4. Handles regression — increments counter and triggers auto-rollback
    ///    when the threshold is reached.
    /// 5. Generates new trial variants via the LLM.
    /// 6. Logs the results.
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
                    // Check if paused.
                    if let Ok(Some(val)) = orch.learning_state.get_value(PAUSED_KEY).await {
                        if val.as_bool().unwrap_or(false) {
                            info!("autotuner is paused — skipping nightly cycle");
                            return Ok(Some("autotuner paused — skipped".to_string()));
                        }
                    }

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

                    // Handle regression: increment counter, trigger rollback when threshold met.
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
                            // Auto-rollback: revert to previous champion.
                            match orch.learning_state.get_value(PREVIOUS_CHAMPION_KEY).await {
                                Ok(Some(prev_json)) => {
                                    if let Ok(prev_champion) =
                                        serde_json::from_value::<Champion>(prev_json)
                                    {
                                        // Mark the reverted trial as Reverted in TrialRepo.
                                        if let Some(trial_id) = champ.trial_id {
                                            if let Err(e) = orch
                                                .trial_repo
                                                .update_trial_status(
                                                    &trial_id.to_string(),
                                                    autotuner::TrialStatus::Reverted.as_str(),
                                                )
                                                .await
                                            {
                                                error!(
                                                    "failed to mark trial {trial_id} as reverted: {e}"
                                                );
                                            }
                                        }

                                        warn!(
                                            "auto-rollback triggered after {days} regression days \
                                             — reverting to previous champion"
                                        );
                                        // Reset regression counter on the previous champion.
                                        let mut restored = prev_champion;
                                        restored.consecutive_regression_days = 0;
                                        // Persist the reverted champion.
                                        if let Ok(json) = serde_json::to_value(&restored) {
                                            let _ = orch
                                                .learning_state
                                                .set(CHAMPION_KEY, &json)
                                                .await;
                                        }
                                        *champ = restored;
                                    }
                                }
                                Ok(None) => {
                                    warn!(
                                        "regression threshold reached but no previous champion \
                                         stored — cannot rollback"
                                    );
                                }
                                Err(e) => {
                                    error!("failed to load previous champion for rollback: {e}");
                                }
                            }
                        }
                    } else {
                        // Reset regression counter on a healthy day.
                        let mut champ = orch.champion.write().await;
                        if champ.consecutive_regression_days > 0 {
                            info!("regression counter reset (healthy day)");
                            champ.consecutive_regression_days = 0;
                        }
                    }

                    // ── GENERATE step: create new trials via LLM ─────────────
                    if let Err(e) = run_llm_generation(&orch).await {
                        error!("autotuner LLM generation failed (non-fatal): {e}");
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

    /// Query recent experiments and return summaries for the history handler.
    pub async fn experiment_history(&self, limit: u32) -> common::Result<Vec<ExperimentSummary>> {
        let experiments = self.trial_repo.get_experiments(limit).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to fetch experiments: {e}"))
        })?;
        let mut summaries = Vec::with_capacity(experiments.len());
        for exp in experiments {
            let id = uuid::Uuid::parse_str(&exp.id).unwrap_or_else(|_| uuid::Uuid::nil());
            let started_at = chrono::DateTime::parse_from_rfc3339(&exp.created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            summaries.push(ExperimentSummary {
                id,
                variant_count: 3,   // We always generate 3 variants per experiment
                messages_scored: 0, // Not tracked at experiment level
                hypothesis: exp.hypothesis,
                started_at,
            });
        }
        Ok(summaries)
    }
}

/// Run the LLM generation step: build context, call the LLM, parse the response,
/// and create an experiment + trial records.
async fn run_llm_generation(orch: &AutoTunerOrchestrator) -> common::Result<()> {
    let champion = orch.champion().await;

    // Build recent trial history for the prompt.
    let recent_completed = orch
        .trial_repo
        .get_recent_completed(10)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("failed to fetch trials: {e}")))?;

    let recent_trials: Vec<TrialSummaryForPrompt> = recent_completed
        .iter()
        .map(|row| TrialSummaryForPrompt {
            id: row.id.chars().take(8).collect(),
            params: row.params.clone(),
            outcome: row.status.clone(),
            result: row.result.clone().unwrap_or_default(),
            reasoning: row.generation_reasoning.clone(),
        })
        .collect();

    // Read experiment pace from learning state (default: "balanced").
    let experiment_pace = match orch.learning_state.get_value(EXPERIMENT_PACE_KEY).await {
        Ok(Some(val)) => val.as_str().unwrap_or("balanced").to_string(),
        _ => "balanced".to_string(),
    };

    // Read previous cycle recommendation.
    let previous_recommendation = match orch.learning_state.get_value(RECOMMENDATION_KEY).await {
        Ok(Some(val)) => val.as_str().map(String::from),
        _ => None,
    };

    let context = GenerationContext {
        champion_params: champion.params.clone(),
        champion_metrics: champion.baseline_metrics.clone(),
        recent_trials,
        trend_summary: "Trend data not yet available.".to_string(),
        behavioral_context: "Behavioral data not yet available.".to_string(),
        memory_snapshot: "Memory snapshot not yet available.".to_string(),
        previous_recommendation,
        experiment_pace,
    };

    let prompt = build_generation_prompt(&context);
    let messages = vec![Message::user(prompt)];
    let params = ChatParams::new(orch.model.clone())
        .with_temperature(0.7)
        .with_response_format(providers::ResponseFormat::JsonObject);

    let response = orch.provider.chat(&messages, None, &params).await?;
    let content = response.content.ok_or_else(|| {
        common::KlyntbotError::Cron("autotuner: LLM returned empty response".into())
    })?;

    let generation: GenerationResponse = serde_json::from_str(&content).map_err(|e| {
        common::KlyntbotError::Cron(format!(
            "autotuner: failed to parse LLM generation response: {e}"
        ))
    })?;

    // Persist the recommendation for the next cycle.
    if let Ok(rec_json) = serde_json::to_value(&generation.recommendation_for_next_cycle) {
        let _ = orch.learning_state.set(RECOMMENDATION_KEY, &rec_json).await;
    }

    // Create experiment record.
    let experiment_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();
    let hypothesis = generation
        .variants
        .first()
        .map(|v| v.hypothesis.clone())
        .unwrap_or_else(|| "LLM-generated experiment".to_string());

    let experiment_row = ExperimentRow {
        id: experiment_id.to_string(),
        hypothesis,
        trend_analysis: generation.trend_analysis.clone(),
        recommendation_for_next: generation.recommendation_for_next_cycle.clone(),
        created_at: now.clone(),
    };
    orch.trial_repo
        .create_experiment(&experiment_row)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("create experiment: {e}")))?;

    // Create trial records for each variant (up to 3).
    let variant_count = generation.variants.len().min(3);
    for variant in generation.variants.into_iter().take(3) {
        let trial_id = uuid::Uuid::new_v4();
        let params_json = serde_json::to_string(&variant.params).unwrap_or_default();
        let trial_row = TrialRow {
            id: trial_id.to_string(),
            experiment_id: experiment_id.to_string(),
            params: params_json,
            generation_reasoning: variant.hypothesis,
            status: autotuner::TrialStatus::Active.as_str().to_string(),
            created_at: now.clone(),
            completed_at: None,
            result: None,
        };
        orch.trial_repo
            .create_trial(&trial_row)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("create trial: {e}")))?;
    }

    info!(
        experiment_id = %experiment_id,
        variants = variant_count,
        "generated new experiment with LLM"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory SQLite pool with the learning_state and trial tables.
    async fn setup_test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        // learning_state table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS learning_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        // autotuner trial tables
        sqlx::query(storage::repos::trial_repo::MIGRATION_SQL)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    /// Build a test orchestrator with in-memory repos and a NoopProvider.
    async fn make_orch(champion: Champion, enabled: bool) -> AutoTunerOrchestrator {
        let pool = setup_test_pool().await;
        let learning_state = LearningStateRepo::new(pool.clone());
        let trial_repo = TrialRepo::new(pool);
        let provider: DynProvider = Arc::new(providers::NoopProvider);
        AutoTunerOrchestrator::new(
            champion,
            enabled,
            learning_state,
            trial_repo,
            provider,
            "test-model".to_string(),
        )
    }

    #[tokio::test]
    async fn inactive_returns_no_params() {
        let orch = make_orch(Champion::default(), false).await;
        assert!(!orch.is_active());
        assert!(orch.current_champion_params().await.is_none());
    }

    #[tokio::test]
    async fn active_default_champion_returns_none() {
        let orch = make_orch(Champion::default(), true).await;
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
        let orch = make_orch(champion, true).await;
        let params = orch.current_champion_params().await;
        assert!(params.is_some());
        assert_eq!(params.unwrap().heuristic_confidence_threshold, Some(0.75));
    }

    #[tokio::test]
    async fn update_champion_persists_to_learning_state() {
        let orch = make_orch(Champion::default(), true).await;
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

        // Verify persistence: the new champion should be stored.
        let stored = orch.learning_state.get_value(CHAMPION_KEY).await.unwrap();
        assert!(stored.is_some());

        // Verify previous champion is also stored.
        let prev = orch
            .learning_state
            .get_value(PREVIOUS_CHAMPION_KEY)
            .await
            .unwrap();
        assert!(prev.is_some());
    }

    #[tokio::test]
    async fn load_champion_returns_default_when_empty() {
        let pool = setup_test_pool().await;
        let repo = LearningStateRepo::new(pool);
        let champion = AutoTunerOrchestrator::load_champion(&repo).await;
        assert!(champion.trial_id.is_none());
    }

    #[tokio::test]
    async fn load_champion_reads_persisted_value() {
        let pool = setup_test_pool().await;
        let repo = LearningStateRepo::new(pool);

        let original = Champion {
            trial_id: Some(uuid::Uuid::new_v4()),
            reason_for_promotion: "test persistence".into(),
            ..Champion::default()
        };
        let json = serde_json::to_value(&original).unwrap();
        repo.set(CHAMPION_KEY, &json).await.unwrap();

        let loaded = AutoTunerOrchestrator::load_champion(&repo).await;
        assert_eq!(loaded.trial_id, original.trial_id);
        assert_eq!(loaded.reason_for_promotion, "test persistence");
    }

    #[tokio::test]
    async fn champion_summary_populates_days_active() {
        let champion = Champion {
            promoted_at: chrono::Utc::now() - chrono::Duration::days(3),
            ..Champion::default()
        };
        let orch = make_orch(champion, true).await;
        let summary = orch.champion_summary().await;
        assert!(summary.days_active >= 3);
    }
}
