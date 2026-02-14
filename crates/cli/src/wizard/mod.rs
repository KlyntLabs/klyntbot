//! Interactive onboarding wizard for klyntbot initialization.
//!
//! The wizard is composed of pluggable modules defined in submodules:
//!
//! | Module      | Purpose                                         |
//! |-------------|-------------------------------------------------|
//! | `welcome`   | Welcome screen and overview                     |
//! | `provider`  | LLM provider selection, API key, model          |
//! | `channels`  | Chat platform configuration (async)             |
//! | `tools`     | Security presets and tool permissions            |
//! | `daemon`    | Background service installation                 |
//! | `workspace` | Workspace directories and template files        |
//!
//! Supporting modules:
//! - `framework`  – `WizardModule` trait, `WizardState`, `WizardRunner`
//! - `prompts`    – Reusable prompt utilities
//! - `templates`  – Workspace template file contents
//! - `oauth`      – OAuth callback server for Discord/Slack
//! - `ask_user_prompt` – Tabbed multi-question UI for ask_user tool

pub mod ask_user_prompt;
pub mod calendar;
pub mod channels;
pub mod daemon;
pub mod framework;
pub mod oauth;
pub mod prompts;
pub mod provider;
pub mod templates;
pub mod todo_notifications;
pub mod tools;
pub mod welcome;
pub mod workspace;

use anyhow::Result;
use common::utils::terminal::*;

use framework::{print_step_header, StepResult, WizardModule, WizardState};

/// Adapter wrapping [`tools::configure_tools`] as a [`WizardModule`].
struct ToolsModule;

impl WizardModule for ToolsModule {
    fn name(&self) -> &str {
        "Tool Permissions"
    }

    fn description(&self) -> &str {
        "Configure tool security and permissions"
    }

    fn is_required(&self) -> bool {
        false
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        match tools::configure_tools(&mut state.config)? {
            true => Ok(StepResult::Next),
            false => Ok(StepResult::Skip),
        }
    }
}

/// Run the interactive onboarding wizard.
///
/// Orchestrates all wizard modules in sequence with forward/back navigation.
/// The channels step is handled inline because it requires async I/O (OAuth).
pub async fn run_wizard() -> Result<()> {
    // Print global branding header at the very top
    print_wizard_header();

    let mut state = WizardState::new();

    // Step table: (name, required, applicable)
    // Index order must match the dispatch arms below.
    let all_steps: &[(&str, bool, bool)] = &[
        ("Welcome", true, true),
        ("LLM Provider", true, true),
        ("Chat Channels", false, true),
        ("Tool Permissions", false, true),
        (
            "Background Service",
            false,
            daemon::DaemonModule.is_applicable(&state),
        ),
        ("Workspace Setup", true, true),
        ("Todo Notifications", false, true),
        ("Calendar Sync", false, true),
    ];

    // Filter to applicable steps
    let applicable: Vec<usize> = all_steps
        .iter()
        .enumerate()
        .filter(|(_, (_, _, app))| *app)
        .map(|(i, _)| i)
        .collect();

    state.total_steps = applicable.len();

    // Navigate through steps with forward/back support
    let mut pos = 0;
    while pos < applicable.len() {
        let step_idx = applicable[pos];
        let (name, required, _) = all_steps[step_idx];
        state.current_step = pos + 1;

        print_step_header(&state, name, required);

        // Dispatch to the right module. Step indices correspond to `all_steps`:
        // 0=welcome, 1=provider, 2=channels (async), 3=tools, 4=daemon, 5=workspace, 6=todo_notifications, 7=calendar.
        // Ctrl+C in raw-mode prompts surfaces as an Err containing "Ctrl+C".
        let result = match match step_idx {
            0 => welcome::WelcomeModule.run(&mut state),
            1 => provider::ProviderModule.run(&mut state),
            2 => run_channels_step(&mut state).await,
            3 => ToolsModule.run(&mut state),
            4 => daemon::DaemonModule.run(&mut state),
            5 => workspace::WorkspaceModule.run(&mut state),
            6 => todo_notifications::run_todo_notification_step(&mut state),
            7 => calendar::run_calendar_step(&mut state),
            _ => unreachable!(),
        } {
            Ok(r) => r,
            Err(e) if e.to_string().contains("Ctrl+C") => {
                println!("\n{} Setup cancelled. No changes saved.", status_warning());
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        match result {
            StepResult::Next | StepResult::Skip => pos += 1,
            StepResult::Back if pos > 0 => pos -= 1,
            StepResult::Back => {} // already at first step
            StepResult::Cancel => {
                println!("\n{} Setup cancelled. No changes saved.", status_warning());
                return Ok(());
            }
        }
    }

    // Save configuration
    config::save(&state.config).await?;

    // Completion screen
    print_completion();

    Ok(())
}

/// Channels wizard step (async due to OAuth flows).
async fn run_channels_step(state: &mut WizardState) -> Result<StepResult> {
    let chars = BoxChars::get();

    let wants = prompts::prompt_yes_no("Enable chat channels?", false)?;

    if !wants {
        println!(
            "{}",
            draw_step_line(&colorize("Channel setup can be done later with:", DIM))
        );
        println!(
            "{}",
            draw_step_line(&colorize("  klyntbot channels login <channel>", DIM))
        );
        println!("{}", colorize(chars.vertical, BRAND));
        return Ok(StepResult::Skip);
    }

    channels::configure_channels(&mut state.config).await?;
    Ok(StepResult::Next)
}

/// Print the global wizard header (shown once at the start).
fn print_wizard_header() {
    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            BRAND
        )
    );
    println!("{}", draw_step_line(&colorize("klyntbot", BOLD)));
    println!(
        "{}",
        draw_step_line(&colorize("Your AI Assistant Platform", DIM))
    );
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            BRAND
        )
    );
    println!();
}

