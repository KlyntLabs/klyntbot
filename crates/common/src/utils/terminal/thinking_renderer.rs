//! Thinking phase renderer — shows pipeline stages and tool calls during AI processing.
//!
//! Normal mode: stage checkmarks + tool names with durations.
//! Verbose mode: adds confidence scores, token budgets, tool args.
//! Collapses to a one-line summary after completion.

use std::io::{self, Write};

use crossterm::{
    cursor,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use super::colors::{colorize, colors_enabled, DIM, SEPARATOR, TOOL};
use super::status_success;

/// Renders pipeline thinking trace during agent execution.
pub struct ThinkingRenderer {
    pub verbose: bool,
    is_tty: bool,
    pub rendered_lines: u16,
    pub tool_count: usize,
    pub iteration_count: usize,
    max_iterations: usize,
}

impl ThinkingRenderer {
    pub fn new(verbose: bool, is_tty: bool) -> Self {
        Self {
            verbose,
            is_tty,
            rendered_lines: 0,
            tool_count: 0,
            iteration_count: 0,
            max_iterations: 0,
        }
    }

    /// Handle classification stage completing.
    pub fn on_classification_complete(
        &mut self,
        strategy: &str,
        confidence: f32,
        source: &str,
        duration_ms: u64,
    ) {
        if self.verbose {
            let line = format!(
                "  {} Classified: {} ({:.2})  {}",
                status_success(),
                colorize(strategy, TOOL),
                confidence,
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
            let detail = format!("    {}", colorize(&format!("method: {}", source), DIM));
            println!("{}", detail);
            self.rendered_lines += 1;
        } else {
            let line = format!(
                "  {} Classified \u{2192} {}  {}",
                status_success(),
                colorize(strategy, TOOL),
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
        }
        let _ = io::stdout().flush();
    }

    /// Handle context assembly completing.
    pub fn on_context_assembled(&mut self, total_tokens: usize, budget: usize, duration_ms: u64) {
        if self.verbose {
            let line = format!(
                "  {} Context: {}/{} tokens  {}",
                status_success(),
                colorize(&total_tokens.to_string(), TOOL),
                budget,
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
        } else {
            let line = format!(
                "  {} Context assembled  {}",
                status_success(),
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
        }
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle execution engine starting.
    pub fn on_execution_started(&mut self, engine: &str, max_iterations: usize) {
        self.max_iterations = max_iterations;
        self.iteration_count = 1;
        let line = format!(
            "  {} Executing ({})",
            colorize("\u{25b8}", TOOL),
            colorize(engine, DIM),
        );
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a new iteration starting.
    pub fn on_iteration_start(&mut self, iteration: usize, max: usize) {
        self.iteration_count = iteration;
        self.max_iterations = max;
        let line = format!(
            "  {} Executing (iteration {}/{})",
            colorize("\u{25b8}", TOOL),
            iteration,
            max,
        );
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution starting.
    pub fn on_tool_start(&mut self, name: &str) {
        self.tool_count += 1;
        let line = format!(
            "    {} {}",
            colorize("\u{27f3}", TOOL),
            colorize(name, TOOL),
        );
        println!("{}", line);
        self.rendered_lines += 1;
        let _ = io::stdout().flush();
    }

    /// Handle a tool execution completing.
    pub fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64) {
        if self.is_tty {
            let mut stdout = io::stdout();
            let _ = stdout.execute(cursor::MoveUp(1));
            let _ = stdout.execute(terminal::Clear(ClearType::CurrentLine));

            let indicator = if success {
                status_success()
            } else {
                colorize("\u{2717}", "\x1b[31m")
            };
            println!(
                "    {} {} {}",
                indicator,
                colorize(name, TOOL),
                colorize(&format_duration(duration_ms), DIM),
            );
            let _ = stdout.flush();
            // No change to rendered_lines — we overwrote in place
        } else {
            let indicator = if success { "\u{2713}" } else { "\u{2717}" };
            println!("{} {} {}", indicator, name, format_duration(duration_ms),);
            self.rendered_lines += 1;
        }
    }

    /// Collapse the thinking trace to a one-line summary.
    ///
    /// Uses crossterm to erase all thinking lines and replace with a summary.
    /// In non-TTY mode, just prints the summary on a new line.
    pub fn collapse(&self, model: &str, elapsed_secs: f64) {
        let summary = self.summary_line(model, elapsed_secs);

        if self.is_tty && self.rendered_lines > 0 {
            let mut stdout = io::stdout();
            let _ = stdout.execute(cursor::MoveUp(self.rendered_lines));
            let _ = stdout.execute(cursor::MoveToColumn(0));
            let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
            println!("{}", summary);
            let _ = stdout.flush();
        } else {
            println!("{}", summary);
        }
    }

    /// Generate the summary line string.
    pub fn summary_line(&self, model: &str, elapsed_secs: f64) -> String {
        let width = terminal::size().map(|(w, _)| w as usize).unwrap_or(60);
        let sep_char = "\u{2500}"; // ─

        let mut parts = vec![format!("{} \u{00b7} {:.1}s", model, elapsed_secs)];

        if self.tool_count > 0 {
            let tool_word = if self.tool_count == 1 {
                "tool"
            } else {
                "tools"
            };
            parts.push(format!("{} {}", self.tool_count, tool_word));
        }

        if self.iteration_count > 1 {
            let iter_word = if self.iteration_count == 1 {
                "iter"
            } else {
                "iters"
            };
            parts.push(format!("{} {}", self.iteration_count, iter_word));
        }

        let label = parts.join(", ");
        let text_with_spaces = format!(" {} ", label);
        let remaining = width.saturating_sub(text_with_spaces.len() + 6);
        let left = sep_char.repeat(3);
        let right = sep_char.repeat(remaining);

        if colors_enabled() {
            colorize(&format!("{}{}{}", left, text_with_spaces, right), SEPARATOR)
        } else {
            format!("{}{}{}", left, text_with_spaces, right)
        }
    }
}

/// Format milliseconds into a human-readable duration string.
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_renderer() -> ThinkingRenderer {
        ThinkingRenderer::new(false, false) // not verbose, not TTY (for test)
    }

    fn verbose_renderer() -> ThinkingRenderer {
        ThinkingRenderer::new(true, false) // verbose, not TTY
    }

    #[test]
    fn test_new_renderer_has_zero_lines() {
        let r = test_renderer();
        assert_eq!(r.rendered_lines, 0);
    }

    #[test]
    fn test_classification_complete() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 312);
        assert_eq!(r.rendered_lines, 1);
        assert_eq!(r.tool_count, 0);
        assert_eq!(r.iteration_count, 0);
    }

    #[test]
    fn test_context_assembled() {
        let mut r = test_renderer();
        r.on_context_assembled(2400, 8192, 48);
        assert_eq!(r.rendered_lines, 1);
    }

    #[test]
    fn test_execution_started() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        assert_eq!(r.rendered_lines, 1);
    }

    #[test]
    fn test_tool_start_increments_count() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        assert_eq!(r.tool_count, 1);
    }

    #[test]
    fn test_tool_end_updates_state() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 832);
        assert_eq!(r.tool_count, 1);
    }

    #[test]
    fn test_iteration_start() {
        let mut r = test_renderer();
        r.on_execution_started("ReactPlus", 5);
        r.on_iteration_start(2, 5);
        assert_eq!(r.iteration_count, 2);
    }

    #[test]
    fn test_summary_line_format() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 100);
        r.on_context_assembled(2400, 8192, 50);
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 100);
        r.on_tool_start("todo_search");
        r.on_tool_end("todo_search", true, 200);
        r.on_iteration_start(2, 5);

        let summary = r.summary_line("o4-mini", 5.1);
        assert!(summary.contains("o4-mini"));
        assert!(summary.contains("5.1s"));
        assert!(summary.contains("2 tools"));
        assert!(summary.contains("2 iters"));
    }

    #[test]
    fn test_summary_with_single_tool() {
        let mut r = test_renderer();
        r.on_execution_started("Direct", 1);
        r.on_tool_start("echo");
        r.on_tool_end("echo", true, 50);

        let summary = r.summary_line("gpt-4o", 0.3);
        assert!(summary.contains("1 tool"));
        // Should not say "1 tools"
        assert!(!summary.contains("1 tools"));
    }

    #[test]
    fn test_summary_no_tools() {
        let r = test_renderer();
        let summary = r.summary_line("o4-mini", 1.2);
        assert!(summary.contains("o4-mini"));
        assert!(summary.contains("1.2s"));
        // No tools or iters mentioned
        assert!(!summary.contains("tool"));
        assert!(!summary.contains("iter"));
    }

    #[test]
    fn test_verbose_mode_flag() {
        let r = verbose_renderer();
        assert!(r.verbose);
    }
}
