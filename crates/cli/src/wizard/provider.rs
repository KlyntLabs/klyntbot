//! Enhanced provider configuration wizard module.
//!
//! Guides the user through selecting an LLM provider, entering API
//! credentials, choosing a model, and optionally setting a custom API base.

use anyhow::Result;
use common::utils::terminal::*;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts::{self, SelectOption};

/// Provider metadata used to drive the selection UI and configuration.
struct ProviderInfo {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    api_url: &'static str,
    default_model: &'static str,
    key_prefix: &'static str,
}

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        name: "Anthropic (Claude)",
        key: "anthropic",
        description: "Recommended for best quality",
        api_url: "https://console.anthropic.com",
        default_model: "claude-sonnet-4-5",
        key_prefix: "sk-ant-",
    },
    ProviderInfo {
        name: "OpenAI (GPT)",
        key: "openai",
        description: "Industry standard models",
        api_url: "https://platform.openai.com/api-keys",
        default_model: "gpt-4o",
        key_prefix: "sk-",
    },
    ProviderInfo {
        name: "DeepSeek",
        key: "deepseek",
        description: "Cost-effective alternative",
        api_url: "https://platform.deepseek.com",
        default_model: "deepseek-chat",
        key_prefix: "sk-",
    },
    ProviderInfo {
        name: "Google (Gemini)",
        key: "gemini",
        description: "Multimodal capabilities",
        api_url: "https://makersuite.google.com/app/apikey",
        default_model: "gemini-2.0-flash",
        key_prefix: "",
    },
    ProviderInfo {
        name: "OpenRouter",
        key: "openrouter",
        description: "Access to many models via unified API",
        api_url: "https://openrouter.ai/keys",
        default_model: "openrouter/auto",
        key_prefix: "sk-or-",
    },
    ProviderInfo {
        name: "Groq",
        key: "groq",
        description: "Ultra-fast inference",
        api_url: "https://console.groq.com/keys",
        default_model: "llama-3.3-70b-versatile",
        key_prefix: "gsk_",
    },
];

pub struct ProviderModule;

impl WizardModule for ProviderModule {
    fn name(&self) -> &str {
        "LLM Provider"
    }

