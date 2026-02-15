//! Interactive menu rendering and event loops for tools configuration.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::{event::KeyCode, terminal};

use super::{
    apply_preset, detect_preset, execute_change_timeout, execute_configure_brave_key,
    execute_edit_allowlist, get_setting_icon, get_setting_status, get_sub_actions, ToolMenuState,
    ToolSubAction, ToolsPreset, TOOL_SETTINGS,
};
use crate::wizard::prompts;
use crate::wizard::ui::{erase_lines, read_key, MenuOutcome};

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full tool settings menu. Returns total lines rendered.
fn render_tool_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &ToolMenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let mut lines = 0;

    for (i, setting) in TOOL_SETTINGS.iter().enumerate() {
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        // Setting icon
        let (icon_str, icon_color) = get_setting_icon(config, setting.key);
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
        let status_text = get_setting_status(config, setting.key);
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

                let label = action.label(config);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize(&label, BOLD)
                } else if *action == ToolSubAction::Close {
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
fn render_menu_hint(out: &mut impl Write, menu: &ToolMenuState, chars: &BoxChars) -> Result<()> {
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
    menu: &ToolMenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_tool_menu(out, config, menu, chars)?;
    render_menu_hint(out, menu, chars)?;
    Ok(new_lines)
}

// ============================================================================
// Interactive Event Loop
// ============================================================================

/// Run the interactive tool permissions menu.
pub(super) fn run_tool_menu(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let mut menu = ToolMenuState::new(can_go_back);
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_tool_menu(&mut out, config, &menu, chars)?;
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
            // Sub-menu navigation
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                if menu.sub_cursor > 0 {
                    menu.sub_cursor -= 1;
                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                let sub_actions = get_sub_actions(config, menu.expanded.unwrap());
                if menu.sub_cursor < sub_actions.len() - 1 {
                    menu.sub_cursor += 1;
                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
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
                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                }
            }
            // Sub-menu Enter
            KeyCode::Enter if menu.in_sub_menu => {
                let setting_idx = menu.expanded.unwrap();
                let sub_actions = get_sub_actions(config, setting_idx);
                let action = &sub_actions[menu.sub_cursor];

                match action {
                    ToolSubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    // Immediate actions (no raw mode exit needed)
                    ToolSubAction::SwitchPreset(preset) => {
                        apply_preset(config, *preset);
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    ToolSubAction::ToggleWorkspace => {
                        config.tools.restrict_to_workspace = !config.tools.restrict_to_workspace;
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    ToolSubAction::SwitchAllowlistMode => {
                        if config.tools.exec.allowed_commands.is_empty() {
                            // Switch to allowlist mode with default safe commands
                            config.tools.exec.allowed_commands = vec![
                                "ls".to_string(),
                                "cat".to_string(),
                                "head".to_string(),
                                "tail".to_string(),
                                "grep".to_string(),
                                "find".to_string(),
                                "wc".to_string(),
                                "echo".to_string(),
                                "pwd".to_string(),
                                "date".to_string(),
                            ];
                        } else {
                            // Switch to deny-list mode
                            config.tools.exec.allowed_commands = Vec::new();
                        }
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    ToolSubAction::RemoveBraveKey => {
                        config.tools.web.brave_api_key = config::schema::Secret::new(String::new());
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    // Input actions (exit raw mode, prompt, re-enter)
                    ToolSubAction::EditAllowlist => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_edit_allowlist(config)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_tool_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    ToolSubAction::ConfigureBraveKey | ToolSubAction::EditBraveKey => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_configure_brave_key(config)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_tool_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    ToolSubAction::ChangeTimeout => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_timeout(config)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_tool_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            // Esc to collapse sub-menu
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            // Esc to go back (main menu level)
            KeyCode::Esc if !menu.in_sub_menu && menu.can_go_back => {
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
pub(super) fn run_tool_menu_fallback(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let back_offset: usize = if can_go_back { 1 } else { 0 };
    let total_options = TOOL_SETTINGS.len() + back_offset + 1; // settings + optional back + done

    loop {
        println!("{}", colorize(chars.vertical, BRAND));
        println!(
            "{} Select a setting to configure:",
            colorize(chars.vertical, BRAND)
        );
        println!("{}", colorize(chars.vertical, BRAND));

        for (i, setting) in TOOL_SETTINGS.iter().enumerate() {
            let status = get_setting_status(config, setting.key);
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
                TOOL_SETTINGS.len() + 1,
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

        // Back option
        if can_go_back && idx == TOOL_SETTINGS.len() {
            return Ok(MenuOutcome::Back);
        }

        // Done option
        if idx == total_options - 1 {
            return Ok(MenuOutcome::Done);
        }

        match TOOL_SETTINGS[idx].key {
            "preset" => {
                let options: Vec<prompts::SelectOption<'_>> = [
                    ToolsPreset::Strict,
                    ToolsPreset::Balanced,
                    ToolsPreset::Permissive,
                ]
                .iter()
                .map(|p| prompts::SelectOption {
                    label: p.name(),
                    description: p.description(),
                })
                .collect();

                let current = match detect_preset(config) {
                    ToolsPreset::Strict => 0,
                    ToolsPreset::Balanced => 1,
                    ToolsPreset::Permissive => 2,
                };

                let sel = prompts::prompt_select("Select preset", &options, current)?;
                let preset = [
                    ToolsPreset::Strict,
                    ToolsPreset::Balanced,
                    ToolsPreset::Permissive,
                ][sel];
                apply_preset(config, preset);
            }
            "workspace" => {
                let restrict = prompts::prompt_yes_no(
                    "Restrict to workspace?",
                    config.tools.restrict_to_workspace,
                )?;
                config.tools.restrict_to_workspace = restrict;
            }
            "allowlist" => {
                execute_edit_allowlist(config)?;
            }
            "brave_api" => {
                execute_configure_brave_key(config)?;
            }
            "timeout" => {
                execute_change_timeout(config)?;
            }
            _ => {}
        }
    }
}
