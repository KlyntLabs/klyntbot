//! Klyntbot Providers - LLM provider abstraction and implementations
//!
//! This crate defines the LlmProvider trait and implementations for various LLM APIs.

pub mod openai_compat;
pub mod registry;
pub mod transcription;
pub mod types;

pub use openai_compat::OpenAiCompatProvider;
pub use registry::{ProviderRegistry, ProviderSpec, PROVIDERS};
pub use transcription::TranscriptionProvider;
pub use types::{
    tool_calls_to_messages, ChatParams, ContentPart, DynProvider, FunctionCall, ImageUrl,
    LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ToolCall, ToolCallDelta,
    ToolCallMessage, Usage, UserContent,
};

use std::sync::Arc;
use tracing::info;

use klyntbot_config::Config;
use klyntbot_core::{ConfigError, Result};

/// Initialize the LLM provider from configuration.
///
/// Resolution order:
/// 1. Check if model name matches a known provider (e.g., "claude-*" → Anthropic)
/// 2. Check for gateway providers (OpenRouter, AiHubMix) by api_key prefix or api_base
/// 3. Fall back to first provider with a non-empty API key
pub fn create_provider(config: &Config) -> Result<DynProvider> {
    let model = &config.agents.defaults.model;

    // Try to find provider by model name
    if let Some(spec) = ProviderRegistry::find_by_model(model) {
        if let Some(provider) = try_create_from_spec(spec, config, model) {
            return Ok(provider);
        }
    }

    // Try gateway detection (OpenRouter key prefix, AiHubMix base URL, etc.)
    let providers_config = &config.providers;
    let candidates = [
        ("openrouter", &providers_config.openrouter),
        ("openai", &providers_config.openai),
        ("anthropic", &providers_config.anthropic),
        ("deepseek", &providers_config.deepseek),
        ("gemini", &providers_config.gemini),
        ("groq", &providers_config.groq),
        ("vllm", &providers_config.vllm),
    ];

    // Check for gateway by api_key prefix or api_base keyword
    for (name, pc) in &candidates {
        if pc.api_key.is_empty() {
            continue;
        }
        if let Some(spec) =
            ProviderRegistry::find_gateway(Some(name), Some(pc.api_key.expose()), pc.api_base.as_deref())
        {
            let api_base = pc.api_base.as_deref().unwrap_or(spec.default_api_base);
            let provider = OpenAiCompatProvider::new(api_base, pc.api_key.expose(), model)?;
            info!("Using {} provider with {}", spec.name, model);
            return Ok(Arc::new(provider));
        }
    }

    // Fallback: find first provider with a non-empty API key
    for (name, pc) in &candidates {
        if !pc.api_key.is_empty() {
            if let Some(spec) = ProviderRegistry::find_by_name(name) {
                let api_base = pc.api_base.as_deref().unwrap_or(spec.default_api_base);
                let provider = OpenAiCompatProvider::new(api_base, pc.api_key.expose(), model)?;
                info!(
                    "Using {} provider with {} (model: {})",
                    spec.name, api_base, model
                );
                return Ok(Arc::new(provider));
            }
        }
    }

    Err(ConfigError::MissingField(
        "No LLM provider configured. Add an API key to config.json (e.g., providers.anthropic.api_key)".to_string(),
    ).into())
}

/// Try to create a provider from a specific provider spec
fn try_create_from_spec(spec: &ProviderSpec, config: &Config, model: &str) -> Option<DynProvider> {
    let pc = match spec.name {
        "anthropic" => &config.providers.anthropic,
        "openai" => &config.providers.openai,
        "openrouter" => &config.providers.openrouter,
        "deepseek" => &config.providers.deepseek,
        "gemini" => &config.providers.gemini,
        "groq" => &config.providers.groq,
        "vllm" => &config.providers.vllm,
        _ => return None,
    };

    if !pc.api_key.is_empty() {
        let api_base = pc.api_base.as_deref().unwrap_or(spec.default_api_base);
        match OpenAiCompatProvider::new(api_base, pc.api_key.expose(), model) {
            Ok(provider) => {
                info!("Using {} provider with {}", spec.name, model);
                Some(Arc::new(provider))
            }
            Err(e) => {
                tracing::warn!("Failed to create {} provider: {}", spec.name, e);
                None
            }
        }
    } else {
        None
    }
}
