//! Task listing, show, and summary commands.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::TodoFilter;

use super::{render_priority, show_task_details};

pub async fn handle_list(
    status: Option<String>,
    tag: Option<String>,
    priority_min: Option<u8>,
    limit: Option<usize>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Parse status filter
    let status_filter = status.as_ref().and_then(|s| {
        use tools::todo_types::TodoStatus;
        match s.to_lowercase().as_str() {
            "todo" => Some(TodoStatus::Todo),
            "doing" => Some(TodoStatus::Doing),
            "done" => Some(TodoStatus::Done),
            "archived" => Some(TodoStatus::Archived),
            _ => None,
        }
    });

    let filter = TodoFilter {
        status: status_filter,
        tag: tag.clone(),
        priority_min,
        limit,
        project_id: None, // Phase 2
        parent_id: None,  // Phase 2
    };

    let todos = store.list(&filter).await?;

    if todos.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    // Count header with filter description
    let header = if status.is_some() || tag.is_some() || priority_min.is_some() {
        let mut filters = Vec::new();
        if let Some(s) = &status {
            filters.push(format!("status={}", s));
        }
        if let Some(t) = &tag {
            filters.push(format!("tag={}", t));
        }
        if let Some(p) = priority_min {
            filters.push(format!("priority≥{}", p));
        }
        format!("{} tasks matching: {}", todos.len(), filters.join(", "))
    } else {
        format!("{} tasks:", todos.len())
    };
    println!("{}", header);
    println!();

    // Render list
    for todo in todos {
        print!("{} ", colorize(&todo.id, TOOL));

        // Focus indicator
        if todo.focused_at.is_some() {
            print!("{} ", colorize("●", BRAND));
        } else {
            print!("  ");
        }

        print!("{}", colorize(&todo.title, BOLD));

        if let Some(pri) = todo.priority {
            print!("  {}", render_priority(pri));
        }

        if !todo.tags.is_empty() {
            print!("  {}", colorize(&todo.tags.join(", "), DIM));
        }

        if let Some(due) = &todo.due_date {
            let now = Utc::now();
            if *due < now {
                print!("  {}", colorize("OVERDUE", ERROR));
            } else {
                print!("  {}", colorize(&due.format("%b %d").to_string(), DIM));
            }
        }

        println!();
    }

    Ok(())
}

pub async fn handle_show(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let todo = store.get(id).await?;

    match todo {
        Some(t) => {
            show_task_details(&t);
            Ok(())
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
            Ok(())
        }
    }
}

pub async fn handle_summary() -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let summary = store.summary().await?;

    println!("{}", colorize("Todo Summary", BRAND));
    println!();

    println!("Total tasks: {}", summary.total);
    println!();

    // By status
    println!("By Status:");
    for (status, count) in &summary.by_status {
        println!("  {:?}: {}", status, count);
    }
    println!();

    // Overdue
    if !summary.overdue.is_empty() {
        println!(
            "{} {} overdue tasks:",
            status_warning(),
            summary.overdue.len()
        );
        for id in &summary.overdue {
            println!("  {}", colorize(id, TOOL));
        }
        println!();
    }

    // Upcoming
    if !summary.upcoming_week.is_empty() {
        println!("Due this week: {}", summary.upcoming_week.len());
        for id in &summary.upcoming_week {
            println!("  {}", colorize(id, TOOL));
        }
    }

    Ok(())
}
