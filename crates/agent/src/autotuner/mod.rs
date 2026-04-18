//! AutoTuner orchestrator — thin L5 glue wiring the autotuner crate to the
//! agent runtime.  Holds champion state, coordinates shadow classification,
//! and metric collection.

pub mod hooks;
pub mod metric_collector;
pub mod shadow_retriever;

use std::sync::Arc;

use autotuner::{
    build_generation_prompt, Champion, ChampionSummary, ExperimentSummary, GenerationContext,
    GenerationResponse, MetricSource, NightlyCycle, TrialSummaryForPrompt,
};
use bus::DomainEventBus;
use common::TrialParams;
use config::AutoTunerConfig;
use providers::{ChatParams, DynProvider, Message};
use scheduling::{CronSchedule, CronService, JobCallback};
use storage::rows::trial::{ExperimentRow, TrialRow};
use storage::{LearningStateRepo, OverallStats, StrategyRepo, TrialRepo};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Cron job name for the nightly autotuner evaluation cycle.
pub const JOB_AUTOTUNER_NIGHTLY: &str = "__klyntbot_autotuner_nightly";

/// Key used to persist the current champion in the learning_state table.
pub const CHAMPION_KEY: &str = "autotuner_champion";
/// Key used to persist the previous champion (for rollback).
pub const PREVIOUS_CHAMPION_KEY: &str = "autotuner_previous_champion";
/// Key used to persist the paused flag.
pub const PAUSED_KEY: &str = "autotuner_paused";
/// Key used to persist the experiment pace preference.
pub const EXPERIMENT_PACE_KEY: &str = "autotuner_experiment_pace";
/// Key used to persist the previous cycle recommendation.
pub const RECOMMENDATION_KEY: &str = "autotuner_recommendation";
/// Key used to persist the promotion toast count for the desktop UI.
pub const TOAST_COUNT_KEY: &str = "autotuner_promotion_toast_count";
/// Key used to persist a Knowledge Trust score snapshot for trend computation.
pub const KNOWLEDGE_TRUST_SNAPSHOT_KEY: &str = "knowledge_trust_snapshot";

/// Thin glue that holds the current champion state and exposes it to the
/// agent runtime.  The nightly cycle updates the champion via
/// [`update_champion`].
pub struct AutoTunerOrchestrator {
    champion: RwLock<Champion>,
    learning_state: LearningStateRepo,
    trial_repo: TrialRepo,
    provider: DynProvider,
    model: String,
    strategy_repo: Option<StrategyRepo>,
    episodic_memory_repo: Option<cognitive::EpisodicMemoryRepo>,
    /// Shared lock for live memory param injection. Written on champion promotion,
    /// read by UnifiedMemoryService on each retrieval.
    memory_param_sink: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
}

impl AutoTunerOrchestrator {
    pub fn new(
        champion: Champion,
        learning_state: LearningStateRepo,
        trial_repo: TrialRepo,
        provider: DynProvider,
        model: String,
    ) -> Self {
        Self {
            champion: RwLock::new(champion),
            learning_state,
            trial_repo,
            provider,
            model,
            strategy_repo: None,
            episodic_memory_repo: None,
            memory_param_sink: None,
        }
    }

