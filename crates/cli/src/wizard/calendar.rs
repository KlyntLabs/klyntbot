//! Calendar sync configuration wizard step.
//!
//! Provides an expand-in-place interactive menu for calendar provider
//! management, modeled after the channels step. Each calendar provider
//! (currently Apple Calendar via CalDAV) is an expandable row with
//! sub-actions for credentials, settings, and connection testing.
//!
//! Designed for future expansion (Google Calendar, Outlook, etc.).

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::schema::Secret;
use config::Config;

use super::prompts;

// ============================================================================
// Calendar Provider Metadata
// ============================================================================

/// Metadata for a calendar provider in the selection UI.
#[allow(dead_code)]
struct CalendarProviderInfo {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    prerequisites: &'static str,
}

const CALENDAR_PROVIDERS: &[CalendarProviderInfo] = &[CalendarProviderInfo {
    name: "Apple Calendar",
    key: "apple",
    description: "CalDAV sync with iCloud",
    prerequisites: "Apple ID + app-specific password",
}];

// ============================================================================
// Configuration Detection Helpers
// ============================================================================

/// Check if a calendar provider is configured (has credentials).
fn is_provider_configured(config: &Config, provider_key: &str) -> bool {
    match provider_key {
        "apple" => {
            !config.calendar.username.is_empty()
                && !config.calendar.password.expose().is_empty()
        }
        _ => false,
    }
}

/// Check if a calendar provider is enabled.
fn is_provider_enabled(config: &Config, provider_key: &str) -> bool {
    match provider_key {
        "apple" => config.calendar.enabled,
        _ => false,
    }
}

/// Get a masked version of provider credentials for display.
fn mask_provider_credentials(config: &Config, provider_key: &str) -> String {
    match provider_key {
        "apple" => config.calendar.username.clone(),
        _ => String::new(),
    }
}

/// Get a human-readable status description for a configured provider.
fn get_provider_status_description(config: &Config, provider_key: &str) -> String {
    match provider_key {
        "apple" => {
            if !config.calendar.calendar_name.is_empty() {
                let interval = format_interval(config.calendar.sync_interval_secs);
                format!("{} / {}", config.calendar.calendar_name, interval)
            } else {
                "configured".to_string()
            }
        }
        _ => String::new(),
    }
}

/// Format sync interval seconds as a human-readable string.
fn format_interval(secs: u64) -> String {
    match secs {
        0..=60 => "1 min".to_string(),
        61..=300 => "5 min".to_string(),
        301..=900 => "15 min".to_string(),
        _ => "30 min".to_string(),
    }
}

// ============================================================================
// State Model and Sub-Actions
// ============================================================================

/// Actions available in the expanded calendar provider sub-menu.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CalendarSubAction {
    ConfigureCredentials,
    TestConnection,
    ToggleEnabled,
    ChangeCalendarName,
    ChangeSyncInterval,
    Close,
}

const SUB_ACTIONS: &[CalendarSubAction] = &[
    CalendarSubAction::ConfigureCredentials,
    CalendarSubAction::TestConnection,
    CalendarSubAction::ToggleEnabled,
    CalendarSubAction::ChangeCalendarName,
    CalendarSubAction::ChangeSyncInterval,
    CalendarSubAction::Close,
];

impl CalendarSubAction {
    /// Check if this action is available given the provider's configuration state.
    fn is_available(&self, configured: bool, _enabled: bool) -> bool {
        match self {
            Self::ConfigureCredentials | Self::Close => true,
            Self::TestConnection | Self::ToggleEnabled => configured,
            Self::ChangeCalendarName | Self::ChangeSyncInterval => configured,
        }
    }

