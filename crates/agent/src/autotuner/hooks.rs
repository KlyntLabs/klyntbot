//! Hook trait and concrete implementation for wiring autotuner shadow scoring
//! into the agent runtime.

use std::sync::Arc;

use async_trait::async_trait;
use autotuner::ShadowContext;
use common::TrialParams;
use config::OrchestratorConfig;
use providers::DynProvider;
use storage::TrialRepo;
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
        user_corrected: bool,
        tokens_used: u32,
        response_time_ms: u64,
    );

    /// Return the current champion trial params, if any.
    fn current_champion_params(&self) -> Option<TrialParams>;
}

/// Concrete implementation of [`AutoTunerHook`] that runs shadow classification
/// for active trials and logs predictions to the shadow_log table.
pub struct AutoTunerHookImpl {
    orchestrator: Arc<AutoTunerOrchestrator>,
    trial_repo: TrialRepo,
    shadow_classifier: super::shadow_classifier::AgentShadowClassifier,
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
        }
    }
}

#[async_trait]
impl AutoTunerHook for AutoTunerHookImpl {
    async fn on_message_received(&self, message: &str, chat_id: &str) {
        if !self.orchestrator.is_active() {
            return;
        }

        // Get active trials from the database
        let active_trials = match self.trial_repo.get_active_trials().await {
            Ok(trials) => trials,
            Err(e) => {
                warn!("autotuner hook: failed to get active trials: {e}");
                return;
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

        // Run shadow classification for each active trial
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

            use autotuner::ShadowClassifier;
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

            // Log prediction to shadow_log (control values are "unknown" — filled
            // in on_message_completed or by the nightly evaluation cycle).
            if let Err(e) = self
                .trial_repo
                .insert_shadow_log(
                    &trial.id,
                    &timestamp,
                    chat_id,
                    &prediction.predicted_orchestrator,
                    &prediction.predicted_mode,
                    prediction.confidence as f64,
                    prediction.predicted_iteration_budget as i64,
                    "pending", // control orchestrator — updated after live classification
                    "pending", // control mode — updated after live classification
                )
                .await
            {
                warn!(
                    "autotuner hook: failed to insert shadow log for trial {}: {e}",
                    trial.id
                );
            } else {
                debug!(
                    "autotuner hook: shadow logged trial {} → mode={}, confidence={:.2}",
                    trial.id, prediction.predicted_mode, prediction.confidence
                );
            }
        }
    }

    async fn on_message_completed(
        &self,
        _chat_id: &str,
        _user_corrected: bool,
        _tokens_used: u32,
        _response_time_ms: u64,
    ) {
        // TODO: Record ground truth — update the most recent shadow_log entries
        // for this chat_id with the actual control_orchestrator, control_mode,
        // and user_corrected values. This requires a TrialRepo method to update
        // the most recent shadow_log row by chat_id.
        //
        // For now, the nightly evaluation cycle handles metric aggregation from
        // the shadow_log + strategy_records tables.
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
