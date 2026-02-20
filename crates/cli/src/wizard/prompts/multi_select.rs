//! Multi-select prompts with checkbox-style toggling.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::event::KeyCode;

use super::select::SelectOption;
use super::{erase_lines, is_interactive, read_key, step_prefix, RawModeGuard};

/// Prompt the user to select multiple options from a list.
///
/// In interactive mode, uses arrow keys to navigate, Space to toggle,
/// `a` to toggle all, and Enter to confirm. Falls back to comma-separated
/// number input for non-TTY.
///
/// Returns 0-based indices of selected options.
pub fn prompt_multi_select(header: &str, options: &[SelectOption<'_>]) -> Result<Vec<usize>> {
    let prefix = step_prefix();

    if is_interactive() {
        prompt_multi_select_interactive(header, options)
    } else {
        prompt_multi_select_fallback(&prefix, header, options)
    }
}

fn prompt_multi_select_interactive(
    header: &str,
    options: &[SelectOption<'_>],
) -> Result<Vec<usize>> {
    let prefix = step_prefix();
    let mut cursor_pos = 0usize;
    let mut checked = vec![false; options.len()];

    if !header.is_empty() {
        println!("{}{}:", prefix, header);
    }

    let _guard = RawModeGuard::enable()?;
    let mut out = io::stdout();

    // Render initial list
    let list_lines = render_multi_select_list(&mut out, &prefix, options, &checked, cursor_pos)?;

    // Hint bar
    let hint = format!(
        "{}{}",
        prefix,
        colorize(
            "  \u{2191}/\u{2193} navigate  \u{00b7}  Space toggle  \u{00b7}  a all  \u{00b7}  Enter confirm",
            DIM
        )
    );
    write!(out, "{}\r\n", hint)?;
    out.flush()?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if cursor_pos > 0 {
                    cursor_pos -= 1;
                    rerender_multi_select_list(
                        &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                    )?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor_pos < options.len() - 1 {
                    cursor_pos += 1;
                    rerender_multi_select_list(
                        &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                    )?;
                }
            }
            KeyCode::Char(' ') => {
                checked[cursor_pos] = !checked[cursor_pos];
                rerender_multi_select_list(
                    &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                )?;
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all_checked = checked.iter().all(|&c| c);
                for c in checked.iter_mut() {
                    *c = !all_checked;
                }
                rerender_multi_select_list(
                    &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                )?;
            }
            KeyCode::Enter => {
                let total_lines = list_lines + 1; // +1 for hint
                drop(_guard);
                erase_lines(total_lines)?;

                let selected: Vec<usize> = checked
                    .iter()
                    .enumerate()
                    .filter(|(_, &c)| c)
                    .map(|(i, _)| i)
                    .collect();

                if selected.is_empty() {
                    println!(
                        "{}  {} {}",
                        prefix,
                        colorize("○", DIM),
                        colorize("None selected", DIM)
                    );
                } else {
                    let names: Vec<&str> = selected.iter().map(|&i| options[i].label).collect();
                    println!(
                        "{}  {} {}",
                        prefix,
                        colorize("●", BRAND),
                        colorize(&names.join(", "), BOLD)
                    );
                }

                return Ok(selected);
            }
            _ => {}
        }
    }
}

fn render_multi_select_list(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectOption<'_>],
    checked: &[bool],
    cursor_pos: usize,
) -> Result<usize> {
    let mut lines = 0;
    for (i, opt) in options.iter().enumerate() {
        let check = if checked[i] {
            colorize("[●]", BRAND)
        } else {
            colorize("[ ]", DIM)
        };
        let pointer = if i == cursor_pos {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };
        let label = if i == cursor_pos {
            colorize(opt.label, BOLD)
        } else {
            opt.label.to_string()
        };

        write!(
            out,
            "{} {} {} {} {}\r\n",
            prefix,
            pointer,
            check,
            label,
            colorize(opt.description, DIM)
        )?;
        lines += 1;
    }
    out.flush()?;
    Ok(lines)
}

fn rerender_multi_select_list(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectOption<'_>],
    checked: &[bool],
    cursor_pos: usize,
    list_lines: usize,
) -> Result<()> {
    let total = list_lines + 1; // +1 for hint
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }

    render_multi_select_list(out, prefix, options, checked, cursor_pos)?;

    let hint = format!(
        "{}{}",
        prefix,
        colorize(
            "  \u{2191}/\u{2193} navigate  \u{00b7}  Space toggle  \u{00b7}  a all  \u{00b7}  Enter confirm",
            DIM
        )
    );
    write!(out, "{}\r\n", hint)?;
    out.flush()?;
    Ok(())
}

