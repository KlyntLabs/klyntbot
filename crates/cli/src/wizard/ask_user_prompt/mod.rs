//! Tabbed multi-question interactive UI for the ask_user tool.
//!
//! Renders an InteractionRequest as a tabbed interface where each question
//! is a separate tab. The user navigates between tabs, answers questions,
//! and reviews all answers before submitting.
//!
//! ## Features
//! - Tab-based navigation with keyboard shortcuts
//! - Single-select and multi-select questions
//! - Yes/No toggle questions
//! - Free-text input questions
//! - Auto-advance to next unanswered question after selection
//! - Submit/Review tab with answer summary
//! - Non-TTY fallback (sequential numbered prompts)
//!
//! ## UI Layout
//! ```text
//! ┌─ Tab Bar ──────────────────────────────────┐
//! │  ○ Auth method  │  ◉ Database  │  ○ Submit  │
//! ├─ Question Pane ────────────────────────────┤
//! │  Which database should we use?             │
//! │    ❯ ● PostgreSQL                          │
//! │        Battle-tested, structured data      │
//! │      ○ SQLite                              │
//! │        Simple, no server needed            │
//! ├─ Hint Bar ─────────────────────────────────┤
//! │  ↑↓/jk navigate  ·  Enter select  ·  Esc   │
//! └────────────────────────────────────────────┘
//! ```

mod box_drawing;
mod fallback;

use std::io::{self, IsTerminal, Write};

use anyhow::{anyhow, Result};
use common::utils::terminal::*;
use common::{Answer, AnswerType, AnswerValue, FormResponse, InteractionRequest};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
};

use box_drawing::{
    write_box_bottom, write_box_empty, write_box_line, write_box_sep, write_box_top,
};
use fallback::prompt_non_interactive;

// ============================================================================
// Terminal Utilities
// ============================================================================

/// RAII guard that disables raw mode on drop.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
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
fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Erases the last `n` rendered rows and positions cursor at column 0.
///
/// Uses MoveUp + ClearFromCursorDown to handle cases where re-renders
/// produce fewer lines than previous renders (no stale content left behind).
fn erase_render(n: usize) -> Result<()> {
    if n == 0 {
        return Ok(());
    }
    let mut out = io::stdout();
    crossterm::execute!(
        out,
        cursor::MoveUp(n as u16),
        cursor::MoveToColumn(0),
        terminal::Clear(ClearType::FromCursorDown)
    )?;
    Ok(())
}

/// Reads a single key event, converting Ctrl+C into cancellation.
fn read_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Err(anyhow!("Cancelled"));
            }
            return Ok(key);
        }
    }
}

// ============================================================================
// Tab State
// ============================================================================

/// Current UI mode.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    /// Answering a specific question tab.
    Answering,
    /// Reviewing all answers on the Submit tab.
    Reviewing,
}

/// State for a single question's answer.
#[derive(Debug, Clone)]
enum QuestionState {
    /// Single-select: current cursor position, selected option index (if any).
    SingleSelect {
        cursor: usize,
        selected: Option<usize>,
    },
    /// Multi-select: current cursor, set of selected indices.
    MultiSelect { cursor: usize, selected: Vec<bool> },
    /// Yes/No: current answer (default if provided, otherwise false).
    YesNo { answer: bool },
    /// Free-text: current input string.
    FreeText { input: String },
}

/// Main state machine for the tabbed form.
struct TabbedFormState {
    /// The interaction request being rendered.
    request: InteractionRequest,
    /// Current active tab index (0..questions.len() for questions, questions.len() for Submit).
    current_tab: usize,
    /// Current UI mode.
    mode: Mode,
    /// Per-question answer state.
    question_states: Vec<QuestionState>,
    /// Terminal width for visual line wrapping calculations.
    terminal_width: u16,
}

