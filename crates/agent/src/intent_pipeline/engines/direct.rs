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
            | CycleOutcome::FabricatedResponse { content } => {
                Ok(EngineResult::complete(content, usage, 1))
            }
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
            CycleOutcome::EmptyResponse => Ok(EngineResult::empty(usage, 1)),
        }
    }

    fn mode(&self) -> &str {
        "direct"
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    fn make_engine(provider: providers::DynProvider) -> DirectEngine {
        DirectEngine::new(make_core(provider))
    }

    #[tokio::test]
    async fn direct_returns_response() {
        let engine = make_engine(MockSequenceProvider::text("Hello! How can I help?"));

        let result = engine
            .execute(
                vec![Message::user("hi")],
                &[],
                &default_params(),
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
        let engine = make_engine(MockSequenceProvider::with_tool_call("web_search"));

        let result = engine
            .execute(
                vec![Message::user("search for Rust docs")],
                &[],
                &default_params(),
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
        let engine = make_engine(MockSequenceProvider::empty());

        let result = engine
            .execute(
                vec![Message::user("...")],
                &[],
                &default_params(),
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
        let engine = DirectEngine::new(make_core(MockSequenceProvider::text("")));
        assert_eq!(engine.mode(), "direct");
    }
}
