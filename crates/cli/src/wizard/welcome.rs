//! Welcome screen wizard module.
//!
//! Displays a branded welcome message and overview of the setup process.

use anyhow::Result;
use common::utils::terminal::*;

use super::framework::{StepResult, WizardModule, WizardState};
use super::prompts;

pub struct WelcomeModule;

impl WizardModule for WelcomeModule {
    fn name(&self) -> &str {
        "Welcome"
    }

    fn description(&self) -> &str {
        "Introduction and setup overview"
    }

    fn run(&self, state: &mut WizardState) -> Result<StepResult> {
        let mode = if state.is_fresh_install {
            "first-time setup"
        } else {
            "reconfiguration"
        };

        // Simple clean header with orange branding - no box, just clean lines
        println!();
        println!("{}", colorize("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", BRAND));
        println!();
        println!("  {}  {}",
            colorize("●", BRAND),
            colorize("klyntbot", BOLD)
        );
        println!("  {}  {}",
            colorize("│", BRAND),
            colorize("Your AI Assistant Platform", DIM)
        );
        println!();
        println!("{}", colorize("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", BRAND));

        // Setup overview with orange highlights
        println!("\nLet's set up your AI assistant ({}).", colorize(mode, HIGHLIGHT));
        println!("This wizard will guide you through:");
        println!();

        println!("  {}  LLM provider & API key", colorize("1", BRAND));
        println!("  {}  Chat channels (Telegram, Discord, Slack, ...)", colorize("2", BRAND));
        println!("  {}  Tool permissions & security", colorize("3", BRAND));
        println!("  {}  Workspace & file templates", colorize("4", BRAND));

        if state.total_steps > 4 {
            println!("  {}  Additional configuration", colorize("+", BRAND));
        }

        println!();
        println!("{} You can press {} at any time to cancel.",
            colorize("→", BRAND),
            colorize("Ctrl+C", BOLD)
        );

        let proceed = prompts::prompt_yes_no("\nReady to begin?", true)?;
        if proceed {
            Ok(StepResult::Next)
        } else {
            Ok(StepResult::Cancel)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome_module_metadata() {
        let module = WelcomeModule;
        assert_eq!(module.name(), "Welcome");
        assert_eq!(module.description(), "Introduction and setup overview");
    }

    #[test]
    fn test_welcome_module_is_required() {
        let module = WelcomeModule;
        assert!(module.is_required()); // default trait impl returns true
    }

    #[test]
    fn test_welcome_module_is_applicable() {
        let module = WelcomeModule;
        let state = WizardState::new();
        assert!(module.is_applicable(&state));
    }
}
