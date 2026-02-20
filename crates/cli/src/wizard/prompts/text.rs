//! Text input prompts: plain text, optional, and list collection.

use std::io::{self, Write};

use anyhow::Result;
use common::utils::terminal::*;

use super::{prompt_yes_no, step_prefix};

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
