//! Thinking phase renderer — shows pipeline stages and tool calls during AI processing.
//!
//! **Normal mode (TTY):** Single animated line that updates in-place as each
//! stage progresses: `⣾ Classifying...` → `⣾ Executing...` → `⣾ ask_user` → …
//!
//! **Verbose mode (TTY):** Multi-line trace with checkmarks, durations, and
//! spinning indicators on active stages.
//!
//! **Non-TTY:** Static lines for each stage (no animation).
//!
//! Animation is driven by the caller invoking [`ThinkingRenderer::tick`] at a
//! regular interval (e.g. every 80 ms from a `tokio::time::Interval` inside the
//! event loop). This avoids background threads and blocking sleeps.

use std::io::{self, Write};

use crossterm::{
    cursor,
    terminal::{self, ClearType},
    ExecutableCommand,
};

use super::colors::{colorize, BRAND, DIM, TOOL};
use super::status_success;

/// Braille spinner patterns (8 frames)
const SPINNER_FRAMES: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// Renders pipeline thinking trace during agent execution.
pub struct ThinkingRenderer {
    pub verbose: bool,
    is_tty: bool,
    /// Lines printed via `println!` (used by verbose mode for collapse).
    pub rendered_lines: u16,
    pub tool_count: usize,
    pub iteration_count: usize,
    max_iterations: usize,
    /// Current spinner label. `Some(msg)` = actively spinning, `None` = idle.
    active_spinner: Option<String>,
    /// Current frame index for the braille animation.
    spinner_frame: usize,
    /// Number of leading spaces before the spinner character.
    spinner_indent: usize,
    /// ANSI color code for the spinner message text.
    spinner_color: &'static str,
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
            active_spinner: None,
            spinner_frame: 0,
            spinner_indent: 2,
            spinner_color: DIM,
        }
    }

    // ── Spinner control ────────────────────────────────────────────────

    /// Set the spinner message with default (2-space) indent and DIM color.
    pub fn set_spinner(&mut self, message: impl Into<String>) {
        self.set_spinner_styled(message, 2, DIM);
    }

    /// Set the spinner for a tool name (cyan, 4-space indent in verbose).
    fn set_tool_spinner(&mut self, message: impl Into<String>) {
        self.set_spinner_styled(message, 4, TOOL);
    }

    /// Set the spinner for a phase with a specific color.
    fn set_phase_spinner(&mut self, message: impl Into<String>, color: &'static str) {
        self.set_spinner_styled(message, 2, color);
    }

    fn set_spinner_styled(
        &mut self,
        message: impl Into<String>,
        indent: usize,
        color: &'static str,
    ) {
        if self.is_tty {
            self.active_spinner = Some(message.into());
            self.spinner_indent = indent;
            self.spinner_color = color;
            self.spinner_frame = 0;
            self.render_spinner_frame();
        }
    }

    /// Stop the spinner and clear the line it was occupying.
    fn stop_spinner(&mut self) {
        if !self.is_tty || self.active_spinner.is_none() {
            return;
        }
        self.active_spinner = None;
        self.spinner_frame = 0;
        print!("\r\x1b[K");
        let _ = io::stdout().flush();
    }

    /// Advance the spinner by one frame. No-op when idle or non-TTY.
    pub fn tick(&mut self) {
        if !self.is_tty || self.active_spinner.is_none() {
            return;
        }
        self.render_spinner_frame();
    }

    /// Render the current spinner frame to the terminal.
    fn render_spinner_frame(&mut self) {
        if let Some(ref message) = self.active_spinner {
            let ch = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
            let indent = " ".repeat(self.spinner_indent);
            print!(
                "\r{}{} {}  \x1b[K",
                indent,
                colorize(ch, self.spinner_color),
                colorize(message, self.spinner_color),
            );
            let _ = io::stdout().flush();
            self.spinner_frame += 1;
        }
    }

    /// Returns `true` when a spinner is active.
    pub fn is_spinning(&self) -> bool {
        self.active_spinner.is_some()
    }

    // ── Event handlers ─────────────────────────────────────────────────

    /// Handle classification stage completing.
    pub fn on_classification_complete(
        &mut self,
        strategy: &str,
        confidence: f32,
        source: &str,
        duration_ms: u64,
    ) {
        if !self.is_tty {
            // Non-TTY: static lines
            println!(
                "  {} Classified \u{2192} {}  {}",
                status_success(),
                strategy,
                format_duration(duration_ms),
            );
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
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
            let _ = io::stdout().flush();
            self.set_phase_spinner("Assembling context...", DIM);
        } else {
            self.set_phase_spinner("Assembling context...", DIM);
        }
    }

    /// Handle context assembly completing.
    pub fn on_context_assembled(&mut self, total_tokens: usize, budget: usize, duration_ms: u64) {
        if !self.is_tty {
            println!(
                "  {} Context assembled  {}",
                status_success(),
                format_duration(duration_ms),
            );
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
            let line = format!(
                "  {} Context: {}/{} tokens  {}",
                status_success(),
                colorize(&total_tokens.to_string(), TOOL),
                budget,
                colorize(&format_duration(duration_ms), DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
            let _ = io::stdout().flush();
            self.set_phase_spinner("Starting execution...", DIM);
        } else {
            self.set_phase_spinner("Executing...", BRAND);
        }
    }

    /// Handle execution engine starting.
    pub fn on_execution_started(&mut self, engine: &str, max_iterations: usize) {
        self.max_iterations = max_iterations;
        self.iteration_count = 1;

        if !self.is_tty {
            println!("  \u{25b8} Executing ({})", engine);
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
            let line = format!(
                "  {} Executing ({})",
                colorize("\u{25b8}", TOOL),
                colorize(engine, DIM),
            );
            println!("{}", line);
            self.rendered_lines += 1;
            let _ = io::stdout().flush();
            self.set_phase_spinner("Thinking...", DIM);
        } else {
            self.set_phase_spinner("Executing...", BRAND);
        }
    }

    /// Handle a new iteration starting.
    pub fn on_iteration_start(&mut self, iteration: usize, max: usize) {
        self.iteration_count = iteration;
        self.max_iterations = max;

        if !self.is_tty {
            println!("  \u{25b8} Executing (iteration {}/{})", iteration, max);
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
            let line = format!(
                "  {} Executing (iteration {}/{})",
                colorize("\u{25b8}", TOOL),
                iteration,
                max,
            );
            println!("{}", line);
            self.rendered_lines += 1;
            let _ = io::stdout().flush();
            self.set_phase_spinner("Thinking...", DIM);
        } else if iteration <= 1 {
            // First iteration: show "Executing..."
            self.set_phase_spinner("Executing...", BRAND);
        } else {
            // Subsequent iterations: LLM is generating the response
            self.set_phase_spinner("Responding...", DIM);
        }
    }

    /// Handle a tool execution starting.
    pub fn on_tool_start(&mut self, name: &str) {
        self.tool_count += 1;

        if !self.is_tty {
            println!("    \u{27f3} {}", name);
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
            self.set_tool_spinner(name.to_string());
            self.rendered_lines += 1;
        } else {
            self.set_phase_spinner(name.to_string(), TOOL);
        }
    }

    /// Handle a tool execution completing.
    pub fn on_tool_end(&mut self, name: &str, success: bool, duration_ms: u64) {
        if !self.is_tty {
            let indicator = if success { "\u{2713}" } else { "\u{2717}" };
            println!("{} {} {}", indicator, name, format_duration(duration_ms));
            self.rendered_lines += 1;
            return;
        }

        if self.verbose {
            self.stop_spinner();
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
            let _ = io::stdout().flush();
            // rendered_lines was already incremented in on_tool_start
        }
        // Normal TTY: keep the tool name visible in the spinner.
        // The next phase change (iteration_start, content_chunk, etc.)
        // will update the spinner naturally.
    }

    // ── Collapse / summary ─────────────────────────────────────────────

    /// Collapse (erase) the thinking trace.
    ///
    /// In normal TTY mode the spinner line is cleared by `stop_spinner()`.
    /// In verbose TTY mode, crossterm erases all rendered lines.
    /// In non-TTY mode this is a no-op.
    pub fn collapse(&mut self) {
        self.stop_spinner();

        if self.is_tty && self.rendered_lines > 0 {
            let mut stdout = io::stdout();
            let _ = stdout.execute(cursor::MoveUp(self.rendered_lines));
            let _ = stdout.execute(cursor::MoveToColumn(0));
            let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
            let _ = stdout.flush();
        }
    }

    /// Build a label string with model, elapsed time, tool count and iterations.
    ///
    /// Suitable for passing to `StreamRenderer::draw_separator()`.
    pub fn separator_label(&self, model: &str, elapsed_secs: f64) -> String {
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

        parts.join(", ")
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
    fn test_classification_complete_non_tty() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 312);
        assert_eq!(r.rendered_lines, 1);
        assert_eq!(r.tool_count, 0);
        assert_eq!(r.iteration_count, 0);
    }

    #[test]
    fn test_normal_tty_no_rendered_lines() {
        // Normal TTY mode: spinner only, no println → rendered_lines stays 0
        let mut r = ThinkingRenderer::new(false, true);
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 100);
        r.on_context_assembled(2400, 8192, 50);
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 100);
        assert_eq!(r.rendered_lines, 0);
        assert_eq!(r.tool_count, 1);
    }

    #[test]
    fn test_verbose_tty_has_rendered_lines() {
        // Verbose TTY mode: multi-line → rendered_lines incremented
        let mut r = ThinkingRenderer::new(true, true);
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 100);
        // Verbose prints 2 lines (classification + method detail)
        assert_eq!(r.rendered_lines, 2);
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
    fn test_separator_label_format() {
        let mut r = test_renderer();
        r.on_classification_complete("ToolAssisted", 0.85, "heuristic", 100);
        r.on_context_assembled(2400, 8192, 50);
        r.on_execution_started("ReactPlus", 5);
        r.on_tool_start("todo_add");
        r.on_tool_end("todo_add", true, 100);
        r.on_tool_start("todo_search");
        r.on_tool_end("todo_search", true, 200);
        r.on_iteration_start(2, 5);

        let label = r.separator_label("o4-mini", 5.1);
        assert!(label.contains("o4-mini"));
        assert!(label.contains("5.1s"));
        assert!(label.contains("2 tools"));
        assert!(label.contains("2 iters"));
    }

    #[test]
    fn test_separator_with_single_tool() {
        let mut r = test_renderer();
        r.on_execution_started("Direct", 1);
        r.on_tool_start("echo");
        r.on_tool_end("echo", true, 50);

        let label = r.separator_label("gpt-4o", 0.3);
        assert!(label.contains("1 tool"));
        assert!(!label.contains("1 tools"));
    }

    #[test]
    fn test_separator_no_tools() {
        let r = test_renderer();
        let label = r.separator_label("o4-mini", 1.2);
        assert!(label.contains("o4-mini"));
        assert!(label.contains("1.2s"));
        assert!(!label.contains("tool"));
        assert!(!label.contains("iter"));
    }

    #[test]
    fn test_verbose_mode_flag() {
        let r = verbose_renderer();
        assert!(r.verbose);
    }

    #[test]
    fn test_spinner_state() {
        // Non-TTY: set_spinner is a no-op
        let mut r = test_renderer();
        r.set_spinner("test");
        assert!(!r.is_spinning());

        // TTY mode
        let mut r = ThinkingRenderer::new(false, true);
        assert!(!r.is_spinning());
        r.set_spinner("working...");
        assert!(r.is_spinning());
        r.stop_spinner();
        assert!(!r.is_spinning());
    }
}
