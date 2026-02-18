//! DirectResponse engine — single LLM call, no tool dispatch.
//!
//! Used for factual questions, explanations, conversational replies,
//! and any request that doesn't need tool use or multi-step planning.

use common::{ProviderError, Result};
use context_engine::AssembledContext;
use providers::{ChatParams, DynProvider};
use tracing::debug;

/// Engine that handles simple Q&A with a single LLM call.
pub struct DirectResponseEngine {
    provider: DynProvider,
}

impl DirectResponseEngine {
    /// Create a new DirectResponseEngine with the given provider.
    pub fn new(provider: DynProvider) -> Self {
        Self { provider }
    }

    /// Execute a direct response: single provider call, return content.
    ///
    /// # Errors
    /// Returns an error if the provider call fails or returns empty content.
    pub async fn execute(
        &self,
        context: AssembledContext,
        params: &ChatParams,
    ) -> Result<DirectResponse> {
        debug!(
            "DirectResponseEngine: executing with {} messages, {} estimated tokens",
            context.messages.len(),
            context.token_count
        );

        let response = self
            .provider
            .chat(&context.messages, None, params)
            .await?;

        let content = response.content.ok_or_else(|| {
            ProviderError::InvalidResponse("LLM returned empty content".to_string())
        })?;

        Ok(DirectResponse {
            content,
            usage: response.usage,
            finish_reason: response.finish_reason,
        })
    }
}

/// Result of a direct response execution.
#[derive(Debug)]
pub struct DirectResponse {
    /// The text content returned by the LLM.
    pub content: String,
    /// Token usage from the provider.
    pub usage: providers::Usage,
    /// Finish reason from the provider.
    pub finish_reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use context_engine::budget::{BudgetReport, Priority};
    use providers::{
        ChatParams, LlmProvider, LlmResponse, Message, Usage,
    };
    use serde_json::Value;
    use std::sync::{Arc, Mutex};

    /// Mock provider that records calls and returns configured responses.
    struct MockProvider {
        response: LlmResponse,
        calls: Mutex<Vec<(Vec<Message>, Option<Vec<Value>>)>>,
        model: String,
    }

    impl MockProvider {
        fn new(response: LlmResponse) -> Self {
            Self {
                response,
                calls: Mutex::new(Vec::new()),
                model: "mock-model".to_string(),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            messages: &[Message],
            tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            self.calls
                .lock()
                .unwrap()
                .push((messages.to_vec(), tools.map(|t| t.to_vec())));
            Ok(self.response.clone())
        }

        fn default_model(&self) -> &str {
            &self.model
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    fn make_context(messages: Vec<Message>) -> AssembledContext {
        AssembledContext {
            messages,
            token_count: 100,
            budget_report: BudgetReport {
                total_window: 128_000,
                total_allocated: 100,
                remaining: 127_900,
                per_priority: vec![(Priority::RecentHistory, 100)],
            },
        }
    }

    fn make_response(content: Option<&str>) -> LlmResponse {
        LlmResponse {
            content: content.map(|s| s.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        }
    }

    #[tokio::test]
    async fn test_direct_response_single_call_returns_content() {
        let provider = Arc::new(MockProvider::new(make_response(Some("Hello! I can help."))));
        let engine = DirectResponseEngine::new(provider.clone());

        let context = make_context(vec![
            Message::system("You are helpful."),
            Message::user("What is Rust?"),
        ]);
        let params = ChatParams::new("mock-model");

        let result = engine.execute(context, &params).await.unwrap();

        assert_eq!(result.content, "Hello! I can help.");
        assert_eq!(result.finish_reason, "stop");
        assert_eq!(result.usage.total_tokens, 30);
        assert_eq!(provider.call_count(), 1);
    }

    #[tokio::test]
    async fn test_direct_response_passes_messages_to_provider() {
        let provider = Arc::new(MockProvider::new(make_response(Some("Response"))));
        let engine = DirectResponseEngine::new(provider.clone());

        let context = make_context(vec![
            Message::system("System prompt"),
            Message::user("User message"),
        ]);
        let params = ChatParams::new("mock-model");

        engine.execute(context, &params).await.unwrap();

        let calls = provider.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        // Tools should be None for direct response
        assert!(calls[0].1.is_none());
        // Should have 2 messages
        assert_eq!(calls[0].0.len(), 2);
    }

    #[tokio::test]
    async fn test_direct_response_empty_content_returns_error() {
        let provider = Arc::new(MockProvider::new(make_response(None)));
        let engine = DirectResponseEngine::new(provider);

        let context = make_context(vec![Message::user("Hello")]);
        let params = ChatParams::new("mock-model");

        let result = engine.execute(context, &params).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("empty content"),
            "Error should mention empty content, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_direct_response_respects_max_tokens() {
        // Create a provider that captures the params
        struct ParamCapture {
            captured_params: Mutex<Option<ChatParams>>,
        }

        #[async_trait]
        impl LlmProvider for ParamCapture {
            async fn chat(
                &self,
                _messages: &[Message],
                _tools: Option<&[Value]>,
                params: &ChatParams,
            ) -> Result<LlmResponse> {
                *self.captured_params.lock().unwrap() = Some(params.clone());
                Ok(make_response(Some("ok")))
            }

            fn default_model(&self) -> &str {
                "mock"
            }

            fn name(&self) -> &str {
                "mock"
            }
        }

        let provider = Arc::new(ParamCapture {
            captured_params: Mutex::new(None),
        });
        let engine = DirectResponseEngine::new(provider.clone());

        let context = make_context(vec![Message::user("Hello")]);
        let params = ChatParams::new("mock-model").with_max_tokens(256);

        engine.execute(context, &params).await.unwrap();

        let captured = provider.captured_params.lock().unwrap();
        assert_eq!(captured.as_ref().unwrap().max_tokens, Some(256));
    }
}
