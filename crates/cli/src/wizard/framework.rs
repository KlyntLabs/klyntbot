//! Core wizard framework: trait, state, and runner.
//!
//! Provides the pluggable module system that drives the setup wizard.
//! Each wizard step implements `WizardModule`, and the `WizardRunner`
//! orchestrates navigation through them.

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;

// ============================================================================
// Step Result
// ============================================================================

/// Outcome of running a single wizard step.
pub enum StepResult {
    /// Advance to the next step.
    Next,
    /// Return to the previous step.
    Back,
    /// Skip this step (optional step the user declined).
    Skip,
    /// User cancelled the entire wizard.
    Cancel,
}

// ============================================================================
// Wizard State
// ============================================================================

/// Shared state passed through all wizard modules.
///
/// Modules read and mutate `config` to build the final configuration.
/// Metadata fields help modules render step counters and contextual UI.
pub struct WizardState {
    /// The configuration being built by the wizard.
    pub config: Config,
    /// Total number of (applicable) steps.
    pub total_steps: usize,
    /// Current step number (1-based, for display).
    pub current_step: usize,
    /// Whether this is a fresh install (no existing config).
    pub is_fresh_install: bool,
}

impl Default for WizardState {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardState {
    /// Create a new wizard state, loading existing config if present.
    pub fn new() -> Self {
        let is_fresh = !config::loader::exists();
        let config = if is_fresh {
            Config::default()
        } else {
            config::load_sync().unwrap_or_default()
        };

        Self {
            config,
            total_steps: 0,
            current_step: 0,
            is_fresh_install: is_fresh,
        }
    }

    /// Format a step header like "Step 2 of 6: Provider Setup"
    pub fn step_header(&self, title: &str) -> String {
        format!(
            "Step {} of {}: {}",
            self.current_step, self.total_steps, title
        )
    }
}

// ============================================================================
// Wizard Module Trait
// ============================================================================

/// A pluggable wizard step.
///
/// Each module handles one section of the setup wizard (e.g., provider
/// configuration, channel setup, tools permissions). Modules receive
/// mutable access to `WizardState` to read/write the config being built.
pub trait WizardModule {
    /// Display name shown in step headers (e.g., "LLM Provider").
    fn name(&self) -> &str;

    /// Short description for the step overview.
    fn description(&self) -> &str;

    /// Whether this step is required or can be skipped.
    fn is_required(&self) -> bool {
        true
    }

    /// Whether this step should be shown given the current state.
    ///
    /// Modules can check `state.config` to decide if they're relevant.
    /// For example, a channel detail step might only show if the user
    /// chose to enable channels.
    fn is_applicable(&self, _state: &WizardState) -> bool {
        true
    }

    /// Run the module's interactive flow.
    ///
    /// The module should:
    /// 1. Print its UI (prompts, selections, etc.)
    /// 2. Collect user input
    /// 3. Validate and store results in `state.config`
    /// 4. Return a `StepResult` to control navigation
    fn run(&self, state: &mut WizardState) -> Result<StepResult>;
}

// ============================================================================
// Wizard Runner
// ============================================================================

/// Orchestrates a sequence of `WizardModule`s with forward/back navigation.
pub struct WizardRunner {
    modules: Vec<Box<dyn WizardModule>>,
}

impl Default for WizardRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardRunner {
    /// Create an empty runner.
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    /// Add a module to the wizard sequence.
    pub fn add_module(&mut self, module: impl WizardModule + 'static) {
        self.modules.push(Box::new(module));
    }

    /// Run the wizard to completion, saving config on success.
    ///
    /// Returns `Ok(true)` if the wizard completed and saved,
    /// `Ok(false)` if the user cancelled.
    pub fn run(&self) -> Result<bool> {
        let mut state = WizardState::new();

        // Filter to applicable modules and set step count
        let applicable: Vec<usize> = self
            .modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_applicable(&state))
            .map(|(i, _)| i)
            .collect();

        state.total_steps = applicable.len();

        if applicable.is_empty() {
            println!("{}", colorize("No wizard steps to run.", DIM));
            return Ok(false);
        }

        // Navigate through steps
        let mut step_idx = 0;
        while step_idx < applicable.len() {
            let module_idx = applicable[step_idx];
            let module = &self.modules[module_idx];

            state.current_step = step_idx + 1;

            // Print step header
            print_step_header(&state, module.name(), module.is_required());

            match module.run(&mut state)? {
                StepResult::Next => {
                    step_idx += 1;
                }
                StepResult::Back => {
                    step_idx = step_idx.saturating_sub(1);
                }
                StepResult::Skip => {
                    step_idx += 1;
                }
                StepResult::Cancel => {
                    println!("\n{} Setup cancelled. No changes saved.", status_warning());
                    return Ok(false);
                }
            }
        }

        // Save configuration
        config::save_sync(&state.config)?;

        Ok(true)
    }
}

