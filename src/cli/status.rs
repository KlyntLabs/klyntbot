//! Status command handlers for configuration and system status

use anyhow::Result;
use crate::utils::terminal::*;

/// Handle brief status (no-args command)
pub async fn handle_brief_status() -> Result<()> {
    let config_path = crate::config::config_path();
    let exists = config_path.exists();

    println!("klyntbot v{}", env!("CARGO_PKG_VERSION"));
    println!();

    if exists {
        let config = crate::config::load()?;

        // Status indicator
        let has_api_key = !config.providers.anthropic.api_key.is_empty()
            || !config.providers.openai.api_key.is_empty()
            || !config.providers.openrouter.api_key.is_empty()
            || !config.providers.deepseek.api_key.is_empty();

        if has_api_key {
            println!("Status: {} Ready", status_success());
        } else {
            println!("Status: {} No API key configured", status_warning());
        }

        // Active provider/model
        let provider = detect_active_provider(&config);

        println!("Provider: {}/{}", provider, config.agents.defaults.model);
        println!();

        // Top commands
        println!("Commands:");
        println!("  chat        Start interactive chat");
        println!("  serve       Start gateway daemon");
        println!("  status      Show detailed status");
        println!("  init        Run setup wizard");
        println!("  --help      Show all commands");
        println!();

        // Hint
        println!("{}", colorize("Try: klyntbot chat", DIM));
    } else {
        println!("Status: {} Configuration not found", status_error());
        println!();
        println!("Run this command to initialize:");
        println!("  klyntbot init");
    }

    Ok(())
}

/// Handle status command
pub async fn handle_status(verbose: bool) -> Result<()> {
    let config_path = crate::config::config_path();
    let exists = config_path.exists();

    // Version header
    println!("klyntbot v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", "━".repeat(50));
    println!();

    if exists {
        let config = crate::config::load()?;
        let workspace = config.workspace_path();

        // Determine active provider
        let provider = detect_active_provider(&config);

        // Provider section
        println!("Provider");
        println!("  {}/{}", provider, config.agents.defaults.model);
        println!();

        // Workspace section
        println!("Workspace");
        println!("  {}", workspace.display());
        println!();

        // Configuration section
        println!("Configuration");
        println!("  {}", config_path.display());
        println!();

        if verbose {
            // Channels table
            println!("{}", "━".repeat(50));
            println!();
            println!("{:<40} Status", "Channels");
            println!("{}", "━".repeat(50));

            let channels = [
                ("telegram", config.channels.telegram.enabled),
                ("discord", config.channels.discord.enabled),
                ("whatsapp", config.channels.whatsapp.enabled),
                ("slack", config.channels.slack.enabled),
                ("qq", config.channels.qq.enabled),
                ("email", config.channels.email.enabled),
            ];

            for (name, enabled) in channels {
                let status_str = if enabled {
                    format!("{} enabled", status_success())
                } else {
                    format!("{} disabled", status_disabled())
                };
                println!("{:<40} {}", name, status_str);
            }
            println!();
        }
    } else {
        println!("{} Configuration not found", status_error());
        println!();
        println!("Run this command to initialize:");
        println!("  klyntbot init");
    }

    Ok(())
}

/// Detect the active provider based on which API keys are configured.
/// NOTE: This is a local helper function. Developer 4 will add this as a method on Config.
fn detect_active_provider(config: &crate::config::Config) -> &str {
    if !config.providers.anthropic.api_key.is_empty() {
        "anthropic"
    } else if !config.providers.openai.api_key.is_empty() {
        "openai"
    } else if !config.providers.openrouter.api_key.is_empty() {
        "openrouter"
    } else if !config.providers.deepseek.api_key.is_empty() {
        "deepseek"
    } else {
        "none"
    }
}
