//! Task move command (reparent/reproject).

use anyhow::Result;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;

pub async fn handle_move(
    id: String,
    parent: Option<String>,
    project: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    if parent.is_none() && project.is_none() {
        println!(
            "{} Specify --parent ID or --project ID to move task. Use \"none\" to unset.",
            status_warning()
        );
        return Ok(());
    }

    // Resolve "none" keyword to unset
    let new_parent = parent.as_ref().map(|p| {
        if p.to_lowercase() == "none" {
            None
        } else {
            Some(p.clone())
        }
    });

    let new_project = project.as_ref().map(|p| {
        if p.to_lowercase() == "none" {
            None
        } else {
            Some(p.clone())
        }
    });

    // Flatten: Option<Option<String>> -> pass inner to move_todo
    // move_todo expects Option<String> for each, where None means "don't change"
    // We need to distinguish "don't change" from "set to None"
    // Since move_todo always sets the value, we need to get current values for unchanged fields
    let todo = store.get(&id).await?;
    let todo = match todo {
        Some(t) => t,
        None => {
            println!("{} Task not found: {}", status_error(), id);
            return Ok(());
        }
    };

    let final_parent = match new_parent {
        Some(val) => val,       // User specified: either Some(id) or None (cleared)
        None => todo.parent_id, // User didn't specify: keep current
    };

    let final_project = match new_project {
        Some(val) => val,        // User specified: either Some(id) or None (cleared)
        None => todo.project_id, // User didn't specify: keep current
    };

    let result = store
        .move_todo(&id, final_parent.clone(), final_project.clone())
        .await?;

    match result {
        Some(todo) => {
            let mut changes = Vec::new();
            if parent.is_some() {
                match &final_parent {
                    Some(pid) => changes.push(format!("parent → {}", pid)),
                    None => changes.push("parent → removed".to_string()),
                }
            }
            if project.is_some() {
                match &final_project {
                    Some(pid) => changes.push(format!("project → {}", pid)),
                    None => changes.push("project → removed".to_string()),
                }
            }
            println!(
                "{} Moved {} — {} ({})",
                status_success(),
                colorize(&todo.id, TOOL),
                colorize(&todo.title, BOLD),
                changes.join(", ")
            );
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
        }
    }

    Ok(())
}
