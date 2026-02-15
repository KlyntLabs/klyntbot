//! Multi-provider configuration wizard module.
//!
//! Presents a management loop where users can configure multiple LLM providers,
//! set API keys, choose an active provider, and select a model.
//! On reconfiguration, shows existing providers with status.

pub(crate) mod detection;
mod menus;

use anyhow::Result;
use common::utils::terminal::*;
use std::io::{self, IsTerminal};

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts::{self, mask_secret};
use detection::{
    get_provider_api_base, get_provider_key, has_any_provider_configured, set_provider_api_base,
    PROVIDERS,
};

/// Actions available in the expanded provider sub-menu.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SubAction {
    EditApiKey,
    SetActive,
    ChangeModel,
    CustomBaseUrl,
    Close,
}

pub(crate) const SUB_ACTIONS: &[SubAction] = &[
    SubAction::EditApiKey,
    SubAction::SetActive,
    SubAction::ChangeModel,
    SubAction::CustomBaseUrl,
    SubAction::Close,
];

impl SubAction {
    pub(crate) fn label(
        &self,
        config: &config::Config,
        provider: &detection::ProviderInfo,
    ) -> String {
        match self {
            SubAction::EditApiKey => {
                let key = get_provider_key(config, provider.key);
                if key.is_empty() {
                    "Set API key".to_string()
                } else {
                    format!("Edit API key ({})", mask_secret(&key))
                }
            }
            SubAction::SetActive => "Set as active provider".to_string(),
            SubAction::ChangeModel => "Change model".to_string(),
            SubAction::CustomBaseUrl => {
                let base = get_provider_api_base(config, provider.key);
                match base {
                    Some(url) => format!("Custom base URL ({})", url),
                    None => "Custom base URL".to_string(),
                }
            }
            SubAction::Close => "Close".to_string(),
        }
    }
}

/// State for the interactive provider menu.
pub(crate) struct MenuState {
    pub cursor: usize,
    pub expanded: Option<usize>,
    pub sub_cursor: usize,
    pub in_sub_menu: bool,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    pub fn total_main_items(&self) -> usize {
        PROVIDERS.len() + 1 // providers + "Done"
    }

    pub fn is_on_done(&self) -> bool {
        self.cursor == PROVIDERS.len()
    }
}

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

        // Header
        let header = if has_any_provider_configured(&state.config) {
            "Provider Management"
        } else {
            "Configure Your First LLM Provider"
        };
        println!("{}", draw_step_line(&colorize(header, BOLD)));
        println!("{}", colorize(chars.vertical, BRAND));

        // Show active provider info
        let active = state
            .config
            .agents
            .defaults
            .provider
            .clone()
            .unwrap_or_else(|| state.config.active_provider_name().to_string());
        if active != "none" && has_any_provider_configured(&state.config) {
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

        // Run interactive menu
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            menus::run_provider_menu(state)?;
        } else {
            menus::run_provider_menu_fallback(state)?;
        }

        // Ensure active provider is set
        if state.config.agents.defaults.provider.is_none() {
            let detected = state.config.active_provider_name().to_string();
            if detected != "none" {
                state.config.agents.defaults.provider = Some(detected);
            }
        }

        // Final confirmation
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
        Ok(StepResult::Next)
    }
}

// ============================================================================
// Sub-action executors
// ============================================================================

pub(crate) fn execute_edit_api_key(state: &mut WizardState, provider_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let provider = &PROVIDERS[provider_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Configuring {}",
        prefix,
        colorize("●", BRAND),
        colorize(provider.name, BOLD)
    );
    println!(
        "{}{} Get your API key at: {}",
        prefix,
        colorize("→", BRAND),
        colorize(provider.api_url, UNDERLINE)
    );

    let existing_key = get_provider_key(&state.config, provider.key);
    let api_key = if existing_key.is_empty() {
        prompts::prompt_secret("API Key", 10)?
    } else {
        match prompts::prompt_secret_with_existing("API Key", &existing_key, 10)? {
            Some(new_key) => new_key,
            None => existing_key.clone(),
        }
    };

    // Validate key prefix
    if !provider.key_prefix.is_empty() && !api_key.starts_with(provider.key_prefix) {
        println!(
            "{}{} {} keys usually start with '{}' — please verify",
            prefix,
            colorize("⚠", WARNING),
            provider.name,
            provider.key_prefix
        );
    } else if !provider.key_prefix.is_empty() {
        println!(
            "{}{} {}",
            prefix,
            colorize("✓", SUCCESS),
            colorize("API key format validated", DIM)
        );
    }

    state.config.set_provider_key(provider.key, api_key);

    println!(
        "{}{} {} configured",
        prefix,
        colorize("✓", SUCCESS),
        colorize(provider.name, BOLD)
    );

    Ok(())
}

pub(crate) fn execute_change_model(state: &mut WizardState, provider_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let provider = &PROVIDERS[provider_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Select model for {}",
        prefix,
        colorize("●", BRAND),
        colorize(provider.name, BOLD)
    );

    let current_model = &state.config.agents.defaults.model;

    let options: Vec<prompts::SelectOption<'_>> = provider
        .models
        .iter()
        .map(|m| {
            let desc = if *m == provider.default_model {
                "default"
            } else {
                ""
            };
            prompts::SelectOption {
                label: m,
                description: desc,
            }
        })
        .collect();

    let default_idx = provider
        .models
        .iter()
        .position(|m| *m == current_model.as_str())
        .unwrap_or(0);

    let idx = prompts::prompt_select("Model", &options, default_idx)?;
    state.config.agents.defaults.model = provider.models[idx].to_string();

    Ok(())
}

pub(crate) fn execute_custom_base_url(state: &mut WizardState, provider_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let provider = &PROVIDERS[provider_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    let existing_base = get_provider_api_base(&state.config, provider.key);

    if let Some(ref base) = existing_base {
        println!("{}Current: {}", prefix, colorize(base, DIM));
    }

    let wants_custom = prompts::prompt_yes_no(
        "Use a custom API base URL? (for proxies/self-hosted)",
        existing_base.is_some(),
    )?;

    if wants_custom {
        let base_url = prompts::prompt_text("API Base URL", existing_base.as_deref(), true)?;
        set_provider_api_base(&mut state.config, provider.key, Some(base_url));
        println!(
            "{}{} Custom API base configured",
            prefix,
            colorize("✓", SUCCESS)
        );
    } else if existing_base.is_some() {
        set_provider_api_base(&mut state.config, provider.key, None);
        println!(
            "{}{} Custom API base removed",
            prefix,
            colorize("✓", SUCCESS)
        );
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

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
            assert!(
                !provider.models.is_empty(),
                "Provider {} has no models",
                provider.name
            );
            assert!(
                provider.models.contains(&provider.default_model),
                "Provider {} default_model not in models list",
                provider.name
            );
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
        assert_eq!(anthropic.default_model, "claude-haiku-4-5");
        assert_eq!(anthropic.key_prefix, "sk-ant-");
    }

    #[test]
    fn test_openai_default_model() {
        let openai = PROVIDERS.iter().find(|p| p.key == "openai").unwrap();
        assert_eq!(openai.default_model, "o4-mini");
    }

    #[test]
    fn test_groq_default_model() {
        let groq = PROVIDERS.iter().find(|p| p.key == "groq").unwrap();
        assert_eq!(groq.default_model, "llama-3.1-8b-instant");
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