    /// Set the strategy repo for generation context enrichment.
    pub fn with_strategy_repo(mut self, repo: StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    /// Set the episodic memory repo for generation context enrichment.
    pub fn with_episodic_memory_repo(mut self, repo: cognitive::EpisodicMemoryRepo) -> Self {
        self.episodic_memory_repo = Some(repo);
        self
    }

    /// Set the shared lock for propagating champion memory params to live retrieval.
    pub fn with_memory_param_sink(
        mut self,
        sink: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        // Seed with current champion params so live retrieval uses them immediately.
        if let Ok(champion) = self.champion.try_read() {
            if champion.trial_id.is_some() {
                let mut guard = sink.write().unwrap_or_else(|e| e.into_inner());
                *guard = Some(champion.params.clone());
            }
        }
        self.memory_param_sink = Some(sink);
        self
    }

    /// Expose the shared lock so the builder can pass it to UnifiedMemoryService.
    pub fn memory_param_sink(&self) -> Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>> {
        self.memory_param_sink.clone()
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
        true
    }

    /// Check if Phase 2 (memory optimization) is ready to activate.
    /// Requires champion stable >= 7 days.
    pub fn is_phase2_ready(&self) -> bool {
        let Some(champion) = self.champion.try_read().ok() else {
            return false;
        };
        let days_since_promotion = (jiff::Timestamp::now().as_millisecond() - champion.promoted_at.as_millisecond()) / 86_400_000;
        days_since_promotion >= 7
    }

    /// Return the current champion's trial params if the autotuner is active
    /// and a non-default champion has been promoted.
    pub async fn current_champion_params(&self) -> Option<TrialParams> {
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
        // Acquire write lock once to avoid double-lock.
        let mut guard = self.champion.write().await;
        let old_champion = guard.clone();

        // Save the old champion as previous (for rollback).
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
        *guard = new_champion;

        // Extract sink data before dropping the tokio write lock — avoids holding
        // the async lock while acquiring the std::sync lock on the sink.
        let sink_value = if guard.trial_id.is_some() {
            Some(guard.params.clone())
        } else {
            None
        };
        drop(guard);

        // Propagate memory params to live retrieval via shared lock.
        if let Some(ref sink) = self.memory_param_sink {
            let mut param_guard = sink.write().unwrap_or_else(|e| e.into_inner());
            *param_guard = sink_value;
        }
    }

    /// Build a summary for the transparency panel / events.
    pub async fn champion_summary(&self) -> ChampionSummary {
        let c = self.champion.read().await;
        let days = ((jiff::Timestamp::now().as_millisecond() - c.promoted_at.as_millisecond()) / 86_400_000).max(0) as u32;
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

    /// Increment the regression counter and trigger auto-rollback if the
    /// threshold is reached. Returns `true` if rollback was performed.
    pub async fn handle_regression(
        &self,
        rollback_threshold: u8,
        domain_event_bus: Option<&Arc<DomainEventBus>>,
    ) -> bool {
        let mut champ = self.champion.write().await;
        champ.consecutive_regression_days = champ.consecutive_regression_days.saturating_add(1);
        let days = champ.consecutive_regression_days;
        warn!(
            consecutive_days = days,
            rollback_threshold, "autotuner: champion regression detected"
        );

        if days < rollback_threshold {
            // Persist updated counter.
            if let Ok(json) = serde_json::to_value(&*champ) {
                if let Err(e) = self.learning_state.set(CHAMPION_KEY, &json).await {
                    error!(error = %e, "failed to persist regression counter");
                }
            }
            return false;
        }

        // Auto-rollback: revert to previous champion.
        match self.learning_state.get_value(PREVIOUS_CHAMPION_KEY).await {
            Ok(Some(prev_json)) => {
                if let Ok(prev_champion) = serde_json::from_value::<Champion>(prev_json) {
                    let rolled_back_trial_id = champ.trial_id;

                    // Mark the reverted trial in TrialRepo.
                    if let Some(trial_id) = rolled_back_trial_id {
                        if let Err(e) = self
                            .trial_repo
                            .update_trial_status(
                                &trial_id.to_string(),
                                autotuner::TrialStatus::Reverted.as_str(),
                            )
                            .await
                        {
                            error!("failed to mark trial {trial_id} as reverted: {e}");
                        }
                    }

                    warn!(
                        "auto-rollback triggered after {days} regression days \
                         — reverting to previous champion"
                    );

                    // Compute event fields before overwriting.
                    let rollback_improvement_pct =
                        if prev_champion.baseline_metrics.correction_rate > 0.0 {
                            (prev_champion.baseline_metrics.correction_rate
                                - champ.baseline_metrics.correction_rate)
                                / prev_champion.baseline_metrics.correction_rate
                                * 100.0
                        } else {
                            0.0
                        };
                    let rollback_affected_params =
                        autotuner::affected_param_names(&champ.params, &prev_champion.params);

                    let mut restored = prev_champion;
                    restored.consecutive_regression_days = 0;
                    if let Ok(json) = serde_json::to_value(&restored) {
                        if let Err(e) = self.learning_state.set(CHAMPION_KEY, &json).await {
                            error!(error = %e, "failed to persist reverted champion");
                        }
                    }
                    *champ = restored;

                    // Emit domain event for rollback.
                    if let Some(bus) = domain_event_bus {
                        let trial_str = rolled_back_trial_id
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        bus.publish(bus::DomainEvent::AutotunerDecision {
                            trial_id: trial_str,
                            verdict: autotuner::TrialStatus::Reverted.as_str().to_string(),
                            improvement_pct: rollback_improvement_pct,
                            affected_params: rollback_affected_params,
                        });
                    }

                    return true;
                }
            }
            Ok(None) => {
                warn!("regression threshold reached but no previous champion — cannot rollback");
            }
            Err(e) => {
                error!("failed to load previous champion for rollback: {e}");
            }
        }
        false
    }

    /// Reset the regression counter on a healthy evaluation day.
    pub async fn reset_regression_counter(&self) {
        let mut champ = self.champion.write().await;
        if champ.consecutive_regression_days > 0 {
            info!("regression counter reset (healthy day)");
            champ.consecutive_regression_days = 0;
            if let Ok(json) = serde_json::to_value(&*champ) {
                if let Err(e) = self.learning_state.set(CHAMPION_KEY, &json).await {
                    error!(error = %e, "failed to persist regression counter reset");
                }
            }
        }
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
    /// Can be called before or after wrapping the `CronService` in `Arc`.
    pub fn register_nightly_cycle(
        orchestrator: Arc<Self>,
        cron_service: &CronService,
        autotuner_config: AutoTunerConfig,
        trial_repo: TrialRepo,
        metric_source: Arc<dyn MetricSource>,
        domain_event_bus: Option<Arc<DomainEventBus>>,
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
            let domain_event_bus = domain_event_bus.clone();
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

                    // Log per-trial evaluation results for observability.
                    for (trial_id, trial_result) in &result.evaluated_trials {
                        let verdict = if result
                            .promotion
                            .as_ref()
                            .map(|(pid, _, _)| pid)
                            == Some(trial_id)
                        {
                            "PASS"
                        } else {
                            "FAIL"
                        };
                        info!(
                            trial_id = %trial_id,
                            correction_rate = %format!("{:.3}", trial_result.correction_rate),
                            accuracy = %format!("{:.3}", trial_result.classification_accuracy),
                            messages = trial_result.messages_scored,
                            verdict = verdict,
                            "Autotuner: trial evaluated"
                        );
                    }

                    // Handle promotion: update champion with new trial params + metrics.
                    if let Some((trial_id, trial_result, params)) = result.promotion {
                        info!(
                            trial_id = %trial_id,
                            correction_rate = trial_result.correction_rate,
                            "promoting trial to champion"
                        );

                        // Compute event fields before moving values into new_champion.
                        let improvement_pct =
                            if champion.baseline_metrics.correction_rate > 0.0 {
                                (champion.baseline_metrics.correction_rate
                                    - trial_result.correction_rate)
                                    / champion.baseline_metrics.correction_rate
                                    * 100.0
                            } else {
                                0.0
                            };
                        let affected_params =
                            autotuner::affected_param_names(&champion.params, &params);

                        let new_champion = Champion {
                            trial_id: Some(trial_id),
                            params,
                            promoted_at: jiff::Timestamp::now(),
                            baseline_metrics: trial_result,
                            reason_for_promotion: format!(
                                "Promoted by nightly cycle (trial {trial_id})"
                            ),
                            impact_summary: format!(
                                "Evaluated {} trials, {} passed constraints",
                                result.completed_count,
                                result.completed_count.saturating_sub(result.failed_constraints.len()),
                            ),
                            consecutive_regression_days: 0,
                        };
                        orch.update_champion(new_champion).await;

                        // Emit domain event for promotion.
                        if let Some(ref bus) = domain_event_bus {
                            bus.publish(bus::DomainEvent::AutotunerDecision {
                                trial_id: trial_id.to_string(),
                                verdict: autotuner::TrialStatus::Promoted
                                    .as_str()
                                    .to_string(),
                                improvement_pct,
                                affected_params,
                            });
                        }
                    }

                    // Handle regression: increment counter, trigger rollback when threshold met.
                    if result.regression {
                        let mut champ = orch.champion.write().await;
                        champ.consecutive_regression_days =
                            champ.consecutive_regression_days.saturating_add(1);
                        let days = champ.consecutive_regression_days;
                        warn!(
                            consecutive_days = days,
                            rollback_threshold,
                            "autotuner: champion regression detected"
                        );

                        if days >= rollback_threshold {
                            // Auto-rollback: revert to previous champion.
                            match orch.learning_state.get_value(PREVIOUS_CHAMPION_KEY).await {
                                Ok(Some(prev_json)) => {
                                    if let Ok(prev_champion) =
                                        serde_json::from_value::<Champion>(prev_json)
                                    {
                                        // Capture trial_id before overwriting champ.
                                        let rolled_back_trial_id = champ.trial_id;

                                        // Mark the reverted trial as Reverted in TrialRepo.
                                        if let Some(trial_id) = rolled_back_trial_id {
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

                                        // Compute event fields before overwriting champ.
                                        let rollback_improvement_pct = if prev_champion
                                            .baseline_metrics
                                            .correction_rate
                                            > 0.0
                                        {
                                            (prev_champion.baseline_metrics.correction_rate
                                                - champ.baseline_metrics.correction_rate)
                                                / prev_champion.baseline_metrics.correction_rate
                                                * 100.0
                                        } else {
                                            0.0
                                        };
                                        let rollback_affected_params =
                                            autotuner::affected_param_names(
                                                &champ.params,
                                                &prev_champion.params,
                                            );

                                        // Reset regression counter on the previous champion.
                                        let mut restored = prev_champion;
                                        restored.consecutive_regression_days = 0;
                                        // Persist the reverted champion.
                                        if let Ok(json) = serde_json::to_value(&restored) {
                                            if let Err(e) = orch
                                                .learning_state
                                                .set(CHAMPION_KEY, &json)
                                                .await
                                            {
                                                error!(error = %e, "Failed to persist reverted champion");
                                            }
                                        }
                                        *champ = restored;

                                        // Emit domain event for rollback.
                                        if let Some(ref bus) = domain_event_bus {
                                            let trial_str = rolled_back_trial_id
                                                .map(|id| id.to_string())
                                                .unwrap_or_default();
                                            bus.publish(
                                                bus::DomainEvent::AutotunerDecision {
                                                    trial_id: trial_str,
                                                    verdict: autotuner::TrialStatus::Reverted
                                                        .as_str()
                                                        .to_string(),
                                                    improvement_pct: rollback_improvement_pct,
                                                    affected_params: rollback_affected_params,
                                                },
                                            );
                                        }
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
                    match run_llm_generation(&orch).await {
                        Ok(created_trials) => {
                            if let Some(ref bus) = domain_event_bus {
                                for trial in &created_trials {
                                    bus.publish(bus::DomainEvent::TrialActivated {
                                        trial_id: trial.id.clone(),
                                        hypothesis: trial.hypothesis.clone(),
                                        params_summary: trial.params_summary.clone(),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            error!("autotuner LLM generation failed (non-fatal): {e}");
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

    /// Query recent experiments and return summaries for the history handler.
    pub async fn experiment_history(&self, limit: u32) -> common::Result<Vec<ExperimentSummary>> {
        let experiments = self.trial_repo.get_experiments(limit).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to fetch experiments: {e}"))
        })?;
        let mut summaries = Vec::with_capacity(experiments.len());
        for exp in experiments {
            let id = uuid::Uuid::parse_str(&exp.id).unwrap_or_else(|_| uuid::Uuid::nil());
            let started_at = exp.created_at.parse::<jiff::Timestamp>().unwrap_or_else(|_| jiff::Timestamp::now());
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

/// Build a human-readable 7-day trend summary from strategy stats and agreement rate.
fn build_trend_summary(stats: &OverallStats, agreement_rate: f64) -> String {
    let satisfaction = stats
        .avg_satisfaction
        .map(|s| format!("{:.0}%", s * 100.0))
        .unwrap_or_else(|| "no data".into());
    format!(
        "Last 7 days: {} messages processed, {:.1}% classification accuracy, \
         {}ms avg response time, {:.1}% routing stability, {} user satisfaction.",
        stats.total_records,
        stats.accuracy * 100.0,
        stats.avg_response_time_ms,
        agreement_rate * 100.0,
        satisfaction,
    )
}

/// Build a behavioral context string from per-strategy summaries.
fn build_behavioral_context(summaries: &[storage::StrategySummaryRow]) -> String {
    if summaries.is_empty() {
        return "No strategy data available for the past 7 days.".to_string();
    }
    let mut lines = Vec::with_capacity(summaries.len() + 1);
    lines.push("Per-strategy breakdown (last 7 days):".to_string());
    for s in summaries {
        let accuracy = if s.sample_count > 0 {
            s.correct_count as f64 / s.sample_count as f64 * 100.0
        } else {
            0.0
        };
        lines.push(format!(
            "- {}: {} samples, {:.1}% accuracy, {:.2} avg escalations",
            s.predicted_strategy, s.sample_count, accuracy, s.avg_escalations,
        ));
    }
    lines.join("\n")
}

/// Build a memory snapshot string from recent episodic memories.
fn build_memory_snapshot(memories: &[cognitive::EpisodicMemory]) -> String {
    if memories.is_empty() {
        return "No recent episodic memories.".to_string();
    }
    let mut lines = Vec::with_capacity(memories.len() + 1);
    lines.push(format!("Recent episodic memories ({}):", memories.len()));
    for m in memories {
        let summary = m
            .summary
            .as_deref()
            .unwrap_or_else(|| &m.content[..m.content.len().min(80)]);
        lines.push(format!("- [{}] {}", m.domain.as_str(), summary));
    }
    lines.join("\n")
}

/// Run the LLM generation step: build context, call the LLM, parse the response,
/// and create an experiment + trial records.
/// Info about a trial created during LLM generation, for emitting domain events.
struct CreatedTrial {
    id: String,
    hypothesis: String,
    params_summary: String,
}

async fn run_llm_generation(orch: &AutoTunerOrchestrator) -> common::Result<Vec<CreatedTrial>> {
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

    // ── Build enriched context from real data (fall back to placeholders) ──
    let seven_days_ago = jiff::Timestamp::now()
        .checked_sub(jiff::SignedDuration::from_secs(7 * 86400))
        .unwrap();

    let trend_summary = if let Some(ref strategy_repo) = orch.strategy_repo {
        let stats = strategy_repo
            .get_stats_since(seven_days_ago)
            .await
            .unwrap_or(OverallStats {
                total_records: 0,
                accuracy: 0.0,
                avg_response_time_ms: 0,
                avg_satisfaction: None,
            });
        let agreement_rate = orch
            .trial_repo
            .shadow_log_agreement_rate(None, seven_days_ago)
            .await
            .unwrap_or(1.0);
        build_trend_summary(&stats, agreement_rate)
    } else {
        "Trend data not yet available.".to_string()
    };

    let behavioral_context = if let Some(ref strategy_repo) = orch.strategy_repo {
        let summaries = strategy_repo
            .get_strategy_summaries(seven_days_ago)
            .await
            .unwrap_or_default();
        build_behavioral_context(&summaries)
    } else {
        "Behavioral data not yet available.".to_string()
    };

    let memory_snapshot = if let Some(ref episodic_repo) = orch.episodic_memory_repo {
        let memories = episodic_repo.list_recent(10).await.unwrap_or_default();
        build_memory_snapshot(&memories)
    } else {
        "Memory snapshot not yet available.".to_string()
    };

    let context = GenerationContext {
        champion_params: champion.params.clone(),
        champion_metrics: champion.baseline_metrics.clone(),
        recent_trials,
        trend_summary,
        behavioral_context,
        memory_snapshot,
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
        if let Err(e) = orch.learning_state.set(RECOMMENDATION_KEY, &rec_json).await {
            warn!(error = %e, "failed to persist recommendation for next cycle");
        }
    }

    // Create experiment record.
    let experiment_id = uuid::Uuid::new_v4();
    let now = jiff::Timestamp::now().to_string();
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
    let mut created_trials = Vec::new();
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
        // Move values out of trial_row after create_trial borrows it
        created_trials.push(CreatedTrial {
            id: trial_row.id,
            hypothesis: trial_row.generation_reasoning,
            params_summary: trial_row.params,
        });
    }

    info!(
        experiment_id = %experiment_id,
        variants = variant_count,
        "generated new experiment with LLM"
    );

    Ok(created_trials)
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
                updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
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
    async fn make_orch(champion: Champion) -> AutoTunerOrchestrator {
        let pool = setup_test_pool().await;
        let learning_state = LearningStateRepo::new(pool.clone());
        let trial_repo = TrialRepo::new(pool);
        let provider: DynProvider = Arc::new(providers::NoopProvider);
        AutoTunerOrchestrator::new(
            champion,
            learning_state,
            trial_repo,
            provider,
            "test-model".to_string(),
        )
    }

    #[tokio::test]
    async fn default_champion_returns_none() {
        let orch = make_orch(Champion::default()).await;
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
        let orch = make_orch(champion).await;
        let params = orch.current_champion_params().await;
        assert!(params.is_some());
        assert_eq!(params.unwrap().heuristic_confidence_threshold, Some(0.75));
    }

    #[tokio::test]
    async fn update_champion_persists_to_learning_state() {
        let orch = make_orch(Champion::default()).await;
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
            promoted_at: jiff::Timestamp::now()
                .checked_sub(jiff::SignedDuration::from_secs(3 * 86400))
                .unwrap(),
            ..Champion::default()
        };
        let orch = make_orch(champion).await;
        let summary = orch.champion_summary().await;
        assert!(summary.days_active >= 3);
    }

    #[test]
    fn build_trend_summary_formats_correctly() {
        let stats = OverallStats {
            total_records: 150,
            accuracy: 0.873,
            avg_response_time_ms: 420,
            avg_satisfaction: Some(0.82),
        };
        let result = build_trend_summary(&stats, 0.91);
        assert!(result.contains("150 messages processed"));
        assert!(result.contains("87.3% classification accuracy"));
        assert!(result.contains("420ms avg response time"));
        assert!(result.contains("91.0% routing stability"));
        assert!(result.contains("82% user satisfaction"));
    }

    #[test]
    fn build_trend_summary_handles_no_satisfaction() {
        let stats = OverallStats {
            total_records: 0,
            accuracy: 0.0,
            avg_response_time_ms: 0,
            avg_satisfaction: None,
        };
        let result = build_trend_summary(&stats, 1.0);
        assert!(result.contains("no data"));
    }

    #[test]
    fn build_behavioral_context_with_data() {
        let summaries = vec![
            storage::StrategySummaryRow {
                predicted_strategy: "DirectResponse".to_string(),
                sample_count: 80,
                correct_count: 72,
                avg_escalations: 0.1,
            },
            storage::StrategySummaryRow {
                predicted_strategy: "ToolAssisted".to_string(),
                sample_count: 40,
                correct_count: 35,
                avg_escalations: 1.5,
            },
        ];
        let result = build_behavioral_context(&summaries);
        assert!(result.contains("DirectResponse"));
        assert!(result.contains("80 samples"));
        assert!(result.contains("90.0% accuracy"));
        assert!(result.contains("ToolAssisted"));
    }

    #[test]
    fn build_behavioral_context_empty() {
        let result = build_behavioral_context(&[]);
        assert!(result.contains("No strategy data"));
    }

    #[test]
    fn build_memory_snapshot_with_data() {
        let memories = vec![
            cognitive::EpisodicMemory {
                id: "m1".into(),
                domain: "productivity".into(),
                content: "Completed sprint review with team".into(),
                summary: Some("Sprint review completed".into()),
                importance: 0.8,
                occurred_at: "2026-03-19T10:00:00".into(),
                recorded_at: "2026-03-19T10:00:00".into(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".into(),
                scope_id: None,
            },
            cognitive::EpisodicMemory {
                id: "m2".into(),
                domain: "finance".into(),
                content: "Reviewed monthly budget allocation".into(),
                summary: None,
                importance: 0.6,
                occurred_at: "2026-03-18T14:00:00".into(),
                recorded_at: "2026-03-18T14:00:00".into(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".into(),
                scope_id: None,
            },
        ];
        let result = build_memory_snapshot(&memories);
        assert!(result.contains("[productivity]"));
        assert!(result.contains("Sprint review completed"));
        assert!(result.contains("[finance]"));
        assert!(result.contains("Reviewed monthly budget allocation"));
    }

    #[test]
    fn build_memory_snapshot_empty() {
        let result = build_memory_snapshot(&[]);
        assert!(result.contains("No recent episodic memories"));
    }
}
