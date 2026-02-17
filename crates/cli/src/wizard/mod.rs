//! Interactive onboarding wizard for klyntbot initialization.
//!
//! Step modules (expand-in-place menus):
//! - `provider`  – LLM provider selection, API key, model (includes config dashboard)
//! - `channels`  – Chat platform configuration (async)
//! - `tools`     – Security presets and tool permissions
//! - `daemon`    – Background service installation
//! - `workspace` – Workspace directories and template files
//! - `calendar`  – Calendar sync configuration
//!
//! Steps under `steps/`:
//! - `welcome`   – Config status dashboard utilities
//! - `todo_notifications` – Notification preferences
//!
//! Supporting modules:
//! - `framework`  – `WizardModule` trait, `WizardState`, `WizardRunner`
//! - `prompts`    – Reusable prompt utilities
//! - `templates`  – Workspace template file contents
//! - `oauth`      – OAuth callback server for Discord/Slack
//! - `ui`         – Shared terminal helpers (erase_lines, read_key)
//! - `ask_user_prompt` – Tabbed multi-question UI for ask_user tool

pub mod ask_user_prompt;
pub mod calendar;
pub mod channels;
pub mod daemon;
pub mod framework;
pub mod learning;
pub mod memory;
pub mod oauth;
pub mod prompts;
pub mod provider;
pub mod search;
pub mod steps;
pub mod templates;
pub mod tools;
pub(crate) mod ui;
pub mod workspace;

use anyhow::Result;
use common::utils::terminal::*;

use framework::{print_step_header, StepResult, WizardModule, WizardState};

/// Adapter wrapping [`tools::configure_tools`] as a [`WizardModule`].
struct ToolsModule;

impl WizardModule for ToolsModule {
    fn name(&self) -> &str {
        "Tool Permissions"
    }

    fn description(&self) -> &str {
        "Configure tool security and permissions"
    }

    fn is_required(&self) -> bool {
        false
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let can_go_back = state.current_step > 1;
        let outcome = tools::configure_tools(&mut state.config, can_go_back)?;
        Ok(if outcome == ui::MenuOutcome::Back {
            StepResult::Back
        } else {
            StepResult::Next
        })
    }
}

/// Adapter wrapping [`workspace::configure_workspace`] as a [`WizardModule`].
struct WorkspaceModule;

impl WizardModule for WorkspaceModule {
    fn name(&self) -> &str {
        "Workspace & Notifications"
    }

    fn description(&self) -> &str {
        "Create workspace and configure notification preferences"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let can_go_back = state.current_step > 1;
        let outcome = workspace::configure_workspace(&mut state.config, can_go_back)?;
        Ok(if outcome == ui::MenuOutcome::Back {
            StepResult::Back
        } else {
            StepResult::Next
        })
    }
}

