//! Hook trait and concrete implementation for wiring autotuner shadow scoring
//! into the agent runtime.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use autotuner::{ShadowContext, ShadowRetriever};
use chrono::{DateTime, Utc};
use common::TrialParams;
use storage::{TrialRepo, TrialRow};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::AutoTunerOrchestrator;

/// Hook into the agent runtime for autotuner shadow scoring.
///
/// Implementations collect signals during normal message processing so the
/// autotuner can evaluate trial candidates without affecting live traffic.
#[async_trait]
pub trait AutoTunerHook: Send + Sync {
    /// Called when a new user message arrives (before classification).
    async fn on_message_received(&self, message: &str, chat_id: &str);

    /// Called after the agent finishes processing a message.
    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        tokens_used: u32,
        response_time_ms: u64,
    );

    /// Return the current champion trial params, if any.
    fn current_champion_params(&self) -> Option<TrialParams>;
}

/// How long to cache active trials before re-querying the DB (seconds).
const ACTIVE_TRIALS_CACHE_TTL_SECS: i64 = 60;

/// Concrete implementation of [`AutoTunerHook`] that runs shadow retrieval
/// for active trials and logs predictions to the shadow_log table.
pub struct AutoTunerHookImpl {
    orchestrator: Arc<AutoTunerOrchestrator>,
    trial_repo: TrialRepo,
    shadow_retriever: Option<Arc<dyn ShadowRetriever>>,
    /// Cached active trials with timestamp to avoid querying DB per message.
    active_trials_cache: RwLock<(DateTime<Utc>, Vec<TrialRow>)>,
}

impl AutoTunerHookImpl {
    pub fn new(orchestrator: Arc<AutoTunerOrchestrator>, trial_repo: TrialRepo) -> Self {
        Self {
            orchestrator,
            trial_repo,
            shadow_retriever: None,
            active_trials_cache: RwLock::new((DateTime::<Utc>::MIN_UTC, Vec::new())),
        }
    }

    pub fn with_shadow_retriever(mut self, retriever: Arc<dyn ShadowRetriever>) -> Self {
        self.shadow_retriever = Some(retriever);
        self
    }
}

