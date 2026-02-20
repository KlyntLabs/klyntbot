//! Single-select and select-with-input prompts.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;
use crossterm::event::KeyCode;

use super::{erase_lines, is_interactive, read_key, step_prefix, RawModeGuard};

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
}
