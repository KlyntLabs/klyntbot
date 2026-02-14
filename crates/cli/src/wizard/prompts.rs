//! Shared prompt utilities for wizard modules.
//!
//! Provides consistent input prompts used across all wizard steps:
//! - Text input with optional defaults and validation
//! - Yes/no confirmation prompts (single-keypress in interactive mode)
//! - Interactive arrow-key selection from a list
//! - Interactive multi-select with space-to-toggle
//! - Secret/password input with masking
//!
//! All prompts fall back to simple line-based input when stdin/stdout
//! are not a TTY (e.g. piped input, CI environments).

use std::io::{self, IsTerminal, Write};

use anyhow::{anyhow, Result};
use common::utils::terminal::*;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
};

// ============================================================================
// Terminal helpers
// ============================================================================

/// RAII guard that disables raw mode on drop (safety against panics/early returns).
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Returns true if both stdin and stdout are interactive terminals.
fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Returns the orange `│ ` prefix string for consistent vertical line.
fn step_prefix() -> String {
    let chars = BoxChars::get();
    format!("{} ", colorize(chars.vertical, BRAND))
}

/// Moves cursor up `n` lines and clears each one.
fn erase_lines(n: usize) -> Result<()> {
    let mut out = io::stdout();
    for _ in 0..n {
        crossterm::execute!(
            out,
            cursor::MoveUp(1),
            terminal::Clear(ClearType::CurrentLine)
        )?;
    }
    Ok(())
}

/// Reads a single key event, converting Ctrl+C into an error.
fn read_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Err(anyhow!("Ctrl+C"));
            }
            return Ok(key);
        }
    }
}

// ============================================================================
// Yes/No prompt
// ============================================================================

/// Prompt for a yes/no answer with a default value.
///
/// In interactive mode, accepts a single keypress (y/n/Enter) without
/// needing to press Enter after y or n. Falls back to read_line for non-TTY.
pub fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    let prefix = step_prefix();

    if is_interactive() {
        let _guard = RawModeGuard::enable()?;
        let mut out = io::stdout();

        write!(out, "{}{} [{}]: ", prefix, prompt, default_str)?;
        out.flush()?;

        loop {
            let key = read_key()?;
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    write!(out, "Yes\r\n")?;
                    out.flush()?;
                    return Ok(true);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    write!(out, "No\r\n")?;
                    out.flush()?;
                    return Ok(false);
                }
                KeyCode::Enter => {
                    let answer = if default { "Yes" } else { "No" };
                    write!(out, "{}\r\n", answer)?;
                    out.flush()?;
                    return Ok(default);
                }
                _ => {}
            }
        }
    } else {
        // Non-TTY fallback
        print!("{}{} [{}]: ", prefix, prompt, default_str);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input.is_empty() {
            return Ok(default);
        }

        Ok(input == "y" || input == "yes")
    }
}

// ============================================================================
// Text input
// ============================================================================

/// Prompt for text input with an optional default value.
///
/// If `default` is provided and the user presses Enter, returns the default.
/// If `required` is true and no default, re-prompts until non-empty input.
pub fn prompt_text(label: &str, default: Option<&str>, required: bool) -> Result<String> {
    let prefix = step_prefix();
    loop {
        if let Some(def) = default {
            print!("{}{} [{}]: ", prefix, label, def);
        } else {
            print!("{}{}: ", prefix, label);
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            if let Some(def) = default {
                return Ok(def.to_string());
            }
            if required {
                println!("{}{}", prefix, colorize("This field is required", ERROR));
                continue;
            }
            return Ok(String::new());
        }

        return Ok(input);
    }
}

/// Prompt for an optional text value. Returns None if empty.
pub fn prompt_optional(label: &str) -> Result<Option<String>> {
    let prefix = step_prefix();
    print!("{}{}: ", prefix, label);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_string();

    if input.is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}

// ============================================================================
// Secret input
// ============================================================================

