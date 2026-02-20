//! Yes/No confirmation prompt.

use std::io::{self, Write};

use anyhow::Result;
use crossterm::event::KeyCode;

use super::{is_interactive, read_key, step_prefix, RawModeGuard};

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
