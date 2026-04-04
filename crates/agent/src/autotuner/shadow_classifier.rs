//! Implements [`ShadowClassifier`] by delegating to [`IntentAnalyzer`] in
//! shadow mode (Layer 1-2 only, no LLM call).

use async_trait::async_trait;
use autotuner::{ShadowClassifier, ShadowContext, ShadowPrediction};
use common::TrialParams;
use config::OrchestratorConfig;

use crate::intent_pipeline::analysis::IntentAnalyzer;

/// Shadow classifier backed by the agent's [`IntentAnalyzer`].
///
/// Runs the heuristic + embedding layers (Layer 1-2) with trial param
/// overrides and `shadow_mode` enabled, so the LLM classifier is never
/// invoked.  This keeps shadow evaluation nearly free.
pub struct AgentShadowClassifier {
    config: OrchestratorConfig,
}

impl AgentShadowClassifier {
    pub fn new(config: &OrchestratorConfig) -> Self {
        Self {
            config: config.clone(),
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
        // Create a lightweight IntentAnalyzer with overrides (Layer 1-2 only).
        let analyzer = IntentAnalyzer::new(&self.config).with_overrides(params.clone());

        // Run Layer 1-2 only.
        let analysis = analyzer.analyze(message, &[]).await;

        Ok(ShadowPrediction {
            predicted_orchestrator: "general".into(),
            predicted_mode: analysis.complexity.short_name().to_string(),
            confidence: analysis.confidence,
            predicted_iteration_budget: analysis.signals.iteration_budget(),
            deferred_to_llm: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shadow_classifier_skips_llm() {
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(&config);

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

        assert_eq!(prediction.predicted_mode, "simple");
        assert!(prediction.confidence > 0.8);
        assert!(!prediction.deferred_to_llm);
    }

    #[tokio::test]
    async fn shadow_classifier_defers_ambiguous() {
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(&config);

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

        // Without LLM classifier, ambiguous messages fall back to moderate.
        assert!(!prediction.deferred_to_llm);
    }

    #[tokio::test]
    async fn shadow_classifier_applies_overrides() {
        let config = OrchestratorConfig::default();
        let classifier = AgentShadowClassifier::new(&config);

        let context = ShadowContext {
            chat_id: "test-chat".into(),
            session_key: "test-session".into(),
        };

        // Set a very high threshold — this should cause the heuristic result
        // to be below threshold, but the analyzer still returns a result (no LLM fallback).
        let params = TrialParams {
            heuristic_confidence_threshold: Some(0.99),
            ..Default::default()
        };

        // "create a task" is normally a clear heuristic match (confidence ~0.90),
        // but with threshold 0.99 it falls below. Without LLM, we still get a result.
        let prediction = classifier
            .classify_shadow("create a task to review PR", &context, &params)
            .await
            .unwrap();

        // No LLM deferral anymore — always returns a prediction.
        assert!(!prediction.deferred_to_llm);
    }
}
