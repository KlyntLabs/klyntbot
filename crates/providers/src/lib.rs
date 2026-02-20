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
    LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ProviderCapabilities,
    ProviderHealth, ResponseFormat, ToolCall, ToolCallDelta, ToolCallMessage, Usage, UserContent,
    DEFAULT_CONTEXT_WINDOW,
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
pub fn create_provider(config: &Config) -> Result<(DynProvider, String)> {
    let configured_model = &config.agents.defaults.model;

    // Priority 1: Explicit provider field
    if let Some(ref provider_name) = config.agents.defaults.provider {
        if !provider_name.is_empty() {
            if let Some(spec) = ProviderRegistry::find_by_name(provider_name) {
                // When the user set a provider but the model doesn't match it,
                // use the provider's own default model instead of the global default.
                let model_belongs_to_provider = spec
                    .keywords
                    .iter()
                    .any(|kw| configured_model.to_lowercase().contains(kw));
                let model = if model_belongs_to_provider {
                    configured_model.as_str()
                } else {
                    info!(
                        "Model '{}' doesn't match provider '{}', using provider default: {}",
                        configured_model, provider_name, spec.default_model
                    );
                    spec.default_model
                };

                if let Some(provider) = try_create_from_spec(spec, config, model) {
                    info!("Using explicitly configured provider: {}", provider_name);
                    return Ok((provider, model.to_string()));
                }
                tracing::warn!(
                    "Provider '{}' configured but API key missing, trying auto-detection",
                    provider_name
                );
            }
        }
    }

    let model = configured_model.as_str();

    // Priority 2: Try to find provider by model name
    if let Some(spec) = ProviderRegistry::find_by_model(model) {
        if let Some(provider) = try_create_from_spec(spec, config, model) {
            return Ok((provider, model.to_string()));
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
        ("zhipu", &providers_config.zhipu),
        ("dashscope", &providers_config.dashscope),
        ("moonshot", &providers_config.moonshot),
        ("minimax", &providers_config.minimax),
        ("aihubmix", &providers_config.aihubmix),
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
            return Ok((Arc::new(provider), model.to_string()));
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
                return Ok((Arc::new(provider), model.to_string()));
            }
        }
    }

    Err(ConfigError::MissingField(
        "No LLM provider configured. Add an API key to config.json (e.g., providers.anthropic.api_key)".to_string(),
    ).into())
}

/// Create a provider with failover support.
///
/// When `config.provider_manager.fallback` is set, wraps the primary provider
/// in a [`ProviderManager`] with retry, failover, and circuit breaker logic.
/// When no fallback is configured, returns a plain `DynProvider` (identical to
/// [`create_provider`]).
pub fn create_provider_with_failover(config: &Config) -> Result<(DynProvider, String)> {
    let (primary, resolved_model) = create_provider(config)?;

    let pm_config = &config.provider_manager;
    let fallback_name = match &pm_config.fallback {
        Some(name) if !name.is_empty() => name,
        _ => return Ok((primary, resolved_model)),
    };

    let fallback = create_fallback_provider(config, fallback_name)?;

    let classifier = pm_config
        .classifier_model
        .as_deref()
        .and_then(|model| create_classifier_provider(config, model));

    info!(
        "Created ProviderManager with fallback provider: {}",
        fallback_name
    );
    Ok((
        Arc::new(ProviderManager::new(
            primary,
            Some(fallback),
            classifier,
        )),
        resolved_model,
    ))
}

/// Create a fallback provider by name from config.
fn create_fallback_provider(config: &Config, provider_name: &str) -> Result<DynProvider> {
    let spec = ProviderRegistry::find_by_name(provider_name).ok_or_else(|| {
        ConfigError::MissingField(format!("Unknown fallback provider: {provider_name}"))
    })?;

    try_create_from_spec(spec, config, &config.agents.defaults.model).ok_or_else(|| {
        ConfigError::MissingField(format!(
            "Fallback provider '{}' has no API key configured",
            provider_name
        ))
        .into()
    })
}

/// Try to create a classifier provider for the given model name.
fn create_classifier_provider(config: &Config, model: &str) -> Option<DynProvider> {
    let spec = ProviderRegistry::find_by_model(model)?;
    try_create_from_spec(spec, config, model)
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
        "zhipu" => &config.providers.zhipu,
        "dashscope" => &config.providers.dashscope,
        "moonshot" => &config.providers.moonshot,
        "minimax" => &config.providers.minimax,
        "aihubmix" => &config.providers.aihubmix,
        _ => return None,
    };

    if !pc.api_key.is_empty() {
        let api_base = pc.api_base.as_deref().unwrap_or(spec.default_api_base);

        // Use native Anthropic provider when native: true
        if pc.native && spec.name == "anthropic" {
            let mut provider = AnthropicNativeProvider::new(
                config::Secret::new(pc.api_key.expose().to_string()),
                api_base.to_string(),
                model.to_string(),
            );
            if let Some(ref version) = pc.api_version {
                provider = provider.with_api_version(version);
            }
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
