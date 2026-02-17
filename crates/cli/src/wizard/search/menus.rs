//! Semantic search menu rendering and interactive event loop.

use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use crossterm::{event::KeyCode, terminal};

use super::{
    execute_configure_threshold, execute_disable, execute_enable_and_download, SearchSubAction,
    SUB_ACTIONS,
};
use crate::wizard::ui;
use crate::wizard::ui::MenuOutcome;

/// Menu state for semantic search configuration.
struct SearchMenuState {
    cursor: usize,           // Main menu cursor position
    expanded: Option<usize>, // Which item is expanded (None = all collapsed)
    sub_cursor: usize,       // Sub-menu cursor position
    in_sub_menu: bool,       // Whether cursor is in sub-menu
    can_go_back: bool,       // Whether "Back" option is available
}

impl SearchMenuState {
    fn new(can_go_back: bool) -> Self {
        Self {
            cursor: 0,
            expanded: None,
            sub_cursor: 0,
            in_sub_menu: false,
            can_go_back,
        }
    }

    fn total_main_items(&self) -> usize {
        // 1 (semantic search) + 1 (Done) + 1 if can_go_back (Back)
        2 + if self.can_go_back { 1 } else { 0 }
    }

    fn is_on_back(&self) -> bool {
        // Back is at index 1 (if available)
        self.can_go_back && self.cursor == 1
    }

    fn is_on_done(&self) -> bool {
        // Done is at index 1 (no Back) or 2 (with Back)
        let done_idx = if self.can_go_back { 2 } else { 1 };
        self.cursor == done_idx
    }

    fn is_on_semantic_search(&self) -> bool {
        self.cursor == 0
    }
}

/// Run the semantic search configuration menu.
pub(super) fn run_search_menu(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    // Fallback for non-TTY
    if !io::stdin().is_terminal() {
        return run_search_menu_fallback(config, can_go_back);
    }

    let mut menu = SearchMenuState::new(can_go_back);
    let mut out = io::stdout();

    // Initial render
    terminal::enable_raw_mode()?;
    let mut list_lines = render_search_menu(&mut out, config, &menu)?;
    render_menu_hint(&mut out, &menu)?;

    loop {
        let key = ui::read_key()?;
        match key.code {
            // 1. Main menu navigation (Up/Down) - FIRST
            KeyCode::Up | KeyCode::Char('k') if !menu.in_sub_menu => {
                if menu.cursor > 0 {
                    menu.cursor -= 1;
                    // Collapse if cursor moved away
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !menu.in_sub_menu => {
                if menu.cursor < menu.total_main_items() - 1 {
                    menu.cursor += 1;
                    // Collapse if cursor moved away
                    if menu.expanded.is_some() && menu.expanded != Some(menu.cursor) {
                        menu.expanded = None;
                    }
                    list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
                }
            }

            // 2. Sub-menu navigation (Up/Down) - SECOND
            KeyCode::Up | KeyCode::Char('k') if menu.in_sub_menu => {
                let enabled = config.todo.search.enabled;
                loop {
                    if menu.sub_cursor == 0 {
                        break;
                    }
                    menu.sub_cursor -= 1;
                    let action = SUB_ACTIONS[menu.sub_cursor];
                    if action.is_available(enabled) {
                        break;
                    }
                }
                list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
            }
            KeyCode::Down | KeyCode::Char('j') if menu.in_sub_menu => {
                let enabled = config.todo.search.enabled;
                loop {
                    if menu.sub_cursor >= SUB_ACTIONS.len() - 1 {
                        break;
                    }
                    menu.sub_cursor += 1;
                    let action = SUB_ACTIONS[menu.sub_cursor];
                    if action.is_available(enabled) {
                        break;
                    }
                }
                list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
            }

            // 3. Enter in main menu - THIRD
            KeyCode::Enter if !menu.in_sub_menu => {
                if menu.is_on_back() {
                    terminal::disable_raw_mode()?;
                    ui::erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Back);
                } else if menu.is_on_done() {
                    terminal::disable_raw_mode()?;
                    ui::erase_lines(list_lines + 1)?;
                    return Ok(MenuOutcome::Done);
                } else if menu.is_on_semantic_search() {
                    // Expand semantic search
                    menu.expanded = Some(0);
                    menu.in_sub_menu = true;

                    // Find first available action
                    let enabled = config.todo.search.enabled;
                    for (i, action) in SUB_ACTIONS.iter().enumerate() {
                        if action.is_available(enabled) {
                            menu.sub_cursor = i;
                            break;
                        }
                    }

                    list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
                }
            }

            // 4. Enter in sub-menu - FOURTH
            KeyCode::Enter if menu.in_sub_menu => {
                let action = SUB_ACTIONS[menu.sub_cursor];
                let enabled = config.todo.search.enabled;

                if !action.is_available(enabled) {
                    continue;
                }

                terminal::disable_raw_mode()?;
                ui::erase_lines(list_lines + 1)?;

                match action {
                    SearchSubAction::EnableAndDownload => {
                        execute_enable_and_download(config)?;
                    }
                    SearchSubAction::Disable => {
                        execute_disable(config)?;
                    }
                    SearchSubAction::ConfigureThreshold => {
                        execute_configure_threshold(config)?;
                    }
                    SearchSubAction::Close => {
                        menu.expanded = None;
                        menu.in_sub_menu = false;
                        terminal::enable_raw_mode()?;
                        list_lines = render_search_menu(&mut out, config, &menu)?;
                        render_menu_hint(&mut out, &menu)?;
                        continue;
                    }
                }

                terminal::enable_raw_mode()?;
                list_lines = render_search_menu(&mut out, config, &menu)?;
                render_menu_hint(&mut out, &menu)?;
            }

            // 5. Esc in sub-menu - FIFTH
            KeyCode::Esc if menu.in_sub_menu => {
                menu.expanded = None;
                menu.in_sub_menu = false;
                list_lines = rerender_menu(&mut out, config, &menu, list_lines)?;
            }

            // 6. Esc in main menu - SIXTH
            KeyCode::Esc if !menu.in_sub_menu && menu.can_go_back => {
                terminal::disable_raw_mode()?;
                ui::erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Back);
            }
            KeyCode::Esc if !menu.in_sub_menu => {
                terminal::disable_raw_mode()?;
                ui::erase_lines(list_lines + 1)?;
                return Ok(MenuOutcome::Done);
            }

            _ => {}
        }
    }
}