/// Print a formatted step header with separator and progress indicator.
pub fn print_step_header(state: &WizardState, name: &str, _required: bool) {
    // Blank line before progress bar for steps after the first
    if state.current_step > 1 {
        println!();
    }
    print!(
        "{}",
        draw_wizard_step_header(state.current_step, state.total_steps, name,)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_state_step_header() {
        let mut state = WizardState::new();
        state.total_steps = 5;
        state.current_step = 2;

        assert_eq!(
            state.step_header("Provider Setup"),
            "Step 2 of 5: Provider Setup"
        );
    }

    #[test]
    fn test_wizard_runner_empty() {
        let runner = WizardRunner::new();
        // Can't easily test run() since it needs stdin, but we can verify creation
        assert_eq!(runner.modules.len(), 0);
    }

    /// A test module that always returns Next.
    struct TestModule;

    impl WizardModule for TestModule {
        fn name(&self) -> &str {
            "Test"
        }
        fn description(&self) -> &str {
            "A test module"
        }
        fn run(&self, _state: &mut WizardState) -> Result<StepResult> {
            Ok(StepResult::Next)
        }
    }

    #[test]
    fn test_wizard_runner_add_module() {
        let mut runner = WizardRunner::new();
        runner.add_module(TestModule);
        assert_eq!(runner.modules.len(), 1);
    }

    /// A module that is never applicable.
    struct SkippedModule;

    impl WizardModule for SkippedModule {
        fn name(&self) -> &str {
            "Skipped"
        }
        fn description(&self) -> &str {
            "Should be skipped"
        }
        fn is_applicable(&self, _state: &WizardState) -> bool {
            false
        }
        fn run(&self, _state: &mut WizardState) -> Result<StepResult> {
            panic!("should not be called");
        }
    }

    #[test]
    fn test_module_trait_defaults() {
        let module = TestModule;
        assert!(module.is_required());

        let state = WizardState::new();
        assert!(module.is_applicable(&state));
    }

    // ========================================================================
    // WizardState tests
    // ========================================================================

    #[test]
    fn test_wizard_state_initial_values() {
        let state = WizardState::new();
        assert_eq!(state.total_steps, 0);
        assert_eq!(state.current_step, 0);
        // Config should be initialized
        assert!(!state.config.agents.defaults.model.is_empty());
    }

    #[test]
    fn test_wizard_state_step_header_first_step() {
        let mut state = WizardState::new();
        state.total_steps = 6;
        state.current_step = 1;

        assert_eq!(state.step_header("Welcome"), "Step 1 of 6: Welcome");
    }

    #[test]
    fn test_wizard_state_step_header_last_step() {
        let mut state = WizardState::new();
        state.total_steps = 6;
        state.current_step = 6;

        assert_eq!(
            state.step_header("Workspace Setup"),
            "Step 6 of 6: Workspace Setup"
        );
    }

    // ========================================================================
    // WizardRunner module management tests
    // ========================================================================

    #[test]
    fn test_wizard_runner_add_multiple_modules() {
        let mut runner = WizardRunner::new();
        runner.add_module(TestModule);
        runner.add_module(TestModule);
        runner.add_module(TestModule);
        assert_eq!(runner.modules.len(), 3);
    }

    #[test]
    fn test_wizard_runner_with_skipped_module() {
        let mut runner = WizardRunner::new();
        runner.add_module(TestModule);
        runner.add_module(SkippedModule);
        runner.add_module(TestModule);
        assert_eq!(runner.modules.len(), 3);

        // Verify the skipped module reports itself as not applicable
        let state = WizardState::new();
        assert!(!runner.modules[1].is_applicable(&state));
    }

    // ========================================================================
    // WizardModule trait tests
    // ========================================================================

    #[test]
    fn test_test_module_name_and_description() {
        let module = TestModule;
        assert_eq!(module.name(), "Test");
        assert_eq!(module.description(), "A test module");
    }

    #[test]
    fn test_skipped_module_not_applicable() {
        let module = SkippedModule;
        let state = WizardState::new();
        assert!(!module.is_applicable(&state));
    }

    /// A module that is optional.
    struct OptionalModule;

    impl WizardModule for OptionalModule {
        fn name(&self) -> &str {
            "Optional"
        }
        fn description(&self) -> &str {
            "An optional module"
        }
        fn is_required(&self) -> bool {
            false
        }
        fn run(&self, _state: &mut WizardState) -> Result<StepResult> {
            Ok(StepResult::Skip)
        }
    }

    #[test]
    fn test_optional_module() {
        let module = OptionalModule;
        assert!(!module.is_required());
        let state = WizardState::new();
        assert!(module.is_applicable(&state)); // Still applicable, just not required
    }

    // ========================================================================
    // StepResult variant tests
    // ========================================================================

    #[test]
    fn test_step_result_next() {
        let result = StepResult::Next;
        assert!(matches!(result, StepResult::Next));
    }

    #[test]
    fn test_step_result_back() {
        let result = StepResult::Back;
        assert!(matches!(result, StepResult::Back));
    }

    #[test]
    fn test_step_result_skip() {
        let result = StepResult::Skip;
        assert!(matches!(result, StepResult::Skip));
    }

    #[test]
    fn test_step_result_cancel() {
        let result = StepResult::Cancel;
        assert!(matches!(result, StepResult::Cancel));
    }
}
