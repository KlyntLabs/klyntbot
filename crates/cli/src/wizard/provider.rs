//! Multi-provider configuration wizard module.
//!
//! Presents a management loop where users can configure multiple LLM providers,
//! set API keys, choose an active provider, and select a model.
//! On reconfiguration, shows existing providers with status.

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
};
use std::io::{self, IsTerminal, Write};

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts::{self, mask_secret};

/// Provider metadata used to drive the selection UI and configuration.
struct ProviderInfo {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    api_url: &'static str,
    default_model: &'static str,
    key_prefix: &'static str,
    models: &'static [&'static str],
}

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        name: "Anthropic (Claude)",
        key: "anthropic",
        description: "Recommended for best quality",
        api_url: "https://console.anthropic.com",
        default_model: "claude-haiku-4-5",
        key_prefix: "sk-ant-",
        models: &["claude-opus-4-6", "claude-sonnet-4-5", "claude-haiku-4-5", "claude-opus-4-5"],
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
        models: &["gemini-3-pro-preview", "gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.5-flash-lite"],
    },
    ProviderInfo {
        name: "OpenRouter",
        key: "openrouter",
        description: "Access to many models via unified API",
        api_url: "https://openrouter.ai/keys",
        default_model: "openrouter/free",
        key_prefix: "sk-or-",
        models: &["openrouter/auto", "openrouter/free", "anthropic/claude-opus-4-6", "openai/o3"],
    },
    ProviderInfo {
        name: "Groq",
        key: "groq",
        description: "Ultra-fast inference",
        api_url: "https://console.groq.com/keys",
        default_model: "llama-3.1-8b-instant",
        key_prefix: "gsk_",
        models: &["llama-4-scout", "llama-4-maverick", "llama-3.3-70b-versatile", "llama-3.1-8b-instant", "gemma2-9b-it"],
    },
];

/// Actions available in the expanded provider sub-menu.
#[derive(Clone, Copy, PartialEq)]
enum SubAction {
    EditApiKey,
    SetActive,
    ChangeModel,
    CustomBaseUrl,
    Close,
}

const SUB_ACTIONS: &[SubAction] = &[
    SubAction::EditApiKey,
    SubAction::SetActive,
    SubAction::ChangeModel,
    SubAction::CustomBaseUrl,
    SubAction::Close,
];

