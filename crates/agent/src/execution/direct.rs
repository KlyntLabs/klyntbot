//! DirectResponse engine — single-cycle LLM call without tools.
//!
//! The simplest execution path: sends messages to the LLM with no tool
//! definitions. If the LLM unexpectedly requests tools, escalates to
//! ToolAssisted strategy by returning the full message history.

use std::sync::Arc;

use common::Result;
use providers::Message;
use tools::RoutingContext;

use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams};

/// Convert an `Arc<Vec<Message>>` into a `Vec<Message>`, avoiding a clone
/// when this is the last Arc reference.
fn arc_into_vec(arc: Arc<Vec<Message>>) -> Vec<Message> {
    Arc::try_unwrap(arc).unwrap_or_else(|arc| (*arc).clone())
}

/// Outcome of a direct (no-tool) execution.
#[derive(Debug)]
pub enum DirectOutcome {
    /// LLM produced a text response.
    Response(String),
    /// LLM requested tools — escalate with full message context.
    EscalateToToolAssisted { messages: Vec<Message> },
}

/// Drives a single LLM call with no tools. Escalates if the LLM
/// unexpectedly wants tool access.
pub struct DirectEngine {
    core: Arc<ExecutionCore>,
}

impl DirectEngine {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self { core }
    }

    /// Execute a direct response cycle (no tools provided to LLM).
    ///
    /// Accepts `Arc<Vec<Message>>` to avoid cloning when the caller can
    /// transfer sole ownership (refcount == 1).
    pub async fn execute(
        &self,
        messages: Arc<Vec<Message>>,
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<DirectOutcome> {
        let mut messages = arc_into_vec(messages);
        let (outcome, _usage) = self
            .core
            .run_cycle(&mut messages, &[], params, ctx, event_tx, None)
            .await?;

        match outcome {
            CycleOutcome::FinalResponse { content } => Ok(DirectOutcome::Response(content)),
            CycleOutcome::FabricatedResponse { content } => Ok(DirectOutcome::Response(content)),
            CycleOutcome::ToolsExecuted { .. } => {
                // LLM wanted tools despite being given none — escalate
                Ok(DirectOutcome::EscalateToToolAssisted { messages })
            }
            CycleOutcome::EmptyResponse => Ok(DirectOutcome::Response(String::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    // ── Mock provider ────────────────────────────────────────────

    struct MockDirectProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl MockDirectProvider {
        fn text(text: &str) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![LlmResponse {
                    content: Some(text.to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                }]),
            })
        }

        fn with_tool_call() -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![LlmResponse {
                    content: None,
                    tool_calls: vec![ToolCall {
                        id: "call_1".to_string(),
                        name: "web_search".to_string(),
                        arguments: serde_json::json!({"q": "test"}),
                    }],
                    finish_reason: "tool_calls".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                }]),
            })
        }

        fn empty() -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![LlmResponse {
                    content: None,
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                }]),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for MockDirectProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(LlmResponse {
                    content: Some("default".to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                })
            } else {
                Ok(responses.remove(0))
            }
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn make_engine(provider: Arc<MockDirectProvider>) -> DirectEngine {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(
            provider as providers::DynProvider,
            registry,
        ));
        DirectEngine::new(core)
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_direct_response_returns_content() {
        let engine = make_engine(MockDirectProvider::text("Hello! How can I help?"));

        let result = engine
            .execute(
                Arc::new(vec![Message::user("hi")]),
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            DirectOutcome::Response(content) => {
                assert_eq!(content, "Hello! How can I help?");
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_escalates_when_tool_calls_present() {
        let engine = make_engine(MockDirectProvider::with_tool_call());

        let result = engine
            .execute(
                Arc::new(vec![Message::user("search for Rust docs")]),
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            DirectOutcome::EscalateToToolAssisted { messages } => {
                // Should contain original user message + assistant tool call + tool result
                assert!(messages.len() >= 2, "Should preserve message history");
            }
            other => panic!("Expected EscalateToToolAssisted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_empty_response_handled() {
        let engine = make_engine(MockDirectProvider::empty());

        let result = engine
            .execute(
                Arc::new(vec![Message::user("...")]),
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            DirectOutcome::Response(content) => {
                assert!(
                    content.is_empty(),
                    "Empty response should yield empty string"
                );
            }
            other => panic!("Expected empty Response, got {:?}", other),
        }
    }
}