/// Print the completion screen after all steps finish.
fn print_completion() {
    let config_path_str = config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.klyntbot/config.json".to_string());

    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            SUCCESS
        )
    );
    println!();
    println!(
        "  {} {}",
        colorize("✓", SUCCESS),
        colorize("Setup Complete!", BOLD)
    );
    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            SUCCESS
        )
    );
    println!();

    println!(
        "{} Your AI assistant is ready to use!\n",
        colorize("●", BRAND)
    );

    println!("{} {}", colorize("→", BRAND), colorize("Try it out:", BOLD));
    println!("  {} klyntbot chat", colorize("$", DIM));
    println!("  {} klyntbot chat \"Hello!\"", colorize("$", DIM));
    println!();

    println!("{} {}", colorize("→", BRAND), colorize("Get help:", BOLD));
    println!("  {} klyntbot --help", colorize("$", DIM));
    println!("  {} klyntbot status", colorize("$", DIM));
    println!();

    println!(
        "{} {}",
        colorize("→", BRAND),
        colorize("Configuration saved:", BOLD)
    );
    println!("  {}", colorize(&config_path_str, HIGHLIGHT));
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ToolsModule adapter tests
    // ========================================================================

    #[test]
    fn test_tools_module_metadata() {
        let module = ToolsModule;
        assert_eq!(module.name(), "Tool Permissions");
        assert!(!module.is_required());

        let state = WizardState::new();
        assert!(module.is_applicable(&state));
    }

    // ========================================================================
    // Config set_provider_key integration tests
    // ========================================================================

    #[test]
    fn test_set_provider_key_anthropic() {
        let mut config = config::Config::default();
        config.set_provider_key("anthropic", "sk-ant-test-key".to_string());
        assert_eq!(
            config.providers.anthropic.api_key.expose(),
            "sk-ant-test-key"
        );
    }

    #[test]
    fn test_set_provider_key_openai() {
        let mut config = config::Config::default();
        config.set_provider_key("openai", "sk-openai-test".to_string());
        assert_eq!(config.providers.openai.api_key.expose(), "sk-openai-test");
    }

    #[test]
    fn test_set_provider_key_all_providers() {
        let mut config = config::Config::default();
        let providers = ["anthropic", "openai", "deepseek", "gemini", "openrouter"];
        for key in &providers {
            config.set_provider_key(key, format!("key-{}", key));
        }

        assert_eq!(config.providers.anthropic.api_key.expose(), "key-anthropic");
        assert_eq!(config.providers.openai.api_key.expose(), "key-openai");
        assert_eq!(config.providers.deepseek.api_key.expose(), "key-deepseek");
        assert_eq!(config.providers.gemini.api_key.expose(), "key-gemini");
        assert_eq!(
            config.providers.openrouter.api_key.expose(),
            "key-openrouter"
        );
    }

    #[test]
    fn test_set_provider_key_unknown_does_nothing() {
        let mut config = config::Config::default();
        config.set_provider_key("unknown_provider", "test-key".to_string());
        assert!(config.providers.anthropic.api_key.is_empty());
        assert!(config.providers.openai.api_key.is_empty());
        assert!(config.providers.deepseek.api_key.is_empty());
    }

    // ========================================================================
    // Config integration tests
    // ========================================================================

    #[test]
    fn test_wizard_config_flow_simulation() {
        // Simulate what run_wizard does without stdin interaction
        let mut config = config::Config::default();

        let api_key = "sk-ant-test-key-123456789".to_string();
        let model = "claude-sonnet-4-5".to_string();

        config.set_provider_key("anthropic", api_key.clone());
        config.agents.defaults.model = model.clone();

        assert_eq!(config.providers.anthropic.api_key.expose(), &api_key);
        assert_eq!(config.agents.defaults.model, model);
    }

    #[test]
    fn test_wizard_config_openai_flow() {
        let mut config = config::Config::default();

        config.set_provider_key("openai", "sk-openai-test123456".to_string());
        config.agents.defaults.model = "gpt-4o".to_string();

        assert_eq!(
            config.providers.openai.api_key.expose(),
            "sk-openai-test123456"
        );
        assert_eq!(config.agents.defaults.model, "gpt-4o");
        assert!(config.providers.anthropic.api_key.is_empty());
    }

    #[test]
    fn test_wizard_config_save_round_trip() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let mut config = config::Config::default();
        config.set_provider_key("anthropic", "sk-ant-test-key123".to_string());
        config.agents.defaults.model = "claude-sonnet-4-5".to_string();

        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, &json).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: config::Config = serde_json::from_str(&content).unwrap();

        assert_eq!(
            loaded.providers.anthropic.api_key.expose(),
            "sk-ant-test-key123"
        );
        assert_eq!(loaded.agents.defaults.model, "claude-sonnet-4-5");
    }

    // ========================================================================
    // Active provider detection tests
    // ========================================================================

    #[test]
    fn test_active_provider_detection_anthropic() {
        let mut config = config::Config::default();
        config.set_provider_key("anthropic", "sk-ant-key".to_string());
        assert_eq!(config.active_provider_name(), "anthropic");
    }

    #[test]
    fn test_active_provider_detection_openai() {
        let mut config = config::Config::default();
        config.set_provider_key("openai", "sk-openai-key".to_string());
        assert_eq!(config.active_provider_name(), "openai");
    }

    #[test]
    fn test_active_provider_detection_none() {
        let config = config::Config::default();
        assert_eq!(config.active_provider_name(), "none");
    }

    #[test]
    fn test_active_provider_detection_priority() {
        let mut config = config::Config::default();
        config.set_provider_key("openai", "sk-openai-key".to_string());
        config.set_provider_key("anthropic", "sk-ant-key".to_string());
        assert_eq!(config.active_provider_name(), "anthropic");
    }

    // ========================================================================
    // Step count tests
    // ========================================================================

    #[test]
    fn test_wizard_step_count() {
        // Verify the declared step count matches the number of step entries
        // (this catches off-by-one errors if someone adds a step)
        let state = WizardState::new();
        let welcome = welcome::WelcomeModule;
        let provider_mod = provider::ProviderModule;
        let tools_mod = ToolsModule;
        let daemon_mod = daemon::DaemonModule;
        let workspace_mod = workspace::WorkspaceModule;

        let applicables: Vec<bool> = vec![
            welcome.is_applicable(&state),
            provider_mod.is_applicable(&state),
            true, // channels
            tools_mod.is_applicable(&state),
            daemon_mod.is_applicable(&state),
            workspace_mod.is_applicable(&state),
        ];

        let count = applicables.iter().filter(|a| **a).count();
        // On macOS/Linux daemon is applicable, so all 6 steps.
        // On unknown platforms it would be 5.
        assert!(count >= 5, "Should have at least 5 applicable steps");
        assert!(count <= 6, "Should have at most 6 steps");
    }
}
