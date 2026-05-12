//! Test fixtures for provider behavior. Compiled under all features.

use async_trait::async_trait;
use std::sync::Arc;

use crate::types::{CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, Message, Usage};
use crate::DynProvider;
use common::Result;

/// Returns a single assistant text response with no tool calls; useful for
/// "clean completion" tests that don't exercise the tool loop.
#[derive(Debug, Clone)]
pub struct SingleResponseProvider {
    text: String,
}

impl SingleResponseProvider {
    pub fn with_text(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn dyn_arc(text: impl Into<String>) -> DynProvider {
        Arc::new(Self::with_text(text))
    }
}

#[async_trait]
impl LlmProvider for SingleResponseProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
        _cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        Ok(LlmResponse {
            content: Some(self.text.clone()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }

    fn default_model(&self) -> &str {
        "test-single-response"
    }

    fn name(&self) -> &str {
        "test-single-response"
    }
}
