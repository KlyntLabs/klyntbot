//! Task completion and deletion commands.

use anyhow::Result;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{TodoPatch, TodoStatus};

pub async fn handle_complete(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let patch = TodoPatch {
        status: Some(TodoStatus::Done),
        ..Default::default()
    };

    let result = store.update(id, patch).await?;

    match result {
        Some(todo) => {
            println!(
                "{} Completed: {} — {}",
                status_success(),
                colorize(&todo.id, TOOL),
                todo.title
            );
            Ok(())
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
            Ok(())
        }
    }
}

pub async fn handle_delete(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let deleted = store.delete(id).await?;

    if deleted {
        println!("{} Deleted task: {}", status_success(), colorize(id, TOOL));
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}