impl TabbedFormState {
    fn new(request: InteractionRequest) -> Self {
        let question_states = request
            .questions
            .iter()
            .map(|q| match &q.answer_type {
                AnswerType::SingleSelect { .. } => QuestionState::SingleSelect {
                    cursor: 0,
                    selected: None,
                },
                AnswerType::MultiSelect { options } => QuestionState::MultiSelect {
                    cursor: 0,
                    selected: vec![false; options.len()],
                },
                AnswerType::YesNo { default } => QuestionState::YesNo {
                    answer: default.unwrap_or(false),
                },
                AnswerType::FreeText { .. } => QuestionState::FreeText {
                    input: String::new(),
                },
            })
            .collect();

        let current_tab = 0;
        let mode = if request.questions.len() == 1 {
            // Single question: skip tabs, go straight to answering
            Mode::Answering
        } else {
            Mode::Answering
        };

        Self {
            request,
            current_tab,
            mode,
            question_states,
            terminal_width: terminal::size().map(|(w, _)| w).unwrap_or(80),
        }
    }

    /// Returns the number of question tabs (excluding Submit tab).
    fn num_questions(&self) -> usize {
        self.request.questions.len()
    }

    /// Returns the total number of tabs (questions + Submit tab).
    fn num_tabs(&self) -> usize {
        if self.num_questions() == 1 {
            1 // No Submit tab for single question
        } else {
            self.num_questions() + 1
        }
    }

    /// Returns true if the current tab is the Submit tab.
    fn is_on_submit_tab(&self) -> bool {
        self.num_questions() > 1 && self.current_tab == self.num_questions()
    }

    /// Returns true if all questions have been answered.
    fn all_answered(&self) -> bool {
        self.question_states.iter().all(|state| {
            match state {
                QuestionState::SingleSelect { selected, .. } => selected.is_some(),
                QuestionState::MultiSelect { selected, .. } => selected.iter().any(|&s| s),
                QuestionState::YesNo { .. } => true, // Always has a value
                QuestionState::FreeText { input } => !input.trim().is_empty(),
            }
        })
    }

