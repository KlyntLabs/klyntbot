//! Configuration status dashboard utilities.
//!
//! Originally the "Welcome" wizard step. Now provides reusable functions
//! for displaying the config status dashboard (used by the provider step).

use common::utils::terminal::*;

use crate::wizard::framework::WizardState;
use crate::wizard::prompts::mask_secret;

/// Print a configuration status dashboard showing what's configured.
pub(crate) fn print_config_status(state: &WizardState) {
    let config = &state.config;

    // Provider status
    let active = config.active_provider_name();
    if active != "none" {
        let model = &config.agents.defaults.model;
        let configured_count = count_configured_providers(config);
        let provider_display = if configured_count > 1 {
            format!(
                "{} ({}) [{} configured]",
                capitalize(active),
                model,
                configured_count
            )
        } else {
            format!("{} ({})", capitalize(active), model)
        };
        print_status_line(true, "Provider", &provider_display);
    } else {
        print_status_line(false, "Provider", "Not configured");
    }

    // Channels status
    let mut enabled_channels = Vec::new();
    if config.channels.telegram.enabled {
        enabled_channels.push("Telegram");
    }
    if config.channels.discord.enabled {
        enabled_channels.push("Discord");
    }
    if config.channels.slack.enabled {
        enabled_channels.push("Slack");
    }
    if config.channels.whatsapp.enabled {
        enabled_channels.push("WhatsApp");
    }
    if config.channels.email.enabled {
        enabled_channels.push("Email");
    }
    if config.channels.qq.enabled {
        enabled_channels.push("QQ");
    }

    if enabled_channels.is_empty() {
        print_status_line(false, "Channels", "None configured");
    } else {
        print_status_line(true, "Channels", &enabled_channels.join(", "));
    }

    // Tools status
    let preset = detect_preset(config);
    print_status_line(true, "Tools", &format!("{} preset", preset));

    // Workspace status
    let workspace = config.workspace_path();
    if workspace.exists() {
        print_status_line(true, "Workspace", &workspace.display().to_string());
    } else {
        print_status_line(false, "Workspace", "Not created");
    }

    // Calendar status
    if config.calendar.is_any_enabled() {
        let enabled = config.calendar.enabled_providers();
        let names: Vec<&str> = enabled.iter().map(|p| p.display_name()).collect();
        print_status_line(true, "Calendar", &names.join(", "));
    } else {
        print_status_line(false, "Calendar", "Not configured");
    }

    // Timezone
    print_status_line(true, "Timezone", &config.timezone);

    // Brave Search
    if !config.tools.web.brave_api_key.is_empty() {
        let masked = mask_secret(config.tools.web.brave_api_key.expose());
        print_status_line(true, "Web Search", &format!("Brave ({})", masked));
    }
}

/// Print a single status line with checkmark or circle.
fn print_status_line(configured: bool, label: &str, value: &str) {
    let icon = if configured {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    let value_str = if configured {
        value.to_string()
    } else {
        colorize(value, DIM)
    };

    println!(
        "{}",
        draw_step_line(&format!(" {} {:<12} {}", icon, label, value_str))
    );
}

/// Count how many providers have API keys configured.
fn count_configured_providers(config: &config::Config) -> usize {
    [
        !config.providers.anthropic.api_key.is_empty(),
        !config.providers.openai.api_key.is_empty(),
        !config.providers.deepseek.api_key.is_empty(),
        !config.providers.gemini.api_key.is_empty(),
        !config.providers.openrouter.api_key.is_empty(),
        !config.providers.groq.api_key.is_empty(),
    ]
    .iter()
    .filter(|&&configured| configured)
    .count()
}

/// Detect which tools preset most closely matches current config.
pub(crate) fn detect_preset(config: &config::Config) -> &'static str {
    if config.tools.restrict_to_workspace
        && !config.tools.exec.allowed_commands.is_empty()
        && config.tools.exec.timeout <= 30
    {
        "Strict"
    } else if !config.tools.restrict_to_workspace && config.tools.exec.timeout >= 120 {
        "Permissive"
    } else {
        "Balanced"
    }
}

/// Capitalize the first letter of a string.
pub(crate) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_preset_balanced_default() {
        let config = config::Config::default();
        assert_eq!(detect_preset(&config), "Balanced");
    }

    #[test]
    fn test_detect_preset_strict() {
        let mut config = config::Config::default();
        config.tools.restrict_to_workspace = true;
        config.tools.exec.timeout = 30;
        config.tools.exec.allowed_commands = vec!["ls".to_string()];
        assert_eq!(detect_preset(&config), "Strict");
    }

    #[test]
    fn test_detect_preset_permissive() {
        let mut config = config::Config::default();
        config.tools.restrict_to_workspace = false;
        config.tools.exec.timeout = 120;
        assert_eq!(detect_preset(&config), "Permissive");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("anthropic"), "Anthropic");
        assert_eq!(capitalize("openai"), "Openai");
        assert_eq!(capitalize(""), "");
    }
}
