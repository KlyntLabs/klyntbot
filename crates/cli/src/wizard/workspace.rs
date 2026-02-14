//! Workspace & Notifications wizard step.
//!
//! Provides an expand-in-place interactive menu for:
//! - Template file management (create/regenerate)
//! - Notification target selection (OS native + enabled channels)
//! - Focus reminder toggle
//! - Daily digest toggle + time configuration
//!
//! On entry, auto-creates workspace dirs and templates at the default path.
//! On reconfiguration, all fields show current values as defaults.

use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
};

use super::prompts;
use super::templates;

// ============================================================================
// Setting Metadata
// ============================================================================

struct WorkspaceSettingInfo {
    key: &'static str,
    name: &'static str,
}

const WORKSPACE_SETTINGS: &[WorkspaceSettingInfo] = &[
    WorkspaceSettingInfo {
        key: "templates",
        name: "Template Files",
    },
    WorkspaceSettingInfo {
        key: "targets",
        name: "Notification Targets",
    },
    WorkspaceSettingInfo {
        key: "digest",
        name: "Daily Digest",
    },
];

/// Known workspace template files (name, path relative to workspace root).
const TEMPLATE_FILES: &[(&str, &str)] = &[
    ("AGENTS.md", "AGENTS.md"),
    ("SOUL.md", "SOUL.md"),
    ("USER.md", "USER.md"),
    ("TOOLS.md", "TOOLS.md"),
    ("IDENTITY.md", "IDENTITY.md"),
    ("MEMORY.md", "memory/MEMORY.md"),
];

// ============================================================================
// Configuration Detection Helpers
// ============================================================================

/// Get the icon and color for a setting based on current config state.
fn get_setting_icon(config: &Config, workspace: &Path, key: &str) -> (&'static str, &'static str) {
    match key {
        "templates" => {
            let existing = count_existing_templates(workspace);
            if existing == TEMPLATE_FILES.len() {
                ("✓", SUCCESS)
            } else {
                ("○", DIM)
            }
        }
        "targets" => {
            if config.todo.notifications.targets.is_empty() {
                ("○", DIM)
            } else {
                ("★", HIGHLIGHT)
            }
        }
        "digest" => {
            if config.todo.notifications.daily_digest {
                ("✓", SUCCESS)
            } else {
                ("○", DIM)
            }
        }
        _ => ("○", DIM),
    }
}

