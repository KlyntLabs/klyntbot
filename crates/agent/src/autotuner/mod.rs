//! AutoTuner orchestrator — thin L5 glue wiring the autotuner crate to the
//! agent runtime.  Holds champion state, coordinates shadow classification,
//! and metric collection.

pub mod hooks;
pub mod metric_collector;
pub mod shadow_classifier;
pub mod shadow_retriever;

use std::sync::Arc;

use autotuner::{
    build_generation_prompt, Champion, ChampionSummary, ExperimentSummary, GenerationContext,
    GenerationResponse, MetricSource, NightlyCycle, ShadowClassifier, TrialSummaryForPrompt,
};
use bus::DomainEventBus;
use chrono::Utc;
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
    strategy_repo: Option<StrategyRepo>,
    episodic_memory_repo: Option<cognitive::EpisodicMemoryRepo>,
    session_repo: Option<storage::SessionRepo>,
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
            strategy_repo: None,
            episodic_memory_repo: None,
            session_repo: None,
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

    /// Set the session repo for bootstrap replay (seeding first experiment from history).
    pub fn with_session_repo(mut self, repo: storage::SessionRepo) -> Self {
        self.session_repo = Some(repo);
        self
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

    /// Check if Phase 2 (memory optimization) is ready to activate.
    /// Requires champion stable >= 7 days.
    pub fn is_phase2_ready(&self) -> bool {
        if !self.active {
            return false;
        }
        let Some(champion) = self.champion.try_read().ok() else {
            return false;
        };
        let days_since_promotion = (Utc::now() - champion.promoted_at).num_days();
        days_since_promotion >= 7
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
                    // Record autotuner start date (first run only) for Phase 2 readiness.
                    if orch
                        .learning_state
                        .get_value("autotuner_started_at")
                        .await
                        .ok()
                        .flatten()
                        .is_none()
                    {
                        let _ = orch
                            .learning_state
                            .set(
                                "autotuner_started_at",
                                &serde_json::Value::String(Utc::now().to_rfc3339()),
                            )
                            .await;
                    }

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
                            promoted_at: chrono::Utc::now(),
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

/// Run bootstrap replay: seed the first experiment from real historical session data.
///
/// Skips if experiments already exist (not first startup). Loads sessions from the
/// last 7 days, runs shadow classification with random parameter perturbations,
/// matches predictions against strategy_records ground truth, and creates an
/// experiment with the top 3 variants ranked by accuracy.
pub async fn run_bootstrap_replay(orch: &AutoTunerOrchestrator) -> common::Result<()> {
    use rand::Rng;
    use rand::SeedableRng;

    // 1. Check if experiments already exist — skip if so (not first startup).
    let existing = orch
        .trial_repo
        .get_experiments(1)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("check experiments: {e}")))?;
    if !existing.is_empty() {
        info!("bootstrap replay: experiments already exist, skipping");
        return Ok(());
    }

    // 2. Need session_repo and strategy_repo.
    let session_repo = match &orch.session_repo {
        Some(r) => r,
        None => {
            info!("bootstrap replay: no session_repo configured, skipping");
            return Ok(());
        }
    };
    let strategy_repo = match &orch.strategy_repo {
        Some(r) => r,
        None => {
            info!("bootstrap replay: no strategy_repo configured, skipping");
            return Ok(());
        }
    };

    // 3. Load sessions from last 7 days.
    let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
    let sessions = session_repo
        .list_sessions_since(seven_days_ago)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("list sessions: {e}")))?;
    if sessions.is_empty() {
        info!("bootstrap replay: no recent sessions found, skipping");
        return Ok(());
    }

    // 4. Load strategy_records for ground truth matching.
    let strategy_records = strategy_repo
        .list_by_date_range(seven_days_ago, chrono::Utc::now())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("list strategy records: {e}")))?;
    if strategy_records.is_empty() {
        info!("bootstrap replay: no strategy records found, skipping");
        return Ok(());
    }

    // 5. Generate 5 random parameter perturbations.
    let mut rng = rand::rngs::SmallRng::from_os_rng();
    let perturbations: Vec<TrialParams> = (0..5)
        .map(|_| TrialParams {
            skill_keyword_weight: Some(rng.random_range(0.30..=0.90)),
            skill_semantic_weight: Some(rng.random_range(0.10..=0.70)),
            skill_activation_threshold: Some(rng.random_range(0.20..=0.70)),
            heuristic_confidence_threshold: Some(rng.random_range(0.60..=0.95)),
            llm_classifier_timeout_ms: None,
            relevance_weight_semantic: Some(rng.random_range(0.10..=0.50)),
            relevance_weight_retrievability: Some(rng.random_range(0.05..=0.40)),
            relevance_weight_situation: Some(rng.random_range(0.10..=0.40)),
            ..Default::default()
        })
        .collect();

    // 6. Build the shadow classifier.
    let config = config::OrchestratorConfig::default();
    let classifier = shadow_classifier::AgentShadowClassifier::new(
        orch.provider.clone(),
        &orch.model,
        &config,
    );

    // 7. For each session's user messages, run shadow classification and compare
    //    against ground truth strategy_records (matched by chat_id + timestamp ±30s).
    let mut variant_scores: Vec<(usize, u32, u32)> = (0..5).map(|i| (i, 0u32, 0u32)).collect();

    // Collect user messages from sessions (limit to 50 sessions to keep it fast).
    let session_limit = sessions.len().min(50);
    for session in sessions.iter().take(session_limit) {
        let messages = session_repo
            .get_messages(&session.key)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("get messages: {e}")))?;

        // Extract chat_id from session key (format: "channel:chat_id").
        let chat_id = session
            .key
            .split_once(':')
            .map(|(_, id)| id)
            .unwrap_or(&session.key)
            .to_string();

        let user_messages: Vec<_> = messages
            .iter()
            .filter(|m| m.role == "user")
            .collect();

        for msg in &user_messages {
            let shadow_ctx = autotuner::ShadowContext {
                chat_id: chat_id.clone(),
                session_key: session.key.clone(),
            };

            // Find matching ground truth: strategy_record with same chat_id
            // and timestamp within 30 seconds.
            let ground_truth = strategy_records.iter().find(|sr| {
                sr.chat_id.as_deref() == Some(&chat_id)
                    && (sr.timestamp - msg.timestamp).num_seconds().abs() <= 30
            });

            let actual_mode = match ground_truth {
                Some(sr) => sr.actual_strategy.clone(),
                None => continue, // No ground truth for this message, skip.
            };

            // Run each perturbation and score it.
            for (idx, params) in perturbations.iter().enumerate() {
                match classifier
                    .classify_shadow(&msg.content, &shadow_ctx, params)
                    .await
                {
                    Ok(prediction) => {
                        variant_scores[idx].2 += 1; // total
                        if prediction.predicted_mode == actual_mode {
                            variant_scores[idx].1 += 1; // correct
                        }
                    }
                    Err(_) => {
                        // Count as attempted but incorrect.
                        variant_scores[idx].2 += 1;
                    }
                }
            }
        }
    }

    // 8. Sort results by accuracy descending.
    variant_scores.sort_by(|a, b| {
        let acc_a = if a.2 > 0 {
            a.1 as f64 / a.2 as f64
        } else {
            0.0
        };
        let acc_b = if b.2 > 0 {
            b.1 as f64 / b.2 as f64
        } else {
            0.0
        };
        acc_b.partial_cmp(&acc_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Need at least one scored variant.
    if variant_scores.iter().all(|v| v.2 == 0) {
        info!("bootstrap replay: no messages matched ground truth, skipping");
        return Ok(());
    }

    // 9. Create experiment with top 3 variants as active trials.
    let experiment_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().to_rfc3339();
    let top_accuracy = if variant_scores[0].2 > 0 {
        variant_scores[0].1 as f64 / variant_scores[0].2 as f64 * 100.0
    } else {
        0.0
    };

    let experiment_row = ExperimentRow {
        id: experiment_id.to_string(),
        hypothesis: format!(
            "Bootstrap replay: seeded from {} sessions, {} strategy records. \
             Top variant accuracy: {:.1}%",
            session_limit,
            strategy_records.len(),
            top_accuracy,
        ),
        trend_analysis: "Initial bootstrap — no prior experiments.".to_string(),
        recommendation_for_next: "Continue with nightly LLM-generated experiments.".to_string(),
        created_at: now.clone(),
    };
    orch.trial_repo
        .create_experiment(&experiment_row)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("create experiment: {e}")))?;

    let variant_count = variant_scores.len().min(3);
    for &(idx, correct, total) in variant_scores.iter().take(3) {
        let accuracy = if total > 0 {
            correct as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let trial_id = uuid::Uuid::new_v4();
        let params_json = serde_json::to_string(&perturbations[idx]).unwrap_or_default();
        let trial_row = TrialRow {
            id: trial_id.to_string(),
            experiment_id: experiment_id.to_string(),
            params: params_json,
            generation_reasoning: format!(
                "Bootstrap replay variant: {correct}/{total} correct ({accuracy:.1}% accuracy)"
            ),
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
        sessions_scanned = session_limit,
        strategy_records = strategy_records.len(),
        "bootstrap replay: seeded initial experiment from historical data"
    );

    Ok(())
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

    // ── Build enriched context from real data (fall back to placeholders) ──
    let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);

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

    #[tokio::test]
    async fn bootstrap_replay_noop_when_experiments_exist() {
        let pool = setup_test_pool().await;
        let learning_state = LearningStateRepo::new(pool.clone());
        let trial_repo = TrialRepo::new(pool.clone());
        let provider: DynProvider = Arc::new(providers::NoopProvider);

        // Pre-create an experiment so bootstrap sees it's not first startup.
        let exp = ExperimentRow {
            id: uuid::Uuid::new_v4().to_string(),
            hypothesis: "existing experiment".into(),
            trend_analysis: "n/a".into(),
            recommendation_for_next: "n/a".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        trial_repo.create_experiment(&exp).await.unwrap();

        let orch = AutoTunerOrchestrator::new(
            Champion::default(),
            true,
            learning_state,
            trial_repo,
            provider,
            "test-model".to_string(),
        );
        // No session/strategy repos needed — should return early.
        let result = run_bootstrap_replay(&orch).await;
        assert!(result.is_ok());

        // Verify no new experiments were created (still just the 1 we inserted).
        let exps = orch.trial_repo.get_experiments(10).await.unwrap();
        assert_eq!(exps.len(), 1);
    }

    #[tokio::test]
    async fn bootstrap_replay_noop_when_no_session_repo() {
        let orch = make_orch(Champion::default(), true).await;
        // No session_repo set — should return Ok early.
        let result = run_bootstrap_replay(&orch).await;
        assert!(result.is_ok());
        // No experiments created.
        let exps = orch.trial_repo.get_experiments(10).await.unwrap();
        assert!(exps.is_empty());
    }

    #[tokio::test]
    async fn bootstrap_replay_creates_experiment_with_history() {
        // Use the full StoragePool to get sessions + strategy tables via migrations.
        let storage_pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let raw_pool = storage_pool.inner().clone();

        // Create autotuner tables on the same pool using individual statements
        // (sqlx::query with multi-statement can leave connection in bad state).
        for stmt in storage::repos::trial_repo::MIGRATION_SQL.split(';') {
            let trimmed = stmt.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&raw_pool).await.unwrap();
            }
        }
        let trial_repo = TrialRepo::new(raw_pool.clone());

        let session_repo = storage::SessionRepo::new(raw_pool.clone());
        let strategy_repo = StrategyRepo::new(raw_pool.clone());
        let learning_state = LearningStateRepo::new(raw_pool.clone());
        let provider: DynProvider = Arc::new(providers::NoopProvider);

        // Create a session with a user message.
        let session_key = "telegram:123";
        session_repo
            .upsert_session(session_key, &serde_json::json!({"title": "test"}), None)
            .await
            .unwrap();
        let msg_ts = chrono::Utc::now();
        session_repo
            .batch_add_messages(
                session_key,
                &[uuid::Uuid::new_v4()],
                &["user".to_string()],
                &["create a task to review PR".to_string()],
                &[msg_ts],
                &[None],
                &[None],
                &[None],
                &[None],
            )
            .await
            .unwrap();

        // Create a matching strategy record (same chat_id, timestamp within 30s).
        let sr = storage::rows::learning::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: msg_ts,
            request_id: "req-1".to_string(),
            predicted_strategy: "ToolAssisted".to_string(),
            actual_strategy: "reactive".to_string(),
            escalation_count: 0,
            iterations_used: 2,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 300,
            chat_id: Some("123".to_string()),
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
            retrieved_memory_count: None,
        };
        strategy_repo.create(&sr).await.unwrap();

        let orch = AutoTunerOrchestrator::new(
            Champion::default(),
            true,
            learning_state,
            trial_repo,
            provider,
            "test-model".to_string(),
        )
        .with_session_repo(session_repo)
        .with_strategy_repo(strategy_repo);

        let result = run_bootstrap_replay(&orch).await;
        assert!(result.is_ok());

        // Verify an experiment was created with trials.
        let exps = orch.trial_repo.get_experiments(10).await.unwrap();
        assert_eq!(exps.len(), 1);
        assert!(exps[0].hypothesis.contains("Bootstrap replay"));
    }
}
