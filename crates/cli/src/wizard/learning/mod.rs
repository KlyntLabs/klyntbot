//! Learning system configuration wizard step.
//!
//! Provides an expand-in-place interactive menu for learning system configuration,
//! following the same pattern as the search and memory steps.

mod menus;

use anyhow::Result;
use config::Config;

use crate::wizard::ui::MenuOutcome;

/// Actions available in the expanded learning system sub-menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LearningSubAction {
    Enable,
    Disable,
    ConfigureInterval,
    ConfigureBounds,
    Close,
}

pub(super) const SUB_ACTIONS: &[LearningSubAction] = &[
    LearningSubAction::Enable,
    LearningSubAction::Disable,
    LearningSubAction::ConfigureInterval,
    LearningSubAction::ConfigureBounds,
    LearningSubAction::Close,
];

impl LearningSubAction {
    /// Check if this action is available given the current configuration state.
    pub(super) fn is_available(&self, enabled: bool) -> bool {
        match self {
            Self::Enable => !enabled,           // Only show if currently disabled
            Self::Disable => enabled,           // Only show if currently enabled
            Self::ConfigureInterval => enabled, // Only configurable when enabled
            Self::ConfigureBounds => enabled,   // Only configurable when enabled
            Self::Close => true,                // Always available
        }
    }

    /// Get the label for this action.
    pub(super) fn label(&self, _enabled: bool) -> String {
        match self {
            Self::Enable => "Enable learning system".to_string(),
            Self::Disable => "Disable learning system".to_string(),
            Self::ConfigureInterval => "Configure analysis interval".to_string(),
            Self::ConfigureBounds => "Configure adaptive bounds".to_string(),
            Self::Close => "Close".to_string(),
        }
    }
}

/// Configure learning system (enable/disable + settings).
///
/// Shows an expandable menu following the search/memory step pattern.
pub(crate) fn configure_learning(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    menus::run_learning_menu(config, can_go_back)
}

/// Execute the "Enable learning system" action.
pub(super) fn execute_enable(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;

    config.learning.enabled = true;

    println!();
    println!("{} Learning system enabled!", colorize("✓", SUCCESS));
    println!();
    println!("The learning system will:");
    println!(
        "  {} Record tool execution outcomes (privacy-safe)",
        colorize("•", BRAND)
    );
    println!(
        "  {} Track enrichment suggestion feedback",
        colorize("•", BRAND)
    );
    println!(
        "  {} Adapt confidence thresholds based on accuracy",
        colorize("•", BRAND)
    );
    println!(
        "  {} Learn your preferences over time",
        colorize("•", BRAND)
    );
    println!();
    println!("{}", colorize("Privacy: ", BOLD));
    println!("Only metadata is stored (tool names, success rates).");
    println!("No user messages or tool arguments are persisted.");
    println!();
    println!(
        "Analysis runs every {} seconds ({})",
        config.learning.analysis_interval_secs,
        format_interval(config.learning.analysis_interval_secs)
    );
    println!();

    Ok(())
}

/// Execute the "Disable learning system" action.
pub(super) fn execute_disable(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;

    config.learning.enabled = false;

    println!();
    println!("{} Learning system disabled", colorize("✓", DIM));
    println!();
    println!("You can enable it later in ~/.klyntbot/config.json:");
    println!("  \"learning\": {{ \"enabled\": true }}");
    println!();

    Ok(())
}

/// Execute the "Configure analysis interval" action.
pub(super) fn execute_configure_interval(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;
    use std::io::Write;

    println!();
    println!("{}", colorize("Analysis Interval", BOLD));
    println!("{}", colorize(&"─".repeat(54), DIM));
    println!();
    println!("How often should the system analyze outcomes and adapt?");
    println!();
    println!(
        "  {} 3600 (1 hour) — Frequent adaptation",
        colorize("•", DIM)
    );
    println!(
        "  {} 86400 (24 hours) — Daily analysis ← recommended",
        colorize("•", BRAND)
    );
    println!("  {} 604800 (1 week) — Weekly analysis", colorize("•", DIM));
    println!();

    print!(
        "{} Interval in seconds [{}]: ",
        colorize("→", BRAND),
        config.learning.analysis_interval_secs
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let interval_str = input.trim();

    if !interval_str.is_empty() {
        match interval_str.parse::<u64>() {
            Ok(secs) if secs >= 60 => {
                config.learning.analysis_interval_secs = secs;
                println!(
                    "{} Interval set to {} ({})",
                    colorize("✓", SUCCESS),
                    secs,
                    format_interval(secs)
                );
                println!();
            }
            _ => {
                println!(
                    "{} Invalid interval. Must be >= 60 seconds",
                    colorize("✗", ERROR)
                );
                println!();
            }
        }
    } else {
        println!("{} Keeping current interval", colorize("ℹ", BRAND));
        println!();
    }

    Ok(())
}

/// Execute the "Configure adaptive bounds" action.
pub(super) fn execute_configure_bounds(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;
    use std::io::Write;

    println!();
    println!("{}", colorize("Adaptive Threshold Bounds", BOLD));
    println!("{}", colorize(&"─".repeat(54), DIM));
    println!();
    println!("The system adapts its confidence threshold within these bounds:");
    println!();
    println!(
        "Current: [{}, {}]",
        config.learning.min_threshold, config.learning.max_threshold
    );
    println!();
    println!("  {} 0.4 — Minimum (asks less often)", colorize("•", DIM));
    println!("  {} 0.9 — Maximum (asks more often)", colorize("•", DIM));
    println!();

    // Min threshold
    print!(
        "{} Min threshold [{}]: ",
        colorize("→", BRAND),
        config.learning.min_threshold
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let min_str = input.trim();

    if !min_str.is_empty() {
        match min_str.parse::<f32>() {
            Ok(t) if (0.0..=1.0).contains(&t) => {
                config.learning.min_threshold = t;
                println!("{} Min threshold set to {}", colorize("✓", SUCCESS), t);
            }
            _ => {
                println!(
                    "{} Invalid threshold. Keeping {}",
                    colorize("✗", ERROR),
                    config.learning.min_threshold
                );
            }
        }
    }

    // Max threshold
    print!(
        "{} Max threshold [{}]: ",
        colorize("→", BRAND),
        config.learning.max_threshold
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let max_str = input.trim();

    if !max_str.is_empty() {
        match max_str.parse::<f32>() {
            Ok(t) if (0.0..=1.0).contains(&t) && t > config.learning.min_threshold => {
                config.learning.max_threshold = t;
                println!("{} Max threshold set to {}", colorize("✓", SUCCESS), t);
            }
            _ => {
                println!(
                    "{} Invalid threshold. Keeping {}",
                    colorize("✗", ERROR),
                    config.learning.max_threshold
                );
            }
        }
    }

    println!();
    println!(
        "{} Bounds: [{}, {}]",
        colorize("✓", SUCCESS),
        config.learning.min_threshold,
        config.learning.max_threshold
    );
    println!();

    Ok(())
}

/// Helper to format interval in human-readable form.
fn format_interval(secs: u64) -> String {
    if secs < 60 {
        format!("{} seconds", secs)
    } else if secs < 3600 {
        format!("{} minutes", secs / 60)
    } else if secs < 86400 {
        format!("{} hours", secs / 3600)
    } else {
        format!("{} days", secs / 86400)
    }
}
