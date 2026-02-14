//! Channel configuration wizard for interactive channel setup.
//!
//! Provides guided configuration for all supported chat channels:
//! Telegram, Discord, Slack, WhatsApp, Email, and QQ.

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::{
    DiscordConfig, EmailConfig, QQConfig, Secret, SlackConfig, TelegramConfig, WhatsAppConfig,
};
use config::Config;

use super::oauth;
use super::prompts;

/// Channel metadata for the selection UI
struct ChannelInfo {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    prerequisites: &'static str,
}

const CHANNELS: &[ChannelInfo] = &[
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
fn is_channel_configured(config: &Config, channel_key: &str) -> bool {
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
fn is_channel_enabled(config: &Config, channel_key: &str) -> bool {
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
fn get_allowlist_count(config: &Config, channel_key: &str) -> usize {
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
fn mask_channel_credentials(config: &Config, channel_key: &str) -> String {
    use super::prompts::mask_secret;

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
        "email" => format!("{}@{}", config.channels.email.imap_username, config.channels.email.imap_host),
        "qq" => config.channels.qq.app_id.clone(),
        _ => String::new(),
    }
}

/// Get a human-readable status description for a configured channel.
fn get_channel_status_description(config: &Config, channel_key: &str) -> String {
    match channel_key {
        "telegram" => {
            // For Telegram, we could potentially show bot username if we stored it
            // For now, just show it's configured
            "configured".to_string()
        }
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
enum ChannelSubAction {
    ConfigureCredentials,
    TestConnection,
    ToggleEnabled,
    ManageAllowlist,
    Reconnect,
    Close,
}

const SUB_ACTIONS: &[ChannelSubAction] = &[
    ChannelSubAction::ConfigureCredentials,
    ChannelSubAction::TestConnection,
    ChannelSubAction::ToggleEnabled,
    ChannelSubAction::ManageAllowlist,
    ChannelSubAction::Reconnect,
    ChannelSubAction::Close,
];

impl ChannelSubAction {
    /// Check if this action is available given the channel's configuration state.
    fn is_available(&self, configured: bool, enabled: bool) -> bool {
        match self {
            Self::ConfigureCredentials | Self::Close => true,
            Self::TestConnection | Self::ToggleEnabled | Self::ManageAllowlist => configured,
            Self::Reconnect => configured && enabled,
        }
    }

    /// Get the display label for this action, adapting to the channel's state.
    fn label(&self, config: &Config, channel: &ChannelInfo) -> String {
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
struct ChannelMenuState {
    cursor: usize,            // 0..CHANNELS.len() for channels, CHANNELS.len() for "Done"
    expanded: Option<usize>,  // Which channel index is expanded
    sub_cursor: usize,        // Position in SUB_ACTIONS
    in_sub_menu: bool,        // Whether navigating inside the sub-menu
}

impl ChannelMenuState {
    fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    fn total_main_items(&self) -> usize {
        CHANNELS.len() + 1 // channels + "Done"
    }

    fn is_on_done(&self) -> bool {
        self.cursor == CHANNELS.len()
    }
}

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full channel menu list. Returns total lines rendered.
fn render_channel_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &ChannelMenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut lines = 0;

    for (i, channel) in CHANNELS.iter().enumerate() {
        let configured = is_channel_configured(config, channel.key);
        let enabled = is_channel_enabled(config, channel.key);
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        // Channel icon
        let icon = if configured && enabled {
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

        // Channel name + status
        let name = if is_cursor || is_expanded {
            colorize(channel.name, BOLD)
        } else if !configured {
            colorize(channel.name, DIM)
        } else {
            channel.name.to_string()
        };

        let status = if configured {
            let desc = get_channel_status_description(config, channel.key);
            let enabled_text = if enabled { "enabled" } else { "disabled" };
            format!(
                " {} — {}",
                colorize(&format!("({})", enabled_text), if enabled { SUCCESS } else { DIM }),
                colorize(&desc, DIM)
            )
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
                let available = action.is_available(configured, enabled);

                let sub_pointer = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize("❯", BRAND)
                } else {
                    " ".to_string()
                };

                let label = action.label(config, channel);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    if available {
                        colorize(&label, BOLD)
                    } else {
                        colorize(&label, DIM)
                    }
                } else if *action == ChannelSubAction::Close {
                    colorize(&format!("── {} ──", label), DIM)
                } else if !available {
                    colorize(&label, DIM)
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
    let done_pointer = if !menu.in_sub_menu && menu.is_on_done() {
        colorize("❯", BRAND)
    } else {
        " ".to_string()
    };
    let done_icon = colorize("●", BRAND);
    let done_label = if !menu.in_sub_menu && menu.is_on_done() {
        colorize("Done", BOLD)
    } else {
        "Done".to_string()
    };
    let done_desc = colorize("(skip channels)", DIM);

    write!(
        out,
        "{}{} {} {} {}\r\n",
        prefix, done_pointer, done_icon, done_label, done_desc
    )?;
    lines += 1;

    out.flush()?;
    Ok(lines)
}

/// Render the keyboard hint bar at the bottom of the menu.
fn render_menu_hint(out: &mut impl Write, menu: &ChannelMenuState, chars: &BoxChars) -> Result<()> {
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

/// Erase and re-render the menu. Returns new line count.
fn rerender_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &ChannelMenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    // Erase previous render (list + hint)
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_channel_menu(out, config, menu, chars)?;
    render_menu_hint(out, menu, chars)?;
    Ok(new_lines)
}

// ============================================================================
// Interactive Event Loop
// ============================================================================

/// Read a keypress event, handling Ctrl+C gracefully.
fn read_key() -> Result<crossterm::event::KeyEvent> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};

    loop {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Err(anyhow::anyhow!("Ctrl+C"));
            }
            return Ok(key);
        }
    }
}

/// Erase `n` lines above cursor using ANSI codes.
fn erase_lines(n: usize) -> Result<()> {
    use crossterm::{cursor, terminal::{self, ClearType}};

    let mut out = io::stdout();
    for _ in 0..n {
        crossterm::execute!(out, cursor::MoveUp(1), terminal::Clear(ClearType::CurrentLine))?;
    }
    Ok(())
}

/// Run the interactive channel menu. Modifies config in place and returns when user selects "Done".
async fn run_channel_menu(config: &mut Config) -> Result<()> {
    use crossterm::{event::KeyCode, terminal};

    let chars = BoxChars::get();
    let mut menu = ChannelMenuState::new();
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_channel_menu(&mut out, config, &menu, &chars)?;
    render_menu_hint(&mut out, &menu, &chars)?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !menu.in_sub_menu => {
                if menu.cursor > 0 {
                    menu.cursor -= 1;
                    // If we moved away from expanded channel, collapse it
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                // Skip disabled actions when navigating up
                let channel_idx = menu.expanded.unwrap();
                let channel = &CHANNELS[channel_idx];
                let configured = is_channel_configured(config, channel.key);
                let enabled = is_channel_enabled(config, channel.key);

                loop {
                    if menu.sub_cursor == 0 {
                        break;
                    }
                    menu.sub_cursor -= 1;
                    let action = SUB_ACTIONS[menu.sub_cursor];
                    if action.is_available(configured, enabled) {
                        break;
                    }
                }
                list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                // Skip disabled actions when navigating down
                let channel_idx = menu.expanded.unwrap();
                let channel = &CHANNELS[channel_idx];
                let configured = is_channel_configured(config, channel.key);
                let enabled = is_channel_enabled(config, channel.key);

                loop {
                    if menu.sub_cursor >= SUB_ACTIONS.len() - 1 {
                        break;
                    }
                    menu.sub_cursor += 1;
                    let action = SUB_ACTIONS[menu.sub_cursor];
                    if action.is_available(configured, enabled) {
                        break;
                    }
                }
                list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
            }
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_done() {
                    // Done — exit
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(());
                } else {
                    // Expand channel sub-menu
                    menu.expanded = Some(menu.cursor);
                    menu.in_sub_menu = true;
                    menu.sub_cursor = 0;

                    // Start on first available action
                    let channel = &CHANNELS[menu.cursor];
                    let configured = is_channel_configured(config, channel.key);
                    let enabled = is_channel_enabled(config, channel.key);

                    for (i, action) in SUB_ACTIONS.iter().enumerate() {
                        if action.is_available(configured, enabled) {
                            menu.sub_cursor = i;
                            break;
                        }
                    }

                    list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
                }
            }
            KeyCode::Enter if menu.in_sub_menu => {
                let channel_idx = menu.expanded.unwrap();
                let action = SUB_ACTIONS[menu.sub_cursor];
                let channel = &CHANNELS[channel_idx];

                // Check if action is available
                let configured = is_channel_configured(config, channel.key);
                let enabled = is_channel_enabled(config, channel.key);
                if !action.is_available(configured, enabled) {
                    // Do nothing on disabled actions
                    continue;
                }

                match action {
                    ChannelSubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
                    }
                    ChannelSubAction::ConfigureCredentials => {
                        // Exit raw mode, erase menu, run configuration, re-enter
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_configure_credentials(config, channel_idx).await?;

                        // Re-enter raw mode and redraw
                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, &chars)?;
                        render_menu_hint(&mut out, &menu, &chars)?;
                    }
                    ChannelSubAction::TestConnection => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_test_connection(config, channel_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, &chars)?;
                        render_menu_hint(&mut out, &menu, &chars)?;
                    }
                    ChannelSubAction::ToggleEnabled => {
                        // Immediate toggle — no need to exit raw mode
                        execute_toggle_enabled(config, channel_idx)?;
                        list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
                    }
                    ChannelSubAction::ManageAllowlist => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_manage_allowlist(config, channel_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, &chars)?;
                        render_menu_hint(&mut out, &menu, &chars)?;
                    }
                    ChannelSubAction::Reconnect => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_reconnect(channel.key).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, &chars)?;
                        render_menu_hint(&mut out, &menu, &chars)?;
                    }
                }
            }
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, &menu, &chars, list_lines)?;
            }
            _ => {}
        }
    }
}

