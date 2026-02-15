//! Channel configuration wizard for interactive channel setup.
//!
//! Provides guided configuration for all supported chat channels:
//! Telegram, Discord, Slack, WhatsApp, Email, and QQ.

mod configure;
mod menus;

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::prompts;
use crate::wizard::ui::MenuOutcome;

/// Channel metadata for the selection UI
#[allow(dead_code)]
pub(super) struct ChannelInfo {
    pub name: &'static str,
    pub key: &'static str,
    pub description: &'static str,
    pub prerequisites: &'static str,
}

pub(super) const CHANNELS: &[ChannelInfo] = &[
    ChannelInfo {
        name: "Telegram",
        key: "telegram",
        description: "Bot API with long polling",
        prerequisites: "Bot token from @BotFather",
    },
    ChannelInfo {
        name: "Discord",
        key: "discord",
        description: "Bot via WebSocket Gateway",
        prerequisites: "Bot token from Discord Developer Portal",
    },
    ChannelInfo {
        name: "Slack",
        key: "slack",
        description: "Socket Mode bot integration",
        prerequisites: "Bot Token (xoxb-) and App Token (xapp-)",
    },
    ChannelInfo {
        name: "WhatsApp",
        key: "whatsapp",
        description: "Via Baileys Node.js bridge",
        prerequisites: "Running WhatsApp bridge at ws://localhost:3001",
    },
    ChannelInfo {
        name: "Email",
        key: "email",
        description: "IMAP polling + SMTP replies",
        prerequisites: "IMAP/SMTP server credentials",
    },
    ChannelInfo {
        name: "QQ",
        key: "qq",
        description: "QQ Bot via official API",
        prerequisites: "App ID and Secret from QQ Bot Platform",
    },
];

// ============================================================================
// Configuration Detection Helpers
// ============================================================================

/// Check if a channel is configured (has credentials).
pub(super) fn is_channel_configured(config: &Config, channel_key: &str) -> bool {
    match channel_key {
        "telegram" => !config.channels.telegram.token.expose().is_empty(),
        "discord" => !config.channels.discord.token.expose().is_empty(),
        "slack" => {
            !config.channels.slack.bot_token.expose().is_empty()
                && !config.channels.slack.app_token.expose().is_empty()
        }
        "whatsapp" => !config.channels.whatsapp.bridge_url.is_empty(),
        "email" => {
            !config.channels.email.imap_host.is_empty()
                && !config.channels.email.smtp_host.is_empty()
        }
        "qq" => {
            !config.channels.qq.app_id.is_empty() && !config.channels.qq.secret.expose().is_empty()
        }
        _ => false,
    }
}

/// Check if a channel is enabled.
pub(super) fn is_channel_enabled(config: &Config, channel_key: &str) -> bool {
    match channel_key {
        "telegram" => config.channels.telegram.enabled,
        "discord" => config.channels.discord.enabled,
        "slack" => config.channels.slack.enabled,
        "whatsapp" => config.channels.whatsapp.enabled,
        "email" => config.channels.email.enabled,
        "qq" => config.channels.qq.enabled,
        _ => false,
    }
}

/// Get the count of allowlist entries for a channel.
pub(super) fn get_allowlist_count(config: &Config, channel_key: &str) -> usize {
    match channel_key {
        "telegram" => config.channels.telegram.allow_from.len(),
        "discord" => config.channels.discord.allow_from.len(),
        "slack" => config.channels.slack.allow_from.len(),
        "whatsapp" => config.channels.whatsapp.allow_from.len(),
        "email" => config.channels.email.allow_from.len(),
        "qq" => config.channels.qq.allow_from.len(),
        _ => 0,
    }
}

/// Get a masked version of channel credentials for display.
pub(super) fn mask_channel_credentials(config: &Config, channel_key: &str) -> String {
    use crate::wizard::prompts::mask_secret;

    match channel_key {
        "telegram" => mask_secret(config.channels.telegram.token.expose()),
        "discord" => mask_secret(config.channels.discord.token.expose()),
        "slack" => mask_secret(config.channels.slack.bot_token.expose()),
        "whatsapp" => {
            let url = &config.channels.whatsapp.bridge_url;
            if url.len() > 20 {
                format!("{}...", &url[..17])
            } else {
                url.clone()
            }
        }
        "email" => format!(
            "{}@{}",
            config.channels.email.imap_username, config.channels.email.imap_host
        ),
        "qq" => config.channels.qq.app_id.clone(),
        _ => String::new(),
    }
}

