//! LLM-based intent classifier that returns structured `IntentAnalysis`.
//!
//! Makes a single cheap LLM call to classify user intent with full
//! `ComplexitySignals`. Falls back to `Reactive { max_iterations: 10 }`
//! on timeout or parse errors.

use std::time::Duration;

use common::Result;
use providers::{ChatParams, DynProvider, Message};

use super::types::{
    AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis, ToolGroup,
};

/// Classifies user intent via a lightweight LLM call, returning structured
/// `IntentAnalysis` with `ComplexitySignals`.
pub struct IntentClassifier {
    provider: DynProvider,
    timeout: Duration,
}

const CLASSIFICATION_PROMPT: &str = r#"Classify this user message and assess its complexity.

Respond ONLY with valid JSON:
{
  "mode": "direct" | "reactive" | "planned",
  "estimated_tool_calls": <0-10>,
  "has_sequential_deps": <true|false>,
  "failure_risk": "low" | "medium" | "high",
  "requires_state_tracking": <true|false>,
  "requires_retries": <true|false>,
  "relevant_tools": ["tool1", "tool2"],
  "confidence": <0.0-1.0>,
  "reasoning": "<brief explanation>"
}

Mode guide:
- "direct": Greetings, factual Q&A, explanations — no tools needed
- "reactive": Single-shot tasks needing tools — search, CRUD, lookups
- "planned": Multi-step tasks with dependencies, state tracking, or high failure risk

For "relevant_tools": list ONLY the tools from the available set that are needed.
Use an empty array for "direct" mode (no tools needed).

User message: "{message}"
Available tools: {tools}"#;

impl IntentClassifier {
    pub fn new(provider: DynProvider, timeout: Duration) -> Self {
        Self { provider, timeout }
    }

    /// Classify a user message using an LLM call.
    ///
    /// `strategy_context` optionally provides historical strategy performance
    /// data to help the LLM make better classification decisions.
    pub async fn classify(
        &self,
        message: &str,
        tool_names: &[&str],
        params: &ChatParams,
        strategy_context: Option<&str>,
    ) -> Result<IntentAnalysis> {
        let mut prompt = CLASSIFICATION_PROMPT
            .replace("{message}", message)
            .replace("{tools}", &tool_names.join(", "));

        if let Some(ctx) = strategy_context {
            prompt.push_str("\n\n");
            prompt.push_str(ctx);
        }

        let messages = vec![Message::user(prompt)];

        let result =
            tokio::time::timeout(self.timeout, self.provider.chat(&messages, None, params)).await;

        let response = match result {
            Ok(Ok(r)) => r,
            _ => return Ok(IntentAnalysis::fallback()),
        };

        let content = response.content.as_deref().unwrap_or("");
        Ok(Self::parse_classification_json(content))
    }

    fn parse_classification_json(content: &str) -> IntentAnalysis {
        let json_str = match common::utils::extract_json_object(content) {
            Some(s) => s,
            None => return IntentAnalysis::fallback(),
        };

        let v: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return IntentAnalysis::fallback(),
        };

        let mode_str = v["mode"].as_str().unwrap_or("reactive");
        let confidence = v["confidence"].as_f64().unwrap_or(0.7) as f32;
        let reasoning = v["reasoning"].as_str().unwrap_or("").to_string();

        let signals = ComplexitySignals {
            estimated_tool_calls: v["estimated_tool_calls"].as_u64().unwrap_or(1) as u8,
            has_sequential_deps: v["has_sequential_deps"].as_bool().unwrap_or(false),
            failure_risk: match v["failure_risk"].as_str().unwrap_or("low") {
                "high" => FailureRisk::High,
                "medium" => FailureRisk::Medium,
                _ => FailureRisk::Low,
            },
            requires_state_tracking: v["requires_state_tracking"].as_bool().unwrap_or(false),
            requires_retries: v["requires_retries"].as_bool().unwrap_or(false),
        };

        let mode = match mode_str {
            "direct" => ExecutionMode::Direct,
            "planned" => ExecutionMode::Planned {
                visibility: domain::PlanVisibility::default(),
                max_steps: signals.estimated_tool_calls.max(5),
            },
            _ => ExecutionMode::Reactive {
                max_iterations: (signals.estimated_tool_calls as u32).max(5),
            },
        };

