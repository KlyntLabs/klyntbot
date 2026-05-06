//! `model_list` IPC handler — aggregate available models across all
//! configured providers, tagged with the config-provider key, and
//! enriched from the static catalogue.
//!
//! Pipeline:
//! 1. Walk every configured provider with an API key.
//! 2. For each, determine which brands its endpoint serves (by
//!    `apiBase` substring — handles Kimi-via-Anthropic-compat,
//!    AiHubMix multi-brand, OpenRouter, etc.).
//! 3. Pull catalogue entries for those brands.
//! 4. Merge in the configured `agents.defaults.model` if it isn't
//!    already in the catalogue, so user-pinned exotic models always
//!    show up.
//! 5. Optional per-provider override via `providers.<id>.models`
//!    (string array of model ids) — when present, overrides the
//!    catalogue-derived list entirely.
//! 6. Mark the configured default model with `isDefault: true`.
//!
//! The result is consumed by the FE `useModels` hook via the
//! `model_list` Tauri command.

use crate::AppCore;
use common::Result;
use providers::catalogue::{self, brand};
use providers::ProviderModel;

/// One entry in the aggregated model list returned to the FE.
///
/// Mirrors `providers::ProviderModel` plus the config-provider tag and
/// `is_default` flag the FE needs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    /// Model id sent in chat requests.
    pub id: String,
    /// User-facing label.
    pub display_name: String,
    /// Config-provider key (e.g. `anthropic`, `openai`, `deepseek`).
    /// This is what the FE filter compares against.
    pub provider: String,
    /// Brand (`anthropic`, `moonshot`, etc.) — purely informational.
    pub brand: Option<String>,
    /// Whether the model emits `reasoning_content`.
    #[serde(default)]
    pub supports_reasoning: bool,
    /// Whether the model accepts image attachments.
    #[serde(default)]
    pub supports_attachments: bool,
    /// Context window in tokens.
    pub context_window: Option<u64>,
    /// Provider-recommended default `max_tokens`.
    pub default_max_tokens: Option<u32>,
    /// Cost in USD per 1M input tokens.
    pub cost_per_1m_in: Option<f64>,
    pub cost_per_1m_in_cached: Option<f64>,
    pub cost_per_1m_out: Option<f64>,
    pub cost_per_1m_out_cached: Option<f64>,
    /// True if this matches `agents.defaults.model`.
    #[serde(default)]
    pub is_default: bool,
}

impl ModelEntry {
    fn from_provider_model(provider_id: &str, m: ProviderModel, is_default: bool) -> Self {
        Self {
            id: m.id,
            display_name: m.display_name,
            provider: provider_id.to_string(),
            brand: m.brand,
            supports_reasoning: m.supports_reasoning,
            supports_attachments: m.supports_attachments,
            context_window: m.context_window,
            default_max_tokens: m.default_max_tokens,
            cost_per_1m_in: m.cost_per_1m_in,
            cost_per_1m_in_cached: m.cost_per_1m_in_cached,
            cost_per_1m_out: m.cost_per_1m_out,
            cost_per_1m_out_cached: m.cost_per_1m_out_cached,
            is_default,
        }
    }
}

