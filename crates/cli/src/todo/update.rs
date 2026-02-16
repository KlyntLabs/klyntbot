//! Task update command.

use anyhow::Result;
use common::utils::terminal::*;
use std::sync::Arc;
use tools::todo_store::TodoStore;
use tools::todo_types::TodoPatch;
use tools::{EmbeddingEngine, EmbeddingEngineImpl, EmbeddingHandler, EmbeddingStore};

use super::{format_date_local, parse_due_date, render_priority};

pub async fn handle_update(
    id: String,
    title: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    due: Option<String>,
    tags: Option<String>,
    status: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Parse status string to enum
    let status_parsed = status.as_ref().and_then(|s| {
        use tools::todo_types::TodoStatus;
        match s.to_lowercase().as_str() {
            "todo" => Some(TodoStatus::Todo),
            "doing" => Some(TodoStatus::Doing),
            "done" => Some(TodoStatus::Done),
            "archived" => Some(TodoStatus::Archived),
            _ => None,
        }
    });

    // Warn on invalid status string
    if let Some(ref s) = status {
        if status_parsed.is_none() {
            println!(
                "{} Invalid status: {}. Use: todo, doing, done, archived",
                status_error(),
                s
            );
            return Ok(());
        }
    }

    // Parse due date (supports natural language via parse_due_date)
    let due_parsed = due
        .as_ref()
        .map(|d| {
            if d.is_empty() || d.to_lowercase() == "none" {
                Ok(None)
            } else {
                parse_due_date(d, &config.timezone).map(Some)
            }
        })
        .transpose()?;

    // Build patch from provided flags
    let patch = TodoPatch {
        title: title.clone(),
        description: description.clone().map(|d| {
            if d.is_empty() || d.to_lowercase() == "none" {
                None
            } else {
                Some(d)
            }
        }),
        priority,
        due_date: due_parsed,
        tags: tags
            .as_ref()
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect()),
        status: status_parsed,
        ..Default::default()
    };

    // Check that at least one field was specified
    if patch.title.is_none()
        && patch.description.is_none()
        && patch.priority.is_none()
        && patch.due_date.is_none()
        && patch.tags.is_none()
        && patch.status.is_none()
    {
        println!(
            "{} No fields specified. Use --title, --description, --priority, --due, --tags, or --status.",
            status_warning()
        );
        return Ok(());
    }

    // Fetch old values before updating (for old → new diff display)
    let old_todo = store.get(&id).await?;
    let old_todo = match old_todo {
        Some(t) => t,
        None => {
            println!("{} Task not found: {}", status_error(), id);
            return Ok(());
        }
    };

    // Apply the update
    let result = store.update(&id, patch).await?;

    match result {
        Some(new_todo) => {
            // Auto-generate embedding if text fields changed (Sprint 5)
            if title.is_some() || description.is_some() || tags.is_some() {
                let emb_store_path = config.embedding_store_path();
                let emb_store = Arc::new(tokio::sync::RwLock::new(EmbeddingStore::new(emb_store_path)));
                let engine = Arc::new(EmbeddingEngine::new());
                let handler = EmbeddingEngineImpl::new(engine, emb_store);

                // Best-effort embedding generation
                if let Err(e) = handler.embed_todo(&new_todo).await {
                    tracing::warn!("Failed to generate embedding for todo {}: {}", new_todo.id, e);
                }
            }

            println!(
                "{} Updated {}:",
                status_success(),
                colorize(&new_todo.id, TOOL),
            );

            // Show old → new for each changed field
            if let Some(ref new_title) = title {
                if *new_title != old_todo.title {
                    println!(
                        "  Title:       {} {} {}",
                        old_todo.title,
                        colorize("→", DIM),
                        colorize(new_title, BOLD),
                    );
                }
            }

            if description.is_some() {
                let old_desc = old_todo.description.as_deref().unwrap_or("(none)");
                let new_desc = new_todo.description.as_deref().unwrap_or("(none)");
                if old_desc != new_desc {
                    println!(
                        "  Description: {} {} {}",
                        old_desc,
                        colorize("→", DIM),
                        new_desc,
                    );
                }
            }

            if priority.is_some() {
                let old_pri = old_todo.priority.unwrap_or(0);
                let new_pri = new_todo.priority.unwrap_or(0);
                if old_pri != new_pri {
                    println!(
                        "  Priority:    {} {} {}",
                        render_priority(old_pri),
                        colorize("→", DIM),
                        render_priority(new_pri),
                    );
                }
            }

            if due.is_some() {
                let old_due = old_todo
                    .due_date
                    .as_ref()
                    .map(|d| format_date_local(d, &config.timezone, "%b %d"))
                    .unwrap_or_else(|| "(none)".to_string());
                let new_due = new_todo
                    .due_date
                    .as_ref()
                    .map(|d| format_date_local(d, &config.timezone, "%b %d"))
                    .unwrap_or_else(|| "(none)".to_string());
                if old_due != new_due {
                    println!(
                        "  Due:         {} {} {}",
                        old_due,
                        colorize("→", DIM),
                        new_due,
                    );
                }
            }

            if tags.is_some() {
                let old_tags = if old_todo.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    old_todo.tags.join(", ")
                };
                let new_tags = if new_todo.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    new_todo.tags.join(", ")
                };
                if old_tags != new_tags {
                    println!(
                        "  Tags:        {} {} {}",
                        colorize(&old_tags, DIM),
                        colorize("→", DIM),
                        colorize(&new_tags, DIM),
                    );
                }
            }

            if status.is_some() {
                let old_status = format!("{:?}", old_todo.status);
                let new_status = format!("{:?}", new_todo.status);
                if old_status != new_status {
                    println!(
                        "  Status:      {} {} {}",
                        old_status,
                        colorize("→", DIM),
                        new_status,
                    );
                }
            }
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
        }
    }

    Ok(())
}
