//! Task creation command.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoStatus};

use super::{parse_due_date, render_priority};

pub async fn handle_add(
    title: String,
    description: Option<String>,
    priority: Option<u8>,
    due: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    // Load config and create store
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Create initial todo from args
    let todo = Todo {
        id: Todo::generate_id(),
        title: title.clone(),
        description: description.clone(),
        priority,
        due_date: due
            .as_ref()
            .and_then(|d| parse_due_date(d, &config.timezone).ok()),
        tags: tags
            .as_ref()
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        // Phase 1 new fields (all default values)
        parent_id: None,
        project_id: None,
        attachments: Vec::new(),
        time_entries: Vec::new(),
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
    };

    // Save to store
    let saved_todo = store.add(todo.clone()).await?;

    // Show created task
    show_task_created_box(&saved_todo, &config.timezone);

    Ok(())
}

/// Show task created box with details
fn show_task_created_box(todo: &Todo, timezone: &str) {
    println!(
        "{}",
        colorize("┌─ ✓ Task Created ──────────────────────────────┐", SUCCESS)
    );
    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );
    println!(
        "{}  {}{}",
        colorize("│", SUCCESS),
        colorize(&todo.title, BOLD),
        colorize("  │", SUCCESS)
    );
    println!(
        "{}  ID: {}{}",
        colorize("│", SUCCESS),
        colorize(&todo.id, TOOL),
        colorize("                                      │", SUCCESS)
    );
    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );

    // Priority
    if let Some(pri) = todo.priority {
        println!(
            "{}  Priority:    {}{}",
            colorize("│", SUCCESS),
            render_priority(pri),
            colorize("                          │", SUCCESS)
        );
    }

    // Due date
    if let Some(due) = &todo.due_date {
        let formatted = super::format_date_local(due, timezone, "%Y-%m-%d");
        println!(
            "{}  Due:         {}{}",
            colorize("│", SUCCESS),
            formatted,
            colorize("                              │", SUCCESS)
        );
    }

    // Tags
    if !todo.tags.is_empty() {
        println!(
            "{}  Tags:        {}{}",
            colorize("│", SUCCESS),
            colorize(&todo.tags.join(", "), DIM),
            colorize("                    │", SUCCESS)
        );
    }

    // Description
    if let Some(desc) = &todo.description {
        println!(
            "{}  Description: {}{}",
            colorize("│", SUCCESS),
            &desc.chars().take(36).collect::<String>(),
            colorize("  │", SUCCESS)
        );
    }

    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );
    println!(
        "{}",
        colorize(
            "└─────────────────────────────────────────────────┘",
            SUCCESS
        )
    );
}
