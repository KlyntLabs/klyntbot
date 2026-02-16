//! Conversation memory configuration wizard step.
//!
//! Provides a simple enable/disable menu for conversation embedding,
//! following a simplified pattern compared to the semantic search step.

use anyhow::Result;
use common::utils::terminal::*;
use config::Config;
use std::io::Write;

use crate::wizard::ui::MenuOutcome;

/// Configure conversation memory (simple enable/disable).
///
/// Simpler than semantic search since it reuses the same embedding model.
pub(crate) fn configure_conversation_memory(
    config: &mut Config,
    _can_go_back: bool,
) -> Result<MenuOutcome> {
    println!();
    println!("{}", colorize("Conversation Memory", BOLD));
    println!("{}", colorize(&"─".repeat(54), DIM));
    println!();
    println!("Enable conversation memory for contextual recall?");
    println!();
    println!("  {} Agent remembers past discussions across sessions", colorize("•", DIM));
    println!("  {} Semantic search finds relevant conversations", colorize("•", DIM));
    println!("  {} Reuses semantic search model (no extra download)", colorize("•", DIM));
    println!("  {} Privacy controls: exclude channels, purge history", colorize("•", DIM));
    println!();

    print!(
        "{} Enable conversation memory [Y/n]: ",
        colorize("→", BRAND)
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim().to_lowercase();

    if choice.is_empty() || choice == "y" || choice == "yes" {
        config.conversation.embedding.enabled = true;
        config.conversation.search.enabled = true;

        println!();
        println!("{} Conversation memory enabled", colorize("✓", SUCCESS));
        println!();
        println!("{}:", colorize("Privacy controls", BOLD));
        println!("  Exclude channels: ~/.klyntbot/config.json → conversation.embedding.excludeChannels");
        println!("  Purge history: klyntbot todo purge --filter <session|date|all>");
        println!();
        println!("{}:", colorize("Search your conversations", BOLD));
        println!("  The agent will automatically search past conversations when relevant");
        println!("  Or ask: \"What did we discuss about authentication?\"");
        println!();
    } else {
        config.conversation.embedding.enabled = false;
        config.conversation.search.enabled = false;

        println!();
        println!("{} Conversation memory disabled", colorize("✓", DIM));
        println!();
        println!("You can enable it later in ~/.klyntbot/config.json:");
        println!("  \"conversation\": {{ \"embedding\": {{ \"enabled\": true }} }}");
        println!();
    }

    Ok(MenuOutcome::Done)
}
