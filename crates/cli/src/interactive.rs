//! Interactive features for the CLI REPL: tab completion, hints, and highlighting.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::{Hint, Hinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

/// Available slash commands for autocomplete.
const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show available commands"),
    ("/paste", "Enter multi-line paste mode"),
    ("/history", "Show recent command history"),
    ("/status", "Show agent status"),
    ("/session", "Show current session ID"),
    ("/clear", "Clear the screen"),
    ("/exit", "Exit the chat"),
    ("/quit", "Exit the chat"),
];

/// Rustyline helper providing tab completion, hints, and highlighting for slash commands.
pub struct SlashCommandHelper {
    commands: Vec<(&'static str, &'static str)>,
}

impl SlashCommandHelper {
    pub fn new() -> Self {
        Self {
            commands: SLASH_COMMANDS.to_vec(),
        }
    }
}

impl Default for SlashCommandHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl Completer for SlashCommandHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Only complete at the beginning of the line for slash commands
        let input = &line[..pos];
        if !input.starts_with('/') {
            return Ok((0, Vec::new()));
        }

        let matches: Vec<Pair> = self
            .commands
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(input))
            .map(|(cmd, desc)| Pair {
                display: format!("{:<12} {}", cmd, desc),
                replacement: cmd.to_string(),
            })
            .collect();

        Ok((0, matches))
    }
}

/// A hint for a slash command.
pub struct CommandHint {
    /// The full text of the hint (portion after what the user typed).
    text: String,
}

impl Hint for CommandHint {
    fn display(&self) -> &str {
        &self.text
    }

    fn completion(&self) -> Option<&str> {
        Some(&self.text)
    }
}

impl Hinter for SlashCommandHelper {
    type Hint = CommandHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<CommandHint> {
        if !line.starts_with('/') || pos == 0 {
            return None;
        }

        let input = &line[..pos];

        // Find the first matching command
        self.commands
            .iter()
            .find(|(cmd, _)| cmd.starts_with(input) && *cmd != input)
            .map(|(cmd, _)| CommandHint {
                text: cmd[pos..].to_string(),
            })
    }
}

impl Highlighter for SlashCommandHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.starts_with('/') {
            // Highlight slash commands in cyan
            Cow::Owned(format!("\x1b[36m{}\x1b[0m", line))
        } else {
            Cow::Borrowed(line)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // Show hints in dim gray
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }

    fn highlight_char(&self, line: &str, _pos: usize, _forced: CmdKind) -> bool {
        // Only re-highlight when the line is a slash command
        line.starts_with('/')
    }
}

impl Validator for SlashCommandHelper {}

impl Helper for SlashCommandHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_command_helper_creation() {
        let helper = SlashCommandHelper::new();
        assert!(!helper.commands.is_empty());
    }

    #[test]
    fn test_commands_all_start_with_slash() {
        for (cmd, _) in SLASH_COMMANDS {
            assert!(cmd.starts_with('/'), "Command {} should start with /", cmd);
        }
    }

    #[test]
    fn test_commands_have_descriptions() {
        for (cmd, desc) in SLASH_COMMANDS {
            assert!(
                !desc.is_empty(),
                "Command {} should have a description",
                cmd
            );
        }
    }
}
