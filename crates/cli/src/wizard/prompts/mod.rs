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

mod multi_select;
mod secret;
mod select;
mod text;
mod yes_no;

pub use self::multi_select::*;
pub use self::secret::*;
pub use self::select::*;
pub use self::text::*;
pub use self::yes_no::*;

use std::io::{self, IsTerminal};

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
pub(crate) struct RawModeGuard;

impl RawModeGuard {
    pub(crate) fn enable() -> Result<Self> {
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
pub(crate) fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Returns the orange `│ ` prefix string for consistent vertical line.
pub(crate) fn step_prefix() -> String {
    let chars = BoxChars::get();
    format!("{} ", colorize(chars.vertical, BRAND))
}

/// Moves cursor up `n` lines and clears each one.
pub(crate) fn erase_lines(n: usize) -> Result<()> {
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
pub(crate) fn read_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Err(anyhow!("Ctrl+C"));
            }
            return Ok(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