/// Run the interactive onboarding wizard.
///
/// Orchestrates all wizard modules in sequence with forward/back navigation.
/// The channels and calendar steps are handled inline because they require async I/O.
pub async fn run_wizard() -> Result<()> {
    // Print global branding header at the very top
    print_wizard_header();

    let mut state = WizardState::new();

    // Step table: (name, required, applicable)
    // Index order must match the dispatch arms below.
    let all_steps: &[(&str, bool, bool)] = &[
        ("LLM Provider", true, true),
        ("Chat Channels", false, true),
        ("Tool Permissions", false, true),
        ("Workspace & Notifications", true, true),
        ("Calendar Sync", false, true),
        ("Semantic Search", false, true),
        ("Conversation Memory", false, true),
        ("Learning System", false, true),
        (
            "Background Service",
            false,
            daemon::DaemonModule.is_applicable(&state),
        ),
        ("Review & Confirm", true, true),
    ];

    // Filter to applicable steps
    let applicable: Vec<usize> = all_steps
        .iter()
        .enumerate()
        .filter(|(_, (_, _, app))| *app)
        .map(|(i, _)| i)
        .collect();

    state.total_steps = applicable.len();

    // Navigate through steps with forward/back support
    let mut pos = 0;
    while pos < applicable.len() {
        let step_idx = applicable[pos];
        let (name, required, _) = all_steps[step_idx];
        state.current_step = pos + 1;

        print_step_header(&state, name, required);

        // Dispatch to the right module. Step indices correspond to `all_steps`:
        // 0=provider, 1=channels (async), 2=tools, 3=workspace+notifications,
        // 4=calendar (async), 5=semantic_search, 6=conversation_memory, 7=learning,
        // 8=daemon, 9=review.
        // Ctrl+C in raw-mode prompts surfaces as an Err containing "Ctrl+C".
        let result = match match step_idx {
            0 => provider::ProviderModule.run(&mut state),
            1 => run_channels_step(&mut state).await,
            2 => ToolsModule.run(&mut state),
            3 => WorkspaceModule.run(&mut state),
            4 => run_calendar_step(&mut state).await,
            5 => run_search_step(&mut state),
            6 => run_memory_step(&mut state),
            7 => run_learning_step(&mut state),
            8 => daemon::DaemonModule.run(&mut state),
            9 => steps::review::ReviewModule.run(&mut state),
            _ => unreachable!(),
        } {
            Ok(r) => r,
            Err(ref e) if e.to_string().contains("Ctrl+C") => {
                println!("\n{} Setup cancelled. No changes saved.", status_warning());
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        match result {
            StepResult::Next | StepResult::Skip => {
                // Auto-save after each completed step so progress isn't lost
                config::save(&state.config).await?;
                pos += 1;
            }
            StepResult::Back if pos > 0 => pos -= 1,
            StepResult::Back => {} // already at first step
            StepResult::Cancel => {
                println!("\n{} Setup cancelled. No changes saved.", status_warning());
                return Ok(());
            }
        }
    }

    // Final safety save (redundant but ensures config is written)
    config::save(&state.config).await?;

    // Completion screen
    print_completion();

    Ok(())
}

/// Channels wizard step (async due to OAuth flows).
///
/// No y/n gate — always shows channel configuration with current state.
async fn run_channels_step(state: &mut WizardState) -> Result<StepResult> {
    let can_go_back = state.current_step > 1;
    let outcome = channels::configure_channels(&mut state.config, can_go_back).await?;
    Ok(if outcome == ui::MenuOutcome::Back {
        StepResult::Back
    } else {
        StepResult::Next
    })
}

/// Calendar wizard step (async due to connection testing).
///
/// No y/n gate — always shows calendar configuration with current state.
async fn run_calendar_step(state: &mut WizardState) -> Result<StepResult> {
    let can_go_back = state.current_step > 1;
    let outcome = calendar::configure_calendars(&mut state.config, can_go_back).await?;
    Ok(if outcome == ui::MenuOutcome::Back {
        StepResult::Back
    } else {
        StepResult::Next
    })
}

/// Semantic search wizard step (sync, but model download can take time).
///
/// Shows enable/disable menu with optional model download.
fn run_search_step(state: &mut WizardState) -> Result<StepResult> {
    let can_go_back = state.current_step > 1;
    let outcome = search::configure_semantic_search(&mut state.config, can_go_back)?;
    Ok(if outcome == ui::MenuOutcome::Back {
        StepResult::Back
    } else {
        StepResult::Next
    })
}

/// Conversation memory wizard step (sync, reuses semantic search model).
///
/// Shows enable/disable menu for conversation embedding and search.
fn run_memory_step(state: &mut WizardState) -> Result<StepResult> {
    let can_go_back = state.current_step > 1;
    let outcome = memory::configure_conversation_memory(&mut state.config, can_go_back)?;
    Ok(if outcome == ui::MenuOutcome::Back {
        StepResult::Back
    } else {
        StepResult::Next
    })
}

/// Learning system wizard step (sync).
///
/// Shows enable/disable menu for adaptive learning and outcome recording.
fn run_learning_step(state: &mut WizardState) -> Result<StepResult> {
    let can_go_back = state.current_step > 1;
    let outcome = learning::configure_learning(&mut state.config, can_go_back)?;
    Ok(if outcome == ui::MenuOutcome::Back {
        StepResult::Back
    } else {
        StepResult::Next
    })
}


/// Print the global wizard header (shown once at the start).
fn print_wizard_header() {
    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            BRAND
        )
    );
    println!("{}", draw_step_line(&colorize("klyntbot", BOLD)));
    println!(
        "{}",
        draw_step_line(&colorize("Your AI Assistant Platform", DIM))
    );
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            BRAND
        )
    );
    println!();
}

/// Print the completion screen after all steps finish.
fn print_completion() {
    let config_path_str = config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.klyntbot/config.json".to_string());

    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            SUCCESS
        )
    );
    println!();
    println!(
        "  {} {}",
        colorize("✓", SUCCESS),
        colorize("Setup Complete!", BOLD)
    );
    println!();
    println!(
        "{}",
        colorize(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
            SUCCESS
        )
    );
    println!();

    println!(
        "{} Your AI assistant is ready to use!\n",
        colorize("●", BRAND)
    );

    println!("{} {}", colorize("→", BRAND), colorize("Try it out:", BOLD));
    println!("  {} klyntbot chat", colorize("$", DIM));
    println!("  {} klyntbot chat \"Hello!\"", colorize("$", DIM));
    println!();

    println!("{} {}", colorize("→", BRAND), colorize("Get help:", BOLD));
    println!("  {} klyntbot --help", colorize("$", DIM));
    println!("  {} klyntbot status", colorize("$", DIM));
    println!();

    println!(
        "{} {}",
        colorize("→", BRAND),
        colorize("Configuration saved:", BOLD)
    );
    println!("  {}", colorize(&config_path_str, HIGHLIGHT));
    println!();
}

#[cfg(test)]
mod tests;
