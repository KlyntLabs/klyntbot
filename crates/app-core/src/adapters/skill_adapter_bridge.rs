//! Bridge that adapts the app-core cognitive LLM provider to the
//! `skills_adapter::AdapterProvider` trait.

use std::sync::Arc;

use async_trait::async_trait;
use common::{KlyntbotError, Result};
use providers::{ChatParams, LlmProvider, Message, ResponseFormat};

use skills_adapter::AdapterProvider;

pub struct CognitiveProviderAdapter {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
}

#[async_trait]
impl AdapterProvider for CognitiveProviderAdapter {
    async fn generate(&self, prompt: &str) -> Result<String> {
        let params = ChatParams {
            model: self.model.clone(),
            temperature: Some(0.2),
            max_tokens: Some(4096),
            response_format: Some(ResponseFormat::JsonObject),
        };
        let messages = vec![Message::user(prompt)];
        let resp = self
            .provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("adapter LLM call: {e}")))?;
        Ok(resp.content.unwrap_or_default())
    }
}