/// Prompt for secret input (API keys, passwords).
///
/// In interactive mode, masks input with `●` characters and supports
/// backspace. Falls back to plain text input for non-TTY.
pub fn prompt_secret(label: &str, min_length: usize) -> Result<String> {
    let prefix = step_prefix();

    if is_interactive() {
        loop {
            let _guard = RawModeGuard::enable()?;
            let mut out = io::stdout();
            let mut secret = String::new();

            write!(out, "{}{}: ", prefix, label)?;
            out.flush()?;

            loop {
                let key = read_key()?;
                match key.code {
                    KeyCode::Enter => {
                        write!(out, "\r\n")?;
                        out.flush()?;
                        break;
                    }
                    KeyCode::Backspace => {
                        if !secret.is_empty() {
                            secret.pop();
                            // Move back, overwrite with space, move back again
                            write!(out, "\x08 \x08")?;
                            out.flush()?;
                        }
                    }
                    KeyCode::Char(c) => {
                        secret.push(c);
                        write!(out, "●")?;
                        out.flush()?;
                    }
                    _ => {}
                }
            }

            // Drop guard before validation print to restore normal mode
            drop(_guard);

            if secret.is_empty() {
                println!("{}{}", prefix, colorize("This field is required", ERROR));
                continue;
            }

            if secret.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(secret);
        }
    } else {
        // Non-TTY fallback
        loop {
            print!("{}{}: ", prefix, label);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                println!("{}{}", prefix, colorize("This field is required", ERROR));
                continue;
            }

            if input.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(input);
        }
    }
}

// ============================================================================
// Single-select
// ============================================================================

/// Option for a selection prompt.
pub struct SelectOption<'a> {
    pub label: &'a str,
    pub description: &'a str,
}

/// Prompt the user to select one option from a list.
///
/// In interactive mode, uses arrow keys (+ j/k) to navigate and Enter to
/// confirm. Falls back to numbered-list input for non-TTY.
///
/// Returns the 0-based index of the selected option.
pub fn prompt_select(
    header: &str,
    options: &[SelectOption<'_>],
    default_idx: usize,
) -> Result<usize> {
    let prefix = step_prefix();

    if is_interactive() {
        prompt_select_interactive(header, options, default_idx)
    } else {
        prompt_select_fallback(&prefix, header, options, default_idx)
    }
}

fn prompt_select_interactive(
    header: &str,
    options: &[SelectOption<'_>],
    default_idx: usize,
) -> Result<usize> {
    let prefix = step_prefix();
    let mut selected = default_idx;

    if !header.is_empty() {
        println!("{}{}:", prefix, header);
    }

    let _guard = RawModeGuard::enable()?;
    let mut out = io::stdout();

    // Render initial list
    let list_lines = render_select_list(&mut out, &prefix, options, selected)?;

    // Hint bar
    let hint = format!(
        "{}{}",
        prefix,
        colorize("  \u{2191}/\u{2193} navigate  \u{00b7}  Enter select", DIM)
    );
    write!(out, "{}\r\n", hint)?;
    out.flush()?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if selected > 0 {
                    selected -= 1;
                    rerender_select_list(&mut out, &prefix, options, selected, list_lines)?;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if selected < options.len() - 1 {
                    selected += 1;
                    rerender_select_list(&mut out, &prefix, options, selected, list_lines)?;
                }
            }
            KeyCode::Enter => {
                // Erase list + hint bar
                let total_lines = list_lines + 1; // +1 for hint
                drop(_guard);
                erase_lines(total_lines)?;

                // Print compact selection
                println!(
                    "{}  {} {}",
                    prefix,
                    colorize("●", BRAND),
                    colorize(options[selected].label, BOLD)
                );

                return Ok(selected);
            }
            _ => {}
        }
    }
}

fn render_select_list(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectOption<'_>],
    selected: usize,
) -> Result<usize> {
    let mut lines = 0;
    for (i, opt) in options.iter().enumerate() {
        if i == selected {
            write!(
                out,
                "{}  {} {} {}\r\n",
                prefix,
                colorize("❯", BRAND),
                colorize(opt.label, BOLD),
                colorize(opt.description, DIM)
            )?;
        } else {
            write!(
                out,
                "{}    {} {}\r\n",
                prefix,
                opt.label,
                colorize(opt.description, DIM)
            )?;
        }
        lines += 1;
    }
    out.flush()?;
    Ok(lines)
}

