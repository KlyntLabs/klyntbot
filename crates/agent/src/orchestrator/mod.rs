//! Adaptive Orchestrator — classifies user intent and routes to execution engines.
//!
//! Two-stage classification:
//! 1. Heuristic pre-filter (zero LLM cost) for obvious patterns
//! 2. LLM classifier fallback for ambiguous messages
//!
//! Strategy feedback: when a `StrategyRepo` is configured, historical strategy
//! performance (accuracy, escalation rate) is included in the LLM prompt to
//! help the classifier learn from past outcomes.

pub mod classifier;
pub mod heuristics;

use context_engine::ExecutionStrategy;
use providers::{ChatParams, DynProvider};
use tracing::debug;

pub use classifier::{ClassificationResult, ClassificationSource, LlmClassifier};
pub use heuristics::classify_heuristic;

/// The main orchestrator that combines heuristic + LLM classification.
pub struct Orchestrator {
    classifier: LlmClassifier,
    classifier_params: ChatParams,
    /// Optional SQL repo for querying historical strategy performance.
    strategy_repo: Option<storage::StrategyRepo>,
}

impl Orchestrator {
    pub fn new(classifier_provider: DynProvider, classifier_model: &str) -> Self {
        Self {
            classifier: LlmClassifier::new(classifier_provider),
            classifier_params: ChatParams::new(classifier_model),
            strategy_repo: None,
        }
    }

    /// Attach a strategy repo for feeding historical performance into classification.
    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    /// Classify a user message into an execution strategy.
    ///
    /// Tries heuristics first (free), then falls back to LLM classification.
    /// When a strategy repo is configured, historical accuracy stats are included
    /// in the LLM prompt to bias toward well-performing strategies.
    /// Low-confidence LLM results get overridden with a safe default.
    pub async fn classify(&self, message: &str, tool_names: &[&str]) -> ClassificationResult {
        // Step 1: Heuristic pre-filter (zero cost)
        if let Some(strategy) = classify_heuristic(message) {
            return ClassificationResult {
                strategy,
                reasoning: "heuristic match".to_string(),
                confidence: 1.0,
                source: ClassificationSource::Heuristic,
            };
        }

        // Step 1.5: Build strategy context from historical performance
        let strategy_context = self.build_strategy_context().await;

        // Step 2: LLM classifier with timeout
        let result = self
            .classifier
            .classify(
                message,
                tool_names,
                &self.classifier_params,
                strategy_context.as_deref(),
            )
            .await;

        match result {
            Ok(r) => {
                // Step 3: Confidence gate
                if r.confidence < 0.5 {
                    ClassificationResult {
                        strategy: ExecutionStrategy::ToolAssisted { max_iterations: 10 },
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

    /// Query strategy performance from the last 30 days and format as LLM context.
    async fn build_strategy_context(&self) -> Option<String> {
        let repo = self.strategy_repo.as_ref()?;
        let since = chrono::Utc::now() - chrono::Duration::days(30);

        match repo.get_strategy_summaries(since).await {
            Ok(summaries) if !summaries.is_empty() => {
                let ctx = format_strategy_context(&summaries);
                debug!("Strategy feedback context: {} strategies", summaries.len());
                Some(ctx)
            }
            Ok(_) => None,
            Err(e) => {
                debug!("Failed to fetch strategy summaries: {}", e);
                None
            }
        }
    }
}

/// Format strategy summaries into a human-readable context for the LLM classifier.
pub(crate) fn format_strategy_context(summaries: &[storage::StrategySummaryRow]) -> String {
    let mut ctx = String::from("Historical strategy performance (last 30 days):\n");
    for s in summaries {
        let accuracy = if s.sample_count > 0 {
            s.correct_count as f32 / s.sample_count as f32 * 100.0
        } else {
            0.0
        };
        use std::fmt::Write;
        let _ = writeln!(
            ctx,
            "- {}: {:.0}% accuracy ({} samples), avg {:.1} escalations",
            s.predicted_strategy, accuracy, s.sample_count, s.avg_escalations
        );
    }
    ctx.push_str("Prefer strategies with higher historical accuracy when confidence is similar.");
    ctx
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
        let mock_json = r#"{"strategy":"autonomous_task","reasoning":"Maybe","confidence":0.3}"#;
        let orch = mock_orchestrator(mock_json);
        let result = orch.classify("something ambiguous and unclear", &[]).await;
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

    #[test]
    fn test_format_strategy_context() {
        let summaries = vec![
            storage::StrategySummaryRow {
                predicted_strategy: "ToolAssisted".to_string(),
                sample_count: 100,
                correct_count: 80,
                avg_escalations: 0.5,
            },
            storage::StrategySummaryRow {
                predicted_strategy: "DirectResponse".to_string(),
                sample_count: 50,
                correct_count: 48,
                avg_escalations: 0.1,
            },
        ];
        let ctx = format_strategy_context(&summaries);
        assert!(ctx.contains("ToolAssisted: 80% accuracy (100 samples)"));
        assert!(ctx.contains("DirectResponse: 96% accuracy (50 samples)"));
        assert!(ctx.contains("Prefer strategies"));
    }

    #[test]
    fn test_format_strategy_context_zero_samples() {
        let summaries = vec![storage::StrategySummaryRow {
            predicted_strategy: "Empty".to_string(),
            sample_count: 0,
            correct_count: 0,
            avg_escalations: 0.0,
        }];
        let ctx = format_strategy_context(&summaries);
        assert!(ctx.contains("Empty: 0% accuracy (0 samples)"));
    }
}
