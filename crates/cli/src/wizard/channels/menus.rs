//! Channel menu rendering and interactive event loop.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

use super::{
    execute_configure_credentials, execute_manage_allowlist, execute_reconnect,
    execute_test_connection, execute_toggle_enabled, get_channel_status_description,
    is_channel_configured, is_channel_enabled, ChannelMenuState, ChannelSubAction, CHANNELS,
    SUB_ACTIONS,
};
use crate::wizard::prompts;
use crate::wizard::ui::{erase_lines, MenuOutcome};

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the full channel menu list. Returns total lines rendered.
pub(super) fn render_channel_menu(
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

/// Run the interactive channel menu. Modifies config in place and returns when user selects "Done" or "Back".
pub(super) async fn run_channel_menu(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    use crate::wizard::ui::read_key;
    use crossterm::{event::KeyCode, terminal};

    let chars = BoxChars::get();
    let mut menu = ChannelMenuState::new(can_go_back);
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_channel_menu(&mut out, config, &menu, chars)?;
    render_menu_hint(&mut out, &menu, chars)?;

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
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
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
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_back() {
                    // Back — return to previous step
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Back);
                } else if menu.is_on_done() {
                    // Done — exit
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Done);
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

                    list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
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
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    ChannelSubAction::ConfigureCredentials => {
                        // Exit raw mode, erase menu, run configuration, re-enter
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_configure_credentials(config, channel_idx).await?;

                        // Re-enter raw mode and redraw
                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    ChannelSubAction::TestConnection => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_test_connection(config, channel_idx).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    ChannelSubAction::ToggleEnabled => {
                        // Immediate toggle — no need to exit raw mode
                        execute_toggle_enabled(config, channel_idx)?;
                        list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
                    }
                    ChannelSubAction::ManageAllowlist => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_manage_allowlist(config, channel_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    ChannelSubAction::Reconnect => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_reconnect(channel.key).await?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_channel_menu(&mut out, config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, &menu, chars, list_lines)?;
            }
            KeyCode::Esc if !menu.in_sub_menu && can_go_back => {
                terminal::disable_raw_mode()?;
                erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            _ => {}
        }
    }
}

/// Run a simplified channel menu for non-TTY environments.
pub(super) async fn run_channel_menu_fallback(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let back_offset: usize = if can_go_back { 1 } else { 0 };
    let total_options = CHANNELS.len() + back_offset + 1; // channels + optional back + done

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

        if can_go_back {
            println!(
                "{}  {}. ← Back",
                colorize(chars.vertical, BRAND),
                CHANNELS.len() + 1
            );
        }

        println!(
            "{}  {}. Done",
            colorize(chars.vertical, BRAND),
            CHANNELS.len() + back_offset + 1
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

        if can_go_back && idx == CHANNELS.len() {
            // Back
            return Ok(MenuOutcome::Back);
        }

        if idx == CHANNELS.len() + back_offset {
            // Done
            return Ok(MenuOutcome::Done);
        }

        // Configure selected channel
        println!("{}", colorize(chars.vertical, BRAND));
        execute_configure_credentials(config, idx).await?;
    }
}
