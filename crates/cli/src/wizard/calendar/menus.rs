//! Calendar menu rendering and interactive event loops.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::{event::KeyCode, terminal};

use super::{
    execute_change_calendar_name, execute_change_sync_interval, execute_configure_credentials,
    execute_test_connection, execute_toggle_enabled, is_provider_configured, is_provider_enabled,
    CalendarMenuState, CalendarSubAction, CALENDAR_PROVIDERS, SUB_ACTIONS,
};
use crate::wizard::ui::MenuOutcome;
use crate::wizard::{prompts, ui};

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
            let desc = super::get_provider_status_description(config, provider.key);
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

/// Render the keyboard hint bar at the bottom of the menu.
fn render_menu_hint(
    out: &mut impl Write,
    menu: &CalendarMenuState,
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

/// Run the interactive calendar menu.
pub(super) async fn run_calendar_menu(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let mut menu = CalendarMenuState::new(can_go_back);
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
    render_menu_hint(&mut out, &menu, chars)?;

    loop {
        let key = ui::read_key()?;
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
                if menu.is_on_back() {
                    terminal::disable_raw_mode()?;
                    ui::erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Back);
                } else if menu.is_on_done() {
                    terminal::disable_raw_mode()?;
                    ui::erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Done);
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
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    CalendarSubAction::ConfigureCredentials => {
                        terminal::disable_raw_mode()?;
                        ui::erase_lines(list_lines + 1)?;

                        execute_configure_credentials(config, provider_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::TestConnection => {
                        terminal::disable_raw_mode()?;
                        ui::erase_lines(list_lines + 1)?;

                        execute_test_connection(config, provider_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::ToggleEnabled => {
                        execute_toggle_enabled(config, provider_idx);
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    CalendarSubAction::ChangeCalendarName => {
                        terminal::disable_raw_mode()?;
                        ui::erase_lines(list_lines + 1)?;

                        execute_change_calendar_name(config, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_calendar_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    CalendarSubAction::ChangeSyncInterval => {
                        terminal::disable_raw_mode()?;
                        ui::erase_lines(list_lines + 1)?;

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
            KeyCode::Esc if !menu.in_sub_menu && menu.can_go_back => {
                terminal::disable_raw_mode()?;
                ui::erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            _ => {}
        }
    }
}

/// Run a simplified calendar menu for non-TTY environments.
pub(super) async fn run_calendar_menu_fallback(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let back_offset: usize = if can_go_back { 1 } else { 0 };
    let total_options = CALENDAR_PROVIDERS.len() + back_offset + 1; // providers + optional back + done

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

        if can_go_back {
            println!(
                "{}  {}. ← Back",
                colorize(chars.vertical, BRAND),
                CALENDAR_PROVIDERS.len() + 1,
            );
        }

        println!(
            "{}  {}. Done",
            colorize(chars.vertical, BRAND),
            CALENDAR_PROVIDERS.len() + back_offset + 1,
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

        // Check for Back
        if can_go_back && idx == CALENDAR_PROVIDERS.len() {
            return Ok(MenuOutcome::Back);
        }

        // Check for Done
        let done_idx = CALENDAR_PROVIDERS.len() + back_offset;
        if idx == done_idx {
            return Ok(MenuOutcome::Done);
        }

        // Configure selected provider
        println!("{}", colorize(chars.vertical, BRAND));
        execute_configure_credentials(config, idx).await?;
    }
}