/// Run a simplified channel menu for non-TTY environments.
async fn run_channel_menu_fallback(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    loop {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} Select a channel to configure:",
            colorize(chars.vertical, BRAND)
        );
        println!("{}", colorize(chars.vertical, BRAND));

        for (i, channel) in CHANNELS.iter().enumerate() {
            let configured = is_channel_configured(config, channel.key);
            let status = if configured {
                colorize("(configured)", SUCCESS)
            } else {
                colorize("(not configured)", DIM)
            };
            println!(
                "{}  {}. {} {}",
                colorize(chars.vertical, BRAND),
                i + 1,
                channel.name,
                status
            );
        }

        println!(
            "{}  {}. {}",
            colorize(chars.vertical, BRAND),
            CHANNELS.len() + 1,
            "Done"
        );
        println!("{}", colorize(chars.vertical, BRAND));

        let choice = prompts::prompt_text("Enter number", None, true)?;
        let idx = match choice.parse::<usize>() {
            Ok(n) if n > 0 && n <= CHANNELS.len() + 1 => n - 1,
            _ => {
                println!(
                    "{} Invalid choice. Please enter a number between 1 and {}.",
                    colorize(chars.vertical, BRAND),
                    CHANNELS.len() + 1
                );
                continue;
            }
        };

        if idx == CHANNELS.len() {
            // Done
            return Ok(());
        }

        // Configure selected channel
        println!("{}", colorize(chars.vertical, BRAND));
        execute_configure_credentials(config, idx).await?;
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Execute the configure credentials action for a channel.
async fn execute_configure_credentials(config: &mut Config, channel_idx: usize) -> Result<()> {
    let channel = &CHANNELS[channel_idx];

    match channel.key {
        "telegram" => {
            configure_telegram(config).await?;
        }
        "discord" => {
            configure_discord(config).await?;
        }
        "slack" => {
            configure_slack(config).await?;
        }
        "whatsapp" => {
            configure_whatsapp(config).await?;
        }
        "email" => {
            configure_email(config).await?;
        }
        "qq" => {
            configure_qq(config).await?;
        }
        _ => {}
    }

    Ok(())
}

