//! Provider Registry - single source of truth for LLM provider metadata.
//!
//! This registry defines all supported LLM providers with their routing rules,
//! API endpoints, and model prefixing logic.

use serde_json::Value;
use std::collections::HashMap;

/// Provider specification with routing metadata
#[derive(Debug, Clone)]
pub struct ProviderSpec {
    /// Config field name (e.g., "anthropic", "deepseek")
    pub name: &'static str,

    /// Model-name keywords for matching (lowercase)
    pub keywords: &'static [&'static str],

    /// Environment variable for API key
    pub env_key: &'static str,

    /// Display name (shown in status)
    pub display_name: &'static str,

    /// Model prefix for routing (e.g., "deepseek/" for deepseek/deepseek-chat)
    pub prefix: &'static str,

    /// Skip prefixes if model already starts with these
    pub skip_prefixes: &'static [&'static str],

    /// Extra environment variables to set (name, value_template)
    pub env_extras: &'static [(&'static str, &'static str)],

    /// Is this a gateway that can route any model?
    pub is_gateway: bool,

    /// Is this a local deployment?
    pub is_local: bool,

    /// Auto-detect by API key prefix (e.g., "sk-or-" for OpenRouter)
    pub detect_by_key_prefix: &'static str,

    /// Auto-detect by substring in api_base URL
    pub detect_by_base_keyword: &'static str,

    /// Default API base URL
    pub default_api_base: &'static str,

    /// Strip existing prefix before re-prefixing (for gateways)
    pub strip_model_prefix: bool,

    /// Per-model parameter overrides (model_pattern, overrides)
    pub model_overrides: &'static [(&'static str, &'static [(&'static str, &'static str)])],
}

impl ProviderSpec {
    /// Get display label
    pub fn label(&self) -> &str {
        if self.display_name.is_empty() {
            self.name
        } else {
            self.display_name
        }
    }
}

