//! Project tasks command (list/tree tasks by project).

use anyhow::Result;
use common::utils::terminal::*;
use tools::project_store::ProjectStore;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoFilter, TodoStatus};

pub async fn handle_tasks(id: String, limit: Option<usize>, tree: bool) -> Result<()> {
    let config = config::load().await?;
    let mut project_store = ProjectStore::new(config.project_store_path());
    let mut todo_store = TodoStore::new(config.todo_store_path());

    // Verify project exists
    let project = project_store.get(&id).await?;
    let project = match project {
        Some(p) => p,
        None => {
            println!("{} Project not found: {}", status_error(), id);
            return Ok(());
        }
    };

    let filter = TodoFilter {
        project_id: Some(id.clone()),
        limit,
        ..Default::default()
    };

    let todos = todo_store.list(&filter).await?;

    if todos.is_empty() {
        println!(
            "No tasks in project {} — {}",
            colorize(&project.id, TOOL),
            colorize(&project.name, BOLD),
        );
        return Ok(());
    }

    println!(
        "{} — {} ({} tasks)",
        colorize(&project.name, BOLD),
        colorize(&project.id, TOOL),
        todos.len(),
    );
    println!();

    if tree {
        render_tree(&todos, &config.timezone);
    } else {
        render_list(&todos, &config.timezone);
    }

    Ok(())
}

/// Render tasks as a compact list.
fn render_list(todos: &[Todo], timezone: &str) {
    for todo in todos {
        let status_icon = match todo.status {
            TodoStatus::Done => colorize("✓", SUCCESS),
            TodoStatus::Doing => colorize("●", BRAND),
            TodoStatus::Todo => colorize("○", DIM),
            TodoStatus::Archived => colorize("▪", DIM),
        };

        print!(
            "  {} {} {}",
            status_icon,
            colorize(&todo.title, BOLD),
            colorize(&format!("[{}]", todo.id), TOOL),
        );

        if let Some(pri) = todo.priority {
            print!("  P{}", pri);
        }

        if let Some(due) = &todo.due_date {
            let now = chrono::Utc::now();
            if *due < now {
                print!("  {}", colorize("OVERDUE", ERROR));
            } else {
                let formatted = common::utils::date::format_datetime_local(due, timezone, "%b %d");
                print!("  {}", colorize(&formatted, DIM));
            }
        }

        println!();
    }
}

/// Render tasks as a tree (reuses tree logic inline).
fn render_tree(todos: &[Todo], timezone: &str) {
    let todo_ids: std::collections::HashSet<&str> = todos.iter().map(|t| t.id.as_str()).collect();

    let roots: Vec<&Todo> = todos
        .iter()
        .filter(|t| match &t.parent_id {
            None => true,
            Some(pid) => !todo_ids.contains(pid.as_str()),
        })
        .collect();

    for (i, root) in roots.iter().enumerate() {
        let is_last = i == roots.len() - 1;
        print_tree_node(root, todos, "", is_last, timezone);
    }
}

fn print_tree_node(todo: &Todo, all_todos: &[Todo], prefix: &str, is_last: bool, timezone: &str) {
    let connector = if is_last { "└─ " } else { "├─ " };

    let status_icon = match todo.status {
        TodoStatus::Done => colorize("✓", SUCCESS),
        TodoStatus::Doing => colorize("●", BRAND),
        TodoStatus::Todo => colorize("○", DIM),
        TodoStatus::Archived => colorize("▪", DIM),
    };

    print!(
        "{}{}{} {} {}",
        colorize(prefix, DIM),
        colorize(connector, DIM),
        status_icon,
        colorize(&todo.title, BOLD),
        colorize(&format!("[{}]", todo.id), TOOL),
    );

    if let Some(pri) = todo.priority {
        print!("  P{}", pri);
    }

    if let Some(due) = &todo.due_date {
        let now = chrono::Utc::now();
        if *due < now {
            print!("  {}", colorize("OVERDUE", ERROR));
        } else {
            let formatted = common::utils::date::format_datetime_local(due, timezone, "%b %d");
            print!("  {}", colorize(&formatted, DIM));
        }
    }

    println!();

    let children: Vec<&Todo> = all_todos
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
        .collect();

    let child_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });

    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        print_tree_node(child, all_todos, &child_prefix, is_last_child, timezone);
    }
}
