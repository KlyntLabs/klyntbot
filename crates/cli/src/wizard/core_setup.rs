//! Phase 1: Core Setup — single-screen provider + database + channel configuration.
//!
//! Uses auto-detection from [`super::detect`] to pre-fill known values,
//! then prompts for any missing fields.

use anyhow::Result;
use common::utils::terminal::*;

use super::detect::{DetectSource, DetectedState};
use super::framework::{StepResult, WizardState};
use super::prompts::{prompt_secret, prompt_select, prompt_yes_no, SelectOption};

// ============================================================================
// Provider & channel metadata
// ============================================================================

/// Provider display metadata for the core setup screen.
pub struct ProviderInfo {
    pub key: &'static str,
    pub name: &'static str,
}

pub static PROVIDER_INFO: &[ProviderInfo] = &[
    ProviderInfo {
        key: "anthropic",
        name: "Anthropic",
    },
    ProviderInfo {
        key: "openai",
        name: "OpenAI",
    },
    ProviderInfo {
        key: "openrouter",
        name: "OpenRouter",
    },
    ProviderInfo {
        key: "deepseek",
        name: "DeepSeek",
    },
    ProviderInfo {
        key: "gemini",
        name: "Google Gemini",
    },
    ProviderInfo {
        key: "groq",
        name: "Groq",
    },
    ProviderInfo {
        key: "vllm",
        name: "vLLM",
    },
    ProviderInfo {
        key: "zhipu",
        name: "Zhipu",
    },
    ProviderInfo {
        key: "dashscope",
        name: "DashScope",
    },
    ProviderInfo {
        key: "moonshot",
        name: "Moonshot",
    },
    ProviderInfo {
        key: "minimax",
        name: "MiniMax",
    },
    ProviderInfo {
        key: "aihubmix",
        name: "AIHubMix",
    },
];

/// Channel display metadata for the core setup screen.
pub struct ChannelInfo {
    pub key: &'static str,
    pub name: &'static str,
}

pub static CHANNEL_INFO: &[ChannelInfo] = &[
    ChannelInfo {
        key: "telegram",
        name: "Telegram",
    },
    ChannelInfo {
        key: "discord",
        name: "Discord",
    },
    ChannelInfo {
        key: "slack",
        name: "Slack",
    },
    ChannelInfo {
        key: "whatsapp",
        name: "WhatsApp",
    },
    ChannelInfo {
        key: "email",
        name: "Email",
    },
    ChannelInfo {
        key: "qq",
        name: "QQ",
    },
];

// ============================================================================
// Core setup orchestrator
// ============================================================================

/// Run the Phase 1 core setup screen.
///
/// 1. Build `DetectedState` from config, env vars, and system probes
/// 2. Render summary of auto-detected values
/// 3. Prompt for missing/editable fields (provider, API key, database, channel)
/// 4. Save results to `state.config`
pub fn run_core_setup(state: &mut WizardState) -> Result<StepResult> {
    // Build detected state
    let mut detected = DetectedState::from_config(&state.config);
    detected.overlay_env_vars();
    detected.check_data_dir();

    // Show auto-detected summary
    render_summary(&detected);

    // Provider setup
    prompt_provider(state, &detected)?;

    // Channel setup (optional)
    prompt_channel(state, &detected)?;

    Ok(StepResult::Next)
}

/// Print the auto-detected summary showing what was found.
fn render_summary(detected: &DetectedState) {
    println!(
        "  {} {}",
        colorize("Auto-detected:", BOLD),
        colorize("(values can be changed below)", DIM)
    );
    println!();

    if let Some((ref name, ref source)) = detected.provider {
        println!(
            "  {} Provider: {} ({})",
            colorize("✓", SUCCESS),
            colorize(name, HIGHLIGHT),
            source
        );
    } else {
        println!(
            "  {} Provider: {}",
            colorize("·", DIM),
            colorize("not configured", DIM)
        );
    }

    if detected.data_dir_writable {
        println!(
            "  {} Data dir: {} {}",
            colorize("✓", SUCCESS),
            colorize(&detected.data_dir, HIGHLIGHT),
            colorize("(writable)", DIM)
        );
    } else {
        println!(
            "  {} Data dir: {} {}",
            colorize("·", DIM),
            colorize(&detected.data_dir, HIGHLIGHT),
            colorize("(will be created on first run)", DIM)
        );
    }

    if let Some((ref name, ref source)) = detected.channel {
        println!(
            "  {} Channel:  {} ({})",
            colorize("✓", SUCCESS),
            colorize(name, HIGHLIGHT),
            source
        );
    } else {
        println!(
            "  {} Channel:  {}",
            colorize("·", DIM),
            colorize("none — CLI only", DIM)
        );
    }

    println!();
}

