//! Implements [`ShadowClassifier`] by delegating to [`IntentAnalyzer`] in
//! shadow mode (Layer 1-2 only, no LLM call).

use async_trait::async_trait;
use autotuner::{ShadowClassifier, ShadowContext, ShadowPrediction};
use common::TrialParams;
use config::OrchestratorConfig;
use providers::DynProvider;

use crate::intent_pipeline::analysis::IntentAnalyzer;
use crate::intent_pipeline::types::AnalysisSource;

/// Shadow classifier backed by the agent's [`IntentAnalyzer`].
///
/// Runs the heuristic + embedding layers (Layer 1-2) with trial param
/// overrides and `shadow_mode` enabled, so the LLM classifier is never
/// invoked.  This keeps shadow evaluation nearly free.
pub struct AgentShadowClassifier {
    config: OrchestratorConfig,
    provider: DynProvider,
    model: String,
}

impl AgentShadowClassifier {
    pub fn new(provider: DynProvider, model: &str, config: &OrchestratorConfig) -> Self {
        Self {
            config: config.clone(),
            provider,
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl ShadowClassifier for AgentShadowClassifier {
    async fn classify_shadow(
        &self,
        message: &str,
        _context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowPrediction> {
        // Create a lightweight IntentAnalyzer with overrides AND shadow mode.
        // shadow_mode prevents the Layer 3 LLM call, keeping this nearly free.
        let analyzer = IntentAnalyzer::new(self.provider.clone(), &self.model, &self.config)
            .with_overrides(params.clone())
            .with_shadow_mode();

        // Run Layer 1-2 only (shadow_mode prevents Layer 3 LLM call).
        let analysis = analyzer.analyze(message, &[]).await;

        Ok(ShadowPrediction {
            predicted_orchestrator: "general".into(),
            predicted_mode: analysis.mode.short_name().to_string(),
            confidence: analysis.confidence,
            predicted_iteration_budget: analysis.mode.max_iterations(),
            deferred_to_llm: matches!(analysis.source, AnalysisSource::ShadowDeferred),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::Result;
    use providers::{ChatParams, LlmProvider, LlmResponse, Message};
    use serde_json::Value;
    use std::sync::Arc;

    /// A provider that panics if called — verifies shadow mode skips LLM.
    struct PanickingProvider;

    #[async_trait]
    impl LlmProvider for PanickingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            panic!("LLM should not be called in shadow mode");
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn shadow_classifier_skips_llm() {
        let provider: DynProvider = Arc::new(PanickingProvider);
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(provider, "test-model", &config);

        let context = ShadowContext {
            chat_id: "test-chat".into(),
            session_key: "test-session".into(),
        };
        let params = TrialParams::default();

        // "hello" is a greeting — heuristic Layer 1 handles it without LLM.
        let prediction = classifier
            .classify_shadow("hello", &context, &params)
            .await
            .unwrap();

        assert_eq!(prediction.predicted_mode, "direct");
        assert!(prediction.confidence > 0.8);
        assert!(!prediction.deferred_to_llm);
    }

    #[tokio::test]
    async fn shadow_classifier_defers_ambiguous() {
        let provider: DynProvider = Arc::new(PanickingProvider);
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(provider, "test-model", &config);

        let context = ShadowContext {
            chat_id: "test-chat".into(),
            session_key: "test-session".into(),
        };
        let params = TrialParams::default();

        // Ambiguous message that heuristic can't classify.
        let prediction = classifier
            .classify_shadow(
                "I need help understanding the architecture of this system",
                &context,
                &params,
            )
            .await
            .unwrap();

        // Shadow mode returns ShadowDeferred when L1-2 fail.
        assert!(prediction.deferred_to_llm);
    }

    #[tokio::test]
    async fn shadow_classifier_applies_overrides() {
        let provider: DynProvider = Arc::new(PanickingProvider);
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(provider, "test-model", &config);

        let context = ShadowContext {
            chat_id: "test-chat".into(),
            session_key: "test-session".into(),
        };

        // Set a very high threshold — this should cause more deferrals.
        let params = TrialParams {
            heuristic_confidence_threshold: Some(0.99),
            ..Default::default()
        };

        // "create a task" is normally a clear heuristic match (confidence ~0.90),
        // but with threshold 0.99 it falls below and gets deferred.
        let prediction = classifier
            .classify_shadow("create a task to review PR", &context, &params)
            .await
            .unwrap();

        assert!(prediction.deferred_to_llm);
    }
}
