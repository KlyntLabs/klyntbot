//! Provider factory — creation, resolution, and failover wiring.
//!
//! This module is the composition root for LLM providers. It reads [`Config`]
//! and constructs the appropriate [`DynProvider`] based on explicit settings,
//! model-name heuristics, gateway detection, or API-key presence.

use std::sync::Arc;
use tracing::info;

use common::{ConfigError, Result};
use config::Config;

use crate::adapters::{AnthropicNativeProvider, OpenAiCompatProvider};
use crate::manager::ProviderManager;
use crate::registry::{ProviderRegistry, ProviderSpec};
use crate::types::{ChatParams, DynProvider};

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
        Arc::new(ProviderManager::new(primary, Some(fallback), classifier)),
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

/// Create a provider for background cognitive tasks.
///
/// Uses `config.cognitive.model` and `config.cognitive.provider` if set,
/// otherwise falls back to the main agent provider. Returns `None` if
/// no provider can be created (no API keys configured).
pub fn create_cognitive_provider(config: &Config) -> Result<Option<DynProvider>> {
    let model = config
        .cognitive
        .model
        .as_deref()
        .unwrap_or(&config.agents.defaults.model);
    let provider_name = config.cognitive.provider.as_deref().or(config
        .agents
        .defaults
        .provider
        .as_deref());

    // Try explicit provider name first, then model-based lookup
    if let Some(name) = provider_name {
        if let Some(spec) = ProviderRegistry::find_by_name(name) {
            if let Some(provider) = try_create_from_spec(spec, config, model) {
                return Ok(Some(provider));
            }
        }
    }
    if let Some(spec) = ProviderRegistry::find_by_model(model) {
        if let Some(provider) = try_create_from_spec(spec, config, model) {
            return Ok(Some(provider));
        }
    }

    // Fall back to main agent provider
    match create_provider(config) {
        Ok((provider, _model)) => Ok(Some(provider)),
        Err(_) => Ok(None), // No provider available — handlers will use heuristics
    }
}

/// Build `ChatParams` for cognitive LLM calls.
pub fn cognitive_chat_params(config: &Config, default_max_tokens: u32) -> ChatParams {
    let model = config
        .cognitive
        .model
        .as_deref()
        .unwrap_or(&config.agents.defaults.model);
    let temperature = config.cognitive.temperature.unwrap_or(0.2);
    let max_tokens = config.cognitive.max_tokens.unwrap_or(default_max_tokens);

    ChatParams::new(model)
        .with_temperature(temperature)
        .with_max_tokens(max_tokens)
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
            )
            .with_cache_system_prompt(pc.cache_system_prompt)
            .with_extended_thinking(pc.extended_thinking.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_cognitive_provider_returns_none_when_no_keys() {
        let config = Config::default();
        let result = create_cognitive_provider(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_cognitive_chat_params_defaults() {
        let config = Config::default();
        let params = cognitive_chat_params(&config, 1024);
        assert_eq!(params.temperature, Some(0.2));
        assert_eq!(params.max_tokens, Some(1024));
    }

    #[test]
    fn test_cognitive_chat_params_with_overrides() {
        let mut config = Config::default();
        config.cognitive.temperature = Some(0.5);
        config.cognitive.max_tokens = Some(2048);
        let params = cognitive_chat_params(&config, 1024);
        assert_eq!(params.temperature, Some(0.5));
        assert_eq!(params.max_tokens, Some(2048));
    }
}
