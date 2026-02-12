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

use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

// ============================================================================
// Braille Spinner
// ============================================================================

/// Braille spinner patterns (8 frames)
const SPINNER_FRAMES: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// A braille spinner for indicating ongoing operations
pub struct Spinner {
    message: String,
    running: Arc<Mutex<bool>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Creates a new spinner with the given message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            running: Arc::new(Mutex::new(false)),
            thread_handle: None,
        }
    }

    /// Starts the spinner animation
    pub fn start(&mut self) {
        let running = Arc::clone(&self.running);
        *running.lock().unwrap() = true;

        let message = self.message.clone();
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            let mut frame = 0;
            while *running_clone.lock().unwrap() {
                let spinner_char = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
                print!("\r{} {}  ", colorize(spinner_char, DIM), message);
                io::stdout().flush().unwrap();

                frame += 1;
                thread::sleep(Duration::from_millis(100));
            }

            // Clear the spinner line completely
            print!("\r\x1b[K");
            io::stdout().flush().unwrap();
        });

        self.thread_handle = Some(handle);
    }

    /// Stops the spinner and clears the line
    pub fn stop(&mut self) {
        // Set the flag via Mutex::lock (not Arc::get_mut, which fails when
        // the spinner thread still holds a clone of the Arc).
        *self.running.lock().unwrap() = false;

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Updates the spinner message while it's running
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// Box Drawing
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

/// Wraps text in a box with an optional header
pub fn draw_box(content: &str, header: Option<&str>) -> String {
    let chars = BoxChars::get();
    let lines: Vec<&str> = content.lines().collect();

    // Calculate the maximum line width
    let max_width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
        .max(header.map(|h| h.len()).unwrap_or(0) + 4);

    let mut result = String::new();

    // Top border with optional header
    if let Some(header_text) = header {
        let padding = max_width.saturating_sub(header_text.len() + 2);
        result.push_str(&format!(
            "{}{}{} {} {}{}{}",
            color(SEPARATOR),
            chars.top_left,
            chars.horizontal,
            header_text,
            chars.horizontal.repeat(padding),
            chars.top_right,
            color(RESET)
        ));
    } else {
        result.push_str(&format!(
            "{}{}{}{}{}",
            color(SEPARATOR),
            chars.top_left,
            chars.horizontal.repeat(max_width + 2),
            chars.top_right,
            color(RESET)
        ));
    }
    result.push('\n');

    // Content lines
    for line in lines {
        let padding = max_width.saturating_sub(line.chars().count());
        result.push_str(&format!(
            "{}{}{} {}{}{}",
            color(SEPARATOR),
            chars.vertical,
            color(RESET),
            line,
            " ".repeat(padding),
            &colorize(chars.vertical, SEPARATOR)
        ));
        result.push('\n');
    }

    // Bottom border
    result.push_str(&format!(
        "{}{}{}{}{}",
        color(SEPARATOR),
        chars.bottom_left,
        chars.horizontal.repeat(max_width + 2),
        chars.bottom_right,
        color(RESET)
    ));

    result
}

/// Renders a code block with language label
pub fn draw_code_block(code: &str, language: Option<&str>) -> String {
    let header = language.map(|lang| format!(" {} ", lang));
    draw_box(code, header.as_deref())
}

/// Renders a table with headers and rows
pub fn draw_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }

    let chars = BoxChars::get();

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut result = String::new();

    // Top border
    result.push_str(color(SEPARATOR));
    result.push_str(chars.top_left);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push_str(chars.horizontal_down);
        }
    }
    result.push_str(chars.top_right);
    result.push_str(color(RESET));
    result.push('\n');

    // Header row
    result.push_str(&colorize(chars.vertical, SEPARATOR));
    for (i, header) in headers.iter().enumerate() {
        result.push_str(&format!(
            " {:<width$} {}",
            colorize(header, BOLD),
            &colorize(chars.vertical, SEPARATOR),
            width = widths[i]
        ));
    }
    result.push('\n');

    // Separator after header
    result.push_str(color(SEPARATOR));
    result.push_str(chars.vertical_right);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push('┼');
        }
    }
    result.push('┤');
    result.push_str(color(RESET));
    result.push('\n');

    // Data rows
    for row in rows {
        result.push_str(&colorize(chars.vertical, SEPARATOR));
        for (i, cell) in row.iter().enumerate() {
            let width = widths.get(i).copied().unwrap_or(0);
            result.push_str(&format!(
                " {:<width$} {}",
                cell,
                &colorize(chars.vertical, SEPARATOR),
                width = width
            ));
        }
        result.push('\n');
    }

    // Bottom border
    result.push_str(color(SEPARATOR));
    result.push_str(chars.bottom_left);
    for (i, &width) in widths.iter().enumerate() {
        result.push_str(&chars.horizontal.repeat(width + 2));
        if i < widths.len() - 1 {
            result.push_str(chars.horizontal_up);
        }
    }
    result.push_str(chars.bottom_right);
    result.push_str(color(RESET));

    result
}

