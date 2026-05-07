use crate::AppCore;
use common::Result;

/// A configured LLM provider with status info.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub has_api_key: bool,
    pub default_model: Option<String>,
    pub is_primary: bool,
    pub is_fallback: bool,
}

/// Result for providers_list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersListResult {
    pub providers: Vec<ProviderInfo>,
    pub primary: Option<String>,
    pub fallback: Option<String>,
}

/// Result for provider_status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusResult {
    pub id: String,
    pub available: bool,
    pub error: Option<String>,
}

/// Known provider IDs and their display names + default models.
pub(crate) const KNOWN_PROVIDERS: &[(&str, &str, &str)] = &[
    ("anthropic", "Anthropic", "claude-sonnet-4-20250514"),
    ("openai", "OpenAI", "gpt-4o"),
    ("openrouter", "OpenRouter", ""),
    ("deepseek", "DeepSeek", "deepseek-chat"),
    ("gemini", "Gemini", "gemini-2.0-flash"),
    ("groq", "Groq", "llama-3.3-70b-versatile"),
    ("vllm", "vLLM", ""),
    ("zhipu", "Zhipu", ""),
    ("dashscope", "DashScope", ""),
    ("moonshot", "Moonshot", ""),
    ("minimax", "MiniMax", ""),
    ("aihubmix", "AiHubMix", ""),
];

impl AppCore {
    /// List all configured LLM providers with their status.
    #[tracing::instrument(skip(self), err)]
    pub async fn providers_list(&self) -> Result<ProvidersListResult> {
        let config = self.config.read().await;
        let providers_cfg = &config.providers;
        let manager_cfg = &config.provider_manager;

        let primary = manager_cfg.primary.clone();
        let fallback = manager_cfg.fallback.clone();

        let mut providers = Vec::new();

        for &(id, name, default_model) in KNOWN_PROVIDERS {
            let has_api_key = Self::has_api_key(id, providers_cfg);

            providers.push(ProviderInfo {
                id: id.to_string(),
                name: name.to_string(),
                has_api_key,
                default_model: if default_model.is_empty() {
                    None
                } else {
                    Some(default_model.to_string())
                },
                is_primary: primary.as_deref() == Some(id),
                is_fallback: fallback.as_deref() == Some(id),
            });
        }

        Ok(ProvidersListResult {
            providers,
            primary,
            fallback,
        })
    }

    /// Check if a specific provider's API key is valid by making a lightweight request.
    ///
    /// For now, this only checks if the key is non-empty. Full validation would
    /// ping the provider's /models endpoint.
    #[tracing::instrument(skip(self), err)]
    pub async fn provider_status(&self, provider_id: &str) -> Result<ProviderStatusResult> {
        let config = self.config.read().await;
        let providers_cfg = &config.providers;

        let has_key = Self::has_api_key(provider_id, providers_cfg);

        if !has_key {
            return Ok(ProviderStatusResult {
                id: provider_id.to_string(),
                available: false,
                error: Some("No API key configured".to_string()),
            });
        }

        // Future: make a lightweight API call to verify the key works
        Ok(ProviderStatusResult {
            id: provider_id.to_string(),
            available: true,
            error: None,
        })
    }

    fn has_api_key(id: &str, cfg: &config::schema::ProvidersConfig) -> bool {
        Self::provider_entry(id, cfg).is_some()
    }

    /// Return `(api_key, api_base)` for a known provider if the key is non-empty.
    pub(crate) fn provider_entry<'a>(
        id: &str,
        cfg: &'a config::schema::ProvidersConfig,
    ) -> Option<(&'a str, &'a str)> {
        let (key, base) = match id {
            "anthropic" => (
                cfg.anthropic.api_key.expose(),
                cfg.anthropic.api_base.as_deref().unwrap_or(""),
            ),
            "openai" => (
                cfg.openai.api_key.expose(),
                cfg.openai.api_base.as_deref().unwrap_or(""),
            ),
            "openrouter" => (
                cfg.openrouter.api_key.expose(),
                cfg.openrouter.api_base.as_deref().unwrap_or(""),
            ),
            "deepseek" => (
                cfg.deepseek.api_key.expose(),
                cfg.deepseek.api_base.as_deref().unwrap_or(""),
            ),
            "gemini" => (
                cfg.gemini.api_key.expose(),
                cfg.gemini.api_base.as_deref().unwrap_or(""),
            ),
            "groq" => (
                cfg.groq.api_key.expose(),
                cfg.groq.api_base.as_deref().unwrap_or(""),
            ),
            "vllm" => (
                cfg.vllm.api_key.expose(),
                cfg.vllm.api_base.as_deref().unwrap_or(""),
            ),
            "zhipu" => (
                cfg.zhipu.api_key.expose(),
                cfg.zhipu.api_base.as_deref().unwrap_or(""),
            ),
            "dashscope" => (
                cfg.dashscope.api_key.expose(),
                cfg.dashscope.api_base.as_deref().unwrap_or(""),
            ),
            "moonshot" => (
                cfg.moonshot.api_key.expose(),
                cfg.moonshot.api_base.as_deref().unwrap_or(""),
            ),
            "minimax" => (
                cfg.minimax.api_key.expose(),
                cfg.minimax.api_base.as_deref().unwrap_or(""),
            ),
            "aihubmix" => (
                cfg.aihubmix.api_key.expose(),
                cfg.aihubmix.api_base.as_deref().unwrap_or(""),
            ),
            _ => return None,
        };
        if key.is_empty() {
            None
        } else {
            Some((key, base))
        }
    }
}
