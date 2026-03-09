//! ProductivityHandlerImpl — agent-side implementation of the ProductivityHandler trait.
//!
//! Uses the LLM provider to generate natural language daily summaries from
//! productivity metrics. The trait itself lives in `feature-productivity`
//! to break the circular dependency.

use async_trait::async_trait;
use feature_productivity::ProductivityHandler;
use providers::{ChatParams, DynProvider, Message};

pub struct ProductivityHandlerImpl {
    provider: DynProvider,
    model: String,
}

impl ProductivityHandlerImpl {
    pub fn new(provider: DynProvider, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl ProductivityHandler for ProductivityHandlerImpl {
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String> {
        let prompt = format!(
            "Generate a brief, friendly 2-3 sentence daily productivity summary based on this data. Be specific about numbers. Mention the top achievement and one improvement suggestion.\n\nData: {}",
            context
        );

        let messages = vec![Message::user(prompt)];
        let params = ChatParams::new(&self.model).with_max_tokens(256);

        let response = self.provider.chat(&messages, None, &params).await?;
        response.content.ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "Empty LLM response for productivity summary".to_string(),
            ))
        })
    }

    async fn generate_narrative(&self, context: &str) -> common::Result<String> {
        let prompt = format!(
            "Write a 3-4 sentence narrative about the user's day, as if telling a story about their productivity. Be encouraging but honest. Use the data below.\n\n{}",
            context
        );

        let messages = vec![Message::user(prompt)];
        let params = ChatParams::new(&self.model).with_max_tokens(512);

        let response = self.provider.chat(&messages, None, &params).await?;
        response.content.ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "Empty LLM response for narrative".to_string(),
            ))
        })
    }

    async fn classify_activity(
        &self,
        app: &str,
        title: &str,
        url: Option<&str>,
    ) -> common::Result<String> {
        let url_str = url.unwrap_or("none");
        let prompt = format!(
            "Classify this activity into a category and session type.\nApp: {app}\nTitle: {title}\nURL: {url_str}\n\nRespond with exactly: category:session_type\nWhere session_type is one of: focus, meeting, break\nExample: coding:focus"
        );

        let messages = vec![Message::user(prompt)];
        let params = ChatParams::new(&self.model).with_max_tokens(32);

        let response = self.provider.chat(&messages, None, &params).await?;
        response.content.ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "Empty LLM response for activity classification".to_string(),
            ))
        })
    }
}
