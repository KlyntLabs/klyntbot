//! No-op LLM provider for when no real provider is configured.
//!
//! Returns a clear error on any LLM call, allowing the app to boot
//! into the setup wizard without panicking.

use async_trait::async_trait;
use serde_json::Value;

use common::{ProviderError, Result};

use crate::types::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, Message, ProviderCapabilities,
    ProviderHealth,
};

/// A stub provider that rejects all LLM calls with a configuration error.
pub struct NoopProvider;

#[async_trait]
impl LlmProvider for NoopProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
        _cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        Err(ProviderError::Http(
            "No LLM provider configured. Please complete setup in Settings → Personalization."
                .to_string(),
        )
        .into())
    }

    fn default_model(&self) -> &str {
        "none"
    }

    fn name(&self) -> &str {
        "noop"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth::Unhealthy(
            "No provider configured".to_string(),
        ))
    }
}
