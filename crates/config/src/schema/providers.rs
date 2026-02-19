//! LLM provider configuration structs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::Secret;

/// LLM providers configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersConfig {
    #[serde(default)]
    pub anthropic: ProviderConfig,

    #[serde(default)]
    pub openai: ProviderConfig,

    #[serde(default)]
    pub openrouter: ProviderConfig,

    #[serde(default)]
    pub deepseek: ProviderConfig,

    #[serde(default)]
    pub gemini: ProviderConfig,

    #[serde(default)]
    pub groq: ProviderConfig,

    #[serde(default)]
    pub vllm: ProviderConfig,

    #[serde(default)]
    pub zhipu: ProviderConfig,

    #[serde(default)]
    pub dashscope: ProviderConfig,

    #[serde(default)]
    pub moonshot: ProviderConfig,

    #[serde(default)]
    pub minimax: ProviderConfig,

    #[serde(default)]
    pub aihubmix: ProviderConfig,
}

/// Individual provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: Secret<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<HashMap<String, String>>,

    /// Use native API format (e.g., Anthropic Messages API instead of OpenAI-compat)
    #[serde(default)]
    pub native: bool,

    /// Enable prompt caching for system prompts (Anthropic-specific)
    #[serde(default)]
    pub cache_system_prompt: bool,

    /// Extended thinking / chain-of-thought configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_thinking: Option<ExtendedThinkingConfig>,

    /// API version header override (Anthropic-specific, e.g., "2023-06-01")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

/// Extended thinking (chain-of-thought) configuration for supported providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedThinkingConfig {
    pub enabled: bool,
    #[serde(default = "default_thinking_budget")]
    pub budget_tokens: usize,
    /// Task types that should use extended thinking (e.g., ["planning", "debugging"])
    #[serde(default)]
    pub use_for: Vec<String>,
}

fn default_thinking_budget() -> usize {
    10_000
}

/// Provider manager configuration for primary/fallback/classifier routing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManagerConfig {
    /// Name of the primary provider (e.g., "anthropic")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,
    /// Name of the fallback provider (e.g., "openai")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Model name for the complexity classifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_model: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_providers_config_default() {
        let config = ProvidersConfig::default();
        assert_eq!(config.anthropic.api_key.expose(), "");
        assert_eq!(config.openai.api_key.expose(), "");
        assert_eq!(config.openrouter.api_key.expose(), "");
        assert_eq!(config.deepseek.api_key.expose(), "");
        assert!(config.anthropic.api_base.is_none());
    }

    #[test]
    fn test_provider_config_with_api_base() {
        let config = ProviderConfig {
            api_key: Secret::new("test-key".to_string()),
            api_base: Some("https://custom.api.com/v1".to_string()),
            extra_headers: None,
            ..Default::default()
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["apiKey"], "test-key");
        assert_eq!(json["apiBase"], "https://custom.api.com/v1");
    }

    #[test]
    fn test_provider_config_without_api_base() {
        let config = ProviderConfig {
            api_key: Secret::new("test-key".to_string()),
            api_base: None,
            extra_headers: None,
            ..Default::default()
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["apiKey"], "test-key");
        assert!(json.get("apiBase").is_none());
    }

    #[test]
    fn test_provider_config_native_defaults_to_false() {
        let config = ProviderConfig::default();
        assert!(!config.native);
        assert!(!config.cache_system_prompt);
        assert!(config.extended_thinking.is_none());
    }

    #[test]
    fn test_provider_config_native_fields_backward_compat() {
        // Old config without native/cache/thinking fields should deserialize fine
        let json = serde_json::json!({
            "apiKey": "sk-test",
            "apiBase": "https://example.com/v1"
        });
        let config: ProviderConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.api_key.expose(), "sk-test");
        assert!(!config.native);
        assert!(!config.cache_system_prompt);
        assert!(config.extended_thinking.is_none());
    }

    #[test]
    fn test_provider_config_with_native_and_thinking() {
        let json = serde_json::json!({
            "apiKey": "sk-ant",
            "native": true,
            "cacheSystemPrompt": true,
            "extendedThinking": {
                "enabled": true,
                "budgetTokens": 20000,
                "useFor": ["planning", "debugging"]
            }
        });
        let config: ProviderConfig = serde_json::from_value(json).unwrap();
        assert!(config.native);
        assert!(config.cache_system_prompt);
        let thinking = config.extended_thinking.unwrap();
        assert!(thinking.enabled);
        assert_eq!(thinking.budget_tokens, 20000);
        assert_eq!(thinking.use_for, vec!["planning", "debugging"]);
    }

    #[test]
    fn test_extended_thinking_budget_default() {
        let json = serde_json::json!({
            "enabled": true,
            "useFor": []
        });
        let config: ExtendedThinkingConfig = serde_json::from_value(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.budget_tokens, 10_000);
    }

    #[test]
    fn test_provider_manager_config_default() {
        let config = ProviderManagerConfig::default();
        assert!(config.primary.is_none());
        assert!(config.fallback.is_none());
        assert!(config.classifier_model.is_none());
    }

    #[test]
    fn test_provider_manager_config_serde() {
        let json = serde_json::json!({
            "primary": "anthropic",
            "fallback": "openai",
            "classifierModel": "claude-haiku"
        });
        let config: ProviderManagerConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.primary.as_deref(), Some("anthropic"));
        assert_eq!(config.fallback.as_deref(), Some("openai"));
        assert_eq!(config.classifier_model.as_deref(), Some("claude-haiku"));
    }

    #[test]
    fn test_provider_manager_config_empty_json() {
        let json = serde_json::json!({});
        let config: ProviderManagerConfig = serde_json::from_value(json).unwrap();
        assert!(config.primary.is_none());
        assert!(config.fallback.is_none());
    }
}