/// Get a human-readable status description for a configured channel.
pub(super) fn get_channel_status_description(config: &Config, channel_key: &str) -> String {
    match channel_key {
        "telegram" => "configured".to_string(),
        "discord" => "configured".to_string(),
        "slack" => "configured".to_string(),
        "whatsapp" => config.channels.whatsapp.bridge_url.clone(),
        "email" => config.channels.email.from_address.clone(),
        "qq" => format!("App ID: {}", config.channels.qq.app_id),
        _ => String::new(),
    }
}

// ============================================================================
// State Model and Sub-Actions
// ============================================================================

/// Actions available in the expanded channel sub-menu.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ChannelSubAction {
    ConfigureCredentials,
    TestConnection,
    ToggleEnabled,
    ManageAllowlist,
    Reconnect,
    Close,
}

pub(super) const SUB_ACTIONS: &[ChannelSubAction] = &[
    ChannelSubAction::ConfigureCredentials,
    ChannelSubAction::TestConnection,
    ChannelSubAction::ToggleEnabled,
    ChannelSubAction::ManageAllowlist,
    ChannelSubAction::Reconnect,
    ChannelSubAction::Close,
];

impl ChannelSubAction {
    /// Check if this action is available given the channel's configuration state.
    pub(super) fn is_available(&self, configured: bool, enabled: bool) -> bool {
        match self {
            Self::ConfigureCredentials | Self::Close => true,
            Self::TestConnection | Self::ToggleEnabled | Self::ManageAllowlist => configured,
            Self::Reconnect => configured && enabled,
        }
    }

    /// Get the display label for this action, adapting to the channel's state.
    pub(super) fn label(&self, config: &Config, channel: &ChannelInfo) -> String {
        let configured = is_channel_configured(config, channel.key);
        let enabled = is_channel_enabled(config, channel.key);

        match self {
            Self::ConfigureCredentials => {
                if configured {
                    let masked = mask_channel_credentials(config, channel.key);
                    format!("Edit credentials ({})", masked)
                } else {
                    "Configure credentials".to_string()
                }
            }
            Self::TestConnection => {
                if configured {
                    "Test connection".to_string()
                } else {
                    "Test connection (requires credentials)".to_string()
                }
            }
            Self::ToggleEnabled => {
                let status = if enabled { "✓ enabled" } else { "disabled" };
                format!("Toggle enable/disable [{}]", status)
            }
            Self::ManageAllowlist => {
                let count = get_allowlist_count(config, channel.key);
                if count == 0 {
                    "Manage allowlist [empty - allows all]".to_string()
                } else {
                    format!("Manage allowlist [{} users]", count)
                }
            }
            Self::Reconnect => "Reconnect channel".to_string(),
            Self::Close => "Close".to_string(),
        }
    }
}

/// State for the interactive channel menu.
pub(super) struct ChannelMenuState {
    pub cursor: usize,
    pub expanded: Option<usize>,
    pub sub_cursor: usize,
    pub in_sub_menu: bool,
    pub can_go_back: bool,
}

