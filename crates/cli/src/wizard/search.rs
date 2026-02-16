//! Semantic search configuration wizard step.
//!
//! Provides an interactive menu for enabling semantic search and optionally
//! downloading the embedding model during initialization.

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use std::sync::Arc;
use std::io::Write;

use crate::wizard::ui::MenuOutcome;

/// Configure semantic search (enable/disable + optional model download).
///
/// Shows a simple menu:
/// 1. Enable & download model now (~420 MB, with progress)
/// 2. Disable semantic search
///
/// Returns MenuOutcome::Done or MenuOutcome::Back.
pub fn configure_semantic_search(config: &mut Config, can_go_back: bool) -> Result<MenuOutcome> {
    println!();
    println!("{}", colorize("Semantic Search Configuration", BOLD));
    println!("{}", colorize(&"━".repeat(54), DIM));
    println!();
    println!("Semantic search finds related concepts, not just keywords:");
    println!("  {} Find \"login bug\" when you search \"auth issue\"", colorize("•", BRAND));
    println!("  {} Discover all security work in one query", colorize("•", BRAND));
    println!("  {} 50+ languages supported", colorize("•", BRAND));
    println!();
    println!("{}", colorize("Requirements:", DIM));
    println!("  {} ~420 MB model download (one-time)", colorize("•", DIM));
    println!("  {} ~1.5 MB per 1000 tasks (embeddings)", colorize("•", DIM));
    println!();

    // Show current status if already configured
    if config.todo.search.enabled {
        println!("{} Currently: {}", colorize("ℹ", BRAND), colorize("Enabled", SUCCESS));
        println!();
    }

    // Simple 2-choice menu
    println!("{}", colorize("Choose an option:", BOLD));
    println!("  {} Enable & download model now (~60 sec)", colorize("1)", BRAND));
    println!("  {} Disable semantic search", colorize("2)", DIM));
    if can_go_back {
        println!("  {} Go back", colorize("b)", DIM));
    }
    println!();

    loop {
        print!("{} ", colorize("Choice [1]:", BRAND));
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "" | "1" => {
                // Enable & download now
                config.todo.search.enabled = true;

                println!();
                println!("{} Downloading embedding model...", colorize("↓", BRAND));

                // Download model with progress feedback
                match download_embedding_model() {
                    Ok(()) => {
                        println!("{} Model downloaded and ready!", colorize("✓", SUCCESS));
                        println!();
                        println!("Try it: {} todo search --semantic \"your query\"", colorize("klyntbot", BRAND));
                        println!();
                        return Ok(MenuOutcome::Done);
                    }
                    Err(e) => {
                        println!("{} Download failed: {}", colorize("✗", ERROR), e);
                        println!();
                        println!("You can retry later with: klyntbot todo search --semantic");
                        println!("(Model will download on first search)");
                        println!();

                        // Still enable semantic search (will download on first use)
                        return Ok(MenuOutcome::Done);
                    }
                }
            }
            "2" => {
                // Disable
                config.todo.search.enabled = false;
                println!();
                println!("{} Semantic search disabled", colorize("✓", DIM));
                println!();
                println!("You can enable it later in ~/.klyntbot/config.json");
                println!();
                return Ok(MenuOutcome::Done);
            }
            "b" | "back" if can_go_back => {
                return Ok(MenuOutcome::Back);
            }
            _ => {
                println!("{} Invalid choice. Try again.", colorize("✗", ERROR));
            }
        }
    }
}

/// Download the embedding model with progress feedback.
///
/// Uses fastembed's built-in download progress display.
fn download_embedding_model() -> Result<()> {
    use tools::EmbeddingEngine;

    // Initialize engine (triggers model download with built-in progress bar)
    let engine = Arc::new(EmbeddingEngine::new());

    // Test that it works by embedding a small string
    let test_text = "test";
    engine.embed(test_text).map_err(|e| {
        anyhow::anyhow!("Model initialization failed: {}", e)
    })?;

    Ok(())
}
