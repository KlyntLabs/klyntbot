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
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["apiKey"], "test-key");
        assert!(json.get("apiBase").is_none());
    }
}