/// Prompt for LLM provider selection and API key.
fn prompt_provider(state: &mut WizardState, detected: &DetectedState) -> Result<()> {
    // Determine default selection index
    let default_idx = detected
        .provider
        .as_ref()
        .and_then(|(key, _)| PROVIDER_INFO.iter().position(|p| p.key == key))
        .unwrap_or(0);

    let options: Vec<SelectOption<'_>> = PROVIDER_INFO
        .iter()
        .map(|p| SelectOption {
            label: p.name,
            description: p.key,
        })
        .collect();

    let selected = prompt_select("LLM Provider", &options, default_idx)?;
    let provider = &PROVIDER_INFO[selected];

    // Set explicit provider
    state.config.agents.defaults.provider = Some(provider.key.to_string());

    // Prompt for API key if not already set
    let existing_key = detected
        .api_key
        .as_ref()
        .map(|(k, _)| k.as_str())
        .unwrap_or("");

    if existing_key.is_empty() {
        let key = prompt_secret(&format!("{} API Key", provider.name), 1)?;
        state.config.set_provider_key(provider.key, key);
    } else {
        let masked = mask_key(existing_key);
        if prompt_yes_no(&format!("Keep existing API key ({})?", masked), true)? {
            // Keep existing key — no changes needed
        } else {
            let key = prompt_secret(&format!("{} API Key", provider.name), 1)?;
            state.config.set_provider_key(provider.key, key);
        }
    }

    Ok(())
}

/// Prompt for optional channel configuration.
fn prompt_channel(state: &mut WizardState, detected: &DetectedState) -> Result<()> {
    // Build options: "None (CLI only)" + all channels
    let mut options: Vec<SelectOption<'_>> = vec![SelectOption {
        label: "None",
        description: "CLI only — no chat platform",
    }];

    for ch in CHANNEL_INFO {
        options.push(SelectOption {
            label: ch.name,
            description: ch.key,
        });
    }

    // Default to detected channel or "None"
    let default_idx = detected
        .channel
        .as_ref()
        .and_then(|(key, _)| CHANNEL_INFO.iter().position(|c| c.key == key))
        .map(|i| i + 1) // +1 because "None" is index 0
        .unwrap_or(0);

    let selected = prompt_select("Chat Channel (optional)", &options, default_idx)?;

    if selected == 0 {
        // No channel — nothing to configure
        return Ok(());
    }

    let channel = &CHANNEL_INFO[selected - 1];

    // Enable the selected channel and prompt for token
    match channel.key {
        "telegram" => {
            state.config.channels.telegram.enabled = true;
            let existing = detected
                .channel_token
                .as_ref()
                .filter(|(_, src)| *src != DetectSource::Detected)
                .map(|(t, _)| t.as_str())
                .unwrap_or("");

            if existing.is_empty() {
                let token = prompt_secret("Telegram Bot Token", 1)?;
                state.config.channels.telegram.token = config::Secret::new(token);
            }
        }
        "discord" => {
            state.config.channels.discord.enabled = true;
            let existing = detected
                .channel_token
                .as_ref()
                .filter(|(_, src)| *src != DetectSource::Detected)
                .map(|(t, _)| t.as_str())
                .unwrap_or("");

            if existing.is_empty() {
                let token = prompt_secret("Discord Bot Token", 1)?;
                state.config.channels.discord.token = config::Secret::new(token);
            }
        }
        "slack" => {
            state.config.channels.slack.enabled = true;
            let existing = detected
                .channel_token
                .as_ref()
                .filter(|(_, src)| *src != DetectSource::Detected)
                .map(|(t, _)| t.as_str())
                .unwrap_or("");

            if existing.is_empty() {
                let token = prompt_secret("Slack Bot Token (xoxb-...)", 1)?;
                state.config.channels.slack.bot_token = config::Secret::new(token);
            }
        }
        _ => {
            // Other channels: just enable, token prompt not yet implemented
            println!(
                "  {} {} channel enabled — configure details via config file",
                colorize("→", BRAND),
                channel.name
            );
        }
    }

    Ok(())
}

/// Mask an API key for display (show prefix + last 4 chars).
fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "●".repeat(key.len());
    }
    let prefix_end = key.find('-').map(|i| i + 1).unwrap_or(4).min(8);
    format!(
        "{}...{}",
        &key[..prefix_end],
        &key[key.len().saturating_sub(4)..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_list_matches_config() {
        // All entries in PROVIDER_INFO should have a valid key
        for info in PROVIDER_INFO {
            assert!(!info.key.is_empty());
            assert!(!info.name.is_empty());
        }
    }

    #[test]
    fn test_channel_list() {
        assert!(CHANNEL_INFO.len() >= 4); // telegram, discord, slack, whatsapp minimum
    }

    #[test]
    fn test_provider_info_count() {
        assert_eq!(PROVIDER_INFO.len(), 12);
    }

    #[test]
    fn test_channel_info_count() {
        assert_eq!(CHANNEL_INFO.len(), 6);
    }

    #[test]
    fn test_provider_keys_unique() {
        let mut keys: Vec<&str> = PROVIDER_INFO.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), PROVIDER_INFO.len());
    }

    #[test]
    fn test_channel_keys_unique() {
        let mut keys: Vec<&str> = CHANNEL_INFO.iter().map(|c| c.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), CHANNEL_INFO.len());
    }

    #[test]
    fn test_mask_key_short() {
        assert_eq!(mask_key("abc"), "●●●");
    }

    #[test]
    fn test_mask_key_with_prefix() {
        assert_eq!(mask_key("sk-ant-api1234567890"), "sk-...7890");
    }

    #[test]
    fn test_mask_key_no_dash() {
        assert_eq!(mask_key("abcdefghijklmnop"), "abcd...mnop");
    }

    #[test]
    fn test_provider_info_has_anthropic() {
        assert!(PROVIDER_INFO.iter().any(|p| p.key == "anthropic"));
    }

    #[test]
    fn test_channel_info_has_telegram() {
        assert!(CHANNEL_INFO.iter().any(|c| c.key == "telegram"));
    }

}
