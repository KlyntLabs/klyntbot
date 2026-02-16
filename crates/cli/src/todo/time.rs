//! Time tracking command.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{TimeEntry, TimeEntrySource, Todo};

pub async fn handle_log_time(id: String, minutes: u32, note: Option<String>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Verify task exists first
    let todo = store.get(&id).await?;
    if todo.is_none() {
        println!("{} Task not found: {}", status_error(), id);
        return Ok(());
    }

    let duration_secs = (minutes as u64) * 60;
    let now = Utc::now();
    let started_at = now - chrono::Duration::seconds(duration_secs as i64);

    let entry = TimeEntry {
        id: Todo::generate_id(),
        started_at,
        ended_at: Some(now),
        duration_secs: Some(duration_secs),
        note: note.clone(),
        source: TimeEntrySource::Manual,
    };

    let added = store.add_time_entry(&id, entry).await?;

    if added {
        let todo = store
            .get(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Task {} was deleted", id))?;
        // Compute total from entries since add_time_entry doesn't update the denormalized field
        let total_secs: u64 = todo
            .time_entries
            .iter()
            .filter_map(|e| e.duration_secs)
            .sum();
        let total_mins = total_secs / 60;
        println!(
            "{} Logged {}m on {} — {}",
            status_success(),
            minutes,
            colorize(&todo.id, TOOL),
            colorize(&todo.title, BOLD),
        );
        if let Some(n) = &note {
            println!("  Note: {}", colorize(n, DIM));
        }
        println!("  Total tracked: {}m", total_mins);
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}
