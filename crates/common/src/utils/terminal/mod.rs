//! Terminal rendering utilities for klyntbot CLI.
//!
//! This module provides terminal rendering capabilities including:
//! - ANSI color output with NO_COLOR and TTY detection support
//! - Braille spinner for "thinking" indicators
//! - Box drawing for response display
//! - Status formatting (success, error, warning, disabled)
//! - Markdown terminal renderer
//!
//! All functionality respects the NO_COLOR environment variable and
//! automatically detects non-TTY output for graceful degradation.

pub mod boxes;
pub mod colors;
pub mod markdown;
pub mod spinners;
pub mod tables;

// Re-export all public items to preserve the existing `use terminal::*` API.
pub use boxes::*;
pub use colors::*;
pub use markdown::*;
pub use spinners::*;
pub use tables::*;
