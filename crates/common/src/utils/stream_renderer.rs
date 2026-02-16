//! Stream renderer for real-time LLM output with post-completion markdown re-rendering.
//!
//! During streaming, raw text is printed token-by-token for instant feedback.
//! On finalize, the raw output is erased and replaced with a markdown-rendered version.
//! In non-TTY environments, cursor manipulation is skipped for graceful degradation.

use std::io::{self, IsTerminal, Write};
use std::time::Instant;

use crossterm::{
    cursor,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use super::terminal::{
    colorize, colors_enabled, status_error, status_success, MarkdownRenderer, DIM,
    SEPARATOR, TOOL,
};

/// Tracks the state of a tool being executed.
struct ToolStatus {
    name: String,
    state: ToolState,
    /// Number of output lines printed after this tool's start line.
    lines_after: u16,
}

#[allow(dead_code)] // duration_ms stored for potential future use (e.g. summary on demand)
enum ToolState {
    Running,
    Completed(u64),
    Failed(u64),
}

/// Renders streamed LLM output in real-time, then re-renders with markdown formatting.
pub struct StreamRenderer {
    buffer: String,
    /// Total lines of output rendered (content + tool lines).
    rendered_lines: u16,
    tool_statuses: Vec<ToolStatus>,
    start_time: Instant,
    is_tty: bool,
    /// Terminal width in columns, used for line-wrap calculations.
    terminal_width: u16,
    /// Whether the response was cancelled (Ctrl+C or unexpected termination).
    cancelled: bool,
}

impl StreamRenderer {
    /// Create a new stream renderer.
    pub fn new() -> Self {
        let terminal_width = terminal::size().map(|(w, _)| w).unwrap_or(80);
        Self {
            buffer: String::new(),
            rendered_lines: 0,
            tool_statuses: Vec::new(),
            start_time: Instant::now(),
            is_tty: io::stdout().is_terminal() && colors_enabled(),
            terminal_width,
            cancelled: false,
        }
    }

    /// Create a non-TTY renderer for testing.
    #[cfg(test)]
    fn new_non_tty(terminal_width: u16) -> Self {
        Self {
            buffer: String::new(),
            rendered_lines: 0,
            tool_statuses: Vec::new(),
            start_time: Instant::now(),
            is_tty: false,
            terminal_width,
            cancelled: false,
        }
    }

    /// Handle a content chunk from the LLM stream.
    pub fn on_content_chunk(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);

        // Calculate visual lines accounting for terminal line wrapping
        let visual_lines = self.visual_line_count(chunk);
        self.rendered_lines += visual_lines;

        // Update lines_after for all running tools
        for status in &mut self.tool_statuses {
            if matches!(status.state, ToolState::Running) {
                status.lines_after += visual_lines;
            }
        }

        print!("{}", chunk);
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution starting.
    pub fn on_tool_start(&mut self, name: &str, args: &serde_json::Value) {
        // Update lines_after for existing running tools before adding the new one
        for status in &mut self.tool_statuses {
            if matches!(status.state, ToolState::Running) {
                status.lines_after += 1;
            }
        }

        self.tool_statuses.push(ToolStatus {
            name: name.to_string(),
            state: ToolState::Running,
            lines_after: 0,
        });

        // Format args inline: show up to 2 key-value pairs
        let args_display = format_tool_args(name, args);

        // Use spinner (⟳) for running state
        let prefix = colorize("⟳", TOOL);

        let line = if args_display.is_empty() {
            format!("  {} {}", prefix, colorize(name, TOOL))
        } else {
            format!(
                "  {} {} {}",
                prefix,
                colorize(name, TOOL),
                colorize(&args_display, DIM),
            )
        };
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution completing.
    pub fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64) {
        // Get lines_after before updating state (find first Running tool with this name)
        let lines_after = self
            .tool_statuses
            .iter()
            .find(|s| s.name == name && matches!(s.state, ToolState::Running))
            .map(|s| s.lines_after)
            .unwrap_or(0);

        // Update the tool status
        if let Some(status) = self
            .tool_statuses
            .iter_mut()
            .find(|s| s.name == name && matches!(s.state, ToolState::Running))
        {
            status.state = if success {
                ToolState::Completed(duration_ms)
            } else {
                ToolState::Failed(duration_ms)
            };
        }

        if self.is_tty {
            // The tool start line is (lines_after + 1) lines above cursor
            // (+1 because println moved cursor to next line after the tool start)
            let lines_back = lines_after + 1;
            let mut stdout = io::stdout();
            let _ = stdout.execute(cursor::MoveUp(lines_back));
            let _ = stdout.execute(terminal::Clear(ClearType::CurrentLine));
            self.print_tool_result(name, success, duration_ms);
            // println in print_tool_result moves cursor down 1, so we need (lines_back - 1) more
            if lines_back > 1 {
                let _ = stdout.execute(cursor::MoveDown(lines_back - 1));
            }
        } else {
            // Non-TTY: print the result on a new line
            self.print_tool_result(name, success, duration_ms);
            self.rendered_lines += 1;

            // Update lines_after for other running tools
            for status in &mut self.tool_statuses {
                if matches!(status.state, ToolState::Running) {
                    status.lines_after += 1;
                }
            }
        }

        let _ = io::stdout().flush();
    }

    /// Handle a new agent iteration starting.
    pub fn on_iteration_start(&mut self, iteration: usize, _max: usize) {
        // Suppress iteration messages entirely for cleaner output
        // Tool calls already show progress, no need for extra "processing..." lines
        let _ = iteration; // Silence unused warning
    }

    /// Pause rendering to allow an interactive prompt to take over the terminal.
    ///
    /// Ensures any in-progress line is terminated so the prompt starts on a fresh line.
    pub fn pause(&mut self) {
        if self.is_tty {
            // Ensure we're on a new line so the prompt renders cleanly
            println!();
            self.rendered_lines += 1;
        }
        let _ = io::stdout().flush();
    }

    /// Resume rendering after an interactive prompt has finished.
    ///
    /// The prompt's output was rendered between pause/resume; we account for it
    /// in rendered_lines so finalize() erases the correct number of lines.
    pub fn resume(&mut self, prompt_lines: u16) {
        self.rendered_lines += prompt_lines;
        let _ = io::stdout().flush();
    }

    /// Mark the response as cancelled (for Ctrl+C or unexpected termination).
    pub fn mark_cancelled(&mut self) {
        self.cancelled = true;
    }

    /// Finalize: erase raw output and replace with markdown-rendered version.
    ///
    /// Returns the formatted markdown string (with cancellation indicator if applicable).
    pub fn finalize(&mut self) -> String {
        let rendered = MarkdownRenderer::render(&self.buffer);

        if self.is_tty && (!self.buffer.is_empty() || self.rendered_lines > 0) {
            let mut stdout = io::stdout();

            // Move cursor to the start of our streamed output
            if self.rendered_lines > 0 {
                let _ = stdout.execute(cursor::MoveUp(self.rendered_lines));
            }
            // Move to column 0 — critical when rendered_lines is 0 and
            // content was printed via print!() leaving cursor mid-line.
            let _ = stdout.execute(cursor::MoveToColumn(0));
            // Clear everything from cursor position downward
            let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
            let _ = stdout.flush();
        }

        // Tool summaries are NOT re-printed here — they were already shown during streaming.
        // This avoids double-rendering tool status lines.

        if self.cancelled {
            format!("{}{}", rendered, colorize("\n[cancelled]", DIM))
        } else {
            rendered
        }
    }

    /// Get the elapsed time since the renderer was created.
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Draw a separator line with an optional label.
    pub fn draw_separator(label: Option<&str>) -> String {
        let width = terminal::size().map(|(w, _)| w as usize).unwrap_or(60);
        let sep_char = "\u{2500}"; // ─

        if let Some(text) = label {
            let text_with_spaces = format!(" {} ", text);
            let remaining = width.saturating_sub(text_with_spaces.len() + 6);
            let left = sep_char.repeat(3);
            let right = sep_char.repeat(remaining);
            colorize(&format!("{}{}{}", left, text_with_spaces, right), SEPARATOR)
        } else {
            colorize(&sep_char.repeat(width.min(60)), SEPARATOR)
        }
    }

    fn print_tool_result(&self, name: &str, success: bool, duration_ms: u64) {
        let indicator = if success {
            status_success()
        } else {
            status_error()
        };
        println!(
            "  {} {} {}",
            indicator,
            colorize(name, TOOL),
            colorize(&format!("{}ms", duration_ms), DIM),
        );
    }

    /// Calculate visual line count for a text chunk, accounting for terminal wrapping.
    ///
    /// Uses deferred-wrap semantics: a segment of exactly `width` chars does NOT wrap,
    /// but `width + 1` chars wraps once.
    fn visual_line_count(&self, text: &str) -> u16 {
        let width = self.terminal_width.max(1) as usize;
        let mut lines = 0u16;

        let segments: Vec<&str> = text.split('\n').collect();

        // Each \n moves cursor to the next line
        if segments.len() > 1 {
            lines += (segments.len() - 1) as u16;
        }

        // Long segments wrap to additional lines
        for segment in &segments {
            let len = segment.len();
            // (len - 1) / width gives extra wrap lines (0 for len <= width)
            if len > 0 {
                lines += (len.saturating_sub(1) / width) as u16;
            }
        }

        lines
    }
}

impl Default for StreamRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Format tool arguments for compact inline display.
///
/// Extracts up to 2 key-value pairs from a JSON object, truncating values to 40 chars.
/// Special handling for `ask_user`: shows title and question count instead of raw JSON.
fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    // ask_user: show clean title + question count
    if name == "ask_user" {
        if let Some(obj) = args.as_object() {
            let title = obj
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("User Input");
            let count = obj
                .get("questions")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            return format!(
                "\"{}\" ({} question{})",
                title,
                count,
                if count == 1 { "" } else { "s" }
            );
        }
    }

    let Some(obj) = args.as_object() else {
        return String::new();
    };

    if obj.is_empty() {
        return String::new();
    }

    let pairs: Vec<String> = obj
        .iter()
        .take(2)
        .map(|(k, v)| {
            let val_str = match v {
                serde_json::Value::String(s) => {
                    if s.len() > 40 {
                        format!("\"{}...\"", &s[..37])
                    } else {
                        format!("\"{}\"", s)
                    }
                }
                other => {
                    let s = other.to_string();
                    if s.len() > 40 {
                        format!("{}...", &s[..37])
                    } else {
                        s
                    }
                }
            };
            format!("{}={}", k, val_str)
        })
        .collect();

    pairs.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_renderer() -> StreamRenderer {
        StreamRenderer::new_non_tty(80)
    }

    #[test]
    fn test_content_chunk_accumulates_buffer() {
        let mut r = test_renderer();
        r.on_content_chunk("hello ");
        r.on_content_chunk("world");
        assert_eq!(r.buffer, "hello world");
    }

    #[test]
    fn test_tool_lifecycle_tracking() {
        let mut r = test_renderer();
        let args = json!({"path": "/src/main.rs"});
        r.on_tool_start("read_file", &args);
        assert_eq!(r.tool_statuses.len(), 1);
        assert!(matches!(r.tool_statuses[0].state, ToolState::Running));

        r.on_tool_end("read_file", true, 42);
        assert!(matches!(r.tool_statuses[0].state, ToolState::Completed(42)));
    }

    #[test]
    fn test_finalize_returns_rendered_markdown() {
        std::env::set_var("NO_COLOR", "1");
        let mut r = test_renderer();
        r.buffer = "**bold** text".to_string();
        let output = r.finalize();
        assert!(output.contains("bold"));
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_cancelled_shows_indicator() {
        std::env::set_var("NO_COLOR", "1");
        let mut r = test_renderer();
        r.buffer = "partial".to_string();
        r.mark_cancelled();
        let output = r.finalize();
        assert!(output.contains("[cancelled]"));
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_iteration_start_silent_for_first() {
        let mut r = test_renderer();
        r.on_iteration_start(1, 20);
        assert_eq!(r.rendered_lines, 0);
    }

    #[test]
    fn test_iteration_start_visible_for_subsequent() {
        let mut r = test_renderer();
        r.on_iteration_start(2, 20);
        assert!(r.rendered_lines > 0);
    }

    #[test]
    fn test_tool_args_compact_display() {
        let args = json!({"path": "/src/main.rs", "query": "hello world"});
        let display = format_tool_args("read_file", &args);
        assert!(display.contains("path="));
    }

    #[test]
    fn test_tool_args_truncation() {
        let long_value = "a".repeat(60);
        let args = json!({"key": long_value});
        let display = format_tool_args("some_tool", &args);
        assert!(display.contains("...\""));
    }

    #[test]
    fn test_tool_args_empty() {
        let args = json!({});
        let display = format_tool_args("some_tool", &args);
        assert!(display.is_empty());
    }

    #[test]
    fn test_tool_args_non_object() {
        let args = json!("just a string");
        let display = format_tool_args("some_tool", &args);
        assert!(display.is_empty());
    }

    #[test]
    fn test_tool_args_ask_user_clean_display() {
        let args = json!({
            "title": "Create a New To-Do Task",
            "questions": [
                {"id": "title", "text": "What is the title?", "type": "free_text"},
                {"id": "priority", "text": "Priority?", "type": "single_select"},
                {"id": "due", "text": "Due date?", "type": "free_text"}
            ]
        });
        let display = format_tool_args("ask_user", &args);
        assert_eq!(display, "\"Create a New To-Do Task\" (3 questions)");
    }

    #[test]
    fn test_tool_args_ask_user_single_question() {
        let args = json!({
            "title": "Confirm",
            "questions": [{"id": "q1", "text": "Proceed?", "type": "yes_no"}]
        });
        let display = format_tool_args("ask_user", &args);
        assert_eq!(display, "\"Confirm\" (1 question)");
    }

    #[test]
    fn test_visual_line_count_simple() {
        let r = test_renderer();
        assert_eq!(r.visual_line_count("hello"), 0); // no newline
        assert_eq!(r.visual_line_count("hello\n"), 1); // one newline
        assert_eq!(r.visual_line_count("hello\nworld"), 1); // one newline
        assert_eq!(r.visual_line_count("\n\n"), 2); // two newlines
    }

    #[test]
    fn test_visual_line_count_wrapping() {
        let r = StreamRenderer::new_non_tty(80);
        // 160 chars on 80-col = 1 wrap line
        let long_line = "a".repeat(160);
        assert_eq!(r.visual_line_count(&long_line), 1);

        // 81 chars = wraps once
        let just_over = "a".repeat(81);
        assert_eq!(r.visual_line_count(&just_over), 1);

        // Exactly 80 chars = no wrap (deferred wrap)
        let exact = "a".repeat(80);
        assert_eq!(r.visual_line_count(&exact), 0);
    }

    #[test]
    fn test_tool_lines_after_tracking() {
        let mut r = test_renderer();
        let args = json!({});

        r.on_tool_start("slow_tool", &args);
        r.on_content_chunk("some output\n");

        assert!(r.tool_statuses[0].lines_after > 0);
    }

    #[test]
    fn test_draw_separator_with_label() {
        std::env::set_var("NO_COLOR", "1");
        let sep = StreamRenderer::draw_separator(Some("1.2s"));
        assert!(sep.contains("1.2s"));
        assert!(sep.contains("\u{2500}"));
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_draw_separator_no_label() {
        std::env::set_var("NO_COLOR", "1");
        let sep = StreamRenderer::draw_separator(None);
        assert!(sep.contains("\u{2500}"));
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_elapsed_time() {
        let renderer = StreamRenderer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(renderer.elapsed_secs() > 0.0);
    }
}