// ============================================================================
// Error Display
// ============================================================================

/// Displays a structured error message with title, problem description, and fix steps
pub fn display_error(title: &str, problem: &str, fix_steps: &[&str], docs: Option<&str>) -> String {
    let mut result = String::new();

    // Error title
    result.push_str(&format!("{} {}\n\n", colorize("Error:", ERROR), title));

    // Problem description
    result.push_str(&format!("{}:\n", colorize("Problem", BOLD)));
    result.push_str(&format!("  {}\n\n", problem));

    // Fix steps
    result.push_str(&format!("{}:\n", colorize("How to fix", BOLD)));
    for (i, step) in fix_steps.iter().enumerate() {
        result.push_str(&format!("  {}. {}\n", i + 1, step));
    }

    // Optional documentation link
    if let Some(doc_link) = docs {
        result.push_str(&format!("\n{}:\n", colorize("Documentation", BOLD)));
        result.push_str(&format!("  {}\n", doc_link));
    }

    result
}

// ============================================================================
// Markdown Terminal Renderer
// ============================================================================

/// Simple markdown to terminal converter
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Renders markdown text to terminal-formatted output
    pub fn render(markdown: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();

        for line in markdown.lines() {
            // Handle code blocks
            if line.starts_with("```") {
                if in_code_block {
                    // End of code block
                    result.push_str(&draw_code_block(
                        code_block_content.trim_end(),
                        if code_block_lang.is_empty() {
                            None
                        } else {
                            Some(&code_block_lang)
                        },
                    ));
                    result.push('\n');
                    code_block_content.clear();
                    code_block_lang.clear();
                    in_code_block = false;
                } else {
                    // Start of code block
                    code_block_lang = line.trim_start_matches('`').trim().to_string();
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                code_block_content.push_str(line);
                code_block_content.push('\n');
                continue;
            }

            // Handle blockquotes
            if line.starts_with('>') {
                let quote_text = line.trim_start_matches('>').trim();
                result.push_str(&format!(
                    "{} {}\n",
                    colorize("│", DIM),
                    Self::render_inline(quote_text)
                ));
                continue;
            }

            // Handle unordered lists
            if line.trim_start().starts_with("- ") || line.trim_start().starts_with("* ") {
                let indent = line.len() - line.trim_start().len();
                let text = line
                    .trim_start()
                    .trim_start_matches("- ")
                    .trim_start_matches("* ");
                result.push_str(&format!(
                    "{}{} {}\n",
                    " ".repeat(indent),
                    colorize("•", DIM),
                    Self::render_inline(text)
                ));
                continue;
            }

            // Handle ordered lists
            if let Some(text) = Self::parse_ordered_list(line) {
                result.push_str(&format!("{}\n", Self::render_inline(text)));
                continue;
            }

            // Handle headers
            if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count();
                let text = line.trim_start_matches('#').trim();
                result.push_str(&format!("{}\n", colorize(text, BOLD)));
                if level <= 2 {
                    let separator_line = "─".repeat(text.len());
                    result.push_str(&format!("{}\n", colorize(&separator_line, SEPARATOR)));
                }
                continue;
            }

            // Regular text with inline formatting
            if !line.trim().is_empty() {
                result.push_str(&format!("{}\n", Self::render_inline(line)));
            } else {
                result.push('\n');
            }
        }

        result
    }

    /// Renders inline markdown formatting (bold, italic, code, links)
    fn render_inline(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                // Bold **text**
                '*' if chars.peek() == Some(&'*') => {
                    chars.next(); // consume second *
                    let mut bold_text = String::new();
                    let mut found_end = false;

                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'*') {
                            chars.next(); // consume second *
                            found_end = true;
                            break;
                        }
                        bold_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&bold_text, BOLD));
                    } else {
                        result.push_str("**");
                        result.push_str(&bold_text);
                    }
                }
                // Italic *text* or _text_
                '*' | '_' => {
                    let mut italic_text = String::new();
                    let mut found_end = false;

                    for c in chars.by_ref() {
                        if c == ch {
                            found_end = true;
                            break;
                        }
                        italic_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&italic_text, ITALIC));
                    } else {
                        result.push(ch);
                        result.push_str(&italic_text);
                    }
                }
                // Inline code `text`
                '`' => {
                    let mut code_text = String::new();
                    let mut found_end = false;

                    for c in chars.by_ref() {
                        if c == '`' {
                            found_end = true;
                            break;
                        }
                        code_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&code_text, DIM));
                    } else {
                        result.push('`');
                        result.push_str(&code_text);
                    }
                }
                // Strikethrough ~~text~~
                '~' if chars.peek() == Some(&'~') => {
                    chars.next(); // consume second ~
                    let mut strike_text = String::new();
                    let mut found_end = false;

                    while let Some(c) = chars.next() {
                        if c == '~' && chars.peek() == Some(&'~') {
                            chars.next(); // consume second ~
                            found_end = true;
                            break;
                        }
                        strike_text.push(c);
                    }

                    if found_end {
                        result.push_str(&colorize(&strike_text, STRIKETHROUGH));
                    } else {
                        result.push_str("~~");
                        result.push_str(&strike_text);
                    }
                }
                // Links [text](url)
                '[' => {
                    let mut link_text = String::new();
                    let mut found_bracket = false;

                    for c in chars.by_ref() {
                        if c == ']' {
                            found_bracket = true;
                            break;
                        }
                        link_text.push(c);
                    }

                    if found_bracket && chars.peek() == Some(&'(') {
                        chars.next(); // consume (
                        let mut url = String::new();

                        for c in chars.by_ref() {
                            if c == ')' {
                                break;
                            }
                            url.push(c);
                        }

                        result.push_str(&colorize(&format!("{} ({})", link_text, url), UNDERLINE));
                    } else {
                        result.push('[');
                        result.push_str(&link_text);
                        if found_bracket {
                            result.push(']');
                        }
                    }
                }
                _ => result.push(ch),
            }
        }

        result
    }

    /// Parses ordered list items (e.g., "1. text")
    fn parse_ordered_list(line: &str) -> Option<&str> {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if let Some(dot_pos) = trimmed.find(". ") {
            let prefix = &trimmed[..dot_pos];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                let spaces = " ".repeat(indent);
                return Some(&line[spaces.len()..]);
            }
        }
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

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
    fn test_spinner_creation() {
        let spinner = Spinner::new("testing");
        assert_eq!(spinner.message, "testing");
    }

    #[test]
    fn test_box_drawing_no_header() {
        env::set_var("NO_COLOR", "1");
        let box_output = draw_box("hello\nworld", None);
        assert!(box_output.contains("hello"));
        assert!(box_output.contains("world"));
        assert!(box_output.contains("+"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_box_drawing_with_header() {
        env::set_var("NO_COLOR", "1");
        let box_output = draw_box("content", Some("Header"));
        assert!(box_output.contains("Header"));
        assert!(box_output.contains("content"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_code_block() {
        env::set_var("NO_COLOR", "1");
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let output = draw_code_block(code, Some("rust"));
        assert!(output.contains("rust"));
        assert!(output.contains("fn main()"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_table_rendering() {
        env::set_var("NO_COLOR", "1");
        let headers = vec!["Name", "Age", "City"];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string(), "NYC".to_string()],
            vec!["Bob".to_string(), "25".to_string(), "LA".to_string()],
        ];
        let output = draw_table(&headers, &rows);
        assert!(output.contains("Name"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_bold() {
        env::set_var("NO_COLOR", "1");
        let md = "This is **bold** text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("bold"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_italic() {
        env::set_var("NO_COLOR", "1");
        let md = "This is *italic* text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("italic"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_code() {
        env::set_var("NO_COLOR", "1");
        let md = "Use `code` here";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("code"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_code_block() {
        env::set_var("NO_COLOR", "1");
        let md = "```rust\nfn main() {}\n```";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("rust"));
        assert!(output.contains("fn main()"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_list() {
        env::set_var("NO_COLOR", "1");
        let md = "- Item 1\n- Item 2\n- Item 3";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
        assert!(output.contains("•"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_ordered_list() {
        env::set_var("NO_COLOR", "1");
        let md = "1. First\n2. Second\n3. Third";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("First"));
        assert!(output.contains("Second"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_blockquote() {
        env::set_var("NO_COLOR", "1");
        let md = "> This is a quote\n> Multiple lines";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("This is a quote"));
        assert!(output.contains("Multiple lines"));
        assert!(output.contains("│"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_link() {
        env::set_var("NO_COLOR", "1");
        let md = "Check [this link](https://example.com)";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("this link"));
        assert!(output.contains("https://example.com"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_header() {
        env::set_var("NO_COLOR", "1");
        let md = "# Header 1\n## Header 2";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Header 1"));
        assert!(output.contains("Header 2"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_strikethrough() {
        env::set_var("NO_COLOR", "1");
        let md = "This is ~~strikethrough~~ text";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("strikethrough"));
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

    #[test]
    fn test_spinner_frames() {
        assert_eq!(SPINNER_FRAMES.len(), 8);
        assert_eq!(SPINNER_FRAMES[0], "⣾");
        assert_eq!(SPINNER_FRAMES[7], "⣷");
    }

    #[test]
    fn test_table_empty_headers() {
        let output = draw_table(&[], &[]);
        assert_eq!(output, "");
    }

    #[test]
    fn test_markdown_nested_list() {
        env::set_var("NO_COLOR", "1");
        let md = "- Item 1\n  - Nested 1\n  - Nested 2\n- Item 2";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("Item 1"));
        assert!(output.contains("Nested 1"));
        assert!(output.contains("•"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_mixed_formatting() {
        env::set_var("NO_COLOR", "1");
        let md = "This has **bold** and *italic* and `code`";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("bold"));
        assert!(output.contains("italic"));
        assert!(output.contains("code"));
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
    fn test_draw_box_multiline_content() {
        env::set_var("NO_COLOR", "1");
        let content = "Line 1\nLine 2\nLine 3";
        let output = draw_box(content, None);
        assert!(output.contains("Line 1"));
        assert!(output.contains("Line 2"));
        assert!(output.contains("Line 3"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_table_column_width_calculation() {
        env::set_var("NO_COLOR", "1");
        let headers = vec!["Short", "Very Long Header"];
        let rows = vec![
            vec!["A".to_string(), "B".to_string()],
            vec!["C".to_string(), "D".to_string()],
        ];
        let output = draw_table(&headers, &rows);
        // Headers should determine column width
        assert!(output.contains("Very Long Header"));
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_incomplete_formatting() {
        env::set_var("NO_COLOR", "1");
        // Test unclosed bold
        let md = "This is **incomplete";
        let output = MarkdownRenderer::render(md);
        assert!(output.contains("incomplete"));

        // Test unclosed code
        let md2 = "This is `incomplete";
        let output2 = MarkdownRenderer::render(md2);
        assert!(output2.contains("incomplete"));

        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_markdown_empty_code_block() {
        env::set_var("NO_COLOR", "1");
        let md = "```\n```";
        let output = MarkdownRenderer::render(md);
        // Should render empty box
        assert!(output.contains("+"));
        env::remove_var("NO_COLOR");
    }
}
