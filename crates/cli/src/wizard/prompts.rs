//! Shared prompt utilities for wizard modules.
//!
//! Provides consistent input prompts used across all wizard steps:
//! - Text input with optional defaults and validation
//! - Yes/no confirmation prompts
//! - Numbered selection from a list
//! - Secret/password input (not echoed)

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;

/// Prompt for a yes/no answer with a default value.
pub fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default);
    }

    Ok(input == "y" || input == "yes")
}

/// Prompt for text input with an optional default value.
///
/// If `default` is provided and the user presses Enter, returns the default.
/// If `required` is true and no default, re-prompts until non-empty input.
pub fn prompt_text(label: &str, default: Option<&str>, required: bool) -> Result<String> {
    loop {
        if let Some(def) = default {
            print!("{} [{}]: ", label, def);
        } else {
            print!("{}: ", label);
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
                println!("{}", colorize("  This field is required", ERROR));
                continue;
            }
            return Ok(String::new());
        }

        return Ok(input);
    }
}

/// Prompt for secret input (API keys, passwords).
///
/// Input is not hidden (terminal raw mode would require crossterm setup),
/// but validation ensures minimum length.
pub fn prompt_secret(label: &str, min_length: usize) -> Result<String> {
    loop {
        print!("{}: ", label);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("{}", colorize("  This field is required", ERROR));
            continue;
        }

        if input.len() < min_length {
            println!(
                "{}",
                colorize(
                    &format!("  Value seems too short (expected at least {} chars)", min_length),
                    WARNING
                )
            );
            continue;
        }

        return Ok(input);
    }
}

/// Option for a selection prompt.
pub struct SelectOption<'a> {
    pub label: &'a str,
    pub description: &'a str,
}

/// Prompt the user to select one option from a numbered list.
///
/// Returns the 0-based index of the selected option.
/// `default_idx` is the 0-based default (shown in brackets).
pub fn prompt_select(
    header: &str,
    options: &[SelectOption<'_>],
    default_idx: usize,
) -> Result<usize> {
    if !header.is_empty() {
        println!("{}:\n", header);
    }

    for (idx, opt) in options.iter().enumerate() {
        println!(
            "  {}. {} - {}",
            colorize(&(idx + 1).to_string(), BOLD),
            opt.label,
            colorize(opt.description, DIM)
        );
    }
    println!();

    loop {
        print!("Choice [{}]: ", default_idx + 1);
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
            "{}",
            colorize(
                &format!("Please enter a number between 1 and {}", options.len()),
                ERROR
            )
        );
    }
}

/// Prompt to collect items one per line until an empty line is entered.
///
/// Returns the collected items.
pub fn prompt_list(header: &str, existing: &[String]) -> Result<Vec<String>> {
    println!("{}", colorize(header, DIM));

    let mut items = existing.to_vec();

    if !items.is_empty() {
        println!("  Current: {}", items.join(", "));
        let clear = prompt_yes_no("  Clear existing and start fresh?", false)?;
        if clear {
            items.clear();
        }
    }

    loop {
        print!("  {} ", colorize("+", SUCCESS));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            break;
        }

        if items.contains(&input) {
            println!("    {}", colorize("(already in list)", DIM));
            continue;
        }

        items.push(input);
    }

    Ok(items)
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
}