/// Get the status text for a setting.
fn get_setting_status(config: &Config, workspace: &Path, key: &str) -> String {
    match key {
        "templates" => {
            let existing = count_existing_templates(workspace);
            let total = TEMPLATE_FILES.len();
            if existing == total {
                format!("{} files intact", total)
            } else {
                format!("{} missing", total - existing)
            }
        }
        "targets" => {
            let targets = &config.todo.notifications.targets;
            if targets.is_empty() {
                "none configured".to_string()
            } else {
                targets
                    .iter()
                    .map(|t| match t.as_str() {
                        "os_native" => "OS Native",
                        "telegram" => "Telegram",
                        "discord" => "Discord",
                        "slack" => "Slack",
                        "email" => "Email",
                        other => other,
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        "digest" => {
            if config.todo.notifications.daily_digest {
                format!("enabled ({})", config.todo.notifications.daily_digest_time)
            } else {
                "disabled".to_string()
            }
        }
        _ => String::new(),
    }
}

/// Count how many of the 6 known template files exist on disk.
fn count_existing_templates(workspace: &Path) -> usize {
    TEMPLATE_FILES
        .iter()
        .filter(|(_, rel_path)| workspace.join(rel_path).exists())
        .count()
}

/// Shorten a path by replacing the home directory with `~`.
fn shorten_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(suffix) = path.strip_prefix(&home) {
            return format!("~/{}", suffix.display());
        }
    }
    path.display().to_string()
}

/// Validate a time string in HH:MM format (0-23 hours, 0-59 minutes).
fn validate_time(input: &str) -> Result<(), String> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 2 {
        return Err("Expected HH:MM format".to_string());
    }
    let hours: u32 = parts[0]
        .parse()
        .map_err(|_| "Invalid hours".to_string())?;
    let minutes: u32 = parts[1]
        .parse()
        .map_err(|_| "Invalid minutes".to_string())?;
    if hours > 23 {
        return Err(format!("Hours must be 0-23, got {}", hours));
    }
    if minutes > 59 {
        return Err(format!("Minutes must be 0-59, got {}", minutes));
    }
    Ok(())
}

// ============================================================================
// State Model and Sub-Actions
// ============================================================================

/// Actions available in expanded setting sub-menus.
#[derive(Clone, PartialEq)]
enum WorkspaceSubAction {
    RegenerateTemplates,
    ToggleTarget(String),
    ToggleDailyDigest,
    ChangeDigestTime,
    Close,
}

/// Known notification target metadata: (key, display_name).
const NOTIFICATION_TARGETS: &[(&str, &str)] = &[
    ("os_native", "OS Native"),
    ("telegram", "Telegram"),
    ("discord", "Discord"),
    ("slack", "Slack"),
    ("email", "Email"),
];

/// Check if a notification target is available (always for os_native, requires enabled channel otherwise).
fn is_target_available(config: &Config, key: &str) -> bool {
    match key {
        "os_native" => true,
        "telegram" => config.channels.telegram.enabled,
        "discord" => config.channels.discord.enabled,
        "slack" => config.channels.slack.enabled,
        "email" => config.channels.email.enabled,
        _ => false,
    }
}

/// Get the display name for a notification target key.
fn target_display_name(key: &str) -> &str {
    NOTIFICATION_TARGETS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, name)| *name)
        .unwrap_or(key)
}

impl WorkspaceSubAction {
    fn label(&self, config: &Config, workspace: &Path) -> String {
        match self {
            Self::RegenerateTemplates => {
                let existing = count_existing_templates(workspace);
                let total = TEMPLATE_FILES.len();
                if existing < total {
                    format!("Create {} missing", total - existing)
                } else {
                    "Regenerate all".to_string()
                }
            }
            Self::ToggleTarget(key) => {
                let name = target_display_name(key);
                let enabled = config.todo.notifications.targets.contains(key);
                let icon = if enabled { "✓" } else { "○" };
                format!("{} {}", icon, name)
            }
            Self::ToggleDailyDigest => {
                if config.todo.notifications.daily_digest {
                    "Disable digest".to_string()
                } else {
                    "Enable digest".to_string()
                }
            }
            Self::ChangeDigestTime => "Change time".to_string(),
            Self::Close => "Close".to_string(),
        }
    }
}

/// Get the dynamic sub-actions for a setting based on current config state.
fn get_sub_actions(config: &Config, setting_idx: usize) -> Vec<WorkspaceSubAction> {
    let key = WORKSPACE_SETTINGS[setting_idx].key;
    match key {
        "templates" => {
            vec![WorkspaceSubAction::RegenerateTemplates, WorkspaceSubAction::Close]
        }
        "targets" => {
            let mut actions: Vec<WorkspaceSubAction> = NOTIFICATION_TARGETS
                .iter()
                .filter(|(key, _)| is_target_available(config, key))
                .map(|(key, _)| WorkspaceSubAction::ToggleTarget(key.to_string()))
                .collect();
            actions.push(WorkspaceSubAction::Close);
            actions
        }
        "digest" => {
            if config.todo.notifications.daily_digest {
                vec![
                    WorkspaceSubAction::ToggleDailyDigest,
                    WorkspaceSubAction::ChangeDigestTime,
                    WorkspaceSubAction::Close,
                ]
            } else {
                vec![WorkspaceSubAction::ToggleDailyDigest, WorkspaceSubAction::Close]
            }
        }
        _ => vec![WorkspaceSubAction::Close],
    }
}

/// State for the interactive workspace menu.
struct WorkspaceMenuState {
    cursor: usize,
    expanded: Option<usize>,
    sub_cursor: usize,
    in_sub_menu: bool,
}