/// All registered providers - order matters for fallback priority
pub static PROVIDERS: &[ProviderSpec] = &[
    // === Gateways (detected by api_key / api_base, not model name) =========

    // OpenRouter: global gateway, keys start with "sk-or-"
    ProviderSpec {
        name: "openrouter",
        keywords: &["openrouter"],
        env_key: "OPENROUTER_API_KEY",
        display_name: "OpenRouter",
        prefix: "openrouter",
        skip_prefixes: &[],
        env_extras: &[],
        is_gateway: true,
        is_local: false,
        detect_by_key_prefix: "sk-or-",
        detect_by_base_keyword: "openrouter",
        default_api_base: "https://openrouter.ai/api/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // AiHubMix: global gateway, OpenAI-compatible
    ProviderSpec {
        name: "aihubmix",
        keywords: &["aihubmix"],
        env_key: "OPENAI_API_KEY",
        display_name: "AiHubMix",
        prefix: "openai",
        skip_prefixes: &[],
        env_extras: &[],
        is_gateway: true,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "aihubmix",
        default_api_base: "https://aihubmix.com/v1",
        strip_model_prefix: true, // Strip "anthropic/" then re-add "openai/"
        model_overrides: &[],
    },
    // === Standard providers (matched by model-name keywords) ===============

    // Anthropic: no prefix needed (claude-* recognized natively)
    ProviderSpec {
        name: "anthropic",
        keywords: &["anthropic", "claude"],
        env_key: "ANTHROPIC_API_KEY",
        display_name: "Anthropic",
        prefix: "",
        skip_prefixes: &[],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.anthropic.com/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // OpenAI: no prefix needed (gpt-* recognized natively)
    ProviderSpec {
        name: "openai",
        keywords: &["openai", "gpt"],
        env_key: "OPENAI_API_KEY",
        display_name: "OpenAI",
        prefix: "",
        skip_prefixes: &[],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.openai.com/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // DeepSeek: needs "deepseek/" prefix
    ProviderSpec {
        name: "deepseek",
        keywords: &["deepseek"],
        env_key: "DEEPSEEK_API_KEY",
        display_name: "DeepSeek",
        prefix: "deepseek",
        skip_prefixes: &["deepseek/"],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.deepseek.com/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // Gemini: needs "gemini/" prefix
    ProviderSpec {
        name: "gemini",
        keywords: &["gemini"],
        env_key: "GEMINI_API_KEY",
        display_name: "Gemini",
        prefix: "gemini",
        skip_prefixes: &["gemini/"],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://generativelanguage.googleapis.com/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // Zhipu: needs "zai/" prefix, also sets ZHIPUAI_API_KEY
    ProviderSpec {
        name: "zhipu",
        keywords: &["zhipu", "glm", "zai"],
        env_key: "ZAI_API_KEY",
        display_name: "Zhipu AI",
        prefix: "zai",
        skip_prefixes: &["zhipu/", "zai/", "openrouter/", "hosted_vllm/"],
        env_extras: &[("ZHIPUAI_API_KEY", "{api_key}")],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://open.bigmodel.cn/api/paas/v4",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // DashScope: Qwen models, needs "dashscope/" prefix
    ProviderSpec {
        name: "dashscope",
        keywords: &["qwen", "dashscope"],
        env_key: "DASHSCOPE_API_KEY",
        display_name: "DashScope",
        prefix: "dashscope",
        skip_prefixes: &["dashscope/", "openrouter/"],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // Moonshot: Kimi models, needs "moonshot/" prefix
    ProviderSpec {
        name: "moonshot",
        keywords: &["moonshot", "kimi"],
        env_key: "MOONSHOT_API_KEY",
        display_name: "Moonshot",
        prefix: "moonshot",
        skip_prefixes: &["moonshot/", "openrouter/"],
        env_extras: &[("MOONSHOT_API_BASE", "{api_base}")],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.moonshot.ai/v1",
        strip_model_prefix: false,
        model_overrides: &[
            ("kimi-k2.5", &[("temperature", "1.0")]), // Kimi K2.5 requires temp >= 1.0
        ],
    },
    // MiniMax: needs "minimax/" prefix
    ProviderSpec {
        name: "minimax",
        keywords: &["minimax"],
        env_key: "MINIMAX_API_KEY",
        display_name: "MiniMax",
        prefix: "minimax",
        skip_prefixes: &["minimax/", "openrouter/"],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.minimax.io/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // === Local deployment ===

    // vLLM / OpenAI-compatible local server
    ProviderSpec {
        name: "vllm",
        keywords: &["vllm"],
        env_key: "HOSTED_VLLM_API_KEY",
        display_name: "vLLM/Local",
        prefix: "hosted_vllm",
        skip_prefixes: &[],
        env_extras: &[],
        is_gateway: false,
        is_local: true,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "http://localhost:8000/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
    // === Auxiliary ===

    // Groq: mainly for Whisper transcription, also usable for LLM
    ProviderSpec {
        name: "groq",
        keywords: &["groq"],
        env_key: "GROQ_API_KEY",
        display_name: "Groq",
        prefix: "groq",
        skip_prefixes: &["groq/"],
        env_extras: &[],
        is_gateway: false,
        is_local: false,
        detect_by_key_prefix: "",
        detect_by_base_keyword: "",
        default_api_base: "https://api.groq.com/openai/v1",
        strip_model_prefix: false,
        model_overrides: &[],
    },
];

/// Provider registry for looking up provider specs
pub struct ProviderRegistry;

impl ProviderRegistry {
    /// Find provider by model name keyword (case-insensitive)
    /// Skips gateways/local - those are matched by api_key/api_base instead
    pub fn find_by_model(model: &str) -> Option<&'static ProviderSpec> {
        let model_lower = model.to_lowercase();
        PROVIDERS.iter().find(|spec| {
            if spec.is_gateway || spec.is_local {
                return false;
            }
            spec.keywords.iter().any(|kw| model_lower.contains(kw))
        })
    }

    /// Find gateway/local provider
    /// Priority: 1) provider_name, 2) api_key prefix, 3) api_base keyword
    pub fn find_gateway(
        provider_name: Option<&str>,
        api_key: Option<&str>,
        api_base: Option<&str>,
    ) -> Option<&'static ProviderSpec> {
        // 1. Direct match by config key
        if let Some(name) = provider_name {
            if let Some(spec) = Self::find_by_name(name) {
                if spec.is_gateway || spec.is_local {
                    return Some(spec);
                }
            }
        }

        // 2. Auto-detect by api_key prefix / api_base keyword
        PROVIDERS.iter().find(|spec| {
            if !spec.detect_by_key_prefix.is_empty() {
                if let Some(key) = api_key {
                    if key.starts_with(spec.detect_by_key_prefix) {
                        return true;
                    }
                }
            }
            if !spec.detect_by_base_keyword.is_empty() {
                if let Some(base) = api_base {
                    if base.contains(spec.detect_by_base_keyword) {
                        return true;
                    }
                }
            }
            false
        })
    }

    /// Find provider by config field name
    pub fn find_by_name(name: &str) -> Option<&'static ProviderSpec> {
        PROVIDERS.iter().find(|spec| spec.name == name)
    }

    /// Resolve model name with prefixing logic
    pub fn resolve_model(model: &str, gateway: Option<&ProviderSpec>) -> String {
        if let Some(gw) = gateway {
            // Gateway mode: apply gateway prefix
            let mut resolved = model.to_string();

            // Strip existing prefix if needed
            if gw.strip_model_prefix && resolved.contains('/') {
                resolved = resolved
                    .split('/')
                    .next_back()
                    .unwrap_or(&resolved)
                    .to_string();
            }

            // Add gateway prefix
            if !gw.prefix.is_empty() && !resolved.starts_with(&format!("{}/", gw.prefix)) {
                resolved = format!("{}/{}", gw.prefix, resolved);
            }

            return resolved;
        }

        // Standard mode: auto-prefix for known providers
        if let Some(spec) = Self::find_by_model(model) {
            if !spec.prefix.is_empty() {
                // Check skip_prefixes
                let should_skip = spec.skip_prefixes.iter().any(|p| model.starts_with(p));
                if !should_skip {
                    return format!("{}/{}", spec.prefix, model);
                }
            }
        }

        model.to_string()
    }

    /// Get model-specific parameter overrides
    pub fn get_model_overrides(model: &str) -> HashMap<String, Value> {
        let model_lower = model.to_lowercase();
        let mut overrides = HashMap::new();

        if let Some(spec) = Self::find_by_model(model) {
            for (pattern, params) in spec.model_overrides {
                if model_lower.contains(pattern) {
                    for (key, value) in *params {
                        // Parse value as JSON (numbers and strings)
                        let parsed_value = if let Ok(num) = value.parse::<f64>() {
                            Value::from(num)
                        } else {
                            Value::from(*value)
                        };
                        overrides.insert(key.to_string(), parsed_value);
                    }
                    break;
                }
            }
        }

        overrides
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_by_model_anthropic() {
        let spec = ProviderRegistry::find_by_model("claude-opus-4");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "anthropic");
    }

    #[test]
    fn test_find_by_model_openai() {
        let spec = ProviderRegistry::find_by_model("gpt-4");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "openai");
    }

    #[test]
    fn test_find_by_model_deepseek() {
        let spec = ProviderRegistry::find_by_model("deepseek-chat");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "deepseek");
    }

    #[test]
    fn test_find_by_model_gemini() {
        let spec = ProviderRegistry::find_by_model("gemini-pro");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "gemini");
    }

    #[test]
    fn test_find_by_model_case_insensitive() {
        let spec = ProviderRegistry::find_by_model("CLAUDE-OPUS-4");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "anthropic");
    }

    #[test]
    fn test_find_by_model_unknown() {
        let spec = ProviderRegistry::find_by_model("unknown-model-xyz");
        assert!(spec.is_none());
    }

    #[test]
    fn test_find_by_model_ignores_gateways() {
        // Even though "openrouter" is a provider, it shouldn't match by model name
        let spec = ProviderRegistry::find_by_model("openrouter");
        assert!(spec.is_none());
    }

    #[test]
    fn test_find_by_name_anthropic() {
        let spec = ProviderRegistry::find_by_name("anthropic");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "anthropic");
    }

    #[test]
    fn test_find_by_name_openrouter() {
        let spec = ProviderRegistry::find_by_name("openrouter");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "openrouter");
        assert!(spec.unwrap().is_gateway);
    }

    #[test]
    fn test_find_by_name_vllm() {
        let spec = ProviderRegistry::find_by_name("vllm");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "vllm");
        assert!(spec.unwrap().is_local);
    }

    #[test]
    fn test_find_by_name_unknown() {
        let spec = ProviderRegistry::find_by_name("nonexistent");
        assert!(spec.is_none());
    }

    #[test]
    fn test_find_gateway_by_name() {
        let spec = ProviderRegistry::find_gateway(Some("openrouter"), None, None);
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "openrouter");
    }

    #[test]
    fn test_find_gateway_by_api_key_prefix() {
        let spec = ProviderRegistry::find_gateway(None, Some("sk-or-v1-test"), None);
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "openrouter");
    }

    #[test]
    fn test_find_gateway_by_api_base() {
        let spec = ProviderRegistry::find_gateway(None, None, Some("https://aihubmix.com/v1"));
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "aihubmix");
    }

    #[test]
    fn test_find_gateway_priority_name_over_key() {
        // Should prefer explicit name over key prefix
        let spec = ProviderRegistry::find_gateway(Some("aihubmix"), Some("sk-or-v1-test"), None);
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "aihubmix");
    }

    #[test]
    fn test_find_gateway_non_gateway_returns_none() {
        // Non-gateway providers shouldn't match
        let spec = ProviderRegistry::find_gateway(Some("anthropic"), None, None);
        assert!(spec.is_none());
    }

    #[test]
    fn test_resolve_model_anthropic_no_prefix() {
        let model = ProviderRegistry::resolve_model("claude-opus-4", None);
        assert_eq!(model, "claude-opus-4");
    }

    #[test]
    fn test_resolve_model_deepseek_adds_prefix() {
        let model = ProviderRegistry::resolve_model("deepseek-chat", None);
        assert_eq!(model, "deepseek/deepseek-chat");
    }

    #[test]
    fn test_resolve_model_deepseek_skip_existing_prefix() {
        let model = ProviderRegistry::resolve_model("deepseek/deepseek-chat", None);
        assert_eq!(model, "deepseek/deepseek-chat");
    }

    #[test]
    fn test_resolve_model_gemini_adds_prefix() {
        let model = ProviderRegistry::resolve_model("gemini-pro", None);
        assert_eq!(model, "gemini/gemini-pro");
    }

    #[test]
    fn test_resolve_model_unknown_no_change() {
        let model = ProviderRegistry::resolve_model("unknown-model", None);
        assert_eq!(model, "unknown-model");
    }

    #[test]
    fn test_resolve_model_gateway_adds_prefix() {
        let gateway = ProviderRegistry::find_by_name("openrouter").unwrap();
        let model = ProviderRegistry::resolve_model("claude-opus-4", Some(gateway));
        assert_eq!(model, "openrouter/claude-opus-4");
    }

    #[test]
    fn test_resolve_model_gateway_strips_then_adds_prefix() {
        let gateway = ProviderRegistry::find_by_name("aihubmix").unwrap();
        let model = ProviderRegistry::resolve_model("anthropic/claude-opus-4", Some(gateway));
        assert_eq!(model, "openai/claude-opus-4");
    }

    #[test]
    fn test_get_model_overrides_kimi_temperature() {
        let overrides = ProviderRegistry::get_model_overrides("kimi-k2.5");
        assert!(overrides.contains_key("temperature"));
        assert_eq!(overrides.get("temperature").unwrap().as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_get_model_overrides_none_for_standard_models() {
        let overrides = ProviderRegistry::get_model_overrides("claude-opus-4");
        assert!(overrides.is_empty());
    }

    #[test]
    fn test_get_model_overrides_case_insensitive() {
        let overrides = ProviderRegistry::get_model_overrides("KIMI-K2.5");
        assert!(overrides.contains_key("temperature"));
    }

    #[test]
    fn test_provider_spec_label() {
        let spec = ProviderRegistry::find_by_name("anthropic").unwrap();
        assert_eq!(spec.label(), "Anthropic");

        let spec = ProviderRegistry::find_by_name("openai").unwrap();
        assert_eq!(spec.label(), "OpenAI");
    }

    #[test]
    fn test_all_providers_have_unique_names() {
        let mut names = std::collections::HashSet::new();
        for spec in PROVIDERS {
            assert!(
                names.insert(spec.name),
                "Duplicate provider name: {}",
                spec.name
            );
        }
    }

    #[test]
    fn test_all_providers_have_valid_env_key() {
        for spec in PROVIDERS {
            assert!(
                !spec.env_key.is_empty(),
                "Provider {} missing env_key",
                spec.name
            );
            assert!(
                spec.env_key.ends_with("_API_KEY") || spec.env_key.ends_with("_KEY"),
                "Provider {} has invalid env_key format: {}",
                spec.name,
                spec.env_key
            );
        }
    }

    #[test]
    fn test_all_providers_have_valid_api_base() {
        for spec in PROVIDERS {
            assert!(
                !spec.default_api_base.is_empty(),
                "Provider {} missing default_api_base",
                spec.name
            );
            assert!(
                spec.default_api_base.starts_with("http://")
                    || spec.default_api_base.starts_with("https://"),
                "Provider {} has invalid api_base format: {}",
                spec.name,
                spec.default_api_base
            );
        }
    }

    #[test]
    fn test_gateway_providers_marked_correctly() {
        let openrouter = ProviderRegistry::find_by_name("openrouter").unwrap();
        assert!(openrouter.is_gateway);
        assert!(!openrouter.is_local);

        let aihubmix = ProviderRegistry::find_by_name("aihubmix").unwrap();
        assert!(aihubmix.is_gateway);
    }

    #[test]
    fn test_local_providers_marked_correctly() {
        let vllm = ProviderRegistry::find_by_name("vllm").unwrap();
        assert!(vllm.is_local);
        assert!(!vllm.is_gateway);
    }

    #[test]
    fn test_standard_providers_not_gateway_or_local() {
        let anthropic = ProviderRegistry::find_by_name("anthropic").unwrap();
        assert!(!anthropic.is_gateway);
        assert!(!anthropic.is_local);

        let openai = ProviderRegistry::find_by_name("openai").unwrap();
        assert!(!openai.is_gateway);
        assert!(!openai.is_local);
    }
}