    fn description(&self) -> &str {
        "Configure your AI model provider and API key"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let chars = BoxChars::get();

        // Check if already configured (reconfiguration scenario)
        let active = state.config.active_provider_name();
        if active != "none" {
            println!(
                "{} {} Current provider: {}",
                colorize(chars.vertical, BRAND),
                colorize("✓", SUCCESS),
                colorize(active, BOLD)
            );
            println!("{}", colorize(chars.vertical, BRAND));
            let reconfigure = prompts::prompt_yes_no("Reconfigure provider?", false)?;
            if !reconfigure {
                println!("{}", draw_step_footer());
                return Ok(StepResult::Next);
            }
            println!("{}", colorize(chars.vertical, BRAND));
        }

        // Step 1: Select provider
        let options: Vec<SelectOption<'_>> = PROVIDERS
            .iter()
            .map(|p| SelectOption {
                label: p.name,
                description: p.description,
            })
            .collect();

        println!(
            "{}",
            draw_step_line(&colorize("Choose Your LLM Provider", BOLD))
        );
        println!("{}", colorize(chars.vertical, BRAND));
        let idx = prompts::prompt_select("Select a provider", &options, 0)?;
        let provider = &PROVIDERS[idx];

        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} {}",
            colorize(chars.vertical, BRAND),
            colorize("●", BRAND),
            colorize(&format!("Selected: {}", provider.name), BOLD)
        );

        // Step 2: API key
        println!("{}", colorize(chars.vertical, BRAND));
        println!("{}", draw_step_line(&colorize("API Configuration", BOLD)));
        println!(
            "{} {} Get your API key at: {}",
            colorize(chars.vertical, BRAND),
            colorize("→", BRAND),
            colorize(provider.api_url, UNDERLINE)
        );
        println!("{}", colorize(chars.vertical, BRAND));

        let api_key = prompts::prompt_secret("API Key", 10)?;

        println!("{}", colorize(chars.vertical, BRAND));

        // Validate key prefix if known
        if !provider.key_prefix.is_empty() && !api_key.starts_with(provider.key_prefix) {
            println!(
                "{} {} {} keys usually start with '{}' - please verify",
                colorize(chars.vertical, BRAND),
                colorize("⚠", WARNING),
                provider.name,
                provider.key_prefix
            );
        } else if !provider.key_prefix.is_empty() {
            println!(
                "{} {} {}",
                colorize(chars.vertical, BRAND),
                colorize("✓", SUCCESS),
                colorize("API key format validated", DIM)
            );
        }

        state.config.set_provider_key(provider.key, api_key);

        // Step 3: Model selection
        println!("{}", colorize(chars.vertical, BRAND));
        let model = prompts::prompt_text("Model", Some(provider.default_model), true)?;
        state.config.agents.defaults.model = model.clone();

        // Step 4: Custom API base (optional, for proxies/self-hosted)
        println!("{}", colorize(chars.vertical, BRAND));
        let custom_base = prompts::prompt_yes_no(
            "Use a custom API base URL? (for proxies/self-hosted)",
            false,
        )?;

        if custom_base {
            println!("{}", colorize(chars.vertical, BRAND));
            let base_url = prompts::prompt_text("API Base URL", None, true)?;
            match provider.key {
                "anthropic" => {
                    state.config.providers.anthropic.api_base = Some(base_url);
                }
                "openai" => {
                    state.config.providers.openai.api_base = Some(base_url);
                }
                "deepseek" => {
                    state.config.providers.deepseek.api_base = Some(base_url);
                }
                "gemini" => {
                    state.config.providers.gemini.api_base = Some(base_url);
                }
                "openrouter" => {
                    state.config.providers.openrouter.api_base = Some(base_url);
                }
                "groq" => {
                    state.config.providers.groq.api_base = Some(base_url);
                }
                _ => {}
            }
            println!("{}", colorize(chars.vertical, BRAND));
            println!(
                "{} {} Custom API base configured",
                colorize(chars.vertical, BRAND),
                status_success()
            );
        }

        // Success summary with vertical line
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} Provider: {} with model {}",
            colorize(chars.vertical, BRAND),
            colorize("✓", SUCCESS),
            colorize(provider.name, BOLD),
            colorize(&model, DIM)
        );
        println!("{}", draw_step_footer());

        Ok(StepResult::Next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_providers_list() {
        // All providers should have non-empty fields
        for provider in PROVIDERS {
            assert!(!provider.name.is_empty());
            assert!(!provider.key.is_empty());
            assert!(!provider.description.is_empty());
            assert!(!provider.api_url.is_empty());
            assert!(!provider.default_model.is_empty());
        }
    }

    #[test]
    fn test_provider_keys_unique() {
        let keys: Vec<&str> = PROVIDERS.iter().map(|p| p.key).collect();
        for (i, key) in keys.iter().enumerate() {
            assert!(
                !keys[i + 1..].contains(key),
                "Duplicate provider key: {}",
                key
            );
        }
    }

    #[test]
    fn test_provider_module_metadata() {
        let module = ProviderModule;
        assert_eq!(module.name(), "LLM Provider");
        assert!(module.is_required());
    }

    #[test]
    fn test_provider_module_is_applicable() {
        let module = ProviderModule;
        let state = WizardState::new();
        assert!(module.is_applicable(&state));
    }

    #[test]
    fn test_anthropic_is_first_and_recommended() {
        assert_eq!(PROVIDERS[0].key, "anthropic");
        assert!(PROVIDERS[0].description.contains("Recommended"));
    }

    #[test]
    fn test_provider_api_urls_are_https() {
        for provider in PROVIDERS {
            assert!(
                provider.api_url.starts_with("https://"),
                "Provider {} API URL should be HTTPS: {}",
                provider.name,
                provider.api_url
            );
        }
    }

    #[test]
    fn test_expected_provider_keys_present() {
        let keys: Vec<&str> = PROVIDERS.iter().map(|p| p.key).collect();
        assert!(keys.contains(&"anthropic"));
        assert!(keys.contains(&"openai"));
        assert!(keys.contains(&"deepseek"));
        assert!(keys.contains(&"gemini"));
        assert!(keys.contains(&"openrouter"));
        assert!(keys.contains(&"groq"));
    }

    #[test]
    fn test_provider_count() {
        assert_eq!(PROVIDERS.len(), 6);
    }

    #[test]
    fn test_anthropic_default_model() {
        let anthropic = &PROVIDERS[0];
        assert_eq!(anthropic.default_model, "claude-sonnet-4-5");
        assert_eq!(anthropic.key_prefix, "sk-ant-");
    }

    #[test]
    fn test_openai_default_model() {
        let openai = PROVIDERS.iter().find(|p| p.key == "openai").unwrap();
        assert_eq!(openai.default_model, "gpt-4o");
    }

    #[test]
    fn test_groq_default_model() {
        let groq = PROVIDERS.iter().find(|p| p.key == "groq").unwrap();
        assert_eq!(groq.default_model, "llama-3.3-70b-versatile");
        assert_eq!(groq.key_prefix, "gsk_");
    }
}
