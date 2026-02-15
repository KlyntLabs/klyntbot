//! Task enrichment command - auto-infer priority, duration, and due date.

use anyhow::Result;
use common::utils::terminal::*;
use tools::enrichment::EnrichmentHandler;
use tools::todo_store::TodoStore;
use tools::todo_types::TodoPatch;

use agent::EnrichmentEngine;

pub async fn handle_enrich(id: String) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Fetch the task
    let todo = match store.get(&id).await? {
        Some(t) => t,
        None => {
            println!("{} Task not found: {}", status_error(), id);
            return Ok(());
        }
    };

    println!(
        "\n{} Analyzing task: {}",
        colorize("📊", BRAND),
        colorize(&todo.title, BOLD)
    );
    println!();

    // Create enrichment engine
    let enrichment_config = config.todo.enrichment.clone();
    let engine = EnrichmentEngine::new(enrichment_config);

    // Run enrichment
    let result = engine.enrich_task(&todo).await?;

    match result {
        None => {
            println!(
                "{} No enrichment suggestions (task already has all fields set or enrichment disabled)",
                colorize("ℹ", DIM)
            );
            return Ok(());
        }
        Some(enrichment) => {
            let mut has_suggestions = false;

            // Show priority suggestion
            if let Some(ref priority_suggestion) = enrichment.priority {
                has_suggestions = true;
                println!(
                    "{}",
                    colorize(
                        "┌─ Priority Suggestion ───────────────────────────────┐",
                        BRAND
                    )
                );
                println!(
                    "{}  Current:  {}",
                    colorize("│", BRAND),
                    colorize(
                        &todo
                            .priority
                            .map(|p| format!("P{}", p))
                            .unwrap_or_else(|| "None".to_string()),
                        DIM
                    )
                );
                println!(
                    "{}  Suggested: {} {}",
                    colorize("│", BRAND),
                    colorize(&format!("P{}", priority_suggestion.value), SUCCESS),
                    colorize(
                        &format!(
                            "(confidence: {:.0}%)",
                            priority_suggestion.confidence * 100.0
                        ),
                        DIM
                    )
                );
                println!(
                    "{}  Reasoning: {}",
                    colorize("│", BRAND),
                    colorize(&priority_suggestion.reasoning, DIM)
                );
                println!(
                    "{}",
                    colorize(
                        "└─────────────────────────────────────────────────────┘",
                        BRAND
                    )
                );
                println!();
            }

            // Show duration suggestion
            if let Some(ref duration_suggestion) = enrichment.estimated_minutes {
                has_suggestions = true;
                println!(
                    "{}",
                    colorize(
                        "┌─ Duration Estimate ─────────────────────────────────┐",
                        BRAND
                    )
                );
                println!(
                    "{}  Current:  {}",
                    colorize("│", BRAND),
                    colorize(
                        &todo
                            .estimated_minutes
                            .map(|m| format!("{} minutes", m))
                            .unwrap_or_else(|| "None".to_string()),
                        DIM
                    )
                );
                println!(
                    "{}  Suggested: {} {}",
                    colorize("│", BRAND),
                    colorize(&format!("{} minutes", duration_suggestion.value), SUCCESS),
                    colorize(
                        &format!(
                            "(confidence: {:.0}%)",
                            duration_suggestion.confidence * 100.0
                        ),
                        DIM
                    )
                );
                println!(
                    "{}  Reasoning: {}",
                    colorize("│", BRAND),
                    colorize(&duration_suggestion.reasoning, DIM)
                );
                println!(
                    "{}",
                    colorize(
                        "└─────────────────────────────────────────────────────┘",
                        BRAND
                    )
                );
                println!();
            }

            // Show due date suggestion
            if let Some(ref due_suggestion) = enrichment.due_date {
                has_suggestions = true;
                let formatted = super::format_date_local(
                    &due_suggestion.value,
                    &config.timezone,
                    "%Y-%m-%d %H:%M",
                );
                println!(
                    "{}",
                    colorize(
                        "┌─ Due Date Suggestion ───────────────────────────────┐",
                        BRAND
                    )
                );
                println!(
                    "{}  Current:  {}",
                    colorize("│", BRAND),
                    colorize(
                        &todo
                            .due_date
                            .map(|d| super::format_date_local(
                                &d,
                                &config.timezone,
                                "%Y-%m-%d %H:%M"
                            ))
                            .unwrap_or_else(|| "None".to_string()),
                        DIM
                    )
                );
                println!(
                    "{}  Suggested: {} {}",
                    colorize("│", BRAND),
                    colorize(&formatted, SUCCESS),
                    colorize(
                        &format!("(confidence: {:.0}%)", due_suggestion.confidence * 100.0),
                        DIM
                    )
                );
                println!(
                    "{}  Reasoning: {}",
                    colorize("│", BRAND),
                    colorize(&due_suggestion.reasoning, DIM)
                );
                println!(
                    "{}",
                    colorize(
                        "└─────────────────────────────────────────────────────┘",
                        BRAND
                    )
                );
                println!();
            }

            if !has_suggestions {
                println!("{} No enrichment suggestions available", colorize("ℹ", DIM));
                return Ok(());
            }

            // Apply updates via TodoPatch
            let mut patch = TodoPatch::default();

            if let Some(ref p) = enrichment.priority {
                patch.priority = Some(p.value);
            }

            if let Some(ref d) = enrichment.estimated_minutes {
                patch.estimated_minutes = Some(Some(d.value));
            }

            if let Some(ref d) = enrichment.due_date {
                patch.due_date = Some(Some(d.value));
            }

            store.update(&id, patch).await?;

            println!(
                "{} Applied enrichment suggestions to task {}",
                status_success(),
                colorize(&id, TOOL)
            );
        }
    }

    Ok(())
}