fn rerender_select_list(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectOption<'_>],
    selected: usize,
    list_lines: usize,
) -> Result<()> {
    // Move up past hint line + all list lines
    let total = list_lines + 1;
    for _ in 0..total {
        write!(out, "\x1b[A\x1b[2K")?;
    }

    render_select_list(out, prefix, options, selected)?;

    // Re-print hint
    let hint = format!(
        "{}{}",
        prefix,
        colorize("  \u{2191}/\u{2193} navigate  \u{00b7}  Enter select", DIM)
    );
    write!(out, "{}\r\n", hint)?;
    out.flush()?;
    Ok(())
}

fn prompt_select_fallback(
    prefix: &str,
    header: &str,
    options: &[SelectOption<'_>],
    default_idx: usize,
) -> Result<usize> {
    if !header.is_empty() {
        println!("{}{}:", prefix, header);
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
        print!("{}Choice [{}]: ", prefix, default_idx + 1);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(default_idx);
        }

        if let Ok(idx) = input.parse::<usize>() {
            if idx > 0 && idx <= options.len() {
                return Ok(idx - 1);
            }
        }

        println!(
            "{}{}",
            prefix,
            colorize(
                &format!("Please enter a number between 1 and {}", options.len()),
                ERROR
            )
        );
    }
}

// ============================================================================
// Multi-select
// ============================================================================

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

// ============================================================================
// Select with expandable text input
// ============================================================================

/// Option for a select-with-input prompt.
pub struct SelectWithInputOption<'a> {
    pub label: &'a str,
    pub description: &'a str,
    pub expandable: bool,
    pub input_hint: Option<&'a str>,
}

/// Result from a select-with-input prompt.
pub struct SelectWithInputResult {
    pub index: usize,
    pub text: Option<String>,
}

