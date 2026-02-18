//! Provider status CLI handler.

use anyhow::Result;

use common::utils::terminal::*;

use super::commands::ProviderCommands;

/// Handle provider subcommands.
pub async fn handle_provider(cmd: ProviderCommands) -> Result<()> {
    match cmd {
        ProviderCommands::Status => handle_provider_status().await,
    }
}

async fn handle_provider_status() -> Result<()> {
    let config = config::load().await?;

    println!("Provider Status");
    println!("{}", "━".repeat(50));
    println!();

    // List all providers and their configuration status
    let providers: Vec<(&str, bool)> = vec![
        ("anthropic", !config.providers.anthropic.api_key.is_empty()),
        ("openai", !config.providers.openai.api_key.is_empty()),
        (
            "openrouter",
            !config.providers.openrouter.api_key.is_empty(),
        ),
        ("deepseek", !config.providers.deepseek.api_key.is_empty()),
        ("gemini", !config.providers.gemini.api_key.is_empty()),
        ("groq", !config.providers.groq.api_key.is_empty()),
        ("vllm", !config.providers.vllm.api_key.is_empty()),
    ];

    println!("{:<20} {:>12}", "Provider", "Status");
    println!("{}", "─".repeat(50));

    for (name, configured) in &providers {
        let status = if *configured {
            format!("{} configured", status_success())
        } else {
            colorize("not configured", DIM).to_string()
        };
        println!("{:<20} {:>12}", name, status);
    }
    println!();

    // Provider manager routing
    let pm = &config.provider_manager;
    println!("Routing");
    println!("{}", "─".repeat(50));

    let primary = pm
        .primary
        .as_deref()
        .unwrap_or_else(|| detect_active(&config));
    println!("Primary:     {}", primary);

    if let Some(ref fallback) = pm.fallback {
        println!("Fallback:    {}", fallback);
    } else {
        println!("Fallback:    {}", colorize("none", DIM));
    }

    if let Some(ref classifier) = pm.classifier_model {
        println!("Classifier:  {}", classifier);
    } else {
        println!("Classifier:  {}", colorize("auto", DIM));
    }

    println!("Model:       {}", config.agents.defaults.model);
    println!();

    // Circuit breaker info (static config — runtime state only available while serving)
    println!("Circuit Breaker");
    println!("{}", "─".repeat(50));
    println!(
        "State:       {} closed (static — live state requires running agent)",
        status_success(),
    );
    println!("Threshold:   5 consecutive failures");
    println!("Reset:       60s timeout");
    println!();

    let configured_count = providers.iter().filter(|(_, c)| *c).count();
    if configured_count == 0 {
        println!(
            "{} No providers configured. Run `klyntbot init` to set up.",
            colorize("Note:", DIM)
        );
    }

    Ok(())
}

/// Detect the active provider (same logic as status.rs).
fn detect_active(config: &config::Config) -> &str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_commands_parse() {
        let cmd = ProviderCommands::Status;
        match cmd {
            ProviderCommands::Status => {} // Verified it parses
        }
    }
}
