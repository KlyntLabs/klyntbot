//! Mock LLM provider for testing without external API calls

use async_trait::async_trait;
use klyntbot::error::{KlyntbotError, ProviderError, Result};
use klyntbot::providers::types::*;
use std::sync::{Arc, Mutex};

/// Mock provider that returns predefined responses
#[derive(Clone)]
pub struct MockProvider {
    responses: Arc<Mutex<Vec<LlmResponse>>>,
    call_count: Arc<Mutex<usize>>,
}

impl MockProvider {
    /// Create a new mock provider with a simple text response
    pub fn new(response: &str) -> Self {
        Self::with_responses(vec![LlmResponse {
            content: Some(response.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }])
    }

    /// Create a mock provider with multiple predefined responses
    pub fn with_responses(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a mock provider that returns a tool call
    pub fn with_tool_call(tool_name: &str, tool_args: serde_json::Value) -> Self {
        Self::with_responses(vec![LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_123".to_string(),
                name: tool_name.to_string(),
                arguments: tool_args,
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }])
    }

    /// Get the number of times the provider was called
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
    ) -> Result<LlmResponse> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;

        let responses = self.responses.lock().unwrap();
        let index = (*count - 1) % responses.len();
        Ok(responses[index].clone())
    }

    fn default_model(&self) -> &str {
        "mock-model"
    }

    fn name(&self) -> &str {
        "mock"
    }
}

/// Mock provider that always returns an error
pub struct ErrorProvider {
    error_message: String,
}

impl ErrorProvider {
    pub fn new(error_message: &str) -> Self {
        Self {
            error_message: error_message.to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for ErrorProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
    ) -> Result<LlmResponse> {
        Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
            self.error_message.clone(),
        )))
    }

    fn default_model(&self) -> &str {
        "error-model"
    }

    fn name(&self) -> &str {
        "error"
    }
}
