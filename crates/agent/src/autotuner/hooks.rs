//! Hook trait and concrete implementation for wiring autotuner shadow scoring
//! into the agent runtime.

use std::sync::Arc;

use async_trait::async_trait;
use autotuner::ShadowContext;
use chrono::{DateTime, Utc};
use common::TrialParams;
use config::OrchestratorConfig;
use providers::DynProvider;
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

/// Concrete implementation of [`AutoTunerHook`] that runs shadow classification
/// for active trials and logs predictions to the shadow_log table.
pub struct AutoTunerHookImpl {
    orchestrator: Arc<AutoTunerOrchestrator>,
    trial_repo: TrialRepo,
    shadow_classifier: super::shadow_classifier::AgentShadowClassifier,
    /// Cached active trials with timestamp to avoid querying DB per message.
    active_trials_cache: RwLock<(DateTime<Utc>, Vec<TrialRow>)>,
}

impl AutoTunerHookImpl {
    pub fn new(
        orchestrator: Arc<AutoTunerOrchestrator>,
        trial_repo: TrialRepo,
        provider: DynProvider,
        model: &str,
        config: &OrchestratorConfig,
    ) -> Self {
        Self {
            orchestrator,
            trial_repo,
            shadow_classifier: super::shadow_classifier::AgentShadowClassifier::new(
                provider, model, config,
            ),
            active_trials_cache: RwLock::new((DateTime::<Utc>::MIN_UTC, Vec::new())),
        }
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

        // Collect shadow predictions, then batch-insert via concurrent futures.
        use autotuner::ShadowClassifier;
        let mut insert_futs = Vec::new();

        for trial in &active_trials {
            let params: TrialParams = match serde_json::from_str(&trial.params) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "autotuner hook: failed to parse trial {} params: {e}",
                        trial.id
                    );
                    continue;
                }
            };

            let prediction = match self
                .shadow_classifier
                .classify_shadow(message, &context, &params)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    debug!(
                        "autotuner hook: shadow classification failed for trial {}: {e}",
                        trial.id
                    );
                    continue;
                }
            };

            let trial_id = trial.id.clone();
            let ts = timestamp.clone();
            let cid = chat_id.to_string();
            let repo = self.trial_repo.clone();

            insert_futs.push(async move {
                if let Err(e) = repo
                    .insert_shadow_log(
                        &trial_id,
                        &ts,
                        &cid,
                        &prediction.predicted_orchestrator,
                        &prediction.predicted_mode,
                        prediction.confidence as f64,
                        prediction.predicted_iteration_budget as i64,
                        "pending",
                        "pending",
                    )
                    .await
                {
                    warn!(
                        "autotuner hook: failed to insert shadow log for trial {trial_id}: {e}"
                    );
                } else {
                    debug!(
                        "autotuner hook: shadow logged trial {trial_id} → mode={}, confidence={:.2}",
                        prediction.predicted_mode, prediction.confidence
                    );
                }
            });
        }

        // Execute all inserts concurrently instead of sequentially.
        futures_util::future::join_all(insert_futs).await;
    }

    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        _tokens_used: u32,
        _response_time_ms: u64,
    ) {
        if !self.orchestrator.is_active() {
            return;
        }
        if let Err(e) = self
            .trial_repo
            .update_shadow_log_ground_truth(chat_id, orchestrator_name, execution_mode)
            .await
        {
            tracing::warn!(error = %e, "Failed to update shadow log ground truth");
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
    use providers::{ChatParams, LlmProvider, LlmResponse, Message};
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

    async fn make_orchestrator(
        pool: &storage::StoragePool,
        enabled: bool,
    ) -> Arc<AutoTunerOrchestrator> {
        let learning_state = storage::LearningStateRepo::new(pool.inner().clone());
        let trial_repo = TrialRepo::new(pool.inner().clone());
        Arc::new(AutoTunerOrchestrator::new(
            autotuner::Champion::default(),
            enabled,
            learning_state,
            trial_repo,
            Arc::new(PanickingProvider) as DynProvider,
            "mock".to_string(),
        ))
    }

    #[tokio::test]
    async fn hook_skips_when_inactive() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let trial_repo = TrialRepo::new(pool.inner().clone());
        trial_repo.migrate().await.unwrap();
        let orchestrator = make_orchestrator(&pool, false).await;

        let hook = AutoTunerHookImpl::new(
            orchestrator,
            trial_repo,
            Arc::new(PanickingProvider),
            "mock",
            &OrchestratorConfig::default(),
        );

        // Should return immediately without panicking (inactive orchestrator).
        hook.on_message_received("hello", "test-chat").await;
    }

    #[tokio::test]
    async fn hook_runs_shadow_classification_for_active_trials() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let trial_repo = TrialRepo::new(pool.inner().clone());
        trial_repo.migrate().await.unwrap();
        let orchestrator = make_orchestrator(&pool, true).await;

        // Create an experiment and an active trial
        let exp = storage::rows::trial::ExperimentRow {
            id: "exp-1".to_string(),
            hypothesis: "test".to_string(),
            trend_analysis: "test".to_string(),
            recommendation_for_next: "test".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        trial_repo.create_experiment(&exp).await.unwrap();

        let trial = storage::rows::trial::TrialRow {
            id: "trial-1".to_string(),
            experiment_id: "exp-1".to_string(),
            params: serde_json::to_string(&TrialParams {
                heuristic_confidence_threshold: Some(0.80),
                ..Default::default()
            })
            .unwrap(),
            generation_reasoning: "test".to_string(),
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            result: None,
        };
        trial_repo.create_trial(&trial).await.unwrap();

        let hook = AutoTunerHookImpl::new(
            orchestrator,
            trial_repo.clone(),
            Arc::new(PanickingProvider) as DynProvider,
            "mock",
            &OrchestratorConfig::default(),
        );

        // "hello" is a greeting — Layer 1 handles it without LLM.
        hook.on_message_received("hello", "test-chat").await;

        // Verify a shadow log entry was created (check via raw SQL since we
        // don't have a list method yet).
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM autotuner_shadow_log")
            .fetch_one(pool.inner())
            .await
            .unwrap();
        assert_eq!(count.0, 1, "Expected 1 shadow log entry");
    }
}
