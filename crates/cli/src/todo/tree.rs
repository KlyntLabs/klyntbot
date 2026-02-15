//! Hierarchical tree view of tasks.

use anyhow::Result;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoFilter};

use super::render_priority;

pub async fn handle_tree(project_id: Option<String>, depth: Option<usize>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let filter = TodoFilter {
        project_id,
        ..Default::default()
    };

    let todos = store.list(&filter).await?;

    if todos.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    // Find root tasks (no parent, or parent not in filtered set)
    let todo_ids: std::collections::HashSet<&str> = todos.iter().map(|t| t.id.as_str()).collect();
    let roots: Vec<&Todo> = todos
        .iter()
        .filter(|t| match &t.parent_id {
            None => true,
            Some(pid) => !todo_ids.contains(pid.as_str()),
        })
        .collect();

    println!("{}", colorize("Task Tree", BRAND));
    println!();

    let max_depth = depth.unwrap_or(usize::MAX);

    for (i, root) in roots.iter().enumerate() {
        let is_last = i == roots.len() - 1;
        print_tree_node(root, &todos, "", is_last, 0, max_depth, &config.timezone);
    }

    Ok(())
}

fn print_tree_node(
    todo: &Todo,
    all_todos: &[Todo],
    prefix: &str,
    is_last: bool,
    current_depth: usize,
    max_depth: usize,
    timezone: &str,
) {
    let connector = if current_depth == 0 {
        if is_last {
            "└─ "
        } else {
            "├─ "
        }
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };

    // Status indicator
    let status_icon = match todo.status {
        tools::todo_types::TodoStatus::Done => colorize("✓", SUCCESS),
        tools::todo_types::TodoStatus::Doing => colorize("●", BRAND),
        tools::todo_types::TodoStatus::Todo => colorize("○", DIM),
        tools::todo_types::TodoStatus::Archived => colorize("▪", DIM),
    };

    // Build the line
    print!(
        "{}{}{} {} {}",
        colorize(prefix, DIM),
        colorize(connector, DIM),
        status_icon,
        colorize(&todo.title, BOLD),
        colorize(&format!("[{}]", todo.id), TOOL),
    );

    // Priority
    if let Some(pri) = todo.priority {
        print!("  {}", render_priority(pri));
    }

    // Due date
    if let Some(due) = &todo.due_date {
        let now = chrono::Utc::now();
        if *due < now {
            print!("  {}", colorize("OVERDUE", ERROR));
        } else {
            let formatted = super::format_date_local(due, timezone, "%b %d");
            print!("  {}", colorize(&formatted, DIM));
        }
    }

    // Focus indicator
    if todo.focused_at.is_some() {
        print!("  {}", colorize("focused", BRAND));
    }

    println!();

    // Render children if within depth limit
    if current_depth >= max_depth {
        // Check if there are hidden children
        let child_count = all_todos
            .iter()
            .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
            .count();
        if child_count > 0 {
            let child_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });
            println!(
                "{}{}",
                colorize(&child_prefix, DIM),
                colorize(&format!("… {} more", child_count), DIM),
            );
        }
        return;
    }

    let children: Vec<&Todo> = all_todos
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
        .collect();

    let child_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });

    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        print_tree_node(
            child,
            all_todos,
            &child_prefix,
            is_last_child,
            current_depth + 1,
            max_depth,
            timezone,
        );
    }
}