#[async_trait]
impl AutoTunerHook for AutoTunerHookImpl {
    async fn on_message_received(&self, message: &str, chat_id: &str) {
        if !self.orchestrator.is_active() {
            return;
        }

        // Read cached active trials; refresh from DB if stale.
        let active_trials = {
            let (last_fetched, cached) = self.active_trials_cache.read().await.clone();
            if Utc::now() - last_fetched > chrono::Duration::seconds(ACTIVE_TRIALS_CACHE_TTL_SECS) {
                let fresh = match self.trial_repo.get_active_trials().await {
                    Ok(trials) => trials,
                    Err(e) => {
                        warn!("autotuner hook: failed to get active trials: {e}");
                        return;
                    }
                };
                *self.active_trials_cache.write().await = (Utc::now(), fresh.clone());
                fresh
            } else {
                cached
            }
        };

        if active_trials.is_empty() {
            return;
        }

        let timestamp = chrono::Utc::now().to_rfc3339();
        let context = ShadowContext {
            chat_id: chat_id.to_string(),
            session_key: format!("shadow:{chat_id}"),
        };

        // Parse trial params once — reused for retrieval.
        let parsed_trials: Vec<(&TrialRow, TrialParams)> = active_trials
            .iter()
            .filter_map(|trial| {
                serde_json::from_str(&trial.params)
                    .map(|p| (trial, p))
                    .map_err(|e| {
                        warn!(
                            "autotuner hook: failed to parse trial {} params: {e}",
                            trial.id
                        );
                        e
                    })
                    .ok()
            })
            .collect();

        // Shadow retrieval (Phase 2) — control retrieval hoisted outside loop
        if let Some(ref retriever) = self.shadow_retriever {
            if self.orchestrator.is_phase2_ready() {
                let champion_params = self
                    .orchestrator
                    .try_current_champion_params()
                    .unwrap_or_default();

                // Retrieve control result once for all trials
                let control_result = match retriever
                    .retrieve_shadow(message, &context, &champion_params)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        debug!("shadow retrieval (control) failed: {e}");
                        return;
                    }
                };
                let control_id_set: HashSet<&str> = control_result
                    .memory_ids
                    .iter()
                    .map(|s| s.as_str())
                    .collect();

                for (trial, params) in &parsed_trials {
                    if !params.has_memory_params() {
                        continue;
                    }

                    let variant_result =
                        match retriever.retrieve_shadow(message, &context, params).await {
                            Ok(r) => r,
                            Err(e) => {
                                debug!("shadow retrieval failed for trial {}: {e}", trial.id);
                                continue;
                            }
                        };

                    let overlap = variant_result
                        .memory_ids
                        .iter()
                        .filter(|id| control_id_set.contains(id.as_str()))
                        .count();
                    if let Err(e) = self
                        .trial_repo
                        .insert_shadow_retrieval_log(
                            &trial.id,
                            chat_id,
                            &timestamp,
                            variant_result.total_retrieved as i64,
                            control_result.total_retrieved as i64,
                            overlap as i64,
                            variant_result.avg_score,
                            variant_result.avg_age_days,
                        )
                        .await
                    {
                        warn!(
                            "failed to insert shadow retrieval log for trial {}: {e}",
                            trial.id
                        );
                    }
                }
            }
        }
    }

    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        tokens_used: u32,
        response_time_ms: u64,
    ) {
        if !self.orchestrator.is_active() {
            return;
        }
        if let Err(e) = self
            .trial_repo
            .update_shadow_log_ground_truth(chat_id, "", orchestrator_name, execution_mode)
            .await
        {
            tracing::warn!(error = %e, "Failed to update shadow log ground truth");
        }
        if let Err(e) = self
            .trial_repo
            .update_shadow_log_metrics(chat_id, tokens_used, response_time_ms)
            .await
        {
            tracing::debug!("Failed to update shadow log metrics: {e}");
        }
    }

    fn current_champion_params(&self) -> Option<TrialParams> {
        // Use try_read to avoid blocking — if the lock is held (e.g. during
        // nightly promotion), return None and let the caller use Config defaults.
        self.orchestrator.try_current_champion_params()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, DynProvider, LlmProvider, LlmResponse, Message};
    use serde_json::Value;

    /// A provider that panics if called — verifies shadow mode skips LLM.
    struct PanickingProvider;

    #[async_trait]
    impl LlmProvider for PanickingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            panic!("LLM should not be called in shadow mode");
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    async fn make_orchestrator(pool: &storage::StoragePool) -> Arc<AutoTunerOrchestrator> {
        let learning_state = storage::LearningStateRepo::new(pool.inner().clone());
        let trial_repo = TrialRepo::new(pool.inner().clone());
        Arc::new(AutoTunerOrchestrator::new(
            autotuner::Champion::default(),
            learning_state,
            trial_repo,
            Arc::new(PanickingProvider) as DynProvider,
            "mock".to_string(),
        ))
    }

    #[tokio::test]
    async fn on_message_completed_updates_ground_truth() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let trial_repo = TrialRepo::new(pool.inner().clone());
        trial_repo.migrate().await.unwrap();
        let orchestrator = make_orchestrator(&pool).await;

        // Create experiment + trial + shadow log entry with pending ground truth
        let exp = storage::rows::trial::ExperimentRow {
            id: "exp-gt".to_string(),
            hypothesis: "test".to_string(),
            trend_analysis: "test".to_string(),
            recommendation_for_next: "test".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        trial_repo.create_experiment(&exp).await.unwrap();

        let trial = storage::rows::trial::TrialRow {
            id: "trial-gt".to_string(),
            experiment_id: "exp-gt".to_string(),
            params: serde_json::to_string(&TrialParams::default()).unwrap(),
            generation_reasoning: "test".to_string(),
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            result: None,
        };
        trial_repo.create_trial(&trial).await.unwrap();

        // Insert a shadow log with pending ground truth
        trial_repo
            .insert_shadow_log(
                "trial-gt",
                &chrono::Utc::now().to_rfc3339(),
                "chat-gt",
                "",
                "general",
                "direct",
                0.9,
                1,
                "pending",
                "pending",
            )
            .await
            .unwrap();

        let hook = AutoTunerHookImpl::new(orchestrator, trial_repo.clone());

        // Call on_message_completed — should update ground truth from "pending"
        hook.on_message_completed("chat-gt", "general", "reactive", 100, 500)
            .await;

        // Verify ground truth was filled: predicted_mode=direct vs control_mode=reactive
        // means 0% agreement for this trial
        let rate = trial_repo
            .shadow_log_agreement_rate(
                Some("trial-gt"),
                chrono::Utc::now() - chrono::Duration::hours(1),
            )
            .await
            .unwrap();
        assert!(
            (rate - 0.0).abs() < f64::EPSILON,
            "Expected 0.0 agreement (predicted=direct, actual=reactive), got {rate}"
        );
    }
}
