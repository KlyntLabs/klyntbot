//! LlmSummaryProvider — calls an LLM to produce abstractive conversation summaries.

use async_trait::async_trait;
use context_engine::SummaryProvider;
use providers::{ChatParams, Message, UserContent};
use tracing::warn;

/// Implements [`SummaryProvider`] by delegating to a configured LLM provider.
pub struct LlmSummaryProvider {
    provider: providers::DynProvider,
    model: String,
}

impl LlmSummaryProvider {
    pub fn new(provider: providers::DynProvider, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl SummaryProvider for LlmSummaryProvider {
    async fn summarize(&self, messages: &[Message]) -> Result<String, String> {
        if messages.is_empty() {
            return Ok(String::new());
        }

        // Format messages for the prompt
        let formatted: String = messages
            .iter()
            .filter_map(|m| match m {
                Message::User { content } => {
                    let text = match content {
                        UserContent::Text(t) => t.as_str(),
                        UserContent::MultiPart(_) => "[multipart]",
                    };
                    Some(format!("User: {}", text))
                }
                Message::Assistant {
                    content: Some(text),
                    ..
                } => Some(format!("Assistant: {}", text)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize this conversation segment in 2-3 sentences, preserving key facts and decisions:\n\n{}",
            formatted
        );

        let request_messages = vec![Message::user(prompt)];
        let params = ChatParams::new(&self.model).with_max_tokens(256);

        match self.provider.chat(&request_messages, None, &params).await {
            Ok(response) => response.content.ok_or_else(|| "Empty LLM response".to_string()),
            Err(e) => {
                warn!("LlmSummaryProvider failed: {}", e);
                Err(e.to_string())
            }
        }
    }
}
