//! Shared test utilities for engine tests.
//!
//! Provides a reusable mock LLM provider and common helpers so each engine's
//! test module doesn't need to duplicate mock infrastructure.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::Result;
use providers::{ChatParams, DynProvider, LlmProvider, LlmResponse, Message, ToolCall, Usage};
use serde_json::Value;
use tokio::sync::RwLock;
use tools::{registry::ToolRegistry, RoutingContext};

use crate::execution::{ExecutionCore, ExecutionParams};

/// Mock LLM provider that returns a sequence of pre-configured responses.
///
/// Falls back to a default text response when the sequence is exhausted.
pub struct MockSequenceProvider {
    responses: Mutex<Vec<LlmResponse>>,
}

impl MockSequenceProvider {
    pub fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses),
        })
    }

    /// Single text response.
    pub fn text(text: &str) -> Arc<Self> {
        Self::new(vec![text_response(text)])
    }

    /// Single tool call response.
    pub fn with_tool_call(tool_name: &str) -> Arc<Self> {
        Self::new(vec![tool_call_response(tool_name)])
    }

    /// Empty response (no content, no tool calls).
    pub fn empty() -> Arc<Self> {
        Self::new(vec![LlmResponse {
            content: None,
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }])
    }
}

#[async_trait]
impl LlmProvider for MockSequenceProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
    ) -> Result<LlmResponse> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            Ok(LlmResponse {
                content: Some("fallback response".to_string()),
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

// ── Response builders ───────────────────────────────────────────

pub fn text_response(text: &str) -> LlmResponse {
    LlmResponse {
        content: Some(text.to_string()),
        tool_calls: vec![],
        finish_reason: "stop".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }
}

pub fn tool_call_response(tool_name: &str) -> LlmResponse {
    LlmResponse {
        content: None,
        tool_calls: vec![ToolCall {
            id: "call_1".to_string(),
            name: tool_name.to_string(),
            arguments: serde_json::json!({}),
        }],
        finish_reason: "tool_calls".to_string(),
        usage: Usage::default(),
        reasoning_content: None,
    }
}

// ── Common helpers ──────────────────────────────────────────────

pub fn routing_ctx() -> RoutingContext {
    RoutingContext::new("test".into(), "test".into())
}

pub fn make_registry() -> Arc<RwLock<ToolRegistry>> {
    Arc::new(RwLock::new(ToolRegistry::new()))
}

pub fn default_params() -> ExecutionParams {
    ExecutionParams::new("mock")
}

pub fn make_core(provider: DynProvider) -> Arc<ExecutionCore> {
    Arc::new(ExecutionCore::new(provider, make_registry()))
}