impl WorkspaceMenuState {
    fn new() -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
        }
    }

    fn total_main_items(&self) -> usize {
        WORKSPACE_SETTINGS.len() + 1 // settings + "Done"
    }

    fn is_on_done(&self) -> bool {
        self.cursor == WORKSPACE_SETTINGS.len()
    }
}

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full workspace settings menu. Returns total lines rendered.
fn render_workspace_menu(
    out: &mut impl Write,
    config: &Config,
    workspace: &Path,
    menu: &WorkspaceMenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut lines = 0;

    for (i, setting) in WORKSPACE_SETTINGS.iter().enumerate() {
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        // Setting icon
        let (icon_str, icon_color) = get_setting_icon(config, workspace, setting.key);
        let icon = colorize(icon_str, icon_color);

        // Expand indicator
        let expand = if is_expanded {
            colorize("▼", BRAND)
        } else {
            " ".to_string()
        };

        // Cursor indicator
        let pointer = if is_cursor {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };

        // Setting name
        let name = if is_cursor || is_expanded {
            colorize(setting.name, BOLD)
        } else {
            setting.name.to_string()
        };

        // Status
        let status_text = get_setting_status(config, workspace, setting.key);
        let status = format!(" — {}", colorize(&status_text, DIM));

        write!(
            out,
            "{}{}{} {} {}{}\r\n",
            prefix, pointer, expand, icon, name, status
        )?;
        lines += 1;

        // Render sub-menu if expanded
        if is_expanded {
            let sub_actions = get_sub_actions(config, i);
            for (si, action) in sub_actions.iter().enumerate() {
                let sub_pointer = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize("❯", BRAND)
                } else {
                    " ".to_string()
                };

                let label = action.label(config, workspace);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize(&label, BOLD)
                } else if *action == WorkspaceSubAction::Close {
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

/// Render the keyboard hint bar.
fn render_menu_hint(
    out: &mut impl Write,
    menu: &WorkspaceMenuState,
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
    workspace: &Path,
    menu: &WorkspaceMenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_workspace_menu(out, config, workspace, menu, chars)?;
    render_menu_hint(out, menu, chars)?;
    Ok(new_lines)
}

// ============================================================================
// Terminal Helpers
// ============================================================================

/// Read a keypress event, handling Ctrl+C gracefully.
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

// ============================================================================
// Workspace Creation Helpers
// ============================================================================

/// Create workspace directories (workspace root, memory, skills).
fn create_workspace_dirs(workspace: &Path) -> Result<()> {
    std::fs::create_dir_all(workspace)?;
    std::fs::create_dir_all(workspace.join("memory"))?;
    std::fs::create_dir_all(workspace.join("skills"))?;
    Ok(())
}

/// Create workspace template files (only if they don't already exist).
/// Returns the number of files newly created.
fn create_workspace_templates(workspace: &Path) -> Result<usize> {
    let mut count = 0;
    count += create_template_file(&workspace.join("AGENTS.md"), templates::AGENTS)?;
    count += create_template_file(&workspace.join("SOUL.md"), templates::SOUL)?;
    count += create_template_file(&workspace.join("USER.md"), templates::USER)?;
    count += create_template_file(&workspace.join("TOOLS.md"), templates::TOOLS)?;
    count += create_template_file(&workspace.join("IDENTITY.md"), templates::IDENTITY)?;

    let memory_file = workspace.join("memory").join("MEMORY.md");
    if !memory_file.exists() {
        std::fs::write(&memory_file, templates::MEMORY)?;
        count += 1;
    }

    Ok(count)
}

/// Create config directories (sessions, cron, media, history).
fn create_config_dirs() -> Result<()> {
    let config_dir = config::config_dir()?;
    std::fs::create_dir_all(config_dir.join("sessions"))?;
    std::fs::create_dir_all(config_dir.join("cron"))?;
    std::fs::create_dir_all(config_dir.join("media"))?;
    std::fs::create_dir_all(config_dir.join("history"))?;
    Ok(())
}

/// Create a template file if it doesn't already exist.
/// Returns 1 if created, 0 if already existed.
fn create_template_file(path: &std::path::Path, content: &str) -> Result<usize> {
    if !path.exists() {
        std::fs::write(path, content)?;
        Ok(1)
    } else {
        Ok(0)
    }
}

// ============================================================================
// Sub-Action Executors
// ============================================================================

/// Execute the change digest time action (exits raw mode for input).
fn execute_change_digest_time(config: &mut Config) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    let current = &config.todo.notifications.daily_digest_time;
    let default = if current.is_empty() { "09:00" } else { current };
    let input = prompts::prompt_text("Daily digest time (HH:MM)", Some(default), false)?;

    match validate_time(&input) {
        Ok(()) => {
            config.todo.notifications.daily_digest_time = input;
            println!(
                "{}{} Digest time updated",
                prefix,
                status_success()
            );
        }
        Err(msg) => {
            println!(
                "{}{}",
                prefix,
                colorize(&format!("{}, keeping current time", msg), WARNING)
            );
        }
    }

    Ok(())
}

/// Execute the regenerate templates action (exits raw mode for feedback).
fn execute_regenerate_templates(workspace: &Path) -> Result<()> {
    let chars = BoxChars::get();
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));

    let count = create_workspace_templates(workspace)?;
    if count > 0 {
        println!(
            "{}{} Created {} new template file(s)",
            prefix,
            status_success(),
            count
        );
    } else {
        println!(
            "{}{} All template files already exist",
            prefix,
            status_success()
        );
    }

    Ok(())
}