    /// Collect all answers into a Vec<Answer>.
    fn collect_answers(&self) -> Vec<Answer> {
        self.request
            .questions
            .iter()
            .zip(&self.question_states)
            .filter_map(|(question, state)| {
                let value = match state {
                    QuestionState::SingleSelect { selected, .. } => {
                        let idx = (*selected)?;
                        if let AnswerType::SingleSelect { options } = &question.answer_type {
                            Some(AnswerValue::Selected {
                                value: options[idx].value.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    QuestionState::MultiSelect { selected, .. } => {
                        if let AnswerType::MultiSelect { options } = &question.answer_type {
                            let values: Vec<String> = selected
                                .iter()
                                .enumerate()
                                .filter_map(|(i, &sel)| {
                                    if sel {
                                        Some(options[i].value.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if values.is_empty() {
                                Some(AnswerValue::Skipped)
                            } else {
                                Some(AnswerValue::MultiSelected { values })
                            }
                        } else {
                            None
                        }
                    }
                    QuestionState::YesNo { answer } => Some(AnswerValue::YesNo { answer: *answer }),
                    QuestionState::FreeText { input } => {
                        if input.trim().is_empty() {
                            Some(AnswerValue::Skipped)
                        } else {
                            Some(AnswerValue::Text {
                                content: input.clone(),
                            })
                        }
                    }
                };

                value.map(|v| Answer {
                    question_id: question.id.clone(),
                    value: v,
                })
            })
            .collect()
    }
}

// ============================================================================
// Rendering
// ============================================================================

impl TabbedFormState {
    /// Render the entire UI inside a rounded-corner box.
    ///
    /// Uses `\r\n` line endings for raw-mode compatibility (plain `\n`
    /// doesn't return cursor to column 0 in raw mode).
    fn render(&self) -> Result<usize> {
        let mut out = io::stdout();
        let mut lines = 0;
        let inner_w = (self.terminal_width as usize).saturating_sub(6).max(20);

        // Leading blank line for spacing
        write!(out, "\r\n")?;
        lines += 1;

        // Box title
        let title = if self.request.title.is_empty() {
            colorize("Question", BOLD)
        } else {
            colorize(&self.request.title, BOLD)
        };
        lines += write_box_top(&mut out, &title, inner_w)?;
        lines += write_box_empty(&mut out, inner_w)?;

        // Tab bar (multi-question only)
        if self.num_questions() > 1 {
            let tab_line = self.render_tab_bar();
            lines += write_box_line(&mut out, &tab_line, inner_w)?;
            lines += write_box_empty(&mut out, inner_w)?;
        }

        // Content lines
        let content_lines = if self.is_on_submit_tab() {
            self.submit_content_lines()
        } else {
            self.question_content_lines()
        };
        for line in &content_lines {
            if line.is_empty() {
                lines += write_box_empty(&mut out, inner_w)?;
            } else {
                lines += write_box_line(&mut out, line, inner_w)?;
            }
        }

        lines += write_box_empty(&mut out, inner_w)?;

        // Separator and hint bar
        lines += write_box_sep(&mut out, inner_w)?;
        let hint_line = self.render_hint_bar();
        lines += write_box_line(&mut out, &colorize(&hint_line, DIM), inner_w)?;
        lines += write_box_bottom(&mut out, inner_w)?;

        out.flush()?;
        Ok(lines)
    }

    /// Render the tab bar.
    fn render_tab_bar(&self) -> String {
        let mut tabs = Vec::new();

        // Question tabs
        for (idx, question) in self.request.questions.iter().enumerate() {
            let is_active = self.current_tab == idx && self.mode == Mode::Answering;
            let is_answered = match &self.question_states[idx] {
                QuestionState::SingleSelect { selected, .. } => selected.is_some(),
                QuestionState::MultiSelect { selected, .. } => selected.iter().any(|&s| s),
                QuestionState::YesNo { .. } => true,
                QuestionState::FreeText { input } => !input.trim().is_empty(),
            };

            let indicator = if is_answered {
                colorize("✔", SUCCESS)
            } else if is_active {
                colorize("◉", BRAND)
            } else {
                colorize("○", DIM)
            };

            let title_text = if is_active {
                if is_answered {
                    colorize(&question.title, SUCCESS)
                } else {
                    colorize(&question.title, BRAND)
                }
            } else if is_answered {
                colorize(&question.title, SUCCESS)
            } else {
                colorize(&question.title, DIM)
            };

            tabs.push(format!("{} {}", indicator, title_text));
        }

        // Submit tab
        if self.num_questions() > 1 {
            let is_active = self.is_on_submit_tab();
            let all_answered = self.all_answered();

            let indicator = if all_answered {
                colorize("✓", SUCCESS)
            } else if is_active {
                colorize("⏎", BRAND)
            } else {
                colorize("⏎", DIM)
            };

            let title_text = if is_active {
                if all_answered {
                    colorize("Submit", SUCCESS)
                } else {
                    colorize("Submit", BRAND)
                }
            } else if all_answered {
                colorize("Submit", SUCCESS)
            } else {
                colorize("Submit", DIM)
            };

            tabs.push(format!("{} {}", indicator, title_text));
        }

        let tab_line = tabs.join(&format!("  {}  ", colorize("·", DIM)));

        // Add progress counter for multi-question forms
        if self.num_questions() > 1 {
            let answered = self
                .question_states
                .iter()
                .filter(|s| match s {
                    QuestionState::SingleSelect { selected, .. } => selected.is_some(),
                    QuestionState::MultiSelect { selected, .. } => selected.iter().any(|&s| s),
                    QuestionState::YesNo { .. } => true,
                    QuestionState::FreeText { input } => !input.trim().is_empty(),
                })
                .count();

            format!(
                "{}  {}",
                tab_line,
                colorize(&format!("({}/{})", answered, self.num_questions()), DIM)
            )
        } else {
            tab_line
        }
    }

    /// Build content lines for the current question (rendered inside the box).
    fn question_content_lines(&self) -> Vec<String> {
        let question = &self.request.questions[self.current_tab];
        let state = &self.question_states[self.current_tab];
        let mut lines = Vec::new();

        // Question text
        lines.push(question.text.clone());
        lines.push(String::new());

        match (&question.answer_type, state) {
            (
                AnswerType::SingleSelect { options },
                QuestionState::SingleSelect { cursor, selected },
            ) => {
                for (idx, option) in options.iter().enumerate() {
                    let is_cursor = idx == *cursor;
                    let is_selected = Some(idx) == *selected;

                    let pointer = if is_cursor {
                        colorize("❯", BRAND)
                    } else {
                        " ".to_string()
                    };

                    let radio = if is_selected {
                        colorize("●", BRAND)
                    } else {
                        colorize("○", DIM)
                    };

                    let label = if is_cursor {
                        colorize(&option.label, BOLD)
                    } else {
                        option.label.clone()
                    };

                    lines.push(format!("  {} {} {}", pointer, radio, label));

                    if let Some(desc) = &option.description {
                        lines.push(format!("      {}", colorize(desc, DIM)));
                    }
                }
            }

            (
                AnswerType::MultiSelect { options },
                QuestionState::MultiSelect { cursor, selected },
            ) => {
                for (idx, option) in options.iter().enumerate() {
                    let is_cursor = idx == *cursor;
                    let is_selected = selected[idx];

                    let pointer = if is_cursor {
                        colorize("❯", BRAND)
                    } else {
                        " ".to_string()
                    };

                    let checkbox = if is_selected {
                        colorize("[●]", SUCCESS)
                    } else {
                        colorize("[ ]", DIM)
                    };

                    let label = if is_cursor {
                        colorize(&option.label, BOLD)
                    } else {
                        option.label.clone()
                    };

                    lines.push(format!("  {} {} {}", pointer, checkbox, label));

                    if let Some(desc) = &option.description {
                        lines.push(format!("      {}", colorize(desc, DIM)));
                    }
                }
            }

            (AnswerType::YesNo { .. }, QuestionState::YesNo { answer }) => {
                let yes_indicator = if *answer {
                    colorize("●", BRAND)
                } else {
                    colorize("○", DIM)
                };
                let no_indicator = if !*answer {
                    colorize("●", BRAND)
                } else {
                    colorize("○", DIM)
                };

                lines.push(format!("  {} Yes", yes_indicator));
                lines.push(format!("  {} No", no_indicator));
            }

            (AnswerType::FreeText { placeholder }, QuestionState::FreeText { input }) => {
                let prompt = colorize("▸", BRAND);
                let caret = colorize("▏", BRAND);
                if input.is_empty() {
                    let hint = placeholder.as_deref().unwrap_or("Type your answer...");
                    lines.push(format!("  {} {}{}", prompt, colorize(hint, DIM), caret));
                } else {
                    lines.push(format!("  {} {}{}", prompt, input, caret));
                }
            }

            _ => unreachable!("Mismatched question type and state"),
        }

        lines
    }

    /// Build content lines for the Submit/Review pane (rendered inside the box).
    fn submit_content_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push("Review your answers:".to_string());
        lines.push(String::new());

        for (question, qstate) in self.request.questions.iter().zip(&self.question_states) {
            let answer_text = match qstate {
                QuestionState::SingleSelect { selected, .. } => {
                    if let Some(idx) = selected {
                        if let AnswerType::SingleSelect { options } = &question.answer_type {
                            options[*idx].label.clone()
                        } else {
                            colorize("(not answered)", WARNING)
                        }
                    } else {
                        colorize("(not answered)", WARNING)
                    }
                }
                QuestionState::MultiSelect { selected, .. } => {
                    if let AnswerType::MultiSelect { options } = &question.answer_type {
                        let labels: Vec<&str> = selected
                            .iter()
                            .enumerate()
                            .filter_map(|(i, &sel)| {
                                if sel {
                                    options.get(i).map(|o| o.label.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if labels.is_empty() {
                            colorize("(none selected)", WARNING)
                        } else {
                            labels.join(", ")
                        }
                    } else {
                        colorize("(not answered)", WARNING)
                    }
                }
                QuestionState::YesNo { answer } => {
                    if *answer {
                        "Yes".to_string()
                    } else {
                        "No".to_string()
                    }
                }
                QuestionState::FreeText { input } => {
                    if input.trim().is_empty() {
                        colorize("(not answered)", WARNING)
                    } else {
                        format!("\"{}\"", input)
                    }
                }
            };

            lines.push(format!(
                "  {}  {}  {}",
                colorize(&question.title, DIM),
                colorize("→", BRAND),
                colorize(&answer_text, BOLD)
            ));
        }

        lines.push(String::new());

        if self.all_answered() {
            lines.push(colorize(
                "Press Enter to submit, or navigate to a tab to change an answer.",
                DIM,
            ));
        } else {
            lines.push(colorize(
                "Answer all questions before submitting. Navigate to unanswered tabs.",
                WARNING,
            ));
        }

        lines
    }

    /// Render the hint bar.
    fn render_hint_bar(&self) -> String {
        let multi = self.num_questions() > 1;
        let switch = if multi { "  ·  ←→ switch" } else { "" };

        if self.is_on_submit_tab() {
            return format!("enter submit{switch}  ·  esc cancel");
        }

        match &self.request.questions[self.current_tab].answer_type {
            AnswerType::SingleSelect { .. } => {
                format!("↑↓ navigate  ·  enter select{switch}  ·  esc cancel")
            }
            AnswerType::MultiSelect { .. } => {
                format!("↑↓ navigate  ·  space toggle  ·  enter confirm{switch}  ·  esc cancel")
            }
            AnswerType::YesNo { .. } => {
                format!("↑↓ toggle  ·  enter confirm{switch}  ·  esc cancel")
            }
            AnswerType::FreeText { .. } => {
                format!("type answer  ·  enter confirm{switch}  ·  esc cancel")
            }
        }
    }
}

// ============================================================================
// Keyboard Handling
// ============================================================================

/// Action to take after processing a key event.
enum Action {
    /// Continue the event loop.
    Continue,
    /// Submit the collected answers.
    Submit,
    /// Cancel the entire interaction.
    Cancel,
}

impl TabbedFormState {
    /// Handle a single key event and return the action to take.
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        let is_free_text = !self.is_on_submit_tab()
            && matches!(
                &self.question_states[self.current_tab],
                QuestionState::FreeText { .. }
            );

        match key.code {
            // Cancel on Esc
            KeyCode::Esc => return Action::Cancel,

            // Tab navigation: ←→ and Tab/Shift+Tab (works in ALL question types)
            KeyCode::Right | KeyCode::Tab if self.num_questions() > 1 => {
                self.navigate_next_tab();
            }
            KeyCode::Left
                if self.num_questions() > 1 && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.navigate_prev_tab();
            }
            KeyCode::BackTab if self.num_questions() > 1 => {
                self.navigate_prev_tab();
            }

            // Enter key - context-specific
            KeyCode::Enter => {
                if self.is_on_submit_tab() {
                    if self.all_answered() {
                        return Action::Submit;
                    }
                    // else: no-op (incomplete answers)
                } else {
                    // Answer the current question and auto-advance
                    self.confirm_current_question();

                    // Single question: submit immediately after confirming
                    if self.num_questions() == 1 && self.all_answered() {
                        return Action::Submit;
                    }

                    self.auto_advance();
                }
            }

            // Free-text input — must come BEFORE j/k/space handlers
            KeyCode::Char(c) if is_free_text => {
                if let QuestionState::FreeText { input } =
                    &mut self.question_states[self.current_tab]
                {
                    input.push(c);
                }
            }
            KeyCode::Backspace if is_free_text => {
                if let QuestionState::FreeText { input } =
                    &mut self.question_states[self.current_tab]
                {
                    input.pop();
                }
            }

            // Option navigation (only reached for non-free-text questions)
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_cursor_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_cursor_down();
            }

            // Space for multi-select toggle or yes/no toggle
            KeyCode::Char(' ') => {
                self.toggle_current_option();
            }

            _ => {}
        }

        Action::Continue
    }

    /// Navigate to the next tab (wraps around).
    fn navigate_next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % self.num_tabs();
        self.mode = if self.is_on_submit_tab() {
            Mode::Reviewing
        } else {
            Mode::Answering
        };
    }

    /// Navigate to the previous tab (wraps around).
    fn navigate_prev_tab(&mut self) {
        if self.current_tab == 0 {
            self.current_tab = self.num_tabs() - 1;
        } else {
            self.current_tab -= 1;
        }
        self.mode = if self.is_on_submit_tab() {
            Mode::Reviewing
        } else {
            Mode::Answering
        };
    }

    /// Move cursor up within the current question's options.
    fn move_cursor_up(&mut self) {
        if self.is_on_submit_tab() {
            return;
        }

        match &mut self.question_states[self.current_tab] {
            QuestionState::SingleSelect { cursor, .. } => {
                if let AnswerType::SingleSelect { options } =
                    &self.request.questions[self.current_tab].answer_type
                {
                    if *cursor > 0 {
                        *cursor -= 1;
                    } else {
                        *cursor = options.len() - 1;
                    }
                }
            }
            QuestionState::MultiSelect { cursor, .. } => {
                if let AnswerType::MultiSelect { options } =
                    &self.request.questions[self.current_tab].answer_type
                {
                    if *cursor > 0 {
                        *cursor -= 1;
                    } else {
                        *cursor = options.len() - 1;
                    }
                }
            }
            QuestionState::YesNo { answer } => {
                *answer = !*answer;
            }
            _ => {}
        }
    }

    /// Move cursor down within the current question's options.
    fn move_cursor_down(&mut self) {
        if self.is_on_submit_tab() {
            return;
        }

        match &mut self.question_states[self.current_tab] {
            QuestionState::SingleSelect { cursor, .. } => {
                if let AnswerType::SingleSelect { options } =
                    &self.request.questions[self.current_tab].answer_type
                {
                    *cursor = (*cursor + 1) % options.len();
                }
            }
            QuestionState::MultiSelect { cursor, .. } => {
                if let AnswerType::MultiSelect { options } =
                    &self.request.questions[self.current_tab].answer_type
                {
                    *cursor = (*cursor + 1) % options.len();
                }
            }
            QuestionState::YesNo { answer } => {
                *answer = !*answer;
            }
            _ => {}
        }
    }

    /// Toggle the current option (multi-select or yes/no).
    fn toggle_current_option(&mut self) {
        if self.is_on_submit_tab() {
            return;
        }

        match &mut self.question_states[self.current_tab] {
            QuestionState::MultiSelect { cursor, selected } => {
                selected[*cursor] = !selected[*cursor];
            }
            QuestionState::YesNo { answer } => {
                *answer = !*answer;
            }
            _ => {}
        }
    }

    /// Confirm the current question's answer (single-select: select cursor position).
    fn confirm_current_question(&mut self) {
        if self.is_on_submit_tab() {
            return;
        }

        if let QuestionState::SingleSelect { cursor, selected } =
            &mut self.question_states[self.current_tab]
        {
            *selected = Some(*cursor);
        }
        // Multi-select and yes/no are already "confirmed" by their state
    }

    /// Auto-advance to the next unanswered question or Submit tab.
    fn auto_advance(&mut self) {
        if self.num_questions() == 1 {
            // Single question: submit immediately
            return;
        }

        // Find next unanswered question
        let mut next_tab = self.current_tab + 1;
        for _ in 0..self.num_questions() {
            if next_tab >= self.num_questions() {
                break; // Go to Submit tab
            }

            let is_answered = match &self.question_states[next_tab] {
                QuestionState::SingleSelect { selected, .. } => selected.is_some(),
                QuestionState::MultiSelect { selected, .. } => selected.iter().any(|&s| s),
                QuestionState::YesNo { .. } => true,
                QuestionState::FreeText { input } => !input.trim().is_empty(),
            };

            if !is_answered {
                self.current_tab = next_tab;
                self.mode = Mode::Answering;
                return;
            }

            next_tab += 1;
        }

        // All answered or reached end: go to Submit tab
        self.current_tab = self.num_questions();
        self.mode = Mode::Reviewing;
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Main entry point: prompt the user with a tabbed multi-question UI.
///
/// Returns FormResponse::Completed with collected answers, or FormResponse::Cancelled.
/// Result of a multi-question prompt, including how many lines the summary occupied.
pub struct PromptResult {
    pub response: FormResponse,
    /// Number of terminal lines the summary/cancellation output occupied.
    pub summary_lines: u16,
}

pub fn prompt_multi_question(request: &InteractionRequest) -> Result<PromptResult> {
    if !is_interactive() {
        let response = prompt_non_interactive(request)?;
        return Ok(PromptResult {
            response,
            summary_lines: 0,
        });
    }

    // Validate request
    if request.questions.is_empty() {
        return Ok(PromptResult {
            response: FormResponse::Completed(Vec::new()),
            summary_lines: 0,
        });
    }

    let mut state = TabbedFormState::new(request.clone());
    let _guard = RawModeGuard::enable()?;

    let mut rendered_lines = 0;

    loop {
        // Erase previous render
        if rendered_lines > 0 {
            erase_render(rendered_lines)?;
        }

        // Render current state
        rendered_lines = state.render()?;

        // Read key and handle
        let key = read_key()?;
        match state.handle_key(key) {
            Action::Continue => continue,
            Action::Submit => {
                // Erase UI and show rich summary
                erase_render(rendered_lines)?;
                let summary_lines = print_submission_summary(&state)? as u16;
                return Ok(PromptResult {
                    response: FormResponse::Completed(state.collect_answers()),
                    summary_lines,
                });
            }
            Action::Cancel => {
                // Erase UI and show cancellation (with progress if any)
                erase_render(rendered_lines)?;
                let summary_lines = print_cancellation_summary(&state)? as u16;
                return Ok(PromptResult {
                    response: FormResponse::Cancelled,
                    summary_lines,
                });
            }
        }
    }
}

/// Print a rich multi-line boxed summary after submission.
///
/// Shows each answer on its own line with the question title, question text,
/// and answer value with descriptions. Provides a clear visual record of
/// what the user submitted.
fn print_submission_summary(state: &TabbedFormState) -> Result<usize> {
    let mut out = io::stdout();
    let inner_w = (state.terminal_width as usize).saturating_sub(6).max(20);
    let mut lines = 0;

    let title = format!("{} {}", colorize("✔", SUCCESS), colorize("Submitted", BOLD));
    lines += write_box_top(&mut out, &title, inner_w)?;
    lines += write_box_empty(&mut out, inner_w)?;

    for (question, qstate) in state.request.questions.iter().zip(&state.question_states) {
        // Question title (bold)
        lines += write_box_line(&mut out, &colorize(&question.title, BOLD), inner_w)?;

        // Question text (dim)
        lines += write_box_line(&mut out, &colorize(&question.text, DIM), inner_w)?;

        // Answer line with arrow and value
        let answer_line = format_answer_display(question, qstate);
        lines += write_box_line(&mut out, &answer_line, inner_w)?;

        // Spacing between questions
        lines += write_box_empty(&mut out, inner_w)?;
    }

    lines += write_box_bottom(&mut out, inner_w)?;

    out.flush()?;
    Ok(lines)
}

/// Format a single answer for display in the submission summary.
fn format_answer_display(question: &common::Question, qstate: &QuestionState) -> String {
    match qstate {
        QuestionState::SingleSelect { selected, .. } => {
            if let Some(idx) = selected {
                if let AnswerType::SingleSelect { options } = &question.answer_type {
                    let opt = &options[*idx];
                    let mut line =
                        format!("{} {}", colorize("→", BRAND), colorize(&opt.label, SUCCESS));
                    if let Some(desc) = &opt.description {
                        line.push_str(&format!(
                            "  {}  {}",
                            colorize("·", DIM),
                            colorize(desc, DIM)
                        ));
                    }
                    line
                } else {
                    format!("{} {}", colorize("→", BRAND), colorize("(error)", ERROR))
                }
            } else {
                format!(
                    "{} {}",
                    colorize("→", BRAND),
                    colorize("(not answered)", WARNING)
                )
            }
        }
        QuestionState::MultiSelect { selected, .. } => {
            if let AnswerType::MultiSelect { options } = &question.answer_type {
                let labels: Vec<&str> = selected
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &sel)| {
                        if sel {
                            options.get(i).map(|o| o.label.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                if labels.is_empty() {
                    format!(
                        "{} {}",
                        colorize("→", BRAND),
                        colorize("(none selected)", WARNING)
                    )
                } else {
                    format!(
                        "{} {}",
                        colorize("→", BRAND),
                        colorize(&labels.join(", "), SUCCESS)
                    )
                }
            } else {
                format!("{} {}", colorize("→", BRAND), colorize("(error)", ERROR))
            }
        }
        QuestionState::YesNo { answer } => {
            format!(
                "{} {}",
                colorize("→", BRAND),
                colorize(if *answer { "Yes" } else { "No" }, SUCCESS)
            )
        }
        QuestionState::FreeText { input } => {
            if input.trim().is_empty() {
                format!(
                    "{} {}",
                    colorize("→", BRAND),
                    colorize("(not answered)", WARNING)
                )
            } else {
                format!(
                    "{} {}",
                    colorize("→", BRAND),
                    colorize(&format!("\"{}\"", input), SUCCESS)
                )
            }
        }
    }
}

/// Print a boxed cancellation summary showing progress before cancellation.
fn print_cancellation_summary(state: &TabbedFormState) -> Result<usize> {
    let mut out = io::stdout();
    let inner_w = (state.terminal_width as usize).saturating_sub(6).max(20);
    let mut lines = 0;

    // Check if any questions were answered
    let any_answered = state.question_states.iter().any(|s| match s {
        QuestionState::SingleSelect { selected, .. } => selected.is_some(),
        QuestionState::MultiSelect { selected, .. } => selected.iter().any(|&s| s),
        QuestionState::FreeText { input } => !input.trim().is_empty(),
        QuestionState::YesNo { .. } => false, // Has a default, don't count
    });

    if !any_answered {
        // Simple cancellation — no progress to show
        write!(out, "\r  {}\r\n", colorize("✗ Cancelled", ERROR))?;
        out.flush()?;
        return Ok(1);
    }

    // Rich cancellation with progress
    let title = format!("{} {}", colorize("✗", ERROR), colorize("Cancelled", BOLD));
    lines += write_box_top(&mut out, &title, inner_w)?;
    lines += write_box_empty(&mut out, inner_w)?;

    for (question, qstate) in state.request.questions.iter().zip(&state.question_states) {
        let status_line = match qstate {
            QuestionState::SingleSelect { selected, .. } => {
                if let Some(idx) = selected {
                    if let AnswerType::SingleSelect { options } = &question.answer_type {
                        format!(
                            "{}  {}  {}  {}",
                            colorize("✔", SUCCESS),
                            colorize(&question.title, SUCCESS),
                            colorize("→", BRAND),
                            options[*idx].label
                        )
                    } else {
                        format!("{}  {}", colorize("○", DIM), colorize(&question.title, DIM))
                    }
                } else {
                    format!("{}  {}", colorize("○", DIM), colorize(&question.title, DIM))
                }
            }
            QuestionState::MultiSelect { selected, .. } => {
                if selected.iter().any(|&s| s) {
                    if let AnswerType::MultiSelect { options } = &question.answer_type {
                        let labels: Vec<&str> = selected
                            .iter()
                            .enumerate()
                            .filter_map(|(i, &sel)| {
                                if sel {
                                    options.get(i).map(|o| o.label.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        format!(
                            "{}  {}  {}  {}",
                            colorize("✔", SUCCESS),
                            colorize(&question.title, SUCCESS),
                            colorize("→", BRAND),
                            labels.join(", ")
                        )
                    } else {
                        format!("{}  {}", colorize("○", DIM), colorize(&question.title, DIM))
                    }
                } else {
                    format!("{}  {}", colorize("○", DIM), colorize(&question.title, DIM))
                }
            }
            QuestionState::FreeText { input } => {
                if !input.trim().is_empty() {
                    format!(
                        "{}  {}  {}  \"{}\"",
                        colorize("✔", SUCCESS),
                        colorize(&question.title, SUCCESS),
                        colorize("→", BRAND),
                        input
                    )
                } else {
                    format!("{}  {}", colorize("○", DIM), colorize(&question.title, DIM))
                }
            }
            QuestionState::YesNo { answer } => {
                format!(
                    "{}  {}  {}  {}",
                    colorize("✔", DIM),
                    colorize(&question.title, DIM),
                    colorize("→", BRAND),
                    if *answer { "Yes" } else { "No" }
                )
            }
        };

        lines += write_box_line(&mut out, &status_line, inner_w)?;
    }

    lines += write_box_empty(&mut out, inner_w)?;
    lines += write_box_bottom(&mut out, inner_w)?;

    out.flush()?;
    Ok(lines)
}
