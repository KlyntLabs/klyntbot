//! Workspace menu rendering and interactive event loop.
//!
//! Implements the expand-in-place menu pattern for workspace settings,
//! including both interactive (TTY) and fallback (non-TTY) modes.

use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::event::KeyCode;
use crossterm::terminal;

use super::{
    execute_change_digest_time, execute_regenerate_templates, get_setting_icon, get_setting_status,
    get_sub_actions, is_target_available, WorkspaceMenuState, WorkspaceSubAction,
    NOTIFICATION_TARGETS, WORKSPACE_SETTINGS,
};
use crate::wizard::prompts;
use crate::wizard::ui::{erase_lines, read_key, MenuOutcome};

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full workspace settings menu. Returns total lines rendered.
pub(super) fn render_workspace_menu(
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

    // Separator before Back/Done
    write!(
        out,
        "{}  {}\r\n",
        prefix,
        colorize("──────────────────────────", DIM)
    )?;
    lines += 1;

    // Back row (only if not first step)
    if menu.can_go_back {
        let back_pointer = if !menu.in_sub_menu && menu.is_on_back() {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };
        let back_label = if !menu.in_sub_menu && menu.is_on_back() {
            colorize("← Back", BOLD)
        } else {
            "← Back".to_string()
        };
        write!(
            out,
            "{}{} {} {}\r\n",
            prefix,
            back_pointer,
            colorize("◁", DIM),
            back_label
        )?;
        lines += 1;
    }

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
        "↑/↓ navigate · Enter select · Esc close"
    } else if menu.can_go_back {
        "↑/↓ navigate · Enter select · Esc back"
    } else {
        "↑/↓ navigate · Enter select"
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
// Interactive Event Loop
// ============================================================================

/// Run the interactive workspace & notifications menu.
pub(super) fn run_workspace_menu(
    config: &mut Config,
    workspace: &Path,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let mut menu = WorkspaceMenuState::new(can_go_back);
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
                if menu.is_on_back() {
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Back);
                } else if menu.is_on_done() {
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Done);
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
                        list_lines =
                            rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                    }
                    // Immediate actions (no raw mode exit needed)
                    WorkspaceSubAction::ToggleDailyDigest => {
                        config.todo.notifications.daily_digest =
                            !config.todo.notifications.daily_digest;
                        list_lines =
                            rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
                    }
                    // Immediate toggle for notification targets
                    WorkspaceSubAction::ToggleTarget(key) => {
                        let targets = &mut config.todo.notifications.targets;
                        if let Some(pos) = targets.iter().position(|t| t == key) {
                            targets.remove(pos);
                        } else {
                            targets.push(key.clone());
                        }
                        list_lines =
                            rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
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
            // Esc to collapse sub-menu
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, workspace, &menu, chars, list_lines)?;
            }
            // Esc to go back (main menu level)
            KeyCode::Esc if !menu.in_sub_menu && can_go_back => {
                terminal::disable_raw_mode()?;
                erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            _ => {}
        }
    }
}

// ============================================================================
// Non-TTY Fallback
// ============================================================================

/// Run a simplified menu for non-TTY environments.
pub(super) fn run_workspace_menu_fallback(
    config: &mut Config,
    workspace: &Path,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let back_offset = if can_go_back { 1 } else { 0 };
    let total_options = WORKSPACE_SETTINGS.len() + back_offset + 1; // settings + optional back + done

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

        if can_go_back {
            println!(
                "{}  {}. ← Back",
                colorize(chars.vertical, BRAND),
                WORKSPACE_SETTINGS.len() + 1,
            );
        }

        println!(
            "{}  {}. Done",
            colorize(chars.vertical, BRAND),
            total_options,
        );
        println!("{}", colorize(chars.vertical, BRAND));

        let choice = prompts::prompt_text("Enter number", None, true)?;
        let idx = match choice.parse::<usize>() {
            Ok(n) if n > 0 && n <= total_options => n - 1,
            _ => {
                println!(
                    "{} Invalid choice. Please enter a number between 1 and {}.",
                    colorize(chars.vertical, BRAND),
                    total_options
                );
                continue;
            }
        };

        if can_go_back && idx == WORKSPACE_SETTINGS.len() {
            return Ok(MenuOutcome::Back);
        }

        if idx == total_options - 1 {
            return Ok(MenuOutcome::Done);
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
                let toggle =
                    prompts::prompt_text("Toggle target (number, or empty to skip)", None, false)?;
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
