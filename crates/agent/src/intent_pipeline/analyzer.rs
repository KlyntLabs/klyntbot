//! IntentAnalyzer — Two-stage classification: heuristics → LLM classifier.
//!
//! Stage 1: Fast heuristic check (0ms). If confidence exceeds threshold, return immediately.
//! Stage 2: LLM classifier for ambiguous messages. Falls back to Reactive on error.

use std::time::Duration;

use config::OrchestratorConfig;
use providers::{ChatParams, DynProvider};
use tracing::{debug, warn};

use super::classifier::IntentClassifier;
use super::heuristics::analyze_heuristic;
use super::types::{AnalysisSource, ExecutionMode, IntentAnalysis};
use crate::orchestrator::format_strategy_context;

/// Two-stage intent analyzer: heuristics → LLM classifier.
pub struct IntentAnalyzer {
    classifier: IntentClassifier,
    classifier_params: ChatParams,
    strategy_repo: Option<storage::StrategyRepo>,
    config: OrchestratorConfig,
}

impl IntentAnalyzer {
    pub fn new(provider: DynProvider, model: &str, config: &OrchestratorConfig) -> Self {
        let timeout = Duration::from_millis(config.llm_classifier_timeout);
        Self {
            classifier: IntentClassifier::new(provider, timeout),
            classifier_params: ChatParams::new(model),
            strategy_repo: None,
            config: config.clone(),
        }
    }

    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    /// Analyze a user message and return the recommended execution mode.
    pub async fn analyze(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
        // Stage 1: Heuristics (0ms)
        if let Some(analysis) = analyze_heuristic(message) {
            if analysis.confidence >= self.config.heuristic_confidence_threshold {
                debug!(
                    mode = ?analysis.mode,
                    confidence = analysis.confidence,
                    "Heuristic classification accepted"
                );
                return analysis;
            }
            debug!(
                confidence = analysis.confidence,
                threshold = self.config.heuristic_confidence_threshold,
                "Heuristic confidence below threshold, falling through to LLM"
            );
        }

        // Stage 2: LLM classifier
        let strategy_context = self.build_strategy_context().await;
        match self
            .classifier
            .classify(
                message,
                tool_names,
                &self.classifier_params,
                strategy_context.as_deref(),
            )
            .await
        {
            Ok(result) => {
                if result.confidence < 0.5 {
                    debug!(
                        confidence = result.confidence,
                        "LLM classifier low confidence, defaulting to Reactive"
                    );
                    return IntentAnalysis {
                        mode: ExecutionMode::Reactive { max_iterations: 10 },
                        source: AnalysisSource::LlmClassifier,
                        ..result
                    };
                }
                result
            }
            Err(e) => {
                warn!("LLM classifier error: {}, using fallback", e);
                IntentAnalysis::fallback()
            }
        }
    }

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
                warn!("Failed to load strategy summaries: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::Result;
    use providers::{LlmProvider, LlmResponse, Message, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    /// Mock that panics if called — verifies heuristic path short-circuits.
    struct PanickingProvider;

    #[async_trait]
    impl LlmProvider for PanickingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            panic!("LLM should not have been called — heuristic should have handled this");
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Mock that returns a fixed response.
    struct FixedProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for FixedProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.response.clone()),
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

    #[tokio::test]
    async fn greeting_bypasses_llm() {
        let analyzer = IntentAnalyzer::new(
            Arc::new(PanickingProvider),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer.analyze("hello", &[]).await;
        assert!(matches!(result.mode, ExecutionMode::Direct));
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }

    #[tokio::test]
    async fn ambiguous_uses_llm() {
        let response = r#"{"mode":"reactive","estimated_tool_calls":2,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Needs search"}"#;
        let analyzer = IntentAnalyzer::new(
            Arc::new(FixedProvider {
                response: response.to_string(),
            }),
            "model",
            &OrchestratorConfig::default(),
        );
        // This message has no clear heuristic signals → falls through to LLM
        let result = analyzer
            .analyze(
                "I need help with understanding the codebase",
                &["web_search"],
            )
            .await;
        assert!(matches!(result.mode, ExecutionMode::Reactive { .. }));
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[tokio::test]
    async fn low_confidence_llm_falls_back_to_reactive() {
        let response = r#"{"mode":"planned","estimated_tool_calls":1,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.3,"reasoning":"Unsure"}"#;
        let analyzer = IntentAnalyzer::new(
            Arc::new(FixedProvider {
                response: response.to_string(),
            }),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer.analyze("I need help with something", &[]).await;
        // Low confidence should force Reactive regardless of LLM's "planned" suggestion
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 10 }
        ));
    }

    #[tokio::test]
    async fn task_crud_bypasses_llm() {
        let analyzer = IntentAnalyzer::new(
            Arc::new(PanickingProvider),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer
            .analyze("create a task to buy groceries", &[])
            .await;
        assert!(matches!(result.mode, ExecutionMode::Reactive { .. }));
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }
}
