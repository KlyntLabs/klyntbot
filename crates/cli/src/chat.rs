//! Chat command handler for interactive CLI mode

use anyhow::Result;
use common::utils::terminal::*;
use agent::AgentLoop;
use bus::MessageBus;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::sync::Arc;

/// Handle chat command
pub async fn handle_chat(
    message: Option<String>,
    session: String,
    _render_markdown: bool, // Markdown always rendered for best UX
) -> Result<()> {
    // Load config
    let config = config::load()?;
    let model = config.agents.defaults.model.clone();

    // Clean startup header
    println!(
        "\n  {} {}",
        colorize("klyntbot", BOLD),
        colorize(&format!("· {}", model), DIM)
    );

    // Initialize LLM provider
    let provider = providers::create_provider(&config)?;

    // Create a minimal message bus (not used in CLI mode, but required for AgentLoop)
    let bus = Arc::new(MessageBus::new(10));

    // Initialize agent loop
    let agent_loop = Arc::new(AgentLoop::new(bus, provider, config).await?);

    // Session key for CLI
    let session_key = format!("cli:{}", session);

    // Handle single message or interactive mode
    if let Some(msg) = message {
        // Single message mode
        println!("\n{} {}", colorize("You:", PROMPT), msg);

        let mut spinner = Spinner::new("thinking");
        spinner.start();

        match agent_loop.process_direct(msg, session_key).await {
            Ok(response) => {
                spinner.stop();
                println!("\n{}\n", MarkdownRenderer::render(&response));
            }
            Err(e) => {
                spinner.stop();
                eprintln!("\n{} {}", status_error(), e);
                return Err(e.into());
            }
        }
    } else {
        // Interactive REPL mode with rustyline
        let history_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".klyntbot")
            .join("history.txt");

        // Ensure the .klyntbot directory exists
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut editor = DefaultEditor::new()?;
        let _ = editor.load_history(&history_path);

        println!(
            "\n{}",
            colorize("Interactive chat mode. Type /help for commands.\n", DIM)
        );

        loop {
            let prompt = format!("{} ", colorize("klyntbot>", PROMPT));
            let readline = editor.readline(&prompt);

            match readline {
                Ok(line) => {
                    let trimmed = line.trim();

                    // Skip empty lines
                    if trimmed.is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = editor.add_history_entry(trimmed);

                    // Handle slash commands
                    if let Some(cmd) = trimmed.strip_prefix('/') {
                        match cmd.to_lowercase().as_str() {
                            "exit" | "quit" => {
                                println!("{}", colorize("Goodbye!", DIM));
                                break;
                            }
                            "clear" => {
                                // Clear screen
                                print!("\x1b[2J\x1b[H");
                                io::stdout().flush()?;
                                continue;
                            }
                            "session" => {
                                println!("\n{} {}", status_success(), session_key);
                                println!();
                                continue;
                            }
                            "help" => {
                                print_help();
                                continue;
                            }
                            "status" => {
                                print_status(&agent_loop).await;
                                continue;
                            }
                            "history" => {
                                print_history(&editor);
                                continue;
                            }
                            "paste" => {
                                // Enter multi-line paste mode
                                println!("\n{}", colorize("[paste mode: type /end or press Ctrl+D to submit, empty line to cancel]", DIM));
                                let mut lines = Vec::new();

                                loop {
                                    let paste_prompt = colorize("... ", DIM);
                                    match editor.readline(&paste_prompt) {
                                        Ok(paste_line) => {
                                            let paste_trimmed = paste_line.trim();

                                            // Check for terminators
                                            if paste_trimmed == "/end" {
                                                break;
                                            }

                                            // Empty line cancels paste mode
                                            if paste_trimmed.is_empty() && lines.is_empty() {
                                                println!(
                                                    "{}",
                                                    colorize("Paste mode cancelled", DIM)
                                                );
                                                lines.clear();
                                                break;
                                            }

                                            lines.push(paste_line);
                                        }
                                        Err(ReadlineError::Eof) => {
                                            // Ctrl+D submits
                                            break;
                                        }
                                        Err(ReadlineError::Interrupted) => {
                                            // Ctrl+C cancels
                                            println!("\n{}", colorize("Paste mode cancelled", DIM));
                                            lines.clear();
                                            break;
                                        }
                                        Err(_) => {
                                            lines.clear();
                                            break;
                                        }
                                    }
                                }

                                if lines.is_empty() {
                                    continue;
                                }

                                // Concatenate all lines
                                let message = lines.join("\n");

                                // Add to history
                                let _ = editor.add_history_entry(&message);

                                // Process the multi-line message
                                let mut spinner = Spinner::new("thinking");
                                spinner.start();

                                match agent_loop
                                    .process_direct(message, session_key.clone())
                                    .await
                                {
                                    Ok(response) => {
                                        spinner.stop();
                                        println!("\n{}\n", MarkdownRenderer::render(&response));
                                    }
                                    Err(e) => {
                                        spinner.stop();
                                        eprintln!("\n{} {}\n", status_error(), e);
                                    }
                                }

                                continue;
                            }
                            _ => {
                                println!("{} Unknown command: /{}", status_error(), cmd);
                                println!("{}", colorize("Type /help for available commands", DIM));
                                println!();
                                continue;
                            }
                        }
                    }

                    // Check for simple exit commands
                    if trimmed.eq_ignore_ascii_case("exit")
                        || trimmed.eq_ignore_ascii_case("quit")
                        || trimmed == ":q"
                    {
                        println!("{}", colorize("Goodbye!", DIM));
                        break;
                    }

                    // Process the message with spinner + markdown rendering
                    let mut spinner = Spinner::new("thinking");
                    spinner.start();

                    match agent_loop
                        .process_direct(trimmed.to_string(), session_key.clone())
                        .await
                    {
                        Ok(response) => {
                            spinner.stop();
                            println!("\n{}\n", MarkdownRenderer::render(&response));
                        }
                        Err(e) => {
                            spinner.stop();
                            eprintln!("\n{} {}\n", status_error(), e);
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl+C
                    println!("{}", colorize("\nGoodbye!", DIM));
                    break;
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl+D
                    println!("{}", colorize("\nGoodbye!", DIM));
                    break;
                }
                Err(err) => {
                    eprintln!("\n{} Error reading input: {}", status_error(), err);
                    return Err(err.into());
                }
            }
        }

        // Save history on exit
        let _ = editor.save_history(&history_path);
    }

    Ok(())
}

/// Print help for REPL commands
pub fn print_help() {
    let help_text = r#"Commands:

  /help       Show this help message
  /paste      Enter multi-line paste mode
  /history    Show recent command history
  /status     Show agent status and configuration
  /session    Show current session ID
  /clear      Clear the screen
  /exit       Exit the chat (also: /quit, exit, quit, :q)

Keyboard Shortcuts:

  Ctrl+C      Exit the chat (or cancel paste mode)
  Ctrl+D      Exit the chat (or submit in paste mode)
  Up/Down     Navigate command history
  Ctrl+L      Clear screen (without resetting context)

Examples:

  Multi-line input:
    klyntbot> /paste
    ... type your message
    ... across multiple lines
    ... /end

  View history:
    klyntbot> /history

Tips:

  - History is saved to ~/.klyntbot/history.txt
  - Use markdown formatting in messages (bold, code, lists)
  - Long responses are automatically formatted"#;

    println!("\n{}", draw_box(help_text, Some("Help")));
    println!();
}

/// Print command history
pub fn print_history(editor: &rustyline::DefaultEditor) {
    use rustyline::history::History;

    let history = editor.history();
    let len = history.len();

    if len == 0 {
        println!("\n{}", colorize("No command history", DIM));
        println!();
        return;
    }

    // Show last 20 entries
    let start = len.saturating_sub(20);
    let mut result = String::new();

    for (i, entry) in history.iter().enumerate().skip(start) {
        // Truncate long entries
        let display = if entry.len() > 60 {
            format!("{}...", &entry[..60])
        } else {
            entry.to_string()
        };
        result.push_str(&format!("  {:3}  {}\n", i + 1, display));
    }

    println!("\n{}", draw_box(result.trim_end(), Some("History")));
    println!();
}

/// Print status information
pub async fn print_status(_agent_loop: &agent::AgentLoop) {
    let status_text = format!(
        "Status: {}\nMode: Interactive CLI\nHistory: ~/.klyntbot/history.txt",
        colorize("Ready", SUCCESS)
    );

    println!("\n{}", draw_box(&status_text, Some("Status")));
    println!();
}