        // Map relevant_tools from LLM response to ToolGroups
        let tool_groups = match mode_str {
            "direct" => vec![ToolGroup::None],
            _ => {
                let relevant: Vec<String> = v["relevant_tools"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                if relevant.is_empty() {
                    vec![ToolGroup::Full]
                } else {
                    map_tool_names_to_groups(&relevant)
                }
            }
        };

        IntentAnalysis {
            mode,
            signals,
            confidence,
            source: AnalysisSource::LlmClassifier,
            reasoning,
            tool_groups,
        }
    }
}

/// Map a list of tool names to the ToolGroups they belong to.
fn map_tool_names_to_groups(tool_names: &[String]) -> Vec<ToolGroup> {
    let all_groups = [
        ToolGroup::TaskManagement,
        ToolGroup::Search,
        ToolGroup::Calendar,
        ToolGroup::Finance,
        ToolGroup::Communication,
        ToolGroup::Automation,
    ];

    let mut matched = Vec::new();
    for group in &all_groups {
        let group_tools = group.tool_names();
        if tool_names
            .iter()
            .any(|name| group_tools.contains(&name.as_str()))
        {
            matched.push(group.clone());
        }
    }

    if matched.is_empty() {
        vec![ToolGroup::Full]
    } else {
        matched
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Arc;

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

    fn mock_provider(text: &str) -> DynProvider {
        Arc::new(MockClassifierProvider::new(text))
    }

    fn default_params() -> ChatParams {
        ChatParams::new("test-model")
    }

    #[tokio::test]
    async fn parses_structured_classification() {
        let response = r#"{"mode":"planned","estimated_tool_calls":5,"has_sequential_deps":true,"failure_risk":"high","requires_state_tracking":true,"requires_retries":false,"confidence":0.9,"reasoning":"Multi-step booking"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify(
                "book a flight",
                &["web_search", "web_fetch"],
                &default_params(),
                None,
            )
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Planned { .. }));
        assert_eq!(result.signals.estimated_tool_calls, 5);
        assert!(result.signals.has_sequential_deps);
        assert_eq!(result.signals.failure_risk, FailureRisk::High);
        assert!((result.confidence - 0.9).abs() < 1e-6);
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[tokio::test]
    async fn parses_direct_mode() {
        let response = r#"{"mode":"direct","estimated_tool_calls":0,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.95,"reasoning":"Simple greeting"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("hello", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Direct));
    }

    #[tokio::test]
    async fn parses_reactive_mode() {
        let response = r#"{"mode":"reactive","estimated_tool_calls":2,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.85,"reasoning":"Needs search"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("search for tasks", &["todo"], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 5 }
        ));
    }

    #[tokio::test]
    async fn invalid_json_returns_fallback() {
        let classifier = IntentClassifier::new(
            mock_provider("I can't classify this"),
            Duration::from_secs(2),
        );
        let result = classifier
            .classify("hello", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 10 }
        ));
        assert_eq!(result.source, AnalysisSource::Heuristic); // fallback uses Heuristic source
    }

    #[tokio::test]
    async fn json_embedded_in_text() {
        let response = r#"Sure: {"mode":"direct","estimated_tool_calls":0,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.9,"reasoning":"Greeting"} done"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("hi", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Direct));
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[test]
    fn extract_json_finds_object() {
        let input = r#"text {"key":"value"} more"#;
        assert_eq!(common::utils::extract_json_object(input), Some(r#"{"key":"value"}"#));
    }

    #[test]
    fn extract_json_handles_no_json() {
        assert_eq!(common::utils::extract_json_object("no json here"), None);
    }

    #[test]
    fn reactive_max_iterations_uses_tool_calls() {
        // When estimated_tool_calls is 8, max_iterations should be max(8, 5) = 8
        let json = r#"{"mode":"reactive","estimated_tool_calls":8,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Many tools"}"#;
        let result = IntentClassifier::parse_classification_json(json);
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 8 }
        ));
    }
}
