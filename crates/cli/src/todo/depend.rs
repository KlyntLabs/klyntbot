//! Task dependency management command.

use anyhow::Result;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;

pub async fn handle_depend(id: String, on: Option<String>, remove: Option<String>) -> Result<()> {
    // Validate exactly one of --on or --remove
    match (&on, &remove) {
        (None, None) => {
            // Show current dependencies
            let config = config::load().await?;
            let store_path = config.todo_store_path();
            let mut store = TodoStore::new(store_path);

            let todo = match store.get(&id).await? {
                Some(t) => t,
                None => {
                    println!("{} Task not found: {}", status_error(), id);
                    return Ok(());
                }
            };

            println!(
                "Dependencies for {} — {}",
                colorize(&todo.id, TOOL),
                colorize(&todo.title, BOLD),
            );

            if todo.blocked_by.is_empty() {
                println!("  Blocked by: {}", colorize("(none)", DIM));
            } else {
                println!("  Blocked by:");
                for dep_id in &todo.blocked_by {
                    let dep_title = store
                        .get(dep_id)
                        .await?
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "(unknown)".to_string());
                    println!("    ⛔ {} — {}", colorize(dep_id, TOOL), dep_title,);
                }
            }

            if todo.blocks.is_empty() {
                println!("  Blocks:     {}", colorize("(none)", DIM));
            } else {
                println!("  Blocks:");
                for dep_id in &todo.blocks {
                    let dep_title = store
                        .get(dep_id)
                        .await?
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "(unknown)".to_string());
                    println!("    → {} — {}", colorize(dep_id, TOOL), dep_title,);
                }
            }

            return Ok(());
        }
        (Some(_), Some(_)) => {
            println!(
                "{} Specify either --on or --remove, not both.",
                status_error()
            );
            return Ok(());
        }
        _ => {} // Exactly one provided — continue
    }

    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    if let Some(blocker_id) = on {
        match store.add_dependency(&id, &blocker_id).await {
            Ok(()) => {
                let task = store
                    .get(&id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Task {} was deleted", id))?;
                let blocker = store
                    .get(&blocker_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Blocker task {} was deleted", blocker_id))?;
                println!(
                    "{} {} is now blocked by {}",
                    status_success(),
                    colorize(&format!("[{}] {}", task.id, task.title), BOLD),
                    colorize(&format!("[{}] {}", blocker.id, blocker.title), TOOL),
                );
            }
            Err(e) => {
                println!("{} {}", status_error(), e);
            }
        }
    } else if let Some(blocker_id) = remove {
        store.remove_dependency(&id, &blocker_id).await?;

        println!(
            "{} {} is no longer blocked by {}",
            status_success(),
            colorize(&id, BOLD),
            colorize(&blocker_id, TOOL),
        );
    }

    Ok(())
}
