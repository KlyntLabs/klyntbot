//! Welcome screen wizard module.
//!
//! Displays a branded welcome message and overview of the setup process.

use anyhow::Result;
use common::utils::terminal::*;

use super::framework::{StepResult, WizardModule, WizardState};

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

        // Setup overview with vertical line throughout
        println!("{}", draw_step_line(&format!("Let's set up your AI assistant ({}).", colorize(mode, HIGHLIGHT))));
        println!("{}", draw_step_line("This wizard will guide you through:"));
        println!("{}", draw_step_line(""));

        println!("{}", draw_step_line(&format!(" {}  LLM provider & API key", colorize("1", BRAND))));
        println!("{}", draw_step_line(&format!(" {}  Chat channels (Telegram, Discord, Slack, ...)", colorize("2", BRAND))));
        println!("{}", draw_step_line(&format!(" {}  Tool permissions & security", colorize("3", BRAND))));
        println!("{}", draw_step_line(&format!(" {}  Workspace & file templates", colorize("4", BRAND))));

        if state.total_steps > 4 {
            println!("{}", draw_step_line(&format!(" {}  Additional configuration", colorize("+", BRAND))));
        }

        println!("{}", draw_step_line(""));
        println!("{}", draw_step_line(&format!("{} You can press {} at any time to cancel.",
            colorize("→", BRAND),
            colorize("Ctrl+C", BOLD)
        )));

        // Running `init` signals intent — no confirmation needed
        Ok(StepResult::Next)
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
