//! Provider menu rendering and interactive event loop.

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::event::KeyCode;
use crossterm::terminal;
use std::io::{self, Write};

use super::detection::{
    get_provider_key, has_any_provider_configured, provider_select_options, PROVIDERS,
};
use super::{
    execute_change_model, execute_custom_base_url, execute_edit_api_key, MenuState, SubAction,
    SUB_ACTIONS,
};
use crate::wizard::framework::WizardState;
use crate::wizard::prompts::{self, mask_secret};
use crate::wizard::ui::{erase_lines, read_key, MenuOutcome};

/// Run the interactive provider menu. Returns `Done` or `Back`.
pub(crate) fn run_provider_menu(state: &mut WizardState, can_go_back: bool) -> Result<MenuOutcome> {
    let chars = BoxChars::get();
    let mut menu = MenuState::new(can_go_back);
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
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
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                if menu.sub_cursor > 0 {
                    menu.sub_cursor -= 1;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                if menu.sub_cursor < SUB_ACTIONS.len() - 1 {
                    menu.sub_cursor += 1;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_back() {
                    terminal::disable_raw_mode()?;
                    erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Back);
                } else if menu.is_on_done() {
                    if has_any_provider_configured(&state.config) {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;
                        return Ok(MenuOutcome::Done);
                    }
                } else {
                    menu.expanded = Some(menu.cursor);
                    menu.in_sub_menu = true;
                    menu.sub_cursor = 0;
                    list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                }
            }
            KeyCode::Enter if menu.in_sub_menu => {
                let provider_idx = menu.expanded.unwrap();
                let action = SUB_ACTIONS[menu.sub_cursor];

                match action {
                    SubAction::SetActive => {
                        let provider = &PROVIDERS[provider_idx];
                        state.config.agents.defaults.provider = Some(provider.key.to_string());
                        state.config.agents.defaults.model = provider.default_model.to_string();

                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines =
                            rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                    }
                    SubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        list_lines =
                            rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
                    }
                    SubAction::EditApiKey => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_edit_api_key(state, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    SubAction::ChangeModel => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_change_model(state, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                    SubAction::CustomBaseUrl => {
                        terminal::disable_raw_mode()?;
                        erase_lines(list_lines + 1)?;

                        execute_custom_base_url(state, provider_idx)?;

                        terminal::enable_raw_mode()?;
                        list_lines = render_provider_menu(&mut out, &state.config, &menu, chars)?;
                        render_menu_hint(&mut out, &menu, chars)?;
                    }
                }
            }
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, &state.config, &menu, chars, list_lines)?;
            }
            KeyCode::Esc if !menu.in_sub_menu && menu.can_go_back => {
                terminal::disable_raw_mode()?;
                erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            _ => {}
        }
    }
}

/// Non-TTY fallback: simple numbered menu for CI/piped environments.
pub(crate) fn run_provider_menu_fallback(
    state: &mut WizardState,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    loop {
        let mut options = provider_select_options();
        if can_go_back {
            options.push(prompts::SelectOption {
                label: "← Back",
                description: "Return to previous step",
            });
        }

        let idx = prompts::prompt_select(
            "Select provider to configure (or Done)",
            &options,
            options.len() - 1,
        )?;

        // Check for Back option
        if can_go_back && idx == options.len() - 1 {
            return Ok(MenuOutcome::Back);
        }

        if idx == PROVIDERS.len() {
            if has_any_provider_configured(&state.config) {
                return Ok(MenuOutcome::Done);
            }
            println!("At least one provider must be configured.");
            continue;
        }

        execute_edit_api_key(state, idx)?;

        if prompts::prompt_yes_no("Set as active provider?", true)? {
            state.config.agents.defaults.provider = Some(PROVIDERS[idx].key.to_string());
            execute_change_model(state, idx)?;
        }
    }
}

/// Render the full provider menu list. Returns total lines rendered.
pub(crate) fn render_provider_menu(
    out: &mut impl Write,
    config: &config::Config,
    menu: &MenuState,
    chars: &BoxChars,
) -> Result<usize> {
    let prefix = format!("{} ", colorize(chars.vertical, BRAND));
    let active_provider = config
        .agents
        .defaults
        .provider
        .as_deref()
        .unwrap_or_else(|| config.active_provider_name());
    let mut lines = 0;

    for (i, provider) in PROVIDERS.iter().enumerate() {
        let key = get_provider_key(config, provider.key);
        let configured = !key.is_empty();
        let is_active = provider.key == active_provider;
        let is_cursor = !menu.in_sub_menu && menu.cursor == i;
        let is_expanded = menu.expanded == Some(i);

        let icon = if is_active {
            colorize("★", HIGHLIGHT)
        } else if configured {
            colorize("✓", SUCCESS)
        } else {
            colorize("○", DIM)
        };

        let expand = if is_expanded { "▼" } else { " " };
        let expand_colored = if is_expanded {
            colorize(expand, BRAND)
        } else {
            " ".to_string()
        };

        let pointer = if is_cursor {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };

        let name = if is_cursor || is_expanded {
            colorize(provider.name, BOLD)
        } else if !configured {
            colorize(provider.name, DIM)
        } else {
            provider.name.to_string()
        };

        let status = if configured {
            format!(" {}", colorize(&format!("({})", mask_secret(&key)), DIM))
        } else {
            format!(" {}", colorize("— not configured", DIM))
        };

        write!(
            out,
            "{}{}{} {} {}{}\r\n",
            prefix, pointer, expand_colored, icon, name, status
        )?;
        lines += 1;

        if is_expanded {
            for (si, action) in SUB_ACTIONS.iter().enumerate() {
                let sub_pointer = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize("❯", BRAND)
                } else {
                    " ".to_string()
                };

                let label = action.label(config, provider);
                let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                    colorize(&label, BOLD)
                } else if *action == SubAction::Close {
                    colorize(&format!("── {} ──", label), DIM)
                } else {
                    label
                };

                write!(out, "{}      {} {}\r\n", prefix, sub_pointer, label_display)?;
                lines += 1;
            }
        }
    }

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
    let done_available = has_any_provider_configured(config);
    let done_pointer = if !menu.in_sub_menu && menu.is_on_done() {
        colorize("❯", BRAND)
    } else {
        " ".to_string()
    };
    let done_icon = if done_available {
        colorize("●", BRAND)
    } else {
        colorize("○", DIM)
    };
    let done_label = if done_available {
        if !menu.in_sub_menu && menu.is_on_done() {
            colorize("Done", BOLD)
        } else {
            "Done".to_string()
        }
    } else {
        colorize("Done", DIM).to_string()
    };
    let done_desc = if done_available {
        colorize("— finish provider setup", DIM)
    } else {
        colorize("— configure at least one provider first", DIM)
    };

    write!(
        out,
        "{}{} {} {} {}\r\n",
        prefix, done_pointer, done_icon, done_label, done_desc
    )?;
    lines += 1;

    out.flush()?;
    Ok(lines)
}

pub(crate) fn render_menu_hint(
    out: &mut impl Write,
    menu: &MenuState,
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

pub(crate) fn rerender_menu(
    out: &mut impl Write,
    config: &config::Config,
    menu: &MenuState,
    chars: &BoxChars,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    let new_lines = render_provider_menu(out, config, menu, chars)?;
    render_menu_hint(out, menu, chars)?;
    Ok(new_lines)
}
