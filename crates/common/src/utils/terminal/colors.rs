//! ANSI color codes, terminal detection, and foundational rendering primitives.

use std::env;
use std::io::{self, IsTerminal};
use unicode_width::UnicodeWidthChar;

// ============================================================================
// Color Scheme (ANSI Codes)
// ============================================================================

/// ANSI escape code to reset all formatting
pub const RESET: &str = "\x1b[0m";

/// Dim blue for prompts
pub const PROMPT: &str = "\x1b[2;34m";

/// Dim white for headers
pub const HEADER: &str = "\x1b[2;37m";

/// Cyan for tool calls
pub const TOOL: &str = "\x1b[36m";

/// Green for success
pub const SUCCESS: &str = "\x1b[32m";

/// Red for errors
pub const ERROR: &str = "\x1b[31m";

/// Yellow for warnings
pub const WARNING: &str = "\x1b[33m";

/// Gray for dim text
pub const DIM: &str = "\x1b[90m";

/// Dim gray for separators
pub const SEPARATOR: &str = "\x1b[2;90m";

/// Orange for brand/primary actions (Klyntbot theme color)
pub const BRAND: &str = "\x1b[38;5;208m";

/// Bright orange for highlights
pub const HIGHLIGHT: &str = "\x1b[38;5;214m";

/// Blue for informational elements
pub const INFO: &str = "\x1b[34m";

/// Magenta for special elements
pub const ACCENT: &str = "\x1b[35m";

/// Bold text
pub const BOLD: &str = "\x1b[1m";

/// Italic text
pub const ITALIC: &str = "\x1b[3m";

/// Underline text
pub const UNDERLINE: &str = "\x1b[4m";

/// Strikethrough text
pub const STRIKETHROUGH: &str = "\x1b[9m";

// ============================================================================
// Terminal State Detection
// ============================================================================

/// Checks if colors should be enabled based on environment and TTY status
pub fn colors_enabled() -> bool {
    // Check NO_COLOR environment variable
    if env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check if stdout is a TTY
    io::stdout().is_terminal()
}

/// Returns the given ANSI code if colors are enabled, empty string otherwise
pub fn color(code: &str) -> &str {
    if colors_enabled() {
        code
    } else {
        ""
    }
}

/// Wraps text with an ANSI color code (only if colors enabled)
pub fn colorize(text: &str, code: &str) -> String {
    if colors_enabled() {
        format!("{}{}{}", code, text, RESET)
    } else {
        text.to_string()
    }
}

/// Calculates the visible display width of a string, ignoring ANSI escape codes
/// and using Unicode character widths for proper terminal column counting.
pub fn display_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;

    for ch in s.chars() {
        if in_escape {
            // End of ANSI escape sequence
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    width
}

/// Pads a string with spaces to reach the target display width.
/// Accounts for ANSI codes and Unicode character widths.
pub fn pad_to_width(s: &str, target: usize) -> String {
    let current = display_width(s);
    if current >= target {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target - current))
    }
}

// ============================================================================
// Status Indicators
// ============================================================================

/// Returns a green checkmark indicator
pub fn status_success() -> String {
    colorize("✓", SUCCESS)
}

/// Returns a gray circle indicator for disabled items
pub fn status_disabled() -> String {
    colorize("○", DIM)
}

/// Returns a red X indicator for errors
pub fn status_error() -> String {
    colorize("✗", ERROR)
}

/// Returns a yellow exclamation mark for warnings
pub fn status_warning() -> String {
    colorize("!", WARNING)
}

/// Returns an orange arrow indicator for in-progress items
pub fn status_progress() -> String {
    colorize("→", BRAND)
}

/// Returns an orange filled circle for active items
pub fn status_active() -> String {
    colorize("●", BRAND)
}

// ============================================================================
// Box Drawing Characters
// ============================================================================

/// Characters used for box drawing
pub struct BoxChars {
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub horizontal_down: &'static str,
    pub horizontal_up: &'static str,
    pub vertical_right: &'static str,
}

impl BoxChars {
    /// Unicode box drawing characters
    pub const UNICODE: BoxChars = BoxChars {
        top_left: "┌",
        top_right: "┐",
        bottom_left: "└",
        bottom_right: "┘",
        horizontal: "─",
        vertical: "│",
        horizontal_down: "┬",
        horizontal_up: "┴",
        vertical_right: "├",
    };

    /// ASCII fallback characters
    pub const ASCII: BoxChars = BoxChars {
        top_left: "+",
        top_right: "+",
        bottom_left: "+",
        bottom_right: "+",
        horizontal: "-",
        vertical: "|",
        horizontal_down: "+",
        horizontal_up: "+",
        vertical_right: "+",
    };

    /// Returns the appropriate box characters based on environment
    pub fn get() -> &'static BoxChars {
        if colors_enabled() {
            &BoxChars::UNICODE
        } else {
            &BoxChars::ASCII
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colors_disabled_with_no_color() {
        env::set_var("NO_COLOR", "1");
        assert!(!colors_enabled());
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_colorize_respects_no_color() {
        env::set_var("NO_COLOR", "1");
        let text = colorize("hello", SUCCESS);
        assert_eq!(text, "hello");
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_status_indicators() {
        env::set_var("NO_COLOR", "1");
        assert_eq!(status_success(), "✓");
        assert_eq!(status_disabled(), "○");
        assert_eq!(status_error(), "✗");
        assert_eq!(status_warning(), "!");
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_color_function_with_no_color() {
        env::set_var("NO_COLOR", "1");
        assert_eq!(color(SUCCESS), "");
        assert_eq!(color(ERROR), "");
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_box_chars_unicode() {
        let chars = &BoxChars::UNICODE;
        assert_eq!(chars.top_left, "┌");
        assert_eq!(chars.horizontal, "─");
        assert_eq!(chars.vertical, "│");
    }

    #[test]
    fn test_box_chars_ascii() {
        let chars = &BoxChars::ASCII;
        assert_eq!(chars.top_left, "+");
        assert_eq!(chars.horizontal, "-");
        assert_eq!(chars.vertical, "|");
    }
}
