//! Adaptive Orchestrator — classifies user intent and routes to execution engines.
//!
//! Two-stage classification:
//! 1. Heuristic pre-filter (zero LLM cost) for obvious patterns
//! 2. LLM classifier fallback for ambiguous messages

pub mod classifier;
pub mod heuristics;

use context_engine::ExecutionStrategy;
use providers::{ChatParams, DynProvider};

pub use classifier::{ClassificationResult, ClassificationSource, LlmClassifier};
pub use heuristics::classify_heuristic;

/// The main orchestrator that combines heuristic + LLM classification.
pub struct Orchestrator {
    classifier: LlmClassifier,
    classifier_params: ChatParams,
}

impl Orchestrator {
    pub fn new(classifier_provider: DynProvider, classifier_model: &str) -> Self {
        Self {
            classifier: LlmClassifier::new(classifier_provider),
            classifier_params: ChatParams::new(classifier_model),
        }
    }

    /// Classify a user message into an execution strategy.
    ///
    /// Tries heuristics first (free), then falls back to LLM classification.
    /// Low-confidence LLM results get overridden with a safe default.
    pub async fn classify(
        &self,
        message: &str,
        tool_names: &[&str],
    ) -> ClassificationResult {
        // Step 1: Heuristic pre-filter (zero cost)
        if let Some(strategy) = classify_heuristic(message) {
            return ClassificationResult {
                strategy,
                reasoning: "heuristic match".to_string(),
                confidence: 1.0,
                source: ClassificationSource::Heuristic,
            };
        }

        // Step 2: LLM classifier with timeout
        let result = self
            .classifier
            .classify(message, tool_names, &self.classifier_params)
            .await;

        match result {
            Ok(r) => {
                // Step 3: Confidence gate
                if r.confidence < 0.5 {
                    ClassificationResult {
                        strategy: ExecutionStrategy::ToolAssisted {
                            max_iterations: 10,
                        },
                        reasoning: format!(
                            "low confidence ({:.2}), using safe default",
                            r.confidence
                        ),
                        confidence: r.confidence,
                        source: ClassificationSource::Fallback,
                    }
                } else {
                    r
                }
            }
            Err(_) => ClassificationResult {
                strategy: LlmClassifier::fallback_strategy(),
                reasoning: "error in classification".to_string(),
                confidence: 0.5,
                source: ClassificationSource::Fallback,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{LlmProvider, LlmResponse, Message, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    struct MockOrchestratorProvider {
        response_text: String,
    }

    impl MockOrchestratorProvider {
        fn new(text: &str) -> Self {
            Self {
                response_text: text.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockOrchestratorProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.response_text.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    fn mock_orchestrator(response: &str) -> Orchestrator {
        Orchestrator::new(
            Arc::new(MockOrchestratorProvider::new(response)),
            "mock-model",
        )
    }

    #[tokio::test]
    async fn test_orchestrator_greeting_uses_heuristic() {
        let orch = mock_orchestrator("should not be called");
        let result = orch.classify("hi", &[]).await;
        assert!(matches!(result.strategy, ExecutionStrategy::DirectResponse));
        assert_eq!(result.source, ClassificationSource::Heuristic);
        assert_eq!(result.confidence, 1.0);
    }

    #[tokio::test]
    async fn test_orchestrator_code_task_uses_heuristic() {
        let orch = mock_orchestrator("should not be called");
        let result = orch.classify("fix the bug in auth.rs", &[]).await;
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert_eq!(result.source, ClassificationSource::Heuristic);
    }

    #[tokio::test]
    async fn test_orchestrator_ambiguous_uses_llm() {
        let mock_json =
            r#"{"strategy":"tool_assisted","reasoning":"Needs tools","confidence":0.85}"#;
        let orch = mock_orchestrator(mock_json);
        let result = orch
            .classify("what do you think about the architecture?", &[])
            .await;
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert_eq!(result.source, ClassificationSource::LlmClassifier);
    }

    #[tokio::test]
    async fn test_orchestrator_low_confidence_falls_back() {
        let mock_json =
            r#"{"strategy":"autonomous_task","reasoning":"Maybe","confidence":0.3}"#;
        let orch = mock_orchestrator(mock_json);
        let result = orch
            .classify("something ambiguous and unclear", &[])
            .await;
        // Low confidence should override to safe default
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert_eq!(result.source, ClassificationSource::Fallback);
    }

    #[tokio::test]
    async fn test_orchestrator_plan_uses_heuristic() {
        let orch = mock_orchestrator("should not be called");
        let result = orch
            .classify("create a plan for the database migration", &[])
            .await;
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::AutonomousTask { .. }
        ));
        assert_eq!(result.source, ClassificationSource::Heuristic);
    }
}
