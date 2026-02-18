//! Box drawing, banner, wizard UI components, and error display.

use super::colors::{
    color, colorize, BoxChars, BOLD, BRAND, DIM, ERROR, RESET, SEPARATOR, SUCCESS,
};

// ============================================================================
// Chat Banner
// ============================================================================

/// Draws the startup banner with ASCII logo, model info, and tips.
///
/// In non-TTY or NO_COLOR environments, falls back to a plain one-liner.
pub fn draw_banner(model: &str) -> String {
    if super::colors::colors_enabled() {
        let logo = [
            "  ╭─╮    ╭─╮",
            "  │ │╭─╮ │ │╭─╮ ╭─╮╭─╮╭───╮",
            "  │ ╰╯ ╰─╯ ╰╯ ╰─╯ ╰╯ ╰╯   │",
            "  ╰──────────────────────────╯",
        ];
        let mut result = String::from("\n");
        for (i, line) in logo.iter().enumerate() {
            result.push_str(&colorize(line, BRAND));
            if i == logo.len() - 1 {
                result.push_str(&format!("  {}", colorize("klyntbot", BOLD)));
            }
            result.push('\n');
        }
        result.push_str(&format!(
            "  {} {}\n",
            colorize("Ready", SUCCESS),
            colorize(&format!("· {}", model), DIM),
        ));
        result.push_str(&format!(
            "\n  {}  {}\n",
            colorize("Tips:", DIM),
            colorize("1. Ask questions or give tasks", DIM),
        ));
        result.push_str(&format!(
            "         {}\n",
            colorize("2. Use /help for commands", DIM),
        ));
        result.push_str(&format!(
            "         {}\n",
            colorize("3. Press Ctrl+C to cancel", DIM),
        ));
        result
    } else {
        format!("\n  klyntbot · {}\n", model)
    }
}

// ============================================================================
// Enhanced Wizard UI Components
// ============================================================================

/// Draws a progress bar for wizard steps with connecting lines
pub fn draw_step_progress(current: usize, total: usize) -> String {
    if total == 0 {
        return String::new();
    }

    let mut result = String::new();

    for i in 1..=total {
        if i == current {
            // Current step - orange filled circle
            result.push_str(&colorize("●", BRAND));
        } else if i < current {
            // Completed step - green checkmark
            result.push_str(&colorize("✓", SUCCESS));
        } else {
            // Future step - dim circle
            result.push_str(&colorize("○", DIM));
        }

        // Add connector line (except after last step)
        if i < total {
            let connector = if i < current {
                colorize("─", SUCCESS) // Completed section - green
            } else if i == current {
                colorize("─", BRAND) // Current section - orange
            } else {
                colorize("─", DIM) // Future section - dim
            };
            result.push_str(&connector);
        }
    }

    result.push('\n');
    result
}

/// Draws an enhanced step header with orange branding and progress
pub fn draw_wizard_step_header(current: usize, total: usize, title: &str) -> String {
    let mut result = String::new();
    let chars = BoxChars::get();

    // Progress bar at left margin (no │ prefix)
    result.push_str(&draw_step_progress(current, total));

    // Step number and title with orange branding and vertical line
    result.push_str(&format!(
        "{} {} {}\n",
        colorize(chars.vertical, BRAND),
        colorize(&format!("Step {} of {}", current, total), BRAND),
        colorize(title, BOLD)
    ));

    // Blank vertical line before content starts
    result.push_str(&format!("{}\n", colorize(chars.vertical, BRAND)));

    result
}

/// Draws text with vertical line prefix (for content within a step)
pub fn draw_step_line(text: &str) -> String {
    let chars = BoxChars::get();
    format!("{} {}", colorize(chars.vertical, BRAND), text)
}

/// Draws the bottom connector for a step (vertical line continues to next step)
pub fn draw_step_footer() -> String {
    let chars = BoxChars::get();
    format!("{}\n", colorize(chars.vertical, BRAND))
}

// ============================================================================
// Box Drawing Functions
// ============================================================================

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

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
    fn test_code_block() {
        env::set_var("NO_COLOR", "1");
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let output = draw_code_block(code, Some("rust"));
        assert!(output.contains("rust"));
        assert!(output.contains("fn main()"));
        env::remove_var("NO_COLOR");
    }
}