/// Execute the test connection action for a channel.
async fn execute_test_connection(config: &Config, channel_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let channel = &CHANNELS[channel_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!("{}{} Testing {} connection...", prefix, colorize("●", BRAND), channel.name);

    match channel.key {
        "telegram" => {
            let token = config.channels.telegram.token.expose();
            let validation = validate_telegram_token(token).await;
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
                    println!(
                        "{}{} Connection failed: {}",
                        prefix,
                        status_error(),
                        e
                    );
                }
            }
        }
        "discord" => {
            let token = config.channels.discord.token.expose();
            let validation = oauth::validate_discord_token(token).await;
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
                    println!(
                        "{}{} Connection failed: {}",
                        prefix,
                        status_error(),
                        e
                    );
                }
            }
        }
        "slack" => {
            let bot_token = config.channels.slack.bot_token.expose();
            let validation = oauth::validate_slack_token(bot_token).await;
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
                    println!(
                        "{}{} Connection failed: {}",
                        prefix,
                        status_error(),
                        e
                    );
                }
            }
        }
        "whatsapp" => {
            let bridge_url = &config.channels.whatsapp.bridge_url;
            let test = test_websocket_connection(bridge_url).await;
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
                    println!(
                        "{}{} Bridge not reachable: {}",
                        prefix,
                        status_error(),
                        e
                    );
                }
            }
        }
        "email" => {
            let test = test_imap_connection(
                &config.channels.email.imap_host,
                config.channels.email.imap_port,
                &config.channels.email.imap_username,
                config.channels.email.imap_password.expose(),
                config.channels.email.imap_use_ssl,
            )
            .await;
            match test {
                Ok(()) => {
                    println!(
                        "{}{} IMAP connection successful",
                        prefix,
                        status_success()
                    );
                }
                Err(e) => {
                    println!(
                        "{}{} IMAP connection failed: {}",
                        prefix,
                        status_error(),
                        e
                    );
                }
            }
        }
        "qq" => {
            let validation = validate_qq_credentials(
                &config.channels.qq.app_id,
                config.channels.qq.secret.expose(),
            )
            .await;
            match validation {
                Ok(()) => {
                    println!(
                        "{}{} Credentials valid",
                        prefix,
                        status_success()
                    );
                }
                Err(e) => {
                    println!(
                        "{}{} Validation failed: {}",
                        prefix,
                        status_error(),
                        e
                    );
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
fn execute_toggle_enabled(config: &mut Config, channel_idx: usize) -> Result<()> {
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
fn execute_manage_allowlist(config: &mut Config, channel_idx: usize) -> Result<()> {
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

    println!(
        "{}{} Allowlist updated",
        prefix,
        status_success()
    );

    Ok(())
}

/// Execute the reconnect action for a channel.
async fn execute_reconnect(channel_key: &str) -> Result<()> {
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
        colorize(
            "Restart klyntbot to reconnect this channel.",
            DIM
        )
    );
    println!("{}", colorize(chars.vertical, BRAND));
    prompts::prompt_text("Press Enter to continue", Some(""), false)?;

    Ok(())
}

// ============================================================================
// Public entry point
// ============================================================================

/// Run the channel configuration wizard step.
/// Returns the list of channel names that were successfully configured.
pub async fn configure_channels(config: &mut Config) -> Result<Vec<String>> {
    let chars = BoxChars::get();

    println!(
        "{} {} Connect klyntbot to your chat platforms.",
        colorize(chars.vertical, BRAND),
        colorize("Channels", BOLD)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "Configure and manage channel connections.",
            DIM
        ))
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
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        run_channel_menu(config).await?;
    } else {
        run_channel_menu_fallback(config).await?;
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
        println!(
            "{}",
            draw_step_line(&colorize("  klyntbot init", DIM))
        );
    }

    Ok(configured)
}

// ============================================================================
// Telegram configuration
// ============================================================================

async fn configure_telegram(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Get a bot token from {} in Telegram.",
        colorize(chars.vertical, BRAND),
        colorize("@BotFather", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let token = prompts::prompt_secret("Bot Token", 10)?;

    // Validate token via getMe API
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = validate_telegram_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!(
                "{} {} Token valid — bot: @{}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bot_name
            );
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Telegram")?;

    // Optional proxy
    let proxy = prompts::prompt_optional("Proxy URL (optional, e.g. socks5://...)")?;

    // Apply config
    config.channels.telegram = TelegramConfig {
        enabled: true,
        token: Secret::new(token),
        allow_from,
        proxy,
    };

    Ok(true)
}

/// Validate a Telegram bot token by calling the getMe API.
async fn validate_telegram_token(token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let resp = client.get(&url).send().await?;
    let data: serde_json::Value = resp.json().await?;

    if data.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        let username = data
            .get("result")
            .and_then(|r| r.get("username"))
            .and_then(|u| u.as_str())
            .unwrap_or("unknown");
        Ok(username.to_string())
    } else {
        let description = data
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("{}", description);
    }
}

// ============================================================================
// Discord configuration
// ============================================================================

async fn configure_discord(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Create a bot at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://discord.com/developers/applications", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let token = prompts::prompt_secret("Bot Token", 10)?;

    // Validate token
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating token...");
    spinner.start();

    let validation = oauth::validate_discord_token(&token).await;
    spinner.stop();

    match validation {
        Ok(bot_name) => {
            println!(
                "{} {} Token valid — bot: {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bot_name
            );

            // Generate invite URL using oauth module's properly URL-encoded helper
            let app_id = get_discord_app_id(&token).await.unwrap_or_default();
            if !app_id.is_empty() {
                let invite_url = oauth::discord_bot_invite_url(&app_id, 274877991936);
                println!(
                    "{} {} Invite your bot to a server:",
                    colorize(chars.vertical, BRAND),
                    colorize("Invite URL:", BOLD)
                );
                println!("{}", draw_step_line(&colorize(&invite_url, UNDERLINE)));
            }
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Discord")?;

    // Apply config
    config.channels.discord = DiscordConfig {
        enabled: true,
        token: Secret::new(token),
        allow_from,
        ..DiscordConfig::default()
    };

    Ok(true)
}

/// Get Discord application ID from the bot token.
async fn get_discord_app_id(token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .get("https://discord.com/api/v10/oauth2/applications/@me")
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    let id = data.get("id").and_then(|i| i.as_str()).unwrap_or("");
    Ok(id.to_string())
}

// ============================================================================
// Slack configuration
// ============================================================================

async fn configure_slack(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Create a Slack app at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://api.slack.com/apps", UNDERLINE)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "You need both a Bot Token (xoxb-) and an App Token (xapp-).",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize("Enable Socket Mode in your app settings.", DIM))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bot token
    let bot_token = prompts::prompt_secret("Bot Token (xoxb-...)", 10)?;

    // App token
    let app_token = prompts::prompt_secret("App Token (xapp-...)", 10)?;

    // Validate bot token
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating tokens...");
    spinner.start();

    let validation = oauth::validate_slack_token(&bot_token).await;
    spinner.stop();

    match validation {
        Ok((_bot_id, team)) => {
            println!(
                "{} {} Tokens valid — workspace: {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                team
            );
        }
        Err(e) => {
            println!(
                "{} {} Token validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Slack")?;

    // Apply config
    config.channels.slack = SlackConfig {
        enabled: true,
        bot_token: Secret::new(bot_token),
        app_token: Secret::new(app_token),
        allow_from,
        ..SlackConfig::default()
    };

    Ok(true)
}

// ============================================================================
// WhatsApp configuration
// ============================================================================

async fn configure_whatsapp(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize(
            "WhatsApp requires a Node.js bridge (Baileys) running separately.",
            WARNING
        )
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "See: https://github.com/WhiskeySockets/Baileys for bridge setup.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Bridge URL
    let bridge_url = prompts::prompt_text("Bridge URL", Some("ws://localhost:3001"), false)?;

    // Test bridge connection
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing bridge connection...");
    spinner.start();

    let bridge_ok = test_websocket_connection(&bridge_url).await;
    spinner.stop();

    match bridge_ok {
        Ok(()) => {
            println!(
                "{} {} Bridge reachable at {}",
                colorize(chars.vertical, BRAND),
                status_success(),
                bridge_url
            );
            println!(
                "{}",
                draw_step_line(&colorize(
                    "Note: You'll need to scan a QR code when the bridge starts.",
                    DIM
                ))
            );
        }
        Err(e) => {
            println!(
                "{} {} Bridge not reachable: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            println!(
                "{}",
                draw_step_line(&colorize(
                    "The bridge may not be running yet. Configuration will be saved anyway.",
                    DIM
                ))
            );
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("WhatsApp")?;

    // Apply config
    config.channels.whatsapp = WhatsAppConfig {
        enabled: true,
        bridge_url,
        allow_from,
    };

    Ok(true)
}

/// Test a WebSocket connection (just attempt to connect, then disconnect).
async fn test_websocket_connection(url: &str) -> Result<()> {
    let connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio_tungstenite::connect_async(url),
    )
    .await;

    match connect_result {
        Ok(Ok((_ws_stream, _))) => Ok(()),
        Ok(Err(e)) => anyhow::bail!("{}", e),
        Err(_) => anyhow::bail!("Connection timed out"),
    }
}

// ============================================================================
// Email configuration
// ============================================================================

async fn configure_email(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{}",
        draw_step_line(&colorize(
            "Email channel reads your mailbox via IMAP and sends replies via SMTP.",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "This requires IMAP/SMTP credentials and explicit consent.",
            WARNING
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Consent
    println!(
        "{} {} Email access grants klyntbot permission to:",
        colorize(chars.vertical, BRAND),
        colorize("Privacy notice:", BOLD)
    );
    println!(
        "{} - Read unread emails from your IMAP mailbox",
        colorize(chars.vertical, BRAND)
    );
    println!(
        "{} - Send replies via SMTP on your behalf",
        colorize(chars.vertical, BRAND)
    );
    println!(
        "{} - Mark messages as read",
        colorize(chars.vertical, BRAND)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    if !prompts::prompt_yes_no("Do you consent to email access?", false)? {
        return Ok(false);
    }

    println!("{}", colorize(chars.vertical, BRAND));

    // IMAP configuration
    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize("IMAP (Incoming Mail)", BOLD)
    );
    let imap_host = prompts::prompt_text("IMAP Host (e.g. imap.gmail.com)", None, true)?;
    let imap_port = prompts::prompt_text("IMAP Port", Some("993"), false)?
        .parse::<u16>()
        .unwrap_or(993);
    let imap_username = prompts::prompt_text("IMAP Username (email)", None, true)?;
    let imap_password = prompts::prompt_secret("IMAP Password", 1)?;
    let imap_use_ssl = prompts::prompt_yes_no("Use SSL?", true)?;

    println!("{}", colorize(chars.vertical, BRAND));

    // SMTP configuration
    println!(
        "{} {}",
        colorize(chars.vertical, BRAND),
        colorize("SMTP (Outgoing Mail)", BOLD)
    );
    let smtp_host = prompts::prompt_text("SMTP Host (e.g. smtp.gmail.com)", None, true)?;
    let smtp_port = prompts::prompt_text("SMTP Port", Some("587"), false)?
        .parse::<u16>()
        .unwrap_or(587);

    // Default SMTP credentials to IMAP values
    let smtp_username = prompts::prompt_text("SMTP Username", Some(&imap_username), false)?;
    let smtp_password_input = prompts::prompt_text(
        "SMTP Password (Enter to use IMAP password)",
        Some(""),
        false,
    )?;
    let smtp_password = if smtp_password_input.is_empty() {
        imap_password.clone()
    } else {
        smtp_password_input
    };
    let smtp_use_tls = prompts::prompt_yes_no("Use TLS?", true)?;

    println!("{}", colorize(chars.vertical, BRAND));

    // From address
    let from_address = prompts::prompt_text("From Address", Some(&smtp_username), false)?;

    // Test connections
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing IMAP connection...");
    spinner.start();

    let imap_test = test_imap_connection(
        &imap_host,
        imap_port,
        &imap_username,
        &imap_password,
        imap_use_ssl,
    )
    .await;
    spinner.stop();

    match imap_test {
        Ok(()) => println!(
            "{} {} IMAP connection successful",
            colorize(chars.vertical, BRAND),
            status_success()
        ),
        Err(e) => {
            println!(
                "{} {} IMAP connection failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Testing SMTP connection...");
    spinner.start();

    let smtp_test =
        test_smtp_connection(&smtp_host, smtp_port, &smtp_username, &smtp_password).await;
    spinner.stop();

    match smtp_test {
        Ok(()) => println!(
            "{} {} SMTP connection successful",
            colorize(chars.vertical, BRAND),
            status_success()
        ),
        Err(e) => {
            println!(
                "{} {} SMTP connection failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("Email")?;

    // Apply config
    config.channels.email = EmailConfig {
        enabled: true,
        consent_granted: true,
        imap_host,
        imap_port,
        imap_username,
        imap_password: Secret::new(imap_password),
        imap_use_ssl,
        imap_mailbox: "INBOX".to_string(),
        smtp_host,
        smtp_port,
        smtp_username,
        smtp_password: Secret::new(smtp_password),
        smtp_use_tls,
        from_address,
        allow_from,
        ..EmailConfig::default()
    };

    Ok(true)
}

/// Test IMAP connection by attempting login.
async fn test_imap_connection(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    use_ssl: bool,
) -> Result<()> {
    use tokio::net::TcpStream;

    let connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        TcpStream::connect((host, port)),
    )
    .await;

    let tcp_stream = match connect_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => anyhow::bail!("TCP connection failed: {}", e),
        Err(_) => anyhow::bail!("Connection timed out"),
    };

    if use_ssl {
        let tls_connector = native_tls::TlsConnector::builder().build()?;
        let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);
        let tls_stream = tls_connector.connect(host, tcp_stream).await?;

        let client = async_imap::Client::new(tls_stream);
        let mut session = client
            .login(username, password)
            .await
            .map_err(|(e, _)| anyhow::anyhow!("Login failed: {}", e))?;
        let _ = session.logout().await;
    } else {
        let client = async_imap::Client::new(tcp_stream);
        let mut session = client
            .login(username, password)
            .await
            .map_err(|(e, _)| anyhow::anyhow!("Login failed: {}", e))?;
        let _ = session.logout().await;
    }

    Ok(())
}

/// Test SMTP connection by attempting relay setup.
async fn test_smtp_connection(host: &str, port: u16, username: &str, password: &str) -> Result<()> {
    let host = host.to_string();
    let username = username.to_string();
    let password = password.to_string();

    tokio::task::spawn_blocking(move || {
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::SmtpTransport;

        let creds = Credentials::new(username, password);
        let mailer = SmtpTransport::relay(&host)?
            .credentials(creds)
            .port(port)
            .build();

        mailer.test_connection()?;
        Ok::<(), anyhow::Error>(())
    })
    .await??;

    Ok(())
}

// ============================================================================
// QQ configuration
// ============================================================================

async fn configure_qq(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();

    println!(
        "{} Register a bot at {}",
        colorize(chars.vertical, BRAND),
        colorize("https://q.qq.com", UNDERLINE)
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // App ID
    let app_id = prompts::prompt_text("App ID", None, true)?;

    // Secret
    let secret = prompts::prompt_secret("App Secret", 1)?;

    // Validate credentials
    print!("{} ", colorize(chars.vertical, BRAND));
    let mut spinner = Spinner::new("Validating credentials...");
    spinner.start();

    let validation = validate_qq_credentials(&app_id, &secret).await;
    spinner.stop();

    match validation {
        Ok(()) => {
            println!(
                "{} {} Credentials valid",
                colorize(chars.vertical, BRAND),
                status_success()
            );
        }
        Err(e) => {
            println!(
                "{} {} Credential validation failed: {}",
                colorize(chars.vertical, BRAND),
                status_warning(),
                e
            );
            if !prompts::prompt_yes_no("Continue anyway?", false)? {
                return Ok(false);
            }
        }
    }

    // Allowlist
    let allow_from = prompt_allowlist("QQ")?;

    // Apply config
    config.channels.qq = QQConfig {
        enabled: true,
        app_id,
        secret: Secret::new(secret),
        allow_from,
    };

    Ok(true)
}

/// Validate QQ credentials by attempting to get an access token.
async fn validate_qq_credentials(app_id: &str, secret: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .post("https://api.sgroup.qq.com/app/getAppAccessToken")
        .json(&serde_json::json!({
            "appId": app_id,
            "clientSecret": secret,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let data: serde_json::Value = resp.json().await?;
    if data.get("access_token").is_some() {
        Ok(())
    } else {
        let msg = data
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Invalid credentials");
        anyhow::bail!("{}", msg);
    }
}

// ============================================================================
// Shared prompt helpers
// ============================================================================

/// Draw a separator line (matches mod.rs style).
fn draw_separator() -> String {
    let chars = BoxChars::get();
    format!(
        "{}{}{}",
        color(SEPARATOR),
        chars.horizontal.repeat(60),
        color(RESET)
    )
}

/// Prompt for allowlist entries (comma-separated user IDs).
fn prompt_allowlist(channel_name: &str) -> Result<Vec<String>> {
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