impl ChannelMenuState {
    pub fn new(can_go_back: bool) -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
            can_go_back,
        }
    }

    pub fn total_main_items(&self) -> usize {
        CHANNELS.len() + 1 + if self.can_go_back { 1 } else { 0 }
    }

    pub fn is_on_back(&self) -> bool {
        self.can_go_back && self.cursor == CHANNELS.len()
    }

    pub fn is_on_done(&self) -> bool {
        let done_idx = CHANNELS.len() + if self.can_go_back { 1 } else { 0 };
        self.cursor == done_idx
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Execute the configure credentials action for a channel.
pub(super) async fn execute_configure_credentials(
    config: &mut Config,
    channel_idx: usize,
) -> Result<()> {
    let channel = &CHANNELS[channel_idx];

    match channel.key {
        "telegram" => {
            configure::configure_telegram(config).await?;
        }
        "discord" => {
            configure::configure_discord(config).await?;
        }
        "slack" => {
            configure::configure_slack(config).await?;
        }
        "whatsapp" => {
            configure::configure_whatsapp(config).await?;
        }
        "email" => {
            configure::configure_email(config).await?;
        }
        "qq" => {
            configure::configure_qq(config).await?;
        }
        _ => {}
    }

    Ok(())
}

/// Execute the test connection action for a channel.
pub(super) async fn execute_test_connection(config: &Config, channel_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let channel = &CHANNELS[channel_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Testing {} connection...",
        prefix,
        colorize("●", BRAND),
        channel.name
    );

    match channel.key {
        "telegram" => {
            let token = config.channels.telegram.token.expose();
            let validation = configure::validate_telegram_token(token).await;
            match validation {
                Ok(bot_name) => {
                    println!(
                        "{}{} Connection successful — bot: @{}",
                        prefix,
                        status_success(),
                        bot_name
                    );
                }
                Err(e) => {
                    println!("{}{} Connection failed: {}", prefix, status_error(), e);
                }
            }
        }
        "discord" => {
            let token = config.channels.discord.token.expose();
            let validation = crate::wizard::oauth::validate_discord_token(token).await;
            match validation {
                Ok(bot_name) => {
                    println!(
                        "{}{} Connection successful — bot: {}",
                        prefix,
                        status_success(),
                        bot_name
                    );
                }
                Err(e) => {
                    println!("{}{} Connection failed: {}", prefix, status_error(), e);
                }
            }
        }
        "slack" => {
            let bot_token = config.channels.slack.bot_token.expose();
            let validation = crate::wizard::oauth::validate_slack_token(bot_token).await;
            match validation {
                Ok((_bot_id, team)) => {
                    println!(
                        "{}{} Connection successful — workspace: {}",
                        prefix,
                        status_success(),
                        team
                    );
                }
                Err(e) => {
                    println!("{}{} Connection failed: {}", prefix, status_error(), e);
                }
            }
        }
        "whatsapp" => {
            let bridge_url = &config.channels.whatsapp.bridge_url;
            let test = configure::test_websocket_connection(bridge_url).await;
            match test {
                Ok(()) => {
                    println!(
                        "{}{} Bridge reachable at {}",
                        prefix,
                        status_success(),
                        bridge_url
                    );
                }
                Err(e) => {
                    println!("{}{} Bridge not reachable: {}", prefix, status_error(), e);
                }
            }
        }
        "email" => {
            let test = configure::test_imap_connection(
                &config.channels.email.imap_host,
                config.channels.email.imap_port,
                &config.channels.email.imap_username,
                config.channels.email.imap_password.expose(),
                config.channels.email.imap_use_ssl,
            )
            .await;
            match test {
                Ok(()) => {
                    println!("{}{} IMAP connection successful", prefix, status_success());
                }
                Err(e) => {
                    println!("{}{} IMAP connection failed: {}", prefix, status_error(), e);
                }
            }
        }
        "qq" => {
            let validation = configure::validate_qq_credentials(
                &config.channels.qq.app_id,
                config.channels.qq.secret.expose(),
            )
            .await;
            match validation {
                Ok(()) => {
                    println!("{}{} Credentials valid", prefix, status_success());
                }
                Err(e) => {
                    println!("{}{} Validation failed: {}", prefix, status_error(), e);
                }
            }
        }
        _ => {}
    }

    println!("{}", colorize(chars.vertical, BRAND));
    prompts::prompt_text("Press Enter to continue", Some(""), false)?;

    Ok(())
}

/// Execute the toggle enabled action for a channel.
pub(super) fn execute_toggle_enabled(config: &mut Config, channel_idx: usize) -> Result<()> {
    let channel = &CHANNELS[channel_idx];

    match channel.key {
        "telegram" => config.channels.telegram.enabled = !config.channels.telegram.enabled,
        "discord" => config.channels.discord.enabled = !config.channels.discord.enabled,
        "slack" => config.channels.slack.enabled = !config.channels.slack.enabled,
        "whatsapp" => config.channels.whatsapp.enabled = !config.channels.whatsapp.enabled,
        "email" => config.channels.email.enabled = !config.channels.email.enabled,
        "qq" => config.channels.qq.enabled = !config.channels.qq.enabled,
        _ => {}
    }

    Ok(())
}

/// Execute the manage allowlist action for a channel.
pub(super) fn execute_manage_allowlist(config: &mut Config, channel_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let channel = &CHANNELS[channel_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Manage allowlist for {}",
        prefix,
        colorize("●", BRAND),
        channel.name
    );

    let new_allowlist = prompt_allowlist(channel.name)?;

    match channel.key {
        "telegram" => config.channels.telegram.allow_from = new_allowlist,
        "discord" => config.channels.discord.allow_from = new_allowlist,
        "slack" => config.channels.slack.allow_from = new_allowlist,
        "whatsapp" => config.channels.whatsapp.allow_from = new_allowlist,
        "email" => config.channels.email.allow_from = new_allowlist,
        "qq" => config.channels.qq.allow_from = new_allowlist,
        _ => {}
    }

    println!("{}{} Allowlist updated", prefix, status_success());

    Ok(())
}

/// Execute the reconnect action for a channel.
pub(super) async fn execute_reconnect(channel_key: &str) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Reconnect feature coming soon for {}",
        prefix,
        colorize("●", BRAND),
        channel_key
    );
    println!(
        "{}{}",
        prefix,
        colorize("Restart klyntbot to reconnect this channel.", DIM)
    );
    println!("{}", colorize(chars.vertical, BRAND));
    prompts::prompt_text("Press Enter to continue", Some(""), false)?;

    Ok(())
}

