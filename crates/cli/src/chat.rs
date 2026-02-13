//! Chat command handler for interactive CLI mode

use agent::{AgentEvent, AgentLoop, UserResponse};
use anyhow::Result;
use bus::MessageBus;
use common::prompts::PromptType;
use common::utils::terminal::*;
use common::utils::StreamRenderer;
use rustyline::error::ReadlineError;
use std::io::{self, Write};
use std::sync::Arc;

use crate::interactive::SlashCommandHelper;
use crate::wizard::prompts::{self, SelectOption, SelectWithInputOption, SelectWithInputResult};

/// Handle chat command
pub async fn handle_chat(message: Option<String>, session: String) -> Result<()> {
    // Load config
    let config = config::load()?;
    let model = config.agents.defaults.model.clone();

    // Clean startup header
    println!(
        "\n  {} {}",
        colorize("klyntbot", "\x1b[1;36m"), // bold cyan
        colorize(&format!("\u{00b7} {}", model), DIM)
    );

    // Initialize LLM provider
    let provider = providers::create_provider(&config)?;

    // Create a minimal message bus (not used in CLI mode, but required for AgentLoop)
    let bus = Arc::new(MessageBus::new(10));

    // Initialize agent loop (Arc for streaming support)
    let agent_loop = Arc::new(AgentLoop::new(bus, provider, config).await?);

    // Session key for CLI
    let session_key = format!("cli:{}", session);

    // Handle single message or interactive mode
    if let Some(msg) = message {
        // Single message mode
        println!("\n{} {}", colorize("You:", PROMPT), msg);
        println!();

        run_with_streaming(&agent_loop, msg, session_key).await?;
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

        let helper = SlashCommandHelper::new();
        let config = rustyline::Config::builder().auto_add_history(false).build();
        let mut editor = rustyline::Editor::with_config(config)?;
        editor.set_helper(Some(helper));
        let _ = editor.load_history(&history_path);

        println!(
            "\n{}",
            colorize("Interactive chat mode. Type /help for commands.\n", DIM)
        );

        loop {
            let prompt = format!("{} ", colorize("klyntbot>", "\x1b[1;36m")); // bold cyan
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
                                let paste_message = lines.join("\n");

                                // Add to history
                                let _ = editor.add_history_entry(&paste_message);

                                // Process with streaming
                                println!();
                                if let Err(e) = run_with_streaming(
                                    &agent_loop,
                                    paste_message,
                                    session_key.clone(),
                                )
                                .await
                                {
                                    eprintln!("\n{} {}\n", status_error(), e);
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

                    // Process the message with streaming
                    println!();
                    if let Err(e) =
                        run_with_streaming(&agent_loop, trimmed.to_string(), session_key.clone())
                            .await
                    {
                        eprintln!("\n{} {}\n", status_error(), e);
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl+C in prompt — exit
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

/// Process a message using the streaming event system with Ctrl+C cancellation.
async fn run_with_streaming(
    agent_loop: &Arc<AgentLoop>,
    message: String,
    session_key: String,
) -> Result<()> {
    let (mut event_rx, user_tx, cancel_token, handle) = agent_loop
        .process_direct_streaming(message, session_key)
        .await?;

    let mut renderer = StreamRenderer::new();

    // Spawn a task that cancels on Ctrl+C
    let cancel_for_signal = cancel_token.clone();
    let signal_handle = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancel_for_signal.cancel();
    });

    // Consume events from the agent, tracking whether we got a clean exit
    let mut clean_exit = false;
    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::ContentChunk(chunk) => renderer.on_content_chunk(&chunk),
            AgentEvent::ToolStart { name, args } => renderer.on_tool_start(&name, &args),
            AgentEvent::ToolEnd {
                name,
                success,
                duration_ms,
            } => renderer.on_tool_end(&name, success, duration_ms),
            AgentEvent::IterationStart { iteration, max } => {
                renderer.on_iteration_start(iteration, max);
            }
            AgentEvent::PromptUser(request) => {
                // Pause streaming output to show interactive prompt
                renderer.pause();

                // Handle the prompt using wizard's interactive components.
                // This blocks the event loop until the user responds — by design.
                let response = handle_interactive_prompt(request.prompt_type);

                // Resume streaming (prompt was ~1 line of compact output)
                renderer.resume(1);

                // Send user's response back to the agent loop
                if let Ok(resp) = response {
                    let _ = user_tx.send(resp).await;
                } else {
                    // Prompt failed (e.g., Ctrl+C) — send Cancelled
                    let _ = user_tx.send(UserResponse::Cancelled).await;
                }
            }
            AgentEvent::Done(_) => {
                clean_exit = true;
                break;
            }
            AgentEvent::Error(e) => {
                eprintln!("\n{} {}", status_error(), e);
                clean_exit = true;
                break;
            }
        }
    }

    // Cancel the signal watcher (no longer needed)
    signal_handle.abort();

    // Mark cancelled if Ctrl+C was pressed or channel closed without Done/Error
    if cancel_token.is_cancelled() || !clean_exit {
        renderer.mark_cancelled();
    }

    // Wait for the agent task to complete
    let result = handle.await?;

    // Finalize: always clean up terminal cursor state, then handle success/failure
    match result {
        Ok(_) => {
            let rendered = renderer.finalize();
            println!("{}", rendered);
        }
        Err(e) => {
            // Still finalize to clean up terminal cursor state
            let rendered = renderer.finalize();
            if !rendered.trim().is_empty() {
                println!("{}", rendered);
            }
            eprintln!("\n{} {}", status_error(), e);
        }
    }

    // Show elapsed time
    let elapsed = renderer.elapsed_secs();
    println!(
        "{}\n",
        StreamRenderer::draw_separator(Some(&format!("{:.1}s", elapsed)))
    );

    Ok(())
}

/// Handle an interactive prompt by delegating to wizard's prompt components.
///
/// Bridges between agent prompt types (from `common::prompts`) and the
/// wizard's interactive terminal prompts (arrow keys, TAB, etc.).
fn handle_interactive_prompt(prompt_type: PromptType) -> Result<UserResponse> {
    match prompt_type {
        PromptType::Select {
            header,
            options,
            default_idx,
        } => {
            let opts: Vec<SelectOption<'_>> = options
                .iter()
                .map(|o| SelectOption {
                    label: &o.label,
                    description: &o.description,
                })
                .collect();

            let idx = prompts::prompt_select(&header, &opts, default_idx)?;
            Ok(UserResponse::Selected(idx))
        }

        PromptType::SelectWithInput {
            header,
            options,
            default_idx,
        } => {
            let opts: Vec<SelectWithInputOption<'_>> = options
                .iter()
                .map(|o| SelectWithInputOption {
                    label: &o.label,
                    description: &o.description,
                    expandable: o.expandable,
                    input_hint: o.input_hint.as_deref(),
                })
                .collect();

            let SelectWithInputResult { index, text } =
                prompts::prompt_select_with_input(&header, &opts, default_idx)?;
            Ok(UserResponse::SelectedWithInput { index, text })
        }

        PromptType::MultiSelect { header, options } => {
            let opts: Vec<SelectOption<'_>> = options
                .iter()
                .map(|o| SelectOption {
                    label: &o.label,
                    description: &o.description,
                })
                .collect();

            let indices = prompts::prompt_multi_select(&header, &opts)?;
            Ok(UserResponse::MultiSelected(indices))
        }

        PromptType::YesNo { question, default } => {
            let answer = prompts::prompt_yes_no(&question, default)?;
            Ok(UserResponse::YesNo(answer))
        }
    }
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

  Ctrl+C      Cancel current response / Exit the chat
  Ctrl+D      Exit the chat (or submit in paste mode)
  Up/Down     Navigate command history
  Tab         Autocomplete slash commands

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
  - Long responses are automatically formatted
  - Press Ctrl+C during a response to cancel"#;

    println!("\n{}", draw_box(help_text, Some("Help")));
    println!();
}

/// Print command history
pub fn print_history(
    editor: &rustyline::Editor<SlashCommandHelper, rustyline::history::DefaultHistory>,
) {
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
pub async fn print_status(agent_loop: &agent::AgentLoop) {
    let status_text = format!(
        "Status: {}\nModel: {}\nMode: Interactive CLI\nHistory: ~/.klyntbot/history.txt",
        colorize("Ready", SUCCESS),
        agent_loop.model_name(),
    );

    println!("\n{}", draw_box(&status_text, Some("Status")));
    println!();
}
