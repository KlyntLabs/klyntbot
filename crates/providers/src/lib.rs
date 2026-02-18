//! Klyntbot Providers - LLM provider abstraction and implementations
//!
//! This crate defines the LlmProvider trait and implementations for various LLM APIs.

pub mod anthropic_native;
pub mod manager;
pub mod openai_compat;
pub mod registry;
pub mod transcription;
pub mod types;

pub use anthropic_native::AnthropicNativeProvider;
pub use manager::{CircuitBreakerConfig, ProviderManager};
pub use openai_compat::OpenAiCompatProvider;
pub use registry::{ProviderRegistry, ProviderSpec, PROVIDERS};
pub use transcription::TranscriptionProvider;
pub use types::{
    tool_calls_to_messages, ChatParams, ContentPart, DynProvider, FunctionCall, ImageUrl,
    LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ProviderCapabilities, ToolCall,
    ToolCallDelta, ToolCallMessage, Usage, UserContent, DEFAULT_CONTEXT_WINDOW,
};

use std::sync::Arc;
use tracing::info;

use common::{ConfigError, Result};
use config::Config;

/// Initialize the LLM provider from configuration.
///
/// Resolution order:
/// 1. Check explicit provider field (`config.agents.defaults.provider`) if set
/// 2. Check if model name matches a known provider (e.g., "claude-*" → Anthropic)
/// 3. Check for gateway providers (OpenRouter, AiHubMix) by api_key prefix or api_base
/// 4. Fall back to first provider with a non-empty API key
pub fn create_provider(config: &Config) -> Result<DynProvider> {
    let model = &config.agents.defaults.model;

    // Priority 1: Explicit provider field
    if let Some(ref provider_name) = config.agents.defaults.provider {
        if !provider_name.is_empty() {
            if let Some(spec) = ProviderRegistry::find_by_name(provider_name) {
                if let Some(provider) = try_create_from_spec(spec, config, model) {
                    info!("Using explicitly configured provider: {}", provider_name);
                    return Ok(provider);
                }
                tracing::warn!(
                    "Provider '{}' configured but API key missing, trying auto-detection",
                    provider_name
                );
            }
        }
    }

    // Priority 2: Try to find provider by model name
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
        if let Some(spec) = ProviderRegistry::find_gateway(
            Some(name),
            Some(pc.api_key.expose()),
            pc.api_base.as_deref(),
        ) {
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

/// Create a ProviderManager from configuration.
///
/// Reads `config.provider_manager` for primary/fallback/classifier names,
/// then creates each provider and wraps them in a `ProviderManager`.
/// Falls back to `create_provider()` if no manager config is set.
pub fn create_provider_manager(config: &Config) -> Result<Arc<ProviderManager>> {
    let mgr_config = &config.provider_manager;

    // Resolve primary: explicit manager config → default create_provider
    let primary = if let Some(ref name) = mgr_config.primary {
        create_named_provider(config, name, &config.agents.defaults.model)?
    } else {
        create_provider(config)?
    };

    // Resolve fallback (optional)
    let fallback = if let Some(ref name) = mgr_config.fallback {
        match create_named_provider(config, name, &config.agents.defaults.model) {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("Failed to create fallback provider '{}': {}", name, e);
                None
            }
        }
    } else {
        None
    };

    // Resolve classifier (optional, uses its own model name)
    let classifier = if let Some(ref model) = mgr_config.classifier_model {
        // Detect the provider from the classifier model name
        if let Some(spec) = ProviderRegistry::find_by_model(model) {
            match try_create_from_spec(spec, config, model) {
                Some(p) => Some(p),
                None => {
                    tracing::warn!("Failed to create classifier provider for model '{}'", model);
                    None
                }
            }
        } else {
            // Use primary provider's API with classifier model
            None
        }
    } else {
        None
    };

    info!(
        "ProviderManager: primary={}, fallback={}, classifier={}",
        primary.name(),
        fallback.as_ref().map_or("none", |f| f.name()),
        classifier.as_ref().map_or("none", |c| c.name()),
    );

    Ok(Arc::new(ProviderManager::new(primary, fallback, classifier)))
}

/// Create a named provider by looking up config for that provider name.
fn create_named_provider(config: &Config, name: &str, model: &str) -> Result<DynProvider> {
    let spec = ProviderRegistry::find_by_name(name).ok_or_else(|| {
        common::ConfigError::Invalid(format!("Unknown provider: {}", name))
    })?;

    try_create_from_spec(spec, config, model).ok_or_else(|| {
        common::ConfigError::MissingField(format!(
            "Provider '{}' configured but API key missing",
            name
        ))
        .into()
    })
}

/// Try to create a provider from a specific provider spec.
/// When `native: true` and provider is anthropic, uses `AnthropicNativeProvider`.
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

        // Use native Anthropic provider when native: true
        if pc.native && spec.name == "anthropic" {
            let provider = AnthropicNativeProvider::new(
                config::Secret::new(pc.api_key.expose().to_string()),
                api_base.to_string(),
                model.to_string(),
            );
            info!("Using native Anthropic provider with {}", model);
            return Some(Arc::new(provider));
        }

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