// ============================================================================
// Public entry point
// ============================================================================

/// Run the channel configuration wizard step.
/// Returns a `MenuOutcome` indicating whether the user chose Done or Back.
pub(crate) async fn configure_channels(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();

    println!(
        "{} {} Connect klyntbot to your chat platforms.",
        colorize(chars.vertical, BRAND),
        colorize("Channels", BOLD)
    );
    println!(
        "{}",
        draw_step_line(&colorize("Configure and manage channel connections.", DIM))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Show currently configured channels summary
    let configured_count = CHANNELS
        .iter()
        .filter(|c| is_channel_configured(config, c.key))
        .count();

    if configured_count > 0 {
        println!(
            "{} {} channels currently configured",
            colorize(chars.vertical, BRAND),
            colorize(&configured_count.to_string(), BOLD)
        );
        println!("{}", colorize(chars.vertical, BRAND));
    }

    // Run interactive menu (TTY) or fallback (non-TTY)
    let outcome = if io::stdin().is_terminal() && io::stdout().is_terminal() {
        menus::run_channel_menu(config, can_go_back).await?
    } else {
        menus::run_channel_menu_fallback(config, can_go_back).await?
    };

    if outcome == MenuOutcome::Back {
        return Ok(MenuOutcome::Back);
    }

    // Collect list of configured channels
    let configured: Vec<String> = CHANNELS
        .iter()
        .filter(|c| is_channel_configured(config, c.key))
        .map(|c| c.key.to_string())
        .collect();

    // Summary
    if !configured.is_empty() {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} {} channel(s) configured: {}",
            colorize(chars.vertical, BRAND),
            status_success(),
            configured.len(),
            configured.join(", ")
        );
    } else {
        println!(
            "{} {} No channels configured. You can set them up later with:",
            colorize(chars.vertical, BRAND),
            colorize("Skipped.", DIM)
        );
        println!("{}", draw_step_line(&colorize("  klyntbot init", DIM)));
    }

    Ok(MenuOutcome::Done)
}

// ============================================================================
// Shared prompt helpers
// ============================================================================

/// Prompt for allowlist entries (comma-separated user IDs).
pub(super) fn prompt_allowlist(channel_name: &str) -> Result<Vec<String>> {
    let chars = BoxChars::get();

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Restrict who can use klyntbot via {}.",
        colorize(chars.vertical, BRAND),
        colorize("Allowlist:", BOLD),
        channel_name
    );
    println!(
        "{}",
        draw_step_line(&colorize("Leave empty to allow everyone.", DIM))
    );

    print!(
        "{} Allowed IDs (comma-separated): ",
        colorize(chars.vertical, BRAND)
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(vec![])
    } else {
        Ok(input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_info_count() {
        assert_eq!(CHANNELS.len(), 6);
    }

    #[test]
    fn test_channel_keys_unique() {
        let keys: Vec<&str> = CHANNELS.iter().map(|c| c.key).collect();
        let mut unique_keys = keys.clone();
        unique_keys.sort();
        unique_keys.dedup();
        assert_eq!(keys.len(), unique_keys.len());
    }

    #[test]
    fn test_channel_names_match_keys() {
        for channel in CHANNELS {
            assert_eq!(channel.key, channel.name.to_lowercase());
        }
    }
}