impl SubAction {
    fn label(&self, config: &config::Config, provider: &ProviderInfo) -> String {
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
struct MenuState {
    cursor: usize,            // 0..PROVIDERS.len() for providers, PROVIDERS.len() for "Done"
    expanded: Option<usize>,  // Which provider index is expanded
    sub_cursor: usize,        // Position in SUB_ACTIONS (0..4)
    in_sub_menu: bool,        // Whether navigating inside the sub-menu
}

impl MenuState {
    fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    fn total_main_items(&self) -> usize {
        PROVIDERS.len() + 1 // providers + "Done"
    }

    fn is_on_done(&self) -> bool {
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
            run_provider_menu(state)?;
        } else {
            // Non-TTY fallback: keep the old linear flow
            run_provider_menu_fallback(state)?;
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

fn run_provider_menu_fallback(state: &mut WizardState) -> Result<()> {
    // Simple numbered menu for CI/piped environments
    loop {
        let options: Vec<prompts::SelectOption<'_>> = PROVIDERS
            .iter()
            .map(|p| prompts::SelectOption {
                label: p.name,
                description: p.description,
            })
            .chain(std::iter::once(prompts::SelectOption {
                label: "Done",
                description: "finish provider setup",
            }))
            .collect();

        let idx = prompts::prompt_select(
            "Select provider to configure (or Done)",
            &options,
            options.len() - 1,
        )?;

        if idx == PROVIDERS.len() {
            if has_any_provider_configured(&state.config) {
                return Ok(());
            }
            println!("At least one provider must be configured.");
            continue;
        }

        execute_edit_api_key(state, idx)?;

        // Ask to set active
        if prompts::prompt_yes_no("Set as active provider?", true)? {
            state.config.agents.defaults.provider = Some(PROVIDERS[idx].key.to_string());
            execute_change_model(state, idx)?;
        }
    }
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

fn read_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Err(anyhow::anyhow!("Ctrl+C"));
            }
            return Ok(key);
        }
    }
}

/// Erase `n` lines above cursor.
fn erase_lines(n: usize) -> Result<()> {
    let mut out = io::stdout();
    for _ in 0..n {
        crossterm::execute!(out, cursor::MoveUp(1), terminal::Clear(ClearType::CurrentLine))?;
    }
    Ok(())
}

fn execute_edit_api_key(state: &mut WizardState, provider_idx: usize) -> Result<()> {
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

fn execute_change_model(state: &mut WizardState, provider_idx: usize) -> Result<()> {
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

    // Find current model in list, default to first (which is the default_model)
    let default_idx = provider
        .models
        .iter()
        .position(|m| *m == current_model.as_str())
        .unwrap_or(0);

    let idx = prompts::prompt_select("Model", &options, default_idx)?;
    state.config.agents.defaults.model = provider.models[idx].to_string();

    Ok(())
}

fn execute_custom_base_url(state: &mut WizardState, provider_idx: usize) -> Result<()> {
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

/// Run the interactive provider menu. Returns when user selects "Done".
fn run_provider_menu(state: &mut WizardState) -> Result<()> {
    let chars = BoxChars::get();
    let mut menu = MenuState::new();
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
    render_menu_hint(&mut out, &menu, chars)?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !menu.in_sub_menu => {
                if menu.cursor > 0 {
                    menu.cursor -= 1;
                    // If we moved away from expanded provider, collapse it
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                if menu.sub_cursor > 0 {
                    menu.sub_cursor -= 1;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                if menu.sub_cursor < SUB_ACTIONS.len() - 1 {
                    menu.sub_cursor += 1;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_done() {
                    // Done — exit if any provider configured
                    if has_any_provider_configured(&state.config) {
                        // Erase menu
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;
                        return Ok(());
                    }
                    // Otherwise do nothing (can't exit without a provider)
                } else {
                    // Expand provider
                    menu.expanded = Some(menu.cursor);
                    menu.in_sub_menu = true;
                    menu.sub_cursor = 0;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Enter if menu.in_sub_menu => {
                let provider_idx = menu.expanded.unwrap();
                let action = SUB_ACTIONS[menu.sub_cursor];

                match action {
                    SubAction::SetActive => {
                        // Immediate — set active and collapse
                        let provider = &PROVIDERS[provider_idx];
                        state.config.agents.defaults.provider = Some(provider.key.to_string());

                        // Also set default model if not already set or switching providers
                        state.config.agents.defaults.model = provider.default_model.to_string();

                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                    }
                    SubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                    }
                    SubAction::EditApiKey => {
                        // Exit raw mode, erase menu, run prompt, re-enter
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_edit_api_key(state, provider_idx)?;

                        // Re-enter raw mode and redraw
                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    SubAction::ChangeModel => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_model(state, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    SubAction::CustomBaseUrl => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_custom_base_url(state, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
            }
            _ => {}
        }
    }
}

/// Render the full provider menu list. Returns total lines rendered.
fn render_provider_menu(
    out: &mut impl Write,
    config: &config::Config,
    menu: &MenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let active_provider = config
        .agents
        .defaults
        .provider
        .as_deref()
        .unwrap_or_else(|| config.active_provider_name());
    let mut lines = 0;

    for (i, provider) in PROVIDERS.iter().enumerate() {
        let key = get_provider_key(config, provider.key);
        let configured = !key.is_empty();
        let is_active = provider.key == active_provider;
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        // Provider icon
        let icon = if is_active {
            colorize("★", HIGHLIGHT)
        } else if configured {
            colorize("✓", SUCCESS)
        } else {
            colorize("○", DIM)
        };

        // Expand indicator
        let expand = if is_expanded { "▼" } else { " " };
        let expand_colored = if is_expanded {
            colorize(expand, BRAND)
        } else {
            " ".to_string()
        };

        // Cursor indicator
        let pointer = if is_cursor {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };

        // Provider name + status
        let name = if is_cursor || is_expanded {
            colorize(provider.name, BOLD)
        } else if !configured {
            colorize(provider.name, DIM)
        } else {
            provider.name.to_string()
        };

        let status = if configured {
            format!(" {}", colorize(&format!("({})", mask_secret(&key)), DIM))
        } else {
            format!(" {}", colorize("— not configured", DIM))
        };

        write!(
            out,
            "{}{}{} {} {}{}\r\n",
            prefix, pointer, expand_colored, icon, name, status
        )?;
        lines += 1;

        // Render sub-menu if expanded
        if is_expanded {
            for (si, action) in SUB_ACTIONS.iter().enumerate() {
                let sub_pointer = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize("❯", BRAND)
                } else {
                    " ".to_string()
                };

                let label = action.label(config, provider);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize(&label, BOLD)
                } else if *action == SubAction::Close {
                    colorize(&format!("── {} ──", label), DIM)
                } else {
                    label
                };

                write!(out, "{}      {} {}\r\n", prefix, sub_pointer, label_display)?;
                lines += 1;
            }
        }
    }

    // Separator before Done
    write!(
        out,
        "{}  {}\r\n",
        prefix,
        colorize("──────────────────────────", DIM)
    )?;
    lines += 1;

    // Done row
    let done_available = has_any_provider_configured(config);
    let done_pointer = if !menu.in_sub_menu && menu.is_on_done() {
        colorize("❯", BRAND)
    } else {
        " ".to_string()
    };
    let done_icon = if done_available {
        colorize("●", BRAND)
    } else {
        colorize("○", DIM)
    };
    let done_label = if done_available {
        if !menu.in_sub_menu && menu.is_on_done() {
            colorize("Done", BOLD)
        } else {
            "Done".to_string()
        }
    } else {
        colorize("Done", DIM).to_string()
    };
    let done_desc = if done_available {
        colorize("— finish provider setup", DIM)
    } else {
        colorize("— configure at least one provider first", DIM)
    };

    write!(
        out,
        "{}{} {} {} {}\r\n",
        prefix, done_pointer, done_icon, done_label, done_desc
    )?;
    lines += 1;

    out.flush()?;
    Ok(lines)
}

fn render_menu_hint(out: &mut impl Write, menu: &MenuState, chars: &BoxChars) -> Result<()> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let hint = if menu.in_sub_menu {
        "↑/↓ navigate · Enter select · Esc back"
    } else {
        "↑/↓ navigate · Enter expand/select"
    };
    write!(out, "{}{}\r\n", prefix, colorize(hint, DIM))?;
    out.flush()?;
    Ok(())
}

fn rerender_menu(
    out: &mut impl Write,
    config: &config::Config,
    menu: &MenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    // Erase previous render (list + hint)
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_provider_menu(out, config, menu, chars)?;
    render_menu_hint(out, menu, chars)?;
    Ok(new_lines)
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
            assert!(!provider.models.is_empty(), "Provider {} has no models", provider.name);
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