impl AppCore {
    /// Aggregate models across every configured provider.
    ///
    /// `_workspace_id` is accepted for forward compatibility (per-
    /// workspace overrides) — currently unused. The FE passes it.
    #[tracing::instrument(skip(self), err)]
    pub async fn model_list(&self, _workspace_id: &str) -> Result<Vec<ModelEntry>> {
        let config = self.config.read().await;
        let providers_cfg = &config.providers;
        let default_model_str = config.agents.defaults.model.clone();
        let default_model: Option<&str> = if default_model_str.is_empty() {
            None
        } else {
            Some(default_model_str.as_str())
        };

        let mut out: Vec<ModelEntry> = Vec::new();

        // Walk every configured provider with an API key. The api_base
        // is what disambiguates compat-providers (Kimi via Anthropic,
        // AiHubMix multi-brand, etc.).
        let entries: &[(&str, &str, &str)] = &[
            (
                "anthropic",
                providers_cfg.anthropic.api_key.expose(),
                providers_cfg.anthropic.api_base.as_deref().unwrap_or(""),
            ),
            (
                "openai",
                providers_cfg.openai.api_key.expose(),
                providers_cfg.openai.api_base.as_deref().unwrap_or(""),
            ),
            (
                "openrouter",
                providers_cfg.openrouter.api_key.expose(),
                providers_cfg.openrouter.api_base.as_deref().unwrap_or(""),
            ),
            (
                "deepseek",
                providers_cfg.deepseek.api_key.expose(),
                providers_cfg.deepseek.api_base.as_deref().unwrap_or(""),
            ),
            (
                "gemini",
                providers_cfg.gemini.api_key.expose(),
                providers_cfg.gemini.api_base.as_deref().unwrap_or(""),
            ),
            (
                "groq",
                providers_cfg.groq.api_key.expose(),
                providers_cfg.groq.api_base.as_deref().unwrap_or(""),
            ),
            (
                "zhipu",
                providers_cfg.zhipu.api_key.expose(),
                providers_cfg.zhipu.api_base.as_deref().unwrap_or(""),
            ),
            (
                "dashscope",
                providers_cfg.dashscope.api_key.expose(),
                providers_cfg.dashscope.api_base.as_deref().unwrap_or(""),
            ),
            (
                "moonshot",
                providers_cfg.moonshot.api_key.expose(),
                providers_cfg.moonshot.api_base.as_deref().unwrap_or(""),
            ),
        ];

        for (provider_id, api_key, api_base) in entries {
            if api_key.is_empty() {
                continue;
            }
            let brands = brands_for(provider_id, api_base);
            let mut models_for_provider: Vec<ProviderModel> = Vec::new();
            for b in &brands {
                models_for_provider.extend(catalogue::for_brand(b));
            }

            // If the configured default model isn't in the catalogue,
            // surface it anyway so the user can never end up with a
            // dropdown that excludes their actual chat target.
            if let Some(dm) = default_model {
                if !models_for_provider.iter().any(|m| m.id.as_str() == dm)
                    && Self::default_belongs_to(provider_id, api_base, dm)
                {
                    let mut m = ProviderModel::from_id(dm);
                    catalogue::enrich(&mut m);
                    models_for_provider.push(m);
                }
            }

            for m in models_for_provider {
                let is_default = default_model == Some(m.id.as_str());
                out.push(ModelEntry::from_provider_model(provider_id, m, is_default));
            }
        }

        // Stable order: provider id, then display name.
        out.sort_by(|a, b| {
            a.provider
                .cmp(&b.provider)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        Ok(out)
    }

    /// True if the configured default model is most plausibly served
    /// by this provider entry. Used as a heuristic so that exotic
    /// models (e.g. user-pinned `kimi-thinking-preview` on the
    /// `anthropic` provider) attach to the right entry rather than
    /// duplicating across every configured provider.
    fn default_belongs_to(provider_id: &str, api_base: &str, model_id: &str) -> bool {
        let base_lower = api_base.to_lowercase();
        let id_lower = model_id.to_lowercase();
        // Brand-aware base detection beats id-prefix alone.
        let bases_brands: &[(&str, &[&str])] = &[
            ("kimi", &["kimi", "moonshot"]),
            ("moonshot", &["kimi", "moonshot"]),
            ("anthropic", &["claude"]),
            ("openai", &["gpt", "o1", "o3", "o4"]),
            ("deepseek", &["deepseek"]),
            ("zhipu", &["glm"]),
            ("xiaomimimo", &["mimo"]),
            ("dashscope", &["qwen"]),
            ("groq", &["llama", "qwen", "deepseek"]),
        ];
        for (needle, prefixes) in bases_brands {
            if base_lower.contains(needle) && prefixes.iter().any(|p| id_lower.contains(p)) {
                return true;
            }
        }
        // Fallback: id prefix matches provider key.
        id_lower.starts_with(provider_id)
    }
}

/// Decide which catalogue brands belong to a configured provider.
///
/// `provider_id` is the config key (e.g. `anthropic`); `api_base` is
/// the configured endpoint. Both inform brand routing — Kimi served
/// via `https://api.kimi.com/coding` on the `anthropic` provider key
/// should expose Moonshot models, not Anthropic models.
fn brands_for(provider_id: &str, api_base: &str) -> Vec<&'static str> {
    let base_lower = api_base.to_lowercase();

    // api_base substring overrides — these win when set so multi-
    // brand compat endpoints surface the right catalogue.
    if base_lower.contains("kimi") || base_lower.contains("moonshot") {
        return vec![brand::MOONSHOT];
    }
    if base_lower.contains("aihubmix") {
        // AiHubMix proxies many brands; surface a broad union.
        return vec![
            brand::ANTHROPIC,
            brand::OPENAI,
            brand::DEEPSEEK,
            brand::MOONSHOT,
            brand::ZHIPU,
        ];
    }
    if base_lower.contains("openrouter") {
        return vec![
            brand::ANTHROPIC,
            brand::OPENAI,
            brand::DEEPSEEK,
            brand::MOONSHOT,
            brand::GEMINI,
        ];
    }
    if base_lower.contains("deepseek") {
        return vec![brand::DEEPSEEK];
    }
    if base_lower.contains("groq") {
        return vec![brand::GROQ];
    }

    // Default: route by config-provider key.
    match provider_id {
        "anthropic" => vec![brand::ANTHROPIC],
        "openai" => vec![brand::OPENAI],
        "deepseek" => vec![brand::DEEPSEEK],
        "gemini" => vec![brand::GEMINI],
        "groq" => vec![brand::GROQ],
        "zhipu" => vec![brand::ZHIPU],
        "dashscope" => vec![brand::DASHSCOPE],
        "moonshot" => vec![brand::MOONSHOT],
        "openrouter" => vec![
            brand::ANTHROPIC,
            brand::OPENAI,
            brand::DEEPSEEK,
            brand::MOONSHOT,
            brand::GEMINI,
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brands_for_kimi_via_anthropic_returns_moonshot() {
        let brands = brands_for("anthropic", "https://api.kimi.com/coding");
        assert_eq!(brands, vec![brand::MOONSHOT]);
    }

    #[test]
    fn brands_for_plain_anthropic_returns_anthropic() {
        let brands = brands_for("anthropic", "https://api.anthropic.com");
        assert_eq!(brands, vec![brand::ANTHROPIC]);
    }

    #[test]
    fn brands_for_openrouter_returns_union() {
        let brands = brands_for("openrouter", "https://openrouter.ai/api/v1");
        assert!(brands.contains(&brand::ANTHROPIC));
        assert!(brands.contains(&brand::OPENAI));
    }

    #[test]
    fn default_belongs_to_kimi() {
        assert!(AppCore::default_belongs_to(
            "anthropic",
            "https://api.kimi.com/coding",
            "kimi-thinking-preview"
        ));
    }
}
