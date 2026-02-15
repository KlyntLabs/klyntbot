//! Shared UI primitives for wizard modules.
//!
//! Terminal helpers used across all wizard steps: raw mode management,
//! key reading, line erasing, and formatting utilities.

use std::io;

use anyhow::{anyhow, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
};

// ============================================================================
// Cursor / Erase
// ============================================================================

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

// ============================================================================
// Key Input
// ============================================================================

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