/// Rerender the menu after a state change (erases old, renders new).
fn rerender_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &SearchMenuState,
    prev_lines: usize,
) -> Result<usize> {
    let total = prev_lines + 1; // +1 for hint bar
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?; // Manual ANSI codes
    }
    let new_lines = render_search_menu(out, config, menu)?;
    render_menu_hint(out, menu)?;
    Ok(new_lines)
}

/// Render the semantic search menu. Returns total lines rendered (WITHOUT hint).
fn render_search_menu(
    out: &mut impl Write,
    config: &Config,
    menu: &SearchMenuState,
) -> Result<usize> {
    let prefix = format!("{} ", colorize("│", BRAND));
    let mut lines = 0;

    // Main row: Semantic Search
    let is_expanded = menu.expanded == Some(0);
    let is_cursor = !menu.in_sub_menu && menu.is_on_semantic_search();

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

    let enabled = config.todo.search.enabled;
    let status_icon = if enabled {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };

    let title = if is_cursor || is_expanded {
        colorize("Semantic Search", BOLD)
    } else {
        "Semantic Search".to_string()
    };

    let status_text = if enabled {
        colorize("(enabled)", SUCCESS)
    } else {
        colorize("(disabled)", DIM)
    };

    write!(
        out,
        "{}{}{} {} {} — {}\r\n",
        prefix, pointer, expand_colored, status_icon, title, status_text
    )?;
    lines += 1;

    // Sub-menu (if expanded)
    if is_expanded {
        for (si, action) in SUB_ACTIONS.iter().enumerate() {
            let available = action.is_available(enabled);

            let sub_pointer = if menu.in_sub_menu && menu.sub_cursor == si {
                colorize("❯", BRAND)
            } else {
                " ".to_string()
            };

            let label = action.label(enabled);
            let label_display = if menu.in_sub_menu && menu.sub_cursor == si {
                if available {
                    colorize(&label, BOLD)
                } else {
                    colorize(&label, DIM)
                }
            } else if *action == SearchSubAction::Close {
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

    // Separator
    write!(
        out,
        "{}  {}\r\n",
        prefix,
        colorize("──────────────────────────", DIM)
    )?;
    lines += 1;

    // Back row (only if available)
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
fn render_menu_hint(out: &mut impl Write, menu: &SearchMenuState) -> Result<()> {
    let prefix = format!("{} ", colorize("│", BRAND));

    let hint = if menu.in_sub_menu {
        "↑↓/jk navigate · Enter select · Esc close"
    } else if menu.can_go_back {
        "↑↓/jk navigate · Enter select · Esc back"
    } else {
        "↑↓/jk navigate · Enter select · Esc done"
    };

    write!(out, "{}  {}\r\n", prefix, colorize(hint, DIM))?;
    out.flush()?;
    Ok(())
}

/// Fallback menu for non-TTY environments.
fn run_search_menu_fallback(config: &mut Config, _can_go_back: bool) -> Result<MenuOutcome> {
    println!();
    println!("{}", colorize("Semantic Search Configuration", BOLD));
    println!();
    println!("Enable semantic search? (finds related concepts, not just keywords)");
    println!("  {} ~420 MB model download (one-time)", colorize("•", DIM));
    println!("  {} 50+ languages supported", colorize("•", DIM));
    println!();
    print!("Enable [Y/n]: ");
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    if choice.is_empty() || choice == "y" || choice == "yes" {
        config.todo.search.enabled = true;
        println!("{} Semantic search enabled", colorize("✓", SUCCESS));
        println!("Model will download on first use (~60 sec)");
    } else {
        config.todo.search.enabled = false;
        println!("{} Semantic search disabled", colorize("✓", DIM));
    }
    println!();

    Ok(MenuOutcome::Done)
}
