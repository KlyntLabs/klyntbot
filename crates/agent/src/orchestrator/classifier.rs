//! LLM-based intent classifier for messages that heuristics can't handle.
//!
//! Makes a single cheap LLM call (~300 token input) to classify user intent.
//! Falls back to `ToolAssisted` on timeout or parse errors.

use std::time::Duration;

use common::Result;
use context_engine::ExecutionStrategy;
use providers::{ChatParams, DynProvider, Message};

/// Result of classification with metadata.
#[derive(Debug)]
pub struct ClassificationResult {
    pub strategy: ExecutionStrategy,
    pub reasoning: String,
    pub confidence: f32,
    pub source: ClassificationSource,
}

/// Where the classification came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassificationSource {
    Heuristic,
    LlmClassifier,
    Fallback,
}

/// Classifies user intent via a lightweight LLM call.
pub struct LlmClassifier {
    provider: DynProvider,
    timeout: Duration,
}

const CLASSIFICATION_PROMPT: &str = r#"Classify this user message into ONE of these strategies:
- "direct_response": Simple question, greeting, factual query (no tools needed)
- "tool_assisted": Task that needs tools (todos, files, web search, etc.)
- "autonomous_task": Complex multi-step task needing planning
- "clarification": Ambiguous — need to ask user

Respond ONLY with valid JSON: {"strategy":"<strategy>","reasoning":"<why>","confidence":<0.0-1.0>}

User message: "{message}"
Available tools: {tools}"#;

impl LlmClassifier {
    pub fn new(provider: DynProvider) -> Self {
        Self {
            provider,
            timeout: Duration::from_secs(2),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Safe default when classification fails or times out.
    pub fn fallback_strategy() -> ExecutionStrategy {
        ExecutionStrategy::ToolAssisted { max_iterations: 10 }
    }

    /// Classify a user message using an LLM call.
    pub async fn classify(
        &self,
        message: &str,
        tool_names: &[&str],
        params: &ChatParams,
    ) -> Result<ClassificationResult> {
        let prompt = CLASSIFICATION_PROMPT
            .replace("{message}", message)
            .replace("{tools}", &tool_names.join(", "));

        let messages = vec![Message::user(prompt)];

        let result =
            tokio::time::timeout(self.timeout, self.provider.chat(&messages, None, params)).await;

        let response = match result {
            Ok(Ok(r)) => r,
            _ => {
                return Ok(ClassificationResult {
                    strategy: Self::fallback_strategy(),
                    reasoning: "timeout or error".to_string(),
                    confidence: 0.5,
                    source: ClassificationSource::Fallback,
                });
            }
        };

        let content = response.content.as_deref().unwrap_or("");
        Ok(Self::parse_strategy_json(content))
    }

    fn parse_strategy_json(content: &str) -> ClassificationResult {
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content[start..=end]) {
                    let strategy_str = v["strategy"].as_str().unwrap_or("tool_assisted");
                    let reasoning = v["reasoning"].as_str().unwrap_or("").to_string();
                    let confidence = v["confidence"].as_f64().unwrap_or(0.7) as f32;

                    let strategy = match strategy_str {
                        "direct_response" => ExecutionStrategy::DirectResponse,
                        "autonomous_task" => {
                            ExecutionStrategy::AutonomousTask { max_iterations: 50 }
                        }
                        "clarification" => ExecutionStrategy::Clarification {
                            reason: reasoning.clone(),
                        },
                        _ => ExecutionStrategy::ToolAssisted { max_iterations: 10 },
                    };

                    return ClassificationResult {
                        strategy,
                        reasoning,
                        confidence,
                        source: ClassificationSource::LlmClassifier,
                    };
                }
            }
        }

        ClassificationResult {
            strategy: Self::fallback_strategy(),
            reasoning: "failed to parse".to_string(),
            confidence: 0.5,
            source: ClassificationSource::Fallback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    /// Minimal mock provider for classifier tests.
    struct MockClassifierProvider {
        response_text: String,
    }

    impl MockClassifierProvider {
        fn new(text: &str) -> Self {
            Self {
                response_text: text.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockClassifierProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
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

    #[tokio::test]
    async fn test_classifier_returns_tool_assisted_strategy() {
        let mock_json =
            r#"{"strategy":"tool_assisted","reasoning":"User wants tasks","confidence":0.9}"#;
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(mock_json)));
        let result = classifier
            .classify("show my tasks", &[], &ChatParams::new("haiku"))
            .await
            .unwrap();
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert_eq!(result.source, ClassificationSource::LlmClassifier);
        assert!((result.confidence - 0.9).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_classifier_returns_direct_response() {
        let mock_json =
            r#"{"strategy":"direct_response","reasoning":"Greeting","confidence":0.95}"#;
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(mock_json)));
        let result = classifier
            .classify("hello", &[], &ChatParams::new("haiku"))
            .await
            .unwrap();
        assert!(matches!(result.strategy, ExecutionStrategy::DirectResponse));
    }

    #[tokio::test]
    async fn test_classifier_returns_autonomous() {
        let mock_json = r#"{"strategy":"autonomous_task","reasoning":"Complex planning needed","confidence":0.85}"#;
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(mock_json)));
        let result = classifier
            .classify(
                "design the new microservices architecture",
                &[],
                &ChatParams::new("haiku"),
            )
            .await
            .unwrap();
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::AutonomousTask { .. }
        ));
    }

    #[tokio::test]
    async fn test_classifier_returns_clarification() {
        let mock_json =
            r#"{"strategy":"clarification","reasoning":"Need more context","confidence":0.6}"#;
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(mock_json)));
        let result = classifier
            .classify("do that thing", &[], &ChatParams::new("haiku"))
            .await
            .unwrap();
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::Clarification { .. }
        ));
    }

    #[tokio::test]
    async fn test_classifier_invalid_json_returns_fallback() {
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(
            "I can't classify this",
        )));
        let result = classifier
            .classify("hello", &[], &ChatParams::new("haiku"))
            .await
            .unwrap();
        assert!(matches!(
            result.strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert_eq!(result.source, ClassificationSource::Fallback);
    }

    #[tokio::test]
    async fn test_classifier_json_embedded_in_text() {
        let response = r#"Sure, here's my classification: {"strategy":"direct_response","reasoning":"Simple greeting","confidence":0.9} Hope that helps!"#;
        let classifier = LlmClassifier::new(Arc::new(MockClassifierProvider::new(response)));
        let result = classifier
            .classify("hi there", &[], &ChatParams::new("haiku"))
            .await
            .unwrap();
        assert!(matches!(result.strategy, ExecutionStrategy::DirectResponse));
        assert_eq!(result.source, ClassificationSource::LlmClassifier);
    }

    #[test]
    fn test_fallback_strategy() {
        let fallback = LlmClassifier::fallback_strategy();
        assert!(matches!(
            fallback,
            ExecutionStrategy::ToolAssisted { max_iterations: 10 }
        ));
    }
}
