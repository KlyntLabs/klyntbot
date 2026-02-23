//! Status command handlers for configuration and system status

use anyhow::Result;
use common::utils::terminal::*;

/// Handle brief status (no-args command)
pub async fn handle_brief_status() -> Result<()> {
    let config_path = config::config_path()?;
    let exists = config_path.exists();

    println!("klyntbot v{}", env!("CARGO_PKG_VERSION"));
    println!();

    if exists {
        let config = config::load().await?;

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
        let (provider, model) = resolve_provider_and_model(&config);

        println!("Provider: {}/{}", provider, model);
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
    let config_path = config::config_path()?;
    let exists = config_path.exists();

    // Version header
    println!("klyntbot v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", "━".repeat(50));
    println!();

    if exists {
        let config = config::load().await?;
        let workspace = config.workspace_path();

        // Determine active provider
        let (provider, model) = resolve_provider_and_model(&config);

        // Provider section
        println!("Provider");
        println!("  {}/{}", provider, model);
        println!();

        // Storage section
        let data_dir = config.data_dir_path();
        let db_path = data_dir.join("data.db");
        println!("Storage");
        if db_path.exists() {
            println!("  SQLite at {}", db_path.display());
        } else {
            println!("  {} not initialized — run {} to set up", status_warning(), colorize("klyntbot init", HIGHLIGHT));
        }
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

/// Resolve the active provider name and effective model for display.
///
/// Priority: explicit `agents.defaults.provider` field, then API-key detection.
/// When an explicit provider is set and the configured model doesn't match it,
/// the provider's default model is shown instead.
fn resolve_provider_and_model(config: &config::Config) -> (String, String) {
    // If explicit provider is set, use it
    if let Some(ref name) = config.agents.defaults.provider {
        if !name.is_empty() {
            if let Some(spec) = providers::ProviderRegistry::find_by_name(name) {
                let model = &config.agents.defaults.model;
                let model_belongs = spec
                    .keywords
                    .iter()
                    .any(|kw| model.to_lowercase().contains(kw));
                let resolved_model = if model_belongs {
                    model.clone()
                } else {
                    spec.default_model.to_string()
                };
                return (name.clone(), resolved_model);
            }
        }
    }

    // Fall back to detecting by API key presence
    let provider = if !config.providers.anthropic.api_key.is_empty() {
        "anthropic"
    } else if !config.providers.openai.api_key.is_empty() {
        "openai"
    } else if !config.providers.openrouter.api_key.is_empty() {
        "openrouter"
    } else if !config.providers.deepseek.api_key.is_empty() {
        "deepseek"
    } else {
        "none"
    };
    (provider.to_string(), config.agents.defaults.model.clone())
}
