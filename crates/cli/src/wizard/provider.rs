//! Multi-provider configuration wizard module.
//!
//! Presents a management loop where users can configure multiple LLM providers,
//! set API keys, choose an active provider, and select a model.
//! On reconfiguration, shows existing providers with status.

use anyhow::Result;
use common::utils::terminal::*;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts::{self, mask_secret, SelectOption};

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
        "Configure your AI model providers and API keys"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let chars = BoxChars::get();

        // On fresh install with no providers, go straight to configure
        if !has_any_provider_configured(&state.config) {
            println!(
                "{}",
                draw_step_line(&colorize("Configure Your First LLM Provider", BOLD))
            );
            println!("{}", colorize(chars.vertical, BRAND));
            configure_provider_flow(state)?;

            // Auto-set as active
            let active = state.config.active_provider_name().to_string();
            if active != "none" {
                state.config.agents.defaults.provider = Some(active.clone());
            }

            // Ask for model
            println!("{}", colorize(chars.vertical, BRAND));
            prompt_model_selection(state)?;

            println!("{}", colorize(chars.vertical, BRAND));
            println!(
                "{} {} Provider configured. You can add more providers later with {}.",
                colorize(chars.vertical, BRAND),
                colorize("✓", SUCCESS),
                colorize("klyntbot init", BOLD)
            );
            println!("{}", draw_step_footer());
            return Ok(StepResult::Next);
        }

        // Management loop for existing configuration
        loop {
            let active = state
                .config
                .agents
                .defaults
                .provider
                .clone()
                .unwrap_or_else(|| state.config.active_provider_name().to_string());

            // Show provider status list
            println!("{}", draw_step_line(&colorize("Provider Management", BOLD)));
            println!("{}", colorize(chars.vertical, BRAND));

            if active != "none" {
                let model = &state.config.agents.defaults.model;
                println!(
                    "{} {} Active: {} ({})",
                    colorize(chars.vertical, BRAND),
                    colorize("★", HIGHLIGHT),
                    colorize(&active, BOLD),
                    colorize(model, DIM)
                );
                println!("{}", colorize(chars.vertical, BRAND));
            }

            print_provider_status_list(&state.config, chars);
            println!("{}", colorize(chars.vertical, BRAND));

            // Menu
            let menu_options = vec![
                SelectOption {
                    label: "Add / edit a provider",
                    description: "Configure API key and settings",
                },
                SelectOption {
                    label: "Switch active provider",
                    description: "Choose which provider to use",
                },
                SelectOption {
                    label: "Set model",
                    description: "Change the default model",
                },
                SelectOption {
                    label: "Done",
                    description: "Finish provider setup",
                },
            ];

            let choice = prompts::prompt_select("Action", &menu_options, 3)?; // Default to "Done"

            match choice {
                0 => {
                    // Configure a provider
                    println!("{}", colorize(chars.vertical, BRAND));
                    configure_provider_flow(state)?;
                }
                1 => {
                    // Switch active provider
                    println!("{}", colorize(chars.vertical, BRAND));
                    set_active_provider(state)?;
                }
                2 => {
                    // Set model
                    println!("{}", colorize(chars.vertical, BRAND));
                    prompt_model_selection(state)?;
                }
                3 => {
                    // Done
                    if has_any_provider_configured(&state.config) {
                        // Ensure active provider is set
                        if state.config.agents.defaults.provider.is_none() {
                            let detected = state.config.active_provider_name().to_string();
                            if detected != "none" {
                                state.config.agents.defaults.provider = Some(detected);
                            }
                        }

                        println!("{}", colorize(chars.vertical, BRAND));
                        let active_name = state.config.active_provider_name();
                        let model = &state.config.agents.defaults.model;
                        println!(
                            "{} {} Provider: {} with model {}",
                            colorize(chars.vertical, BRAND),
                            colorize("✓", SUCCESS),
                            colorize(active_name, BOLD),
                            colorize(model, DIM)
                        );
                        println!("{}", draw_step_footer());
                        return Ok(StepResult::Next);
                    }

                    println!(
                        "{} {} At least one provider must be configured.",
                        colorize(chars.vertical, BRAND),
                        colorize("⚠", WARNING)
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}

/// Display the status of all providers (configured or not).
fn print_provider_status_list(config: &config::Config, chars: &BoxChars) {
    let active = config
        .agents
        .defaults
        .provider
        .as_deref()
        .unwrap_or_else(|| config.active_provider_name());

    for provider in PROVIDERS {
        let key = get_provider_key(config, provider.key);
        let configured = !key.is_empty();
        let is_active = provider.key == active;

        let icon = if is_active {
            colorize("★", HIGHLIGHT)
        } else if configured {
            colorize("✓", SUCCESS)
        } else {
            colorize("○", DIM)
        };

        let status = if configured {
            let masked = mask_secret(&key);
            format!(
                "{} {}",
                provider.name,
                colorize(&format!("({})", masked), DIM)
            )
        } else {
            format!("{} {}", provider.name, colorize("— not configured", DIM))
        };

        println!("{}  {}  {}", colorize(chars.vertical, BRAND), icon, status);
    }
}

/// Configure a specific provider: select which one, enter API key, optional base URL.
fn configure_provider_flow(state: &mut WizardState) -> Result<()> {
    let chars = BoxChars::get();

    // Select which provider to configure
    let options: Vec<SelectOption<'_>> = PROVIDERS
        .iter()
        .map(|p| {
            let key = get_provider_key(&state.config, p.key);
            let desc = if key.is_empty() {
                p.description
            } else {
                "configured — edit to update"
            };
            SelectOption {
                label: p.name,
                description: desc,
            }
        })
        .collect();

    println!(
        "{}",
        draw_step_line(&colorize("Select Provider to Configure", BOLD))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    let idx = prompts::prompt_select("Provider", &options, 0)?;
    let provider = &PROVIDERS[idx];

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Configuring {}",
        colorize(chars.vertical, BRAND),
        colorize("●", BRAND),
        colorize(provider.name, BOLD)
    );

    // API key
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Get your API key at: {}",
        colorize(chars.vertical, BRAND),
        colorize("→", BRAND),
        colorize(provider.api_url, UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    let existing_key = get_provider_key(&state.config, provider.key);
    let api_key = if existing_key.is_empty() {
        prompts::prompt_secret("API Key", 10)?
    } else {
        match prompts::prompt_secret_with_existing("API Key", &existing_key, 10)? {
            Some(new_key) => new_key,
            None => existing_key.clone(),
        }
    };

    println!("{}", colorize(chars.vertical, BRAND));

    // Validate key prefix
    if !provider.key_prefix.is_empty() && !api_key.starts_with(provider.key_prefix) {
        println!(
            "{} {} {} keys usually start with '{}' — please verify",
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

    // Custom API base (optional)
    let existing_base = get_provider_api_base(&state.config, provider.key);
    println!("{}", colorize(chars.vertical, BRAND));

    if let Some(ref base) = existing_base {
        println!(
            "{} Current API base: {}",
            colorize(chars.vertical, BRAND),
            colorize(base, DIM)
        );
    }

    let custom_base = prompts::prompt_yes_no(
        "Use a custom API base URL? (for proxies/self-hosted)",
        existing_base.is_some(),
    )?;

    if custom_base {
        println!("{}", colorize(chars.vertical, BRAND));
        let base_url = prompts::prompt_text("API Base URL", existing_base.as_deref(), true)?;
        set_provider_api_base(&mut state.config, provider.key, Some(base_url));
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} Custom API base configured",
            colorize(chars.vertical, BRAND),
            status_success()
        );
    } else if existing_base.is_some() {
        set_provider_api_base(&mut state.config, provider.key, None);
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} {} configured successfully",
        colorize(chars.vertical, BRAND),
        colorize("✓", SUCCESS),
        colorize(provider.name, BOLD)
    );

    Ok(())
}

/// Set the active provider from configured providers.
fn set_active_provider(state: &mut WizardState) -> Result<()> {
    let chars = BoxChars::get();

    let configured: Vec<&ProviderInfo> = PROVIDERS
        .iter()
        .filter(|p| !get_provider_key(&state.config, p.key).is_empty())
        .collect();

    if configured.is_empty() {
        println!(
            "{} {} No providers configured yet. Add one first.",
            colorize(chars.vertical, BRAND),
            colorize("⚠", WARNING)
        );
        return Ok(());
    }

    let options: Vec<SelectOption<'_>> = configured
        .iter()
        .map(|p| SelectOption {
            label: p.name,
            description: p.description,
        })
        .collect();

    let current_active = state
        .config
        .agents
        .defaults
        .provider
        .as_deref()
        .unwrap_or_else(|| state.config.active_provider_name());
    let default_idx = configured
        .iter()
        .position(|p| p.key == current_active)
        .unwrap_or(0);

    let idx = prompts::prompt_select("Active provider", &options, default_idx)?;
    let selected = configured[idx];

    state.config.agents.defaults.provider = Some(selected.key.to_string());

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} {} set as active provider",
        colorize(chars.vertical, BRAND),
        colorize("★", HIGHLIGHT),
        colorize(selected.name, BOLD)
    );

    Ok(())
}

/// Prompt for model selection with current value as default.
fn prompt_model_selection(state: &mut WizardState) -> Result<()> {
    let active_key = state
        .config
        .agents
        .defaults
        .provider
        .as_deref()
        .unwrap_or_else(|| state.config.active_provider_name());

    let provider_default = PROVIDERS
        .iter()
        .find(|p| p.key == active_key)
        .map(|p| p.default_model)
        .unwrap_or("claude-sonnet-4-5");

    let current_model = &state.config.agents.defaults.model;
    let model_default = if current_model.is_empty() {
        provider_default
    } else {
        current_model
    };

    let model = prompts::prompt_text("Model", Some(model_default), true)?;
    state.config.agents.defaults.model = model;
    Ok(())
}

/// Check if any provider has an API key configured.
fn has_any_provider_configured(config: &config::Config) -> bool {
    PROVIDERS
        .iter()
        .any(|p| !get_provider_key(config, p.key).is_empty())
}

/// Get the current API key for a provider.
fn get_provider_key(config: &config::Config, provider_key: &str) -> String {
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
fn get_provider_api_base(config: &config::Config, provider_key: &str) -> Option<String> {
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
fn set_provider_api_base(config: &mut config::Config, provider_key: &str, base: Option<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_providers_list() {
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

    #[test]
    fn test_get_provider_key_anthropic() {
        let mut config = config::Config::default();
        config.set_provider_key("anthropic", "sk-ant-test123".to_string());
        assert_eq!(get_provider_key(&config, "anthropic"), "sk-ant-test123");
    }

    #[test]
    fn test_get_provider_key_unknown() {
        let config = config::Config::default();
        assert_eq!(get_provider_key(&config, "unknown"), "");
    }

    #[test]
    fn test_get_provider_api_base_none() {
        let config = config::Config::default();
        assert!(get_provider_api_base(&config, "anthropic").is_none());
    }

    #[test]
    fn test_set_provider_api_base() {
        let mut config = config::Config::default();
        set_provider_api_base(
            &mut config,
            "anthropic",
            Some("https://proxy.example.com".to_string()),
        );
        assert_eq!(
            config.providers.anthropic.api_base,
            Some("https://proxy.example.com".to_string())
        );
    }

    #[test]
    fn test_has_any_provider_configured_empty() {
        let config = config::Config::default();
        assert!(!has_any_provider_configured(&config));
    }

    #[test]
    fn test_has_any_provider_configured_with_key() {
        let mut config = config::Config::default();
        config.set_provider_key("deepseek", "sk-test".to_string());
        assert!(has_any_provider_configured(&config));
    }

    #[test]
    fn test_has_any_provider_configured_multiple() {
        let mut config = config::Config::default();
        config.set_provider_key("anthropic", "sk-ant-test".to_string());
        config.set_provider_key("openai", "sk-openai-test".to_string());
        assert!(has_any_provider_configured(&config));
    }
}
