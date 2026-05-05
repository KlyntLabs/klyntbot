//! Shared test utilities for the agent crate.

use std::sync::Mutex;

use providers::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, Message, ProviderCapabilities,
    ProviderHealth, ToolCall, Usage,
};
use serde_json::Value;

/// Unified mock LLM provider for unit tests.
///
/// Supports queued responses, error injection, tool calls, and configurable
/// provider metadata.
pub struct MockProvider {
    responses: Mutex<Vec<std::result::Result<LlmResponse, String>>>,
    context_window: usize,
    capabilities: ProviderCapabilities,
    health: ProviderHealth,
}

impl MockProvider {
    /// Create a mock that returns a single text response.
    pub fn with_text(text: &str) -> Self {
        Self::with_response(LlmResponse {
            content: Some(text.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }

    /// Create a mock that returns a single tool call response.
    pub fn with_tool_call(name: &str, args: Value) -> Self {
        Self::with_response(LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: name.to_string(),
                arguments: args,
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }

    /// Create a mock that returns a single LLM response.
    pub fn with_response(response: LlmResponse) -> Self {
        Self {
            responses: Mutex::new(vec![Ok(response)]),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Create a mock that returns an error on every call.
    pub fn with_error(msg: &str) -> Self {
        Self {
            responses: Mutex::new(vec![Err(msg.to_string())]),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Create a mock with a queue of responses (or errors).
    /// Each call to `chat()` pops the next item from the queue.
    pub fn with_responses(responses: Vec<std::result::Result<LlmResponse, String>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Set the context window size.
    pub fn context_window(mut self, window: usize) -> Self {
        self.context_window = window;
        self
    }

    /// Set provider capabilities.
    pub fn capabilities(mut self, caps: ProviderCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set the health check response.
    pub fn health(mut self, health: ProviderHealth) -> Self {
        self.health = health;
        self
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
        _cache_breakpoints: &[CacheBreakpoint],
    ) -> common::Result<LlmResponse> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(
                    "MockProvider: no more responses in queue".to_string(),
                ),
            ));
        }
        match responses.remove(0) {
            Ok(r) => Ok(r),
            Err(e) => Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(e),
            )),
        }
    }

    fn default_model(&self) -> &str {
        "mock"
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }

    fn context_window(&self) -> usize {
        self.context_window
    }

    async fn health_check(&self) -> common::Result<ProviderHealth> {
        Ok(self.health.clone())
    }
}
