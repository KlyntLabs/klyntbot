//! Provider metadata, auto-detection, and config access helpers.

use super::super::prompts;

/// Provider metadata used to drive the selection UI and configuration.
pub(crate) struct ProviderInfo {
    pub name: &'static str,
    pub key: &'static str,
    pub description: &'static str,
    pub api_url: &'static str,
    pub default_model: &'static str,
    pub key_prefix: &'static str,
    pub models: &'static [&'static str],
}

pub(crate) const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        name: "Anthropic (Claude)",
        key: "anthropic",
        description: "Recommended for best quality",
        api_url: "https://console.anthropic.com",
        default_model: "claude-haiku-4-5",
        key_prefix: "sk-ant-",
        models: &[
            "claude-opus-4-6",
            "claude-sonnet-4-5",
            "claude-haiku-4-5",
            "claude-opus-4-5",
        ],
    },
    ProviderInfo {
        name: "OpenAI (GPT)",
        key: "openai",
        description: "Industry standard models",
        api_url: "https://platform.openai.com/api-keys",
        default_model: "o4-mini",
        key_prefix: "sk-",
        models: &["o3", "o4-mini", "gpt-4o", "o3-pro"],
    },
    ProviderInfo {
        name: "DeepSeek",
        key: "deepseek",
        description: "Cost-effective alternative",
        api_url: "https://platform.deepseek.com",
        default_model: "deepseek-chat",
        key_prefix: "sk-",
        models: &["deepseek-chat", "deepseek-reasoner"],
    },
    ProviderInfo {
        name: "Google (Gemini)",
        key: "gemini",
        description: "Multimodal capabilities",
        api_url: "https://makersuite.google.com/app/apikey",
        default_model: "gemini-2.5-flash-lite",
        key_prefix: "",
        models: &[
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.5-flash-lite",
        ],
    },
    ProviderInfo {
        name: "OpenRouter",
        key: "openrouter",
        description: "Access to many models via unified API",
        api_url: "https://openrouter.ai/keys",
        default_model: "openrouter/free",
        key_prefix: "sk-or-",
        models: &[
            "openrouter/auto",
            "openrouter/free",
            "anthropic/claude-opus-4-6",
            "openai/o3",
        ],
    },
    ProviderInfo {
        name: "Groq",
        key: "groq",
        description: "Ultra-fast inference",
        api_url: "https://console.groq.com/keys",
        default_model: "llama-3.1-8b-instant",
        key_prefix: "gsk_",
        models: &[
            "llama-4-scout",
            "llama-4-maverick",
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
            "gemma2-9b-it",
        ],
    },
];

/// Check if any provider has an API key configured.
pub(crate) fn has_any_provider_configured(config: &config::Config) -> bool {
    PROVIDERS
        .iter()
        .any(|p| !get_provider_key(config, p.key).is_empty())
}

/// Get the current API key for a provider.
pub(crate) fn get_provider_key(config: &config::Config, provider_key: &str) -> String {
    match provider_key {
        "anthropic" => config.providers.anthropic.api_key.expose().clone(),
        "openai" => config.providers.openai.api_key.expose().clone(),
        "deepseek" => config.providers.deepseek.api_key.expose().clone(),
        "gemini" => config.providers.gemini.api_key.expose().clone(),
        "openrouter" => config.providers.openrouter.api_key.expose().clone(),
        "groq" => config.providers.groq.api_key.expose().clone(),
        _ => String::new(),
    }
}

/// Get the current API base URL for a provider.
pub(crate) fn get_provider_api_base(config: &config::Config, provider_key: &str) -> Option<String> {
    match provider_key {
        "anthropic" => config.providers.anthropic.api_base.clone(),
        "openai" => config.providers.openai.api_base.clone(),
        "deepseek" => config.providers.deepseek.api_base.clone(),
        "gemini" => config.providers.gemini.api_base.clone(),
        "openrouter" => config.providers.openrouter.api_base.clone(),
        "groq" => config.providers.groq.api_base.clone(),
        _ => None,
    }
}

/// Set the API base URL for a provider.
pub(crate) fn set_provider_api_base(
    config: &mut config::Config,
    provider_key: &str,
    base: Option<String>,
) {
    match provider_key {
        "anthropic" => config.providers.anthropic.api_base = base,
        "openai" => config.providers.openai.api_base = base,
        "deepseek" => config.providers.deepseek.api_base = base,
        "gemini" => config.providers.gemini.api_base = base,
        "openrouter" => config.providers.openrouter.api_base = base,
        "groq" => config.providers.groq.api_base = base,
        _ => {}
    }
}

/// Build select options from the PROVIDERS list (used by fallback menu).
pub(crate) fn provider_select_options() -> Vec<prompts::SelectOption<'static>> {
    PROVIDERS
        .iter()
        .map(|p| prompts::SelectOption {
            label: p.name,
            description: p.description,
        })
        .chain(std::iter::once(prompts::SelectOption {
            label: "Done",
            description: "finish provider setup",
        }))
        .collect()
}