/// Hybrid select/text-input prompt.
///
/// Each option can optionally be "expandable" — pressing TAB on an expandable
/// option reveals an inline text field. User can type text, then press Enter
/// to confirm the selection with the text.
///
/// Returns the selected index and optional text input.
pub fn prompt_select_with_input(
    header: &str,
    options: &[SelectWithInputOption<'_>],
    default_idx: usize,
) -> Result<SelectWithInputResult> {
    if is_interactive() {
        prompt_select_with_input_interactive(header, options, default_idx)
    } else {
        prompt_select_with_input_fallback(header, options, default_idx)
    }
}

/// State for the select-with-input component.
enum InputState {
    Selecting,
    Expanded { buffer: String },
}

fn prompt_select_with_input_interactive(
    header: &str,
    options: &[SelectWithInputOption<'_>],
    default_idx: usize,
) -> Result<SelectWithInputResult> {
    let prefix = step_prefix();
    let mut selected = default_idx;
    let mut state = InputState::Selecting;

    if !header.is_empty() {
        println!("{}{}:", prefix, header);
    }

    let _guard = RawModeGuard::enable()?;
    let mut out = io::stdout();

    // Initial render
    render_select_with_input_list(&mut out, &prefix, options, selected, &state)?;
    render_select_input_hint_bar(&mut out, &prefix, &options[selected], &state)?;

    loop {
        let key = read_key()?;
        match key.code {
            KeyCode::Up => {
                if matches!(state, InputState::Selecting) && selected > 0 {
                    selected -= 1;
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Down => {
                if matches!(state, InputState::Selecting) && selected < options.len() - 1 {
                    selected += 1;
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Char('k') => {
                if matches!(state, InputState::Selecting) && selected > 0 {
                    selected -= 1;
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                } else if let InputState::Expanded { ref mut buffer } = state {
                    buffer.push('k');
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Char('j') => {
                if matches!(state, InputState::Selecting) && selected < options.len() - 1 {
                    selected += 1;
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                } else if let InputState::Expanded { ref mut buffer } = state {
                    buffer.push('j');
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Tab => {
                if matches!(state, InputState::Selecting) && options[selected].expandable {
                    state = InputState::Expanded {
                        buffer: String::new(),
                    };
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Esc => {
                if matches!(state, InputState::Expanded { .. }) {
                    state = InputState::Selecting;
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Char(c) => {
                if let InputState::Expanded { ref mut buffer } = state {
                    buffer.push(c);
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Backspace => {
                if let InputState::Expanded { ref mut buffer } = state {
                    buffer.pop();
                    rerender_select_with_input(&mut out, &prefix, options, selected, &state)?;
                }
            }
            KeyCode::Enter => {
                drop(_guard);
                erase_lines(count_select_input_lines(options, &state) + 1)?;

                let text = match state {
                    InputState::Expanded { buffer } => Some(buffer),
                    InputState::Selecting => None,
                };

                // Compact final output
                print!(
                    "{}  {} {}",
                    prefix,
                    colorize("●", BRAND),
                    options[selected].label
                );
                if let Some(ref t) = text {
                    if !t.is_empty() {
                        print!("\n{}  \"{}\"", prefix, colorize(t, DIM));
                    }
                }
                println!();

                return Ok(SelectWithInputResult {
                    index: selected,
                    text,
                });
            }
            _ => {}
        }
    }
}

fn render_select_with_input_list(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectWithInputOption<'_>],
    selected: usize,
    state: &InputState,
) -> Result<()> {
    for (i, opt) in options.iter().enumerate() {
        let pointer = if i == selected {
            colorize("❯", BRAND)
        } else {
            " ".to_string()
        };
        let label = if i == selected {
            colorize(opt.label, BOLD)
        } else {
            opt.label.to_string()
        };
        let desc = colorize(opt.description, DIM);

        // Show TAB hint on expandable options when selected
        let hint = if i == selected && opt.expandable && matches!(state, InputState::Selecting) {
            format!("  {}", colorize("[TAB to expand]", DIM))
        } else {
            String::new()
        };

        write!(
            out,
            "{}  {} {} — {}{}\r\n",
            prefix, pointer, label, desc, hint
        )?;

        // If this option is selected and expanded, show input line
        if i == selected {
            if let InputState::Expanded { ref buffer } = state {
                let hint_text = opt.input_hint.unwrap_or("Type here");
                let display = if buffer.is_empty() {
                    colorize(hint_text, DIM)
                } else {
                    buffer.clone()
                };
                write!(out, "{}    > {}█\r\n", prefix, display)?;
            }
        }
    }

    out.flush()?;
    Ok(())
}

fn render_select_input_hint_bar(
    out: &mut impl Write,
    prefix: &str,
    current_opt: &SelectWithInputOption<'_>,
    state: &InputState,
) -> Result<()> {
    let hint = match state {
        InputState::Selecting if current_opt.expandable => {
            "[↑↓] Navigate  [TAB] Expand  [Enter] Confirm"
        }
        InputState::Selecting => "[↑↓] Navigate  [Enter] Confirm",
        InputState::Expanded { .. } => "[Esc] Back  [Enter] Submit",
    };

    write!(out, "{}{}\r\n", prefix, colorize(hint, DIM))?;
    out.flush()?;
    Ok(())
}

fn count_select_input_lines(options: &[SelectWithInputOption<'_>], state: &InputState) -> usize {
    let mut count = options.len();
    if matches!(state, InputState::Expanded { .. }) {
        count += 1; // Extra line for input field
    }
    count
}

fn rerender_select_with_input(
    out: &mut impl Write,
    prefix: &str,
    options: &[SelectWithInputOption<'_>],
    selected: usize,
    state: &InputState,
) -> Result<()> {
    let total_lines = count_select_input_lines(options, state) + 1; // +1 for hint bar
    for _ in 0..total_lines {
        write!(out, "\x1b[A\x1b[2K")?;
    }
    render_select_with_input_list(out, prefix, options, selected, state)?;
    render_select_input_hint_bar(out, prefix, &options[selected], state)?;
    Ok(())
}

fn prompt_select_with_input_fallback(
    header: &str,
    options: &[SelectWithInputOption<'_>],
    default_idx: usize,
) -> Result<SelectWithInputResult> {
    let prefix = step_prefix();

    if !header.is_empty() {
        println!("{}{}:", prefix, header);
    }

    for (i, opt) in options.iter().enumerate() {
        let expandable = if opt.expandable {
            " (type text after number to add input)"
        } else {
            ""
        };
        println!(
            "{}  {}. {} — {}{}",
            prefix,
            colorize(&(i + 1).to_string(), BOLD),
            opt.label,
            colorize(opt.description, DIM),
            expandable
        );
    }

    loop {
        print!("{}Choice [{}]: ", prefix, default_idx + 1);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Parse "N text" format
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let index = if let Some(first) = parts.first() {
            first.parse::<usize>().ok().and_then(|n| {
                if n > 0 && n <= options.len() {
                    Some(n - 1)
                } else {
                    None
                }
            })
        } else {
            None
        };

        let selected = if input.is_empty() {
            default_idx
        } else if let Some(idx) = index {
            idx
        } else {
            println!(
                "{}{}",
                prefix,
                colorize(
                    &format!("Please enter a number between 1 and {}", options.len()),
                    ERROR
                )
            );
            continue;
        };

        let text_input = if parts.len() > 1 {
            Some(parts[1].to_string())
        } else {
            None
        };

        return Ok(SelectWithInputResult {
            index: selected,
            text: text_input,
        });
    }
}

// ============================================================================
// Secret masking & edit-in-place helpers
// ============================================================================

/// Mask a secret for display, showing a prefix and last 4 chars.
///
/// Examples: `"sk-ant-api03-abc...xyz1234"` → `"sk-ant-****1234"`
///           `"short"` → `"●●●●●"`
pub fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }

    let len = s.len();
    if len <= 8 {
        return "●".repeat(len);
    }

    // Find a recognizable prefix (e.g. "sk-ant-", "sk-or-", "gsk_", "sk-", "BSA-")
    let prefix_len = if s.starts_with("sk-ant-") {
        7
    } else if s.starts_with("sk-or-") {
        6
    } else if s.starts_with("gsk_") || s.starts_with("BSA-") {
        4
    } else if s.starts_with("sk-") {
        3
    } else {
        0
    };

    let suffix_len = 4;
    if prefix_len + suffix_len >= len {
        return "●".repeat(len);
    }

    let prefix = &s[..prefix_len];
    let suffix = &s[len - suffix_len..];
    format!("{}****{}", prefix, suffix)
}

/// Prompt for a secret with an existing value. Shows masked preview, Enter to keep.
///
/// Returns `None` if the user pressed Enter (keep existing), `Some(new)` if they
/// entered a new value.
pub fn prompt_secret_with_existing(
    label: &str,
    existing: &str,
    min_length: usize,
) -> Result<Option<String>> {
    let prefix = step_prefix();
    let masked = mask_secret(existing);

    if is_interactive() {
        loop {
            let _guard = RawModeGuard::enable()?;
            let mut out = io::stdout();
            let mut secret = String::new();

            write!(out, "{}{} [{}]: ", prefix, label, masked)?;
            out.flush()?;

            loop {
                let key = read_key()?;
                match key.code {
                    KeyCode::Enter => {
                        write!(out, "\r\n")?;
                        out.flush()?;
                        break;
                    }
                    KeyCode::Backspace => {
                        if !secret.is_empty() {
                            secret.pop();
                            write!(out, "\x08 \x08")?;
                            out.flush()?;
                        }
                    }
                    KeyCode::Char(c) => {
                        secret.push(c);
                        write!(out, "●")?;
                        out.flush()?;
                    }
                    _ => {}
                }
            }

            drop(_guard);

            if secret.is_empty() {
                // Keep existing
                return Ok(None);
            }

            if secret.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(Some(secret));
        }
    } else {
        // Non-TTY fallback
        loop {
            print!("{}{} [{}]: ", prefix, label, masked);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                return Ok(None);
            }

            if input.len() < min_length {
                println!(
                    "{}{}",
                    prefix,
                    colorize(
                        &format!(
                            "Value seems too short (expected at least {} chars)",
                            min_length
                        ),
                        WARNING
                    )
                );
                continue;
            }

            return Ok(Some(input));
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

// ============================================================================
// List input
// ============================================================================

/// Prompt to collect items one per line until an empty line is entered.
pub fn prompt_list(header: &str, existing: &[String]) -> Result<Vec<String>> {
    let prefix = step_prefix();
    println!("{}{}", prefix, colorize(header, DIM));

    let mut items = existing.to_vec();

    if !items.is_empty() {
        println!("{}  Current: {}", prefix, items.join(", "));
        let clear = prompt_yes_no("Clear existing and start fresh?", false)?;
        if clear {
            items.clear();
        }
    }

    loop {
        print!("{}  {} ", prefix, colorize("+", SUCCESS));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            break;
        }

        if items.contains(&input) {
            println!("{}    {}", prefix, colorize("(already in list)", DIM));
            continue;
        }

        items.push(input);
    }

    Ok(items)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_option_creation() {
        let opt = SelectOption {
            label: "Test",
            description: "A test option",
        };
        assert_eq!(opt.label, "Test");
        assert_eq!(opt.description, "A test option");
    }

    #[test]
    fn test_step_prefix_non_empty() {
        let prefix = step_prefix();
        assert!(!prefix.is_empty());
    }

    #[test]
    fn test_is_interactive_runs() {
        // Just ensure it doesn't panic — actual value depends on test runner
        let _ = is_interactive();
    }

    #[test]
    fn test_select_with_input_option_creation() {
        let opt = SelectWithInputOption {
            label: "Test",
            description: "A test option",
            expandable: true,
            input_hint: Some("Type here"),
        };
        assert_eq!(opt.label, "Test");
        assert_eq!(opt.description, "A test option");
        assert!(opt.expandable);
        assert_eq!(opt.input_hint, Some("Type here"));
    }

    #[test]
    fn test_select_with_input_option_no_hint() {
        let opt = SelectWithInputOption {
            label: "Simple",
            description: "No expansion",
            expandable: false,
            input_hint: None,
        };
        assert_eq!(opt.label, "Simple");
        assert!(!opt.expandable);
        assert_eq!(opt.input_hint, None);
    }

    #[test]
    fn test_select_with_input_result() {
        let result = SelectWithInputResult {
            index: 2,
            text: Some("user input".to_string()),
        };
        assert_eq!(result.index, 2);
        assert_eq!(result.text, Some("user input".to_string()));
    }

    #[test]
    fn test_select_with_input_result_no_text() {
        let result = SelectWithInputResult {
            index: 0,
            text: None,
        };
        assert_eq!(result.index, 0);
        assert_eq!(result.text, None);
    }

    // ========================================================================
    // mask_secret tests
    // ========================================================================

    #[test]
    fn test_mask_secret_empty() {
        assert_eq!(mask_secret(""), "");
    }

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(mask_secret("abc"), "●●●");
        assert_eq!(mask_secret("12345678"), "●●●●●●●●");
    }

    #[test]
    fn test_mask_secret_anthropic_key() {
        assert_eq!(
            mask_secret("sk-ant-api03-abcdefghijklmnop"),
            "sk-ant-****mnop"
        );
    }

    #[test]
    fn test_mask_secret_openai_key() {
        assert_eq!(mask_secret("sk-proj-abcdefghij"), "sk-****ghij");
    }

    #[test]
    fn test_mask_secret_openrouter_key() {
        assert_eq!(mask_secret("sk-or-v1-abcdefghij"), "sk-or-****ghij");
    }

    #[test]
    fn test_mask_secret_groq_key() {
        assert_eq!(mask_secret("gsk_abcdefghijklm"), "gsk_****jklm");
    }

    #[test]
    fn test_mask_secret_brave_key() {
        assert_eq!(mask_secret("BSA-abcdefghijklm"), "BSA-****jklm");
    }

    #[test]
    fn test_mask_secret_no_prefix() {
        assert_eq!(mask_secret("abcdefghijklmnop"), "****mnop");
    }
}
