//! A scripted LLM provider that returns pre-defined responses in a cycling pattern.
//!
//! Used in simulation harness tests to produce deterministic LLM outputs
//! without hitting any real provider API.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use common::Result;
use providers::types::{
    ChatParams, DynProvider, LlmProvider, LlmResponse, LlmStream, Message, ProviderCapabilities,
    ProviderHealth, Usage,
};

/// An [`LlmProvider`] that returns pre-defined text responses, cycling through
/// them in order. Useful for deterministic simulation scenarios.
pub struct ScriptedProvider {
    responses: Vec<String>,
    call_count: AtomicUsize,
}

impl ScriptedProvider {
    /// Create a new `ScriptedProvider` with the given responses.
    ///
    /// # Panics
    /// Panics if `responses` is empty.
    pub fn new(responses: Vec<String>) -> Self {
        assert!(
            !responses.is_empty(),
            "ScriptedProvider requires at least one response"
        );
        Self {
            responses,
            call_count: AtomicUsize::new(0),
        }
    }

    /// Create a `ScriptedProvider` with a single default response.
    pub fn default_response() -> Self {
        Self::new(vec!["I understand. Let me help you with that.".to_string()])
    }

    /// Return the number of `chat()` calls made so far.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
    ) -> Result<LlmResponse> {
        let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
        let response_text = &self.responses[idx % self.responses.len()];

        Ok(LlmResponse {
            content: Some(response_text.clone()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
    ) -> Result<LlmStream> {
        Err(common::KlyntbotError::Provider(
            common::ProviderError::InvalidResponse(
                "ScriptedProvider does not support streaming".to_string(),
            ),
        ))
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn default_model(&self) -> &str {
        "scripted-sim"
    }

    fn name(&self) -> &str {
        "scripted-simulator"
    }

    async fn count_tokens(&self, _messages: &[Message], _tools: Option<&[Value]>) -> Result<usize> {
        Ok(150)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            extended_thinking: false,
            structured_outputs: false,
            prompt_caching: false,
            native_token_counting: false,
            vision: false,
            streaming: false,
            tool_choice_required: false,
            parallel_tool_calls: false,
        }
    }

    fn context_window(&self) -> usize {
        128_000
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth::Healthy)
    }

    fn classifier_provider(&self) -> Option<DynProvider> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scripted_provider_cycles_responses() {
        let provider = ScriptedProvider::new(vec!["first".to_string(), "second".to_string()]);

        let params = ChatParams::new("scripted-sim");
        let messages = vec![Message::user("hello")];

        let r1 = provider.chat(&messages, None, &params).await.unwrap();
        assert_eq!(r1.content.as_deref(), Some("first"));

        let r2 = provider.chat(&messages, None, &params).await.unwrap();
        assert_eq!(r2.content.as_deref(), Some("second"));

        // Cycles back to first
        let r3 = provider.chat(&messages, None, &params).await.unwrap();
        assert_eq!(r3.content.as_deref(), Some("first"));

        assert_eq!(provider.call_count(), 3);
    }

    #[tokio::test]
    async fn default_response_works() {
        let provider = ScriptedProvider::default_response();
        let params = ChatParams::new("scripted-sim");
        let messages = vec![Message::user("hi")];

        let r = provider.chat(&messages, None, &params).await.unwrap();
        assert_eq!(
            r.content.as_deref(),
            Some("I understand. Let me help you with that.")
        );
        assert_eq!(provider.call_count(), 1);
    }

    #[tokio::test]
    async fn chat_stream_returns_error() {
        let provider = ScriptedProvider::default_response();
        let params = ChatParams::new("scripted-sim");
        let messages = vec![Message::user("hi")];

        let result = provider.chat_stream(&messages, None, &params).await;
        assert!(result.is_err());
    }

    #[test]
    fn metadata_methods() {
        let provider = ScriptedProvider::default_response();
        assert_eq!(provider.name(), "scripted-simulator");
        assert_eq!(provider.default_model(), "scripted-sim");
        assert!(!provider.supports_streaming());
        assert_eq!(provider.context_window(), 128_000);
        assert!(provider.classifier_provider().is_none());

        let caps = provider.capabilities();
        assert!(!caps.vision);
        assert!(!caps.streaming);
    }

    #[tokio::test]
    async fn health_check_returns_healthy() {
        let provider = ScriptedProvider::default_response();
        let health = provider.health_check().await.unwrap();
        assert_eq!(health, ProviderHealth::Healthy);
    }

    #[tokio::test]
    async fn count_tokens_returns_fixed_value() {
        let provider = ScriptedProvider::default_response();
        let messages = vec![Message::user("test")];
        let count = provider.count_tokens(&messages, None).await.unwrap();
        assert_eq!(count, 150);
    }

    #[test]
    #[should_panic(expected = "ScriptedProvider requires at least one response")]
    fn empty_responses_panics() {
        ScriptedProvider::new(vec![]);
    }
}
