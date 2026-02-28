//! DirectEngine — handles Direct execution mode (single LLM call, no tools).
//!
//! Ported from `execution/direct.rs` but returns `EngineResult` instead of
//! `DirectOutcome`. Escalates to Reactive if the LLM generates tool calls.

use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use providers::Message;
use tools::RoutingContext;

use super::{EngineResult, ExecutionEngine};
use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams};
use crate::intent_pipeline::router::EscalationContext;

/// Executes Direct mode: single LLM call with no tools.
pub struct DirectEngine {
    core: Arc<ExecutionCore>,
}

impl DirectEngine {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl ExecutionEngine for DirectEngine {
    async fn execute(
        &self,
        messages: Vec<Message>,
        _tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        let mut messages = messages;
        let (outcome, usage) = self
            .core
            .run_cycle(&mut messages, &[], params, ctx, event_tx.as_ref(), None)
            .await?;

        match outcome {
            CycleOutcome::FinalResponse { content }
            | CycleOutcome::FabricatedResponse { content } => Ok(EngineResult::Complete {
                content,
                usage,
                iterations: 1,
                traces: vec![],
                tool_name: None,
            }),
            CycleOutcome::ToolsExecuted { .. } => {
                // LLM wanted tools despite being given none — escalate
                Ok(EngineResult::Escalate {
                    reason: "LLM requested tools in Direct mode".to_string(),
                    carried_context: EscalationContext {
                        messages,
                        completed_work: vec![],
                        original_message: String::new(),
                    },
                    usage,
                })
            }
            CycleOutcome::EmptyResponse => Ok(EngineResult::Complete {
                content: String::new(),
                usage,
                iterations: 1,
                traces: vec![],
                tool_name: None,
            }),
        }
    }

    fn mode(&self) -> &str {
        "direct"
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
        ) -> Result<LlmResponse> {
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
    async fn direct_returns_response() {
        let engine = make_engine(MockDirectProvider::text("Hello! How can I help?"));

        let result = engine
            .execute(
                vec![Message::user("hi")],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete {
                content,
                iterations,
                ..
            } => {
                assert_eq!(content, "Hello! How can I help?");
                assert_eq!(iterations, 1);
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[tokio::test]
    async fn escalates_when_tool_calls_present() {
        let engine = make_engine(MockDirectProvider::with_tool_call());

        let result = engine
            .execute(
                vec![Message::user("search for Rust docs")],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Escalate {
                reason,
                carried_context,
                ..
            } => {
                assert!(reason.contains("Direct mode"));
                assert!(!carried_context.messages.is_empty());
            }
            EngineResult::Complete { .. } => panic!("Expected Escalate, got Complete"),
        }
    }

    #[tokio::test]
    async fn empty_response_handled() {
        let engine = make_engine(MockDirectProvider::empty());

        let result = engine
            .execute(
                vec![Message::user("...")],
                &[],
                &ExecutionParams::new("mock"),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete { content, .. } => {
                assert!(content.is_empty());
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[test]
    fn mode_returns_direct() {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = Arc::new(ExecutionCore::new(
            MockDirectProvider::text("") as providers::DynProvider,
            registry,
        ));
        let engine = DirectEngine::new(core);
        assert_eq!(engine.mode(), "direct");
    }
}