    /// Get the display label for this action, adapting to the provider's state.
    fn label(&self, config: &Config, provider: &CalendarProviderInfo) -> String {
        let configured = is_provider_configured(config, provider.key);
        let enabled = is_provider_enabled(config, provider.key);

        match self {
            Self::ConfigureCredentials => {
                if configured {
                    let masked = mask_provider_credentials(config, provider.key);
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
            Self::ChangeCalendarName => {
                let name = &config.calendar.calendar_name;
                if name.is_empty() {
                    "Calendar name".to_string()
                } else {
                    format!("Calendar name [{}]", name)
                }
            }
            Self::ChangeSyncInterval => {
                let interval = format_interval(config.calendar.sync_interval_secs);
                format!("Sync interval [{}]", interval)
            }
            Self::Close => "Close".to_string(),
        }
    }
}

/// State for the interactive calendar menu.
struct CalendarMenuState {
    cursor: usize,
    expanded: Option<usize>,
    sub_cursor: usize,
    in_sub_menu: bool,
}

impl CalendarMenuState {
    fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    fn total_main_items(&self) -> usize {
        CALENDAR_PROVIDERS.len() + 1 // providers + "Done"
    }

    fn is_on_done(&self) -> bool {
        self.cursor == CALENDAR_PROVIDERS.len()
    }
}

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full calendar menu list. Returns total lines rendered.
fn render_calendar_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &CalendarMenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut lines = 0;

    for (i, provider) in CALENDAR_PROVIDERS.iter().enumerate() {
        let configured = is_provider_configured(config, provider.key);
        let enabled = is_provider_enabled(config, provider.key);
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        // Provider icon
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

        // Provider name + status
        let name = if is_cursor || is_expanded {
            colorize(provider.name, BOLD)
        } else if !configured {
            colorize(provider.name, DIM)
        } else {
            provider.name.to_string()
        };

        let status = if configured {
            let desc = get_provider_status_description(config, provider.key);
            let enabled_text = if enabled { "enabled" } else { "disabled" };
            format!(
                " {} — {}",
                colorize(
                    &format!("({})", enabled_text),
                    if enabled { SUCCESS } else { DIM }
                ),
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

                let label = action.label(config, provider);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    if available {
                        colorize(&label, BOLD)
                    } else {
                        colorize(&label, DIM)
                    }
                } else if *action == CalendarSubAction::Close {
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

    write!(
        out,
        "{}{} {} {}\r\n",
        prefix, done_pointer, done_icon, done_label
    )?;
    lines += 1;

    out.flush()?;
    Ok(lines)
}

/// Render the keyboard hint bar at the bottom of the menu.
fn render_menu_hint(
    out: &mut impl Write,
    menu: &CalendarMenuState,
    chars: &BoxChars,
) -> Result<()> {
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
    menu: &CalendarMenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_calendar_menu(out, config, menu, chars)?;
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
    use crossterm::{
        cursor,
        terminal::{self, ClearType},
    };

    let mut out = io::stdout();
    for _ in 0..n {
        crossterm::execute!(out, cursor::MoveUp(1), terminal::Clear(ClearType::CurrentLine))?;
    }
    Ok(())
}

/// Run the interactive calendar menu.
async fn run_calendar_menu(config: &mut Config) -> Result<()> {
    use crossterm::{event::KeyCode, terminal};

    let chars = BoxChars::get();
    let mut menu = CalendarMenuState::new();
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
    render_menu_hint(&mut out, &menu, chars)?;


    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !menu.in_sub_menu => {
                if menu.cursor > 0 {
                    menu.cursor -= 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                let provider_idx = menu.expanded.unwrap();
                let provider = &CALENDAR_PROVIDERS[provider_idx];
                let configured = is_provider_configured(config, provider.key);
                let enabled = is_provider_enabled(config, provider.key);

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
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                let provider_idx = menu.expanded.unwrap();
                let provider = &CALENDAR_PROVIDERS[provider_idx];
                let configured = is_provider_configured(config, provider.key);
                let enabled = is_provider_enabled(config, provider.key);

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
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_done() {
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(());
                } else {
                    // Expand provider sub-menu
                    menu.expanded = Some(menu.cursor);
                    menu.in_sub_menu = true;
                    menu.sub_cursor = 0;

                    // Start on first available action
                    let provider = &CALENDAR_PROVIDERS[menu.cursor];
                    let configured = is_provider_configured(config, provider.key);
                    let enabled = is_provider_enabled(config, provider.key);

                    for (i, action) in SUB_ACTIONS.iter().enumerate() {
                        if action.is_available(configured, enabled) {
                            menu.sub_cursor = i;
                            break;
                        }
                    }

                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Enter if menu.in_sub_menu => {
                let provider_idx = menu.expanded.unwrap();
                let action = SUB_ACTIONS[menu.sub_cursor];
                let provider = &CALENDAR_PROVIDERS[provider_idx];

                let configured = is_provider_configured(config, provider.key);
                let enabled = is_provider_enabled(config, provider.key);
                if !action.is_available(configured, enabled) {
                    continue;
                }

                match action {
                    CalendarSubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines =
                            rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    CalendarSubAction::ConfigureCredentials => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_configure_credentials(config, provider_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::TestConnection => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_test_connection(config, provider_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::ToggleEnabled => {
                        execute_toggle_enabled(config, provider_idx);
                        list_lines =
                            rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    CalendarSubAction::ChangeCalendarName => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_calendar_name(config, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::ChangeSyncInterval => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_sync_interval(config, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            _ => {}
        }
    }
}

/// Run a simplified calendar menu for non-TTY environments.
async fn run_calendar_menu_fallback(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    loop {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} Select a calendar provider to configure:",
            colorize(chars.vertical, BRAND)
        );
        println!("{}", colorize(chars.vertical, BRAND));

        for (i, provider) in CALENDAR_PROVIDERS.iter().enumerate() {
            let configured = is_provider_configured(config, provider.key);
            let status = if configured {
                colorize("(configured)", SUCCESS)
            } else {
                colorize("(not configured)", DIM)
            };
            println!(
                "{}  {}. {} {} — {}",
                colorize(chars.vertical, BRAND),
                i + 1,
                provider.name,
                status,
                colorize(provider.description, DIM)
            );
        }

        println!(
            "{}  {}. Done",
            colorize(chars.vertical, BRAND),
            CALENDAR_PROVIDERS.len() + 1,
        );
        println!("{}", colorize(chars.vertical, BRAND));

        let choice = prompts::prompt_text("Enter number", None, true)?;
        let idx = match choice.parse::<usize>() {
            Ok(n) if n > 0 && n <= CALENDAR_PROVIDERS.len() + 1 => n - 1,
            _ => {
                println!(
                    "{} Invalid choice. Please enter a number between 1 and {}.",
                    colorize(chars.vertical, BRAND),
                    CALENDAR_PROVIDERS.len() + 1
                );
                continue;
            }
        };

        if idx == CALENDAR_PROVIDERS.len() {
            return Ok(());
        }

        // Configure selected provider
        println!("{}", colorize(chars.vertical, BRAND));
        execute_configure_credentials(config, idx).await?;
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Execute the configure credentials action for a calendar provider.
#[allow(clippy::single_match)]
async fn execute_configure_credentials(config: &mut Config, provider_idx: usize) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => configure_apple_calendar(config).await?,
        _ => {}
    }

    Ok(())
}

/// Execute the test connection action for a calendar provider.
#[allow(clippy::single_match)]
async fn execute_test_connection(config: &Config, provider_idx: usize) -> Result<()> {
    let chars = BoxChars::get();
    let provider = &CALENDAR_PROVIDERS[provider_idx];
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    println!(
        "{}{} Testing {} connection...",
        prefix,
        colorize("●", BRAND),
        provider.name
    );

    match provider.key {
        "apple" => {
            let result = test_caldav_connection(
                &config.calendar.caldav_url,
                &config.calendar.username,
                config.calendar.password.expose(),
            )
            .await;

            match result {
                Ok(()) => {
                    println!(
                        "{}{} Connection successful — {}",
                        prefix,
                        status_success(),
                        config.calendar.username
                    );
                }
                Err(e) => {
                    println!("{}{} Connection failed: {}", prefix, status_error(), e);
                }
            }
        }
        _ => {}
    }

    println!("{}", colorize(chars.vertical, BRAND));
    prompts::prompt_text("Press Enter to continue", Some(""), false)?;

    Ok(())
}

/// Execute the toggle enabled action for a calendar provider.
#[allow(clippy::single_match)]
fn execute_toggle_enabled(config: &mut Config, provider_idx: usize) {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => config.calendar.enabled = !config.calendar.enabled,
        _ => {}
    }
}

/// Execute the change calendar name action.
#[allow(clippy::single_match)]
fn execute_change_calendar_name(config: &mut Config, provider_idx: usize) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => {
            let current = if config.calendar.calendar_name.is_empty() {
                "Klyntbot"
            } else {
                &config.calendar.calendar_name
            };
            let name = prompts::prompt_text("Calendar name", Some(current), false)?;
            config.calendar.calendar_name = name;
        }
        _ => {}
    }

    Ok(())
}

/// Execute the change sync interval action.
#[allow(clippy::single_match)]
fn execute_change_sync_interval(config: &mut Config, provider_idx: usize) -> Result<()> {
    let provider = &CALENDAR_PROVIDERS[provider_idx];

    match provider.key {
        "apple" => {
            let interval_options = vec![
                prompts::SelectOption {
                    label: "1 minute",
                    description: "Most frequent (higher battery usage)",
                },
                prompts::SelectOption {
                    label: "5 minutes",
                    description: "Recommended balance",
                },
                prompts::SelectOption {
                    label: "15 minutes",
                    description: "Less frequent updates",
                },
                prompts::SelectOption {
                    label: "30 minutes",
                    description: "Least frequent",
                },
            ];

            let interval_default = match config.calendar.sync_interval_secs {
                0..=60 => 0,
                61..=300 => 1,
                301..=900 => 2,
                _ => 3,
            };

            let selected =
                prompts::prompt_select("Sync interval", &interval_options, interval_default)?;

            config.calendar.sync_interval_secs = match selected {
                0 => 60,
                1 => 300,
                2 => 900,
                3 => 1800,
                _ => 300,
            };
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
// Apple Calendar Configuration
// ============================================================================

/// Configure Apple Calendar credentials (Apple ID + app-specific password).
async fn configure_apple_calendar(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    // Apple ID email
    let existing_username = &config.calendar.username;
    let username_default = if existing_username.is_empty() {
        None
    } else {
        Some(existing_username.as_str())
    };
    let username =
        prompt_text_with_validation("Apple ID email", username_default, true, validate_email)?;
    config.calendar.username = username;

    // App-specific password
    let existing_password = config.calendar.password.expose().clone();
    let password = if existing_password.is_empty() {
        display_password_instructions(chars);
        prompts::prompt_secret("App-specific password", 16)?
    } else {
        match prompts::prompt_secret_with_existing("App-specific password", &existing_password, 16)?
        {
            Some(new_password) => new_password,
            None => existing_password,
        }
    };
    config.calendar.password = Secret::new(password);

    // Calendar name
    let cal_default = if config.calendar.calendar_name.is_empty() {
        "Klyntbot"
    } else {
        &config.calendar.calendar_name
    };
    let calendar_name = prompts::prompt_text("Calendar name", Some(cal_default), false)?;
    config.calendar.calendar_name = calendar_name;

    // Sync interval
    let interval_options = vec![
        prompts::SelectOption {
            label: "1 minute",
            description: "Most frequent (higher battery usage)",
        },
        prompts::SelectOption {
            label: "5 minutes",
            description: "Recommended balance",
        },
        prompts::SelectOption {
            label: "15 minutes",
            description: "Less frequent updates",
        },
        prompts::SelectOption {
            label: "30 minutes",
            description: "Least frequent",
        },
    ];

    let interval_default = match config.calendar.sync_interval_secs {
        0..=60 => 0,
        61..=300 => 1,
        301..=900 => 2,
        _ => 3,
    };
    let selected =
        prompts::prompt_select("Sync interval", &interval_options, interval_default)?;

    config.calendar.sync_interval_secs = match selected {
        0 => 60,
        1 => 300,
        2 => 900,
        3 => 1800,
        _ => 300,
    };

    // Set defaults
    config.calendar.enabled = true;
    config.calendar.caldav_url = "https://caldav.icloud.com".to_string();

    Ok(())
}

/// Display instructions for generating an app-specific password.
fn display_password_instructions(chars: &BoxChars) {
    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{}",
        draw_step_line(&colorize("Generate an app-specific password:", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize("  1. Visit appleid.apple.com", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize("  2. Sign in with your Apple ID", DIM))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  3. Go to Security → App-Specific Passwords",
            DIM
        ))
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "  4. Generate a password for \"Klyntbot\"",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));
}

// ============================================================================
// Connection Testing
// ============================================================================

/// Test a CalDAV connection by attempting to reach the server.
async fn test_caldav_connection(url: &str, username: &str, password: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND")?, url)
        .header("Depth", "0")
        .header("Content-Type", "application/xml")
        .basic_auth(username, Some(password))
        .body(
            r#"<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <current-user-principal/>
  </prop>
</propfind>"#,
        )
        .send()
        .await?;

    if resp.status().is_success() || resp.status().as_u16() == 207 {
        Ok(())
    } else if resp.status().as_u16() == 401 {
        anyhow::bail!("Authentication failed — check credentials")
    } else {
        anyhow::bail!("HTTP {}", resp.status())
    }
}

// ============================================================================
// Validation Helpers
// ============================================================================

/// Prompt for text input with validation.
fn prompt_text_with_validation<F>(
    label: &str,
    default: Option<&str>,
    required: bool,
    validator: F,
) -> Result<String>
where
    F: Fn(&str) -> Result<(), String>,
{
    let prefix = {
        let chars = BoxChars::get();
        format!("{} ", colorize(chars.vertical, BRAND))
    };

    loop {
        let input = prompts::prompt_text(label, default, required)?;

        if input.is_empty() && !required {
            return Ok(input);
        }

        match validator(&input) {
            Ok(()) => return Ok(input),
            Err(msg) => {
                println!("{}{}", prefix, colorize(&msg, ERROR));
                continue;
            }
        }
    }
}

/// Validate email format (basic check).
fn validate_email(email: &str) -> Result<(), String> {
    if email.is_empty() {
        return Err("Email cannot be empty".to_string());
    }

    if !email.contains('@') {
        return Err("Invalid email format (missing @)".to_string());
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Invalid email format".to_string());
    }

    if !parts[1].contains('.') {
        return Err("Invalid email domain".to_string());
    }

    Ok(())
}

// ============================================================================
// Public Entry Point
// ============================================================================

/// Run the calendar configuration wizard step.
///
/// Presents an interactive expand-in-place menu for calendar provider
/// management, similar to the channels step.
pub async fn configure_calendars(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();

    println!(
        "{} {} Sync your calendar with klyntbot.",
        colorize(chars.vertical, BRAND),
        colorize("Calendar", BOLD)
    );
    println!(
        "{}",
        draw_step_line(&colorize(
            "Configure calendar integrations for todo sync.",
            DIM
        ))
    );
    println!("{}", colorize(chars.vertical, BRAND));

    // Show currently configured providers summary
    let configured_count = CALENDAR_PROVIDERS
        .iter()
        .filter(|p| is_provider_configured(config, p.key))
        .count();

    if configured_count > 0 {
        println!(
            "{} {} calendar(s) currently configured",
            colorize(chars.vertical, BRAND),
            colorize(&configured_count.to_string(), BOLD)
        );
        println!("{}", colorize(chars.vertical, BRAND));
    }

    // Run interactive menu (TTY) or fallback (non-TTY)
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        run_calendar_menu(config).await?;
    } else {
        run_calendar_menu_fallback(config).await?;
    }

    // Summary
    let configured: Vec<&str> = CALENDAR_PROVIDERS
        .iter()
        .filter(|p| is_provider_configured(config, p.key))
        .map(|p| p.name)
        .collect();

    if !configured.is_empty() {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} {} {} calendar(s) configured: {}",
            colorize(chars.vertical, BRAND),
            status_success(),
            configured.len(),
            configured.join(", ")
        );
    } else {
        println!(
            "{} {} No calendars configured. You can set them up later with:",
            colorize(chars.vertical, BRAND),
            colorize("Skipped.", DIM)
        );
        println!(
            "{}",
            draw_step_line(&colorize("  klyntbot init", DIM))
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
    fn test_provider_count() {
        assert_eq!(CALENDAR_PROVIDERS.len(), 1);
    }

    #[test]
    fn test_provider_keys_unique() {
        let keys: Vec<&str> = CALENDAR_PROVIDERS.iter().map(|p| p.key).collect();
        let mut unique_keys = keys.clone();
        unique_keys.sort();
        unique_keys.dedup();
        assert_eq!(keys.len(), unique_keys.len());
    }

    #[test]
    fn test_sub_action_count() {
        assert_eq!(SUB_ACTIONS.len(), 6);
    }

    #[test]
    fn test_sub_action_close_is_last() {
        assert_eq!(*SUB_ACTIONS.last().unwrap(), CalendarSubAction::Close);
    }

    #[test]
    fn test_sub_action_availability_unconfigured() {
        // When not configured, only ConfigureCredentials and Close are available
        for action in SUB_ACTIONS {
            let available = action.is_available(false, false);
            match action {
                CalendarSubAction::ConfigureCredentials | CalendarSubAction::Close => {
                    assert!(available, "{:?} should be available", action);
                }
                _ => {
                    assert!(!available, "should not be available when unconfigured");
                }
            }
        }
    }

    #[test]
    fn test_sub_action_availability_configured() {
        // When configured, all actions are available
        for action in SUB_ACTIONS {
            assert!(action.is_available(true, true));
        }
    }

    #[test]
    fn test_is_provider_configured_empty() {
        let config = Config::default();
        assert!(!is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_with_credentials() {
        let mut config = Config::default();
        config.calendar.username = "test@example.com".to_string();
        config.calendar.password = Secret::new("test-password".to_string());
        assert!(is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_partial() {
        let mut config = Config::default();
        config.calendar.username = "test@example.com".to_string();
        // No password
        assert!(!is_provider_configured(&config, "apple"));
    }

    #[test]
    fn test_is_provider_enabled() {
        let mut config = Config::default();
        assert!(!is_provider_enabled(&config, "apple"));
        config.calendar.enabled = true;
        assert!(is_provider_enabled(&config, "apple"));
    }

    #[test]
    fn test_is_provider_configured_unknown() {
        let config = Config::default();
        assert!(!is_provider_configured(&config, "google"));
        assert!(!is_provider_enabled(&config, "google"));
    }

    #[test]
    fn test_mask_provider_credentials() {
        let mut config = Config::default();
        config.calendar.username = "user@example.com".to_string();
        assert_eq!(
            mask_provider_credentials(&config, "apple"),
            "user@example.com"
        );
    }

    #[test]
    fn test_format_interval() {
        assert_eq!(format_interval(60), "1 min");
        assert_eq!(format_interval(300), "5 min");
        assert_eq!(format_interval(900), "15 min");
        assert_eq!(format_interval(1800), "30 min");
        assert_eq!(format_interval(3600), "30 min");
    }

    #[test]
    fn test_get_provider_status_description() {
        let mut config = Config::default();
        config.calendar.calendar_name = "My Calendar".to_string();
        config.calendar.sync_interval_secs = 300;
        let desc = get_provider_status_description(&config, "apple");
        assert!(desc.contains("My Calendar"));
        assert!(desc.contains("5 min"));
    }

    #[test]
    fn test_menu_state_new() {
        let menu = CalendarMenuState::new();
        assert_eq!(menu.cursor, 0);
        assert_eq!(menu.expanded, None);
        assert_eq!(menu.sub_cursor, 0);
        assert!(!menu.in_sub_menu);
    }

    #[test]
    fn test_menu_state_total_items() {
        let menu = CalendarMenuState::new();
        assert_eq!(menu.total_main_items(), CALENDAR_PROVIDERS.len() + 1);
    }

    #[test]
    fn test_menu_state_is_on_done() {
        let mut menu = CalendarMenuState::new();
        assert!(!menu.is_on_done());
        menu.cursor = CALENDAR_PROVIDERS.len();
        assert!(menu.is_on_done());
    }

    #[test]
    fn test_sub_action_labels_unconfigured() {
        let config = Config::default();
        let provider = &CALENDAR_PROVIDERS[0];

        let label = CalendarSubAction::ConfigureCredentials.label(&config, provider);
        assert_eq!(label, "Configure credentials");

        let label = CalendarSubAction::Close.label(&config, provider);
        assert_eq!(label, "Close");
    }

    #[test]
    fn test_sub_action_labels_configured() {
        let mut config = Config::default();
        config.calendar.username = "user@example.com".to_string();
        config.calendar.password = Secret::new("pass".to_string());
        config.calendar.enabled = true;
        config.calendar.calendar_name = "Klyntbot".to_string();
        config.calendar.sync_interval_secs = 300;
        let provider = &CALENDAR_PROVIDERS[0];

        let label = CalendarSubAction::ConfigureCredentials.label(&config, provider);
        assert!(label.contains("user@example.com"));

        let label = CalendarSubAction::ToggleEnabled.label(&config, provider);
        assert!(label.contains("✓ enabled"));

        let label = CalendarSubAction::ChangeCalendarName.label(&config, provider);
        assert!(label.contains("Klyntbot"));

        let label = CalendarSubAction::ChangeSyncInterval.label(&config, provider);
        assert!(label.contains("5 min"));
    }

    #[test]
    fn test_toggle_enabled() {
        let mut config = Config::default();
        assert!(!config.calendar.enabled);
        execute_toggle_enabled(&mut config, 0);
        assert!(config.calendar.enabled);
        execute_toggle_enabled(&mut config, 0);
        assert!(!config.calendar.enabled);
    }

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("test.user@domain.co.uk").is_ok());
        assert!(validate_email("user+tag@example.com").is_ok());
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(validate_email("").is_err());
        assert!(validate_email("notanemail").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("user@").is_err());
        assert!(validate_email("user@domain").is_err());
    }
}
