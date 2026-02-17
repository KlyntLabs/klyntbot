//! Semantic search configuration wizard step.
//!
//! Provides an expand-in-place interactive menu for semantic search configuration,
//! following the same pattern as the calendar step.

mod menus;

use anyhow::Result;
use config::Config;

use crate::wizard::ui::MenuOutcome;

/// Actions available in the expanded semantic search sub-menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SearchSubAction {
    EnableAndDownload,
    Disable,
    ConfigureThreshold,
    Close,
}

pub(super) const SUB_ACTIONS: &[SearchSubAction] = &[
    SearchSubAction::EnableAndDownload,
    SearchSubAction::Disable,
    SearchSubAction::ConfigureThreshold,
    SearchSubAction::Close,
];

impl SearchSubAction {
    /// Check if this action is available given the current configuration state.
    pub(super) fn is_available(&self, enabled: bool) -> bool {
        match self {
            Self::EnableAndDownload => !enabled, // Only show if currently disabled
            Self::Disable => enabled,            // Only show if currently enabled
            Self::ConfigureThreshold => enabled, // Only configurable when enabled
            Self::Close => true,                 // Always available
        }
    }

    /// Get the label for this action.
    pub(super) fn label(&self, _enabled: bool) -> String {
        match self {
            Self::EnableAndDownload => "Enable & download model now (~60 sec, 420 MB)".to_string(),
            Self::Disable => "Disable semantic search".to_string(),
            Self::ConfigureThreshold => "Configure similarity threshold".to_string(),
            Self::Close => "Close".to_string(),
        }
    }
}

/// Configure semantic search (enable/disable + model download).
///
/// Shows an expandable menu following the calendar step pattern.
pub(crate) fn configure_semantic_search(
    config: &mut Config,
    can_go_back: bool,
) -> Result<MenuOutcome> {
    menus::run_search_menu(config, can_go_back)
}

/// Execute the "Enable & download model now" action.
pub(super) fn execute_enable_and_download(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;
    use std::sync::Arc;
    use tools::EmbeddingEngine;

    println!();
    println!("{} Downloading embedding model...", colorize("↓", BRAND));
    println!("{}", colorize("This may take 30-60 seconds...", DIM));
    println!();

    // Initialize engine (triggers model download with built-in progress bar)
    let engine = Arc::new(EmbeddingEngine::new());

    // Test that it works
    match engine.embed("test") {
        Ok(_) => {
            config.todo.search.enabled = true;
            println!();
            println!("{} Model downloaded and ready!", colorize("✓", SUCCESS));
            println!();
            println!(
                "Try it: {} todo search --semantic \"your query\"",
                colorize("klyntbot", BRAND)
            );
            println!();
            Ok(())
        }
        Err(e) => {
            println!("{} Download failed: {}", colorize("✗", ERROR), e);
            println!();
            println!("You can retry later with: klyntbot todo search --semantic");
            println!("(Model will download on first search)");
            println!();

            // Still enable semantic search (will download on first use)
            config.todo.search.enabled = true;
            Ok(())
        }
    }
}

/// Execute the "Disable semantic search" action.
pub(super) fn execute_disable(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;

    config.todo.search.enabled = false;

    println!();
    println!("{} Semantic search disabled", colorize("✓", DIM));
    println!();
    println!("You can enable it later in ~/.klyntbot/config.json:");
    println!("  \"todo\": {{ \"search\": {{ \"enabled\": true }} }}");
    println!();

    Ok(())
}

/// Execute the "Configure threshold" action.
pub(super) fn execute_configure_threshold(config: &mut Config) -> Result<()> {
    use common::utils::terminal::*;
    use std::io::Write;

    println!();
    println!("{}", colorize("Similarity Threshold", BOLD));
    println!("{}", colorize(&"─".repeat(54), DIM));
    println!();
    println!("Lower threshold = more results (less strict)");
    println!("Higher threshold = fewer results (more precise)");
    println!();
    println!(
        "  {} 0.3 — Loose (finds distant concepts)",
        colorize("•", DIM)
    );
    println!(
        "  {} 0.5 — Moderate (balanced) ← default",
        colorize("•", BRAND)
    );
    println!("  {} 0.7 — Strict (only close matches)", colorize("•", DIM));
    println!();

    print!(
        "{} Threshold [{}]: ",
        colorize("→", BRAND),
        config.todo.search.semantic_threshold
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let threshold_str = input.trim();

    if !threshold_str.is_empty() {
        match threshold_str.parse::<f64>() {
            Ok(t) if (0.0..=1.0).contains(&t) => {
                config.todo.search.semantic_threshold = t;
                println!("{} Threshold set to {}", colorize("✓", SUCCESS), t);
                println!();
            }
            _ => {
                println!(
                    "{} Invalid threshold. Must be between 0.0 and 1.0",
                    colorize("✗", ERROR)
                );
                println!();
            }
        }
    } else {
        println!("{} Keeping current threshold", colorize("ℹ", BRAND));
        println!();
    }

    Ok(())
}