fn prompt_multi_select_fallback(
    prefix: &str,
    header: &str,
    options: &[SelectOption<'_>],
) -> Result<Vec<usize>> {
    if !header.is_empty() {
        println!(
            "{}{} (comma-separated numbers, or Enter to skip):",
            prefix, header
        );
    }
    println!("{}", prefix);

    for (idx, opt) in options.iter().enumerate() {
        println!(
            "{}  {}. {} - {}",
            prefix,
            colorize(&(idx + 1).to_string(), BOLD),
            opt.label,
            colorize(opt.description, DIM)
        );
    }
    println!("{}", prefix);

    loop {
        print!("{}Selection []: ", prefix);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(vec![]);
        }

        let mut selected = Vec::new();
        let mut valid = true;

        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.parse::<usize>() {
                Ok(n) if n >= 1 && n <= options.len() => {
                    if !selected.contains(&(n - 1)) {
                        selected.push(n - 1);
                    }
                }
                _ => {
                    println!(
                        "{}{}",
                        prefix,
                        colorize(
                            &format!(
                                "Invalid selection '{}'. Enter numbers 1-{}.",
                                part,
                                options.len()
                            ),
                            ERROR
                        )
                    );
                    valid = false;
                    break;
                }
            }
        }

        if valid {
            return Ok(selected);
        }
    }
}

/// Prompt the user to select multiple options from a list with pre-checked defaults.
///
/// Like `prompt_multi_select`, but accepts a `defaults` slice indicating which
/// options should be pre-checked.
///
/// Returns 0-based indices of selected options.
pub fn prompt_multi_select_with_defaults(
    header: &str,
    options: &[SelectOption<'_>],
    defaults: &[bool],
) -> Result<Vec<usize>> {
    let prefix = step_prefix();

    if is_interactive() {
        prompt_multi_select_with_defaults_interactive(header, options, defaults)
    } else {
        // Fallback to standard multi-select (defaults shown in header)
        prompt_multi_select_fallback(&prefix, header, options)
    }
}

fn prompt_multi_select_with_defaults_interactive(
    header: &str,
    options: &[SelectOption<'_>],
    defaults: &[bool],
) -> Result<Vec<usize>> {
    let prefix = step_prefix();
    let mut cursor_pos = 0usize;
    let mut checked: Vec<bool> = if defaults.len() == options.len() {
        defaults.to_vec()
    } else {
        vec![false; options.len()]
    };

    if !header.is_empty() {
        println!("{}{}:", prefix, header);
    }

    let _guard = RawModeGuard::enable()?;
    let mut out = io::stdout();

    let list_lines = render_multi_select_list(&mut out, &prefix, options, &checked, cursor_pos)?;

    let hint = format!(
        "{}{}",
        prefix,
        colorize(
            "  \u{2191}/\u{2193} navigate  \u{00b7}  Space toggle  \u{00b7}  a all  \u{00b7}  Enter confirm",
            DIM
        )
    );
    write!(out, "{}\r\n", hint)?;
    out.flush()?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if cursor_pos > 0 {
                    cursor_pos -= 1;
                    rerender_multi_select_list(
                        &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                    )?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor_pos < options.len() - 1 {
                    cursor_pos += 1;
                    rerender_multi_select_list(
                        &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                    )?;
                }
            }
            KeyCode::Char(' ') => {
                checked[cursor_pos] = !checked[cursor_pos];
                rerender_multi_select_list(
                    &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                )?;
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all_checked = checked.iter().all(|&c| c);
                for c in checked.iter_mut() {
                    *c = !all_checked;
                }
                rerender_multi_select_list(
                    &mut out, &prefix, options, &checked, cursor_pos, list_lines,
                )?;
            }
            KeyCode::Enter => {
                let total_lines = list_lines + 1;
                drop(_guard);
                erase_lines(total_lines)?;

                let selected: Vec<usize> = checked
                    .iter()
                    .enumerate()
                    .filter(|(_, &c)| c)
                    .map(|(i, _)| i)
                    .collect();

                if selected.is_empty() {
                    println!(
                        "{}  {} {}",
                        prefix,
                        colorize("○", DIM),
                        colorize("None selected", DIM)
                    );
                } else {
                    let names: Vec<&str> = selected.iter().map(|&i| options[i].label).collect();
                    println!(
                        "{}  {} {}",
                        prefix,
                        colorize("●", BRAND),
                        colorize(&names.join(", "), BOLD)
                    );
                }

                return Ok(selected);
            }
            _ => {}
        }
    }
}