// ============================================================================
// Interactive Event Loop
// ============================================================================

/// Run the interactive workspace & notifications menu.
fn run_workspace_menu(config: &mut Config, workspace: &Path) -> Result<()> {
    let chars = BoxChars::get();
    let mut menu = WorkspaceMenuState::new();
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_workspace_menu(&mut out, config, workspace, &menu, chars)?;
    render_menu_hint(&mut out, &menu, chars)?;

    loop {
        let key = read_key()?;
        match key.code {
            // Main menu navigation
            KeyCode::Up | KeyCode::Char('k') if !menu.in_sub_menu => {
                if menu.cursor > 0 {
                    menu.cursor -= 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines =
                        rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines =
                        rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                }
            }
            // Sub-menu navigation
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                if menu.sub_cursor > 0 {
                    menu.sub_cursor -= 1;
                    list_lines =
                        rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                let sub_actions = get_sub_actions(config, menu.expanded.unwrap());
                if menu.sub_cursor < sub_actions.len() - 1 {
                    menu.sub_cursor += 1;
                    list_lines =
                        rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                }
            }
            // Main menu Enter
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_done() {
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(());
                } else {
                    menu.expanded = Some(menu.cursor);
                    menu.in_sub_menu = true;
                    menu.sub_cursor = 0;
                    list_lines =
                        rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                }
            }
            // Sub-menu Enter
            KeyCode::Enter if menu.in_sub_menu => {
                let setting_idx = menu.expanded.unwrap();
                let sub_actions = get_sub_actions(config, setting_idx);
                let action = &sub_actions[menu.sub_cursor];

                match action {
                    WorkspaceSubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(
                            &mut out, config, workspace, &menu, chars, list_lines,
                        )?;
                    }
                    // Immediate actions (no raw mode exit needed)
                    WorkspaceSubAction::ToggleDailyDigest => {
                        config.todo.notifications.daily_digest =
                            !config.todo.notifications.daily_digest;
                        list_lines = rerender_menu(
                            &mut out, config, workspace, &menu, chars, list_lines,
                        )?;
                    }
                    // Immediate toggle for notification targets
                    WorkspaceSubAction::ToggleTarget(key) => {
                        let targets = &mut config.todo.notifications.targets;
                        if let Some(pos) = targets.iter().position(|t| t == key) {
                            targets.remove(pos);
                        } else {
                            targets.push(key.clone());
                        }
                        list_lines = rerender_menu(
                            &mut out, config, workspace, &menu, chars, list_lines,
                        )?;
                    }
                    // Input actions (exit raw mode, prompt, re-enter)
                    WorkspaceSubAction::RegenerateTemplates => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_regenerate_templates(workspace)?;

                        terminal::enable_raw_mode()?;
                        list_lines =
                            render_workspace_menu(&mut out, config, workspace, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    WorkspaceSubAction::ChangeDigestTime => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_digest_time(config)?;

                        terminal::enable_raw_mode()?;
                        list_lines =
                            render_workspace_menu(&mut out, config, workspace, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            // Esc to collapse
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines =
                    rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
            }
            _ => {}
        }
    }
}

// ============================================================================
// Non-TTY Fallback
// ============================================================================

/// Run a simplified menu for non-TTY environments.
fn run_workspace_menu_fallback(config: &mut Config, workspace: &Path) -> Result<()> {
    let chars = BoxChars::get();

    loop {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} Select a setting to configure:",
            colorize(chars.vertical, BRAND)
        );
        println!("{}", colorize(chars.vertical, BRAND));

        for (i, setting) in WORKSPACE_SETTINGS.iter().enumerate() {
            let status = get_setting_status(config, workspace, setting.key);
            println!(
                "{}  {}. {} — {}",
                colorize(chars.vertical, BRAND),
                i + 1,
                setting.name,
                colorize(&status, DIM)
            );
        }

        println!(
            "{}  {}. Done",
            colorize(chars.vertical, BRAND),
            WORKSPACE_SETTINGS.len() + 1,
        );
        println!("{}", colorize(chars.vertical, BRAND));

        let choice = prompts::prompt_text("Enter number", None, true)?;
        let idx = match choice.parse::<usize>() {
            Ok(n) if n > 0 && n <= WORKSPACE_SETTINGS.len() + 1 => n - 1,
            _ => {
                println!(
                    "{} Invalid choice. Please enter a number between 1 and {}.",
                    colorize(chars.vertical, BRAND),
                    WORKSPACE_SETTINGS.len() + 1
                );
                continue;
            }
        };

        if idx == WORKSPACE_SETTINGS.len() {
            return Ok(());
        }

        match WORKSPACE_SETTINGS[idx].key {
            "templates" => {
                execute_regenerate_templates(workspace)?;
            }
            "targets" => {
                // List available targets with current state
                let available: Vec<(&str, &str)> = NOTIFICATION_TARGETS
                    .iter()
                    .filter(|(key, _)| is_target_available(config, key))
                    .copied()
                    .collect();
                for (i, (key, name)) in available.iter().enumerate() {
                    let enabled = config.todo.notifications.targets.contains(&key.to_string());
                    let icon = if enabled { "✓" } else { "○" };
                    println!(
                        "{}    {}. {} {}",
                        colorize(chars.vertical, BRAND),
                        i + 1,
                        icon,
                        name
                    );
                }
                let toggle = prompts::prompt_text("Toggle target (number, or empty to skip)", None, false)?;
                if let Ok(n) = toggle.parse::<usize>() {
                    if n > 0 && n <= available.len() {
                        let key = available[n - 1].0.to_string();
                        let targets = &mut config.todo.notifications.targets;
                        if let Some(pos) = targets.iter().position(|t| t == &key) {
                            targets.remove(pos);
                        } else {
                            targets.push(key);
                        }
                    }
                }
            }
            "digest" => {
                let enabled = prompts::prompt_yes_no(
                    "Enable daily digest?",
                    config.todo.notifications.daily_digest,
                )?;
                config.todo.notifications.daily_digest = enabled;
                if enabled {
                    execute_change_digest_time(config)?;
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Public Entry Point
// ============================================================================

/// Run the workspace & notifications configuration wizard step.
/// Auto-creates workspace at the default path, then presents an interactive
/// menu for templates and notification preferences.
/// Returns true if configuration was completed.
pub fn configure_workspace(config: &mut Config) -> Result<bool> {
    let chars = BoxChars::get();
    let workspace = config.workspace_path();

    // 1. Auto-create workspace and enable focus reminders
    config.todo.notifications.focus_reminders = true;
    create_workspace_dirs(&workspace)?;
    let new_count = create_workspace_templates(&workspace)?;
    create_config_dirs()?;

    // 2. Show summary line
    let total = TEMPLATE_FILES.len();
    let existing = count_existing_templates(&workspace);
    let summary = if new_count > 0 {
        format!(
            "{} Workspace ready at {} ({} new, {} total templates)",
            status_success(),
            colorize(&shorten_path(&workspace), BOLD),
            new_count,
            total
        )
    } else {
        format!(
            "{} Workspace ready at {} ({} templates)",
            status_success(),
            colorize(&shorten_path(&workspace), BOLD),
            existing
        )
    };
    println!("{} {}", colorize(chars.vertical, BRAND), summary);
    println!("{}", colorize(chars.vertical, BRAND));

    // 3. Run menu
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        run_workspace_menu(config, &workspace)?;
    } else {
        run_workspace_menu_fallback(config, &workspace)?;
    }

    println!("{}", colorize(chars.vertical, BRAND));
    println!(
        "{} {} Workspace & notifications configured",
        colorize(chars.vertical, BRAND),
        status_success()
    );

    Ok(true)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================================================
    // create_template_file tests (preserved from original)
    // ========================================================================

    #[test]
    fn test_create_template_file_creates_new() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        let count = create_template_file(&path, "test content").unwrap();

        assert!(path.exists());
        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test content");
    }

    #[test]
    fn test_create_template_file_skips_existing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        std::fs::write(&path, "original").unwrap();
        let count = create_template_file(&path, "replacement").unwrap();

        assert_eq!(count, 0);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "original",
            "Should not overwrite existing file"
        );
    }

    #[test]
    fn test_create_template_file_with_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("empty.md");

        let count = create_template_file(&path, "").unwrap();

        assert!(path.exists());
        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn test_create_template_file_with_unicode() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("unicode.md");

        let content = "# 你好世界\n\nEmoji: 🤖✅❌\nCyrillc: Привет";
        let count = create_template_file(&path, content).unwrap();

        assert_eq!(count, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    // ========================================================================
    // Setting info tests
    // ========================================================================

    #[test]
    fn test_workspace_settings_count() {
        assert_eq!(WORKSPACE_SETTINGS.len(), 3);
    }

    #[test]
    fn test_workspace_settings_keys_unique() {
        let keys: Vec<&str> = WORKSPACE_SETTINGS.iter().map(|s| s.key).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(keys.len(), unique.len());
    }

    #[test]
    fn test_template_files_count() {
        assert_eq!(TEMPLATE_FILES.len(), 6);
    }

    // ========================================================================
    // Icon tests
    // ========================================================================

    #[test]
    fn test_setting_icon_templates_all_present() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();

        // Create all template files
        std::fs::create_dir_all(ws.join("memory")).unwrap();
        for (_, rel_path) in TEMPLATE_FILES {
            std::fs::write(ws.join(rel_path), "content").unwrap();
        }

        let config = Config::default();
        let (icon, _) = get_setting_icon(&config, ws, "templates");
        assert_eq!(icon, "✓");
    }

    #[test]
    fn test_setting_icon_templates_some_missing() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();

        let config = Config::default();
        let (icon, _) = get_setting_icon(&config, ws, "templates");
        assert_eq!(icon, "○");
    }

    #[test]
    fn test_setting_icon_targets_with_targets() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default(); // has ["os_native"] by default
        let (icon, _) = get_setting_icon(&config, temp_dir.path(), "targets");
        assert_eq!(icon, "★");
    }

    #[test]
    fn test_setting_icon_targets_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.targets = Vec::new();
        let (icon, _) = get_setting_icon(&config, temp_dir.path(), "targets");
        assert_eq!(icon, "○");
    }

    #[test]
    fn test_setting_icon_digest_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.daily_digest = true;
        let (icon, _) = get_setting_icon(&config, temp_dir.path(), "digest");
        assert_eq!(icon, "✓");
    }

    #[test]
    fn test_setting_icon_digest_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.daily_digest = false;
        let (icon, _) = get_setting_icon(&config, temp_dir.path(), "digest");
        assert_eq!(icon, "○");
    }

    // ========================================================================
    // Status text tests
    // ========================================================================

    #[test]
    fn test_status_templates_all_present() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();
        std::fs::create_dir_all(ws.join("memory")).unwrap();
        for (_, rel_path) in TEMPLATE_FILES {
            std::fs::write(ws.join(rel_path), "content").unwrap();
        }

        let config = Config::default();
        assert_eq!(get_setting_status(&config, ws, "templates"), "6 files intact");
    }

    #[test]
    fn test_status_templates_some_missing() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();
        assert_eq!(
            get_setting_status(&config, temp_dir.path(), "templates"),
            "6 missing"
        );
    }

    #[test]
    fn test_status_targets_default() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();
        assert_eq!(
            get_setting_status(&config, temp_dir.path(), "targets"),
            "OS Native"
        );
    }

    #[test]
    fn test_status_targets_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.targets = Vec::new();
        assert_eq!(
            get_setting_status(&config, temp_dir.path(), "targets"),
            "none configured"
        );
    }

    #[test]
    fn test_status_targets_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.targets =
            vec!["os_native".to_string(), "telegram".to_string()];
        assert_eq!(
            get_setting_status(&config, temp_dir.path(), "targets"),
            "OS Native, Telegram"
        );
    }

    #[test]
    fn test_status_digest_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::default();
        let status = get_setting_status(&config, temp_dir.path(), "digest");
        assert!(status.starts_with("enabled"));
        assert!(status.contains("09:00"));
    }

    #[test]
    fn test_status_digest_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.todo.notifications.daily_digest = false;
        assert_eq!(
            get_setting_status(&config, temp_dir.path(), "digest"),
            "disabled"
        );
    }

    // ========================================================================
    // Sub-action generation tests
    // ========================================================================

    #[test]
    fn test_templates_sub_actions() {
        let config = Config::default();
        let actions = get_sub_actions(&config, 0); // templates
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&WorkspaceSubAction::RegenerateTemplates));
        assert!(actions.contains(&WorkspaceSubAction::Close));
    }

    #[test]
    fn test_targets_sub_actions_default() {
        let config = Config::default();
        let actions = get_sub_actions(&config, 1); // targets
        // Only os_native is available by default (no channels enabled) + Close
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&WorkspaceSubAction::ToggleTarget("os_native".to_string())));
        assert!(actions.contains(&WorkspaceSubAction::Close));
    }

    #[test]
    fn test_targets_sub_actions_with_channels() {
        let mut config = Config::default();
        config.channels.telegram.enabled = true;
        config.channels.discord.enabled = true;
        let actions = get_sub_actions(&config, 1); // targets
        // os_native + telegram + discord + Close
        assert_eq!(actions.len(), 4);
        assert!(actions.contains(&WorkspaceSubAction::ToggleTarget("os_native".to_string())));
        assert!(actions.contains(&WorkspaceSubAction::ToggleTarget("telegram".to_string())));
        assert!(actions.contains(&WorkspaceSubAction::ToggleTarget("discord".to_string())));
        assert!(actions.contains(&WorkspaceSubAction::Close));
    }

    #[test]
    fn test_digest_sub_actions_enabled() {
        let config = Config::default(); // digest enabled by default
        let actions = get_sub_actions(&config, 2); // digest
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&WorkspaceSubAction::ToggleDailyDigest));
        assert!(actions.contains(&WorkspaceSubAction::ChangeDigestTime));
        assert!(actions.contains(&WorkspaceSubAction::Close));
    }

    #[test]
    fn test_digest_sub_actions_disabled() {
        let mut config = Config::default();
        config.todo.notifications.daily_digest = false;
        let actions = get_sub_actions(&config, 2); // digest
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&WorkspaceSubAction::ToggleDailyDigest));
        assert!(actions.contains(&WorkspaceSubAction::Close));
    }

    // ========================================================================
    // Menu state tests
    // ========================================================================

    #[test]
    fn test_menu_state_initial() {
        let menu = WorkspaceMenuState::new();
        assert_eq!(menu.cursor, 0);
        assert!(menu.expanded.is_none());
        assert_eq!(menu.sub_cursor, 0);
        assert!(!menu.in_sub_menu);
    }

    #[test]
    fn test_menu_state_total_items() {
        let menu = WorkspaceMenuState::new();
        assert_eq!(menu.total_main_items(), 4); // 3 settings + Done
    }

    #[test]
    fn test_menu_state_is_on_done() {
        let mut menu = WorkspaceMenuState::new();
        assert!(!menu.is_on_done());
        menu.cursor = WORKSPACE_SETTINGS.len();
        assert!(menu.is_on_done());
    }

    // ========================================================================
    // validate_time tests
    // ========================================================================

    #[test]
    fn test_validate_time_valid() {
        assert!(validate_time("00:00").is_ok());
        assert!(validate_time("09:00").is_ok());
        assert!(validate_time("12:30").is_ok());
        assert!(validate_time("23:59").is_ok());
    }

    #[test]
    fn test_validate_time_invalid_hours() {
        assert!(validate_time("24:00").is_err());
        assert!(validate_time("25:00").is_err());
        assert!(validate_time("99:00").is_err());
    }

    #[test]
    fn test_validate_time_invalid_minutes() {
        assert!(validate_time("12:60").is_err());
        assert!(validate_time("12:99").is_err());
    }

    #[test]
    fn test_validate_time_invalid_format() {
        assert!(validate_time("1200").is_err());
        assert!(validate_time("12:00:00").is_err());
        assert!(validate_time("abc").is_err());
        assert!(validate_time("").is_err());
        assert!(validate_time("ab:cd").is_err());
    }

    // ========================================================================
    // count_existing_templates tests
    // ========================================================================

    #[test]
    fn test_count_existing_templates_empty() {
        let temp_dir = TempDir::new().unwrap();
        assert_eq!(count_existing_templates(temp_dir.path()), 0);
    }

    #[test]
    fn test_count_existing_templates_some() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();
        std::fs::write(ws.join("AGENTS.md"), "content").unwrap();
        std::fs::write(ws.join("SOUL.md"), "content").unwrap();
        assert_eq!(count_existing_templates(ws), 2);
    }

    #[test]
    fn test_count_existing_templates_all() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();
        std::fs::create_dir_all(ws.join("memory")).unwrap();
        for (_, rel_path) in TEMPLATE_FILES {
            std::fs::write(ws.join(rel_path), "content").unwrap();
        }
        assert_eq!(count_existing_templates(ws), TEMPLATE_FILES.len());
    }

    // ========================================================================
    // shorten_path tests
    // ========================================================================

    #[test]
    fn test_shorten_path_with_home() {
        if let Some(home) = dirs::home_dir() {
            let path = home.join("workspace");
            assert_eq!(shorten_path(&path), "~/workspace");
        }
    }

    #[test]
    fn test_shorten_path_without_home() {
        let path = std::path::Path::new("/tmp/workspace");
        let shortened = shorten_path(path);
        assert_eq!(shortened, "/tmp/workspace");
    }

    // ========================================================================
    // Workspace creation helper tests
    // ========================================================================

    #[test]
    fn test_create_workspace_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path().join("workspace");

        create_workspace_dirs(&ws).unwrap();

        assert!(ws.exists());
        assert!(ws.join("memory").exists());
        assert!(ws.join("skills").exists());
    }

    #[test]
    fn test_create_workspace_templates_count() {
        let temp_dir = TempDir::new().unwrap();
        let ws = temp_dir.path();
        std::fs::create_dir_all(ws.join("memory")).unwrap();

        let count = create_workspace_templates(ws).unwrap();
        assert_eq!(count, TEMPLATE_FILES.len());

        // Second call should create 0
        let count2 = create_workspace_templates(ws).unwrap();
        assert_eq!(count2, 0);
    }
}
