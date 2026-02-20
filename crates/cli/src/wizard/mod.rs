//! Interactive onboarding wizard for klyntbot initialization.
//!
//! Two-phase flow:
//! - **Phase 1: Core Setup** — auto-detects provider, database, and channel;
//!   prompts for missing fields on a single screen.
//! - **Phase 2: Pack Selection** — presents feature packs grouped by tier,
//!   lets user toggle with spacebar, applies config mutations.
//!
//! Legacy step modules are still compiled for potential pack-specific credential
//! prompts but are no longer called from the main wizard flow.
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
pub mod core_setup;
pub mod daemon;
pub mod detect;
pub mod framework;
pub mod learning;
pub mod memory;
pub mod oauth;
pub mod pack_selection;
pub mod packs;
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

use framework::{StepResult, WizardState};

// ============================================================================
// 2-phase wizard orchestrator
// ============================================================================

/// Run the interactive onboarding wizard.
///
/// Two-phase flow:
/// 1. **Core Setup** — auto-detect provider, database, channel; prompt for missing
/// 2. **Pack Selection** — choose feature packs; apply config mutations
///
/// Flags:
/// - `packs_only`: Skip Phase 1, jump directly to pack selection.
/// - `reset`: Wipe config to defaults before starting.
pub async fn run_wizard(packs_only: bool, reset: bool) -> Result<()> {
    print_wizard_header();

    let mut state = if reset {
        WizardState::fresh()
    } else {
        WizardState::new()
    };

    if !packs_only {
        // Phase 1: Smart Core Setup
        print_phase_header(1, 2, "Core Setup");
        match core_setup::run_core_setup(&mut state).await {
            Ok(StepResult::Cancel) => {
                println!("\n{} Setup cancelled.", status_warning());
                return Ok(());
            }
            Ok(_) => {
                config::save(&state.config).await?;
            }
            Err(ref e) if e.to_string().contains("Ctrl+C") => {
                println!("\n{} Setup cancelled.", status_warning());
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    }

    // Phase 2: Pack Selection
    print_phase_header(
        if packs_only { 1 } else { 2 },
        if packs_only { 1 } else { 2 },
        "Feature Packs",
    );
    match pack_selection::run_pack_selection(&mut state) {
        Ok(StepResult::Cancel) => {
            println!("\n{} Setup cancelled.", status_warning());
            return Ok(());
        }
        Ok(StepResult::Back) if !packs_only => {
            // Go back to Phase 1 — recurse without packs_only
            return Box::pin(run_wizard(false, false)).await;
        }
        Ok(_) => {
            config::save(&state.config).await?;
        }
        Err(ref e) if e.to_string().contains("Ctrl+C") => {
            println!("\n{} Setup cancelled.", status_warning());
            return Ok(());
        }
        Err(e) => return Err(e),
    }

    print_completion();
    Ok(())
}

// ============================================================================
// UI helpers
// ============================================================================

/// Print a phase header (e.g., "Phase 1 of 2 — Core Setup").
fn print_phase_header(phase: usize, total: usize, name: &str) {
    println!();
    println!(
        "  {} Phase {} of {} — {}",
        colorize("●", BRAND),
        phase,
        total,
        colorize(name, BOLD),
    );
    println!();
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

/// Print the completion screen after all phases finish.
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
