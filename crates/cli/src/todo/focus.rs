//! Focus mode commands and rendering.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::Todo;

use super::render_priority;

pub async fn handle_focus(id: Option<String>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    if let Some(task_id) = id {
        // Focus on a specific task
        let focused = store
            .focus(
                &task_id,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
            )
            .await?;

        if focused {
            if let Some(todo) = store.get(&task_id).await? {
                println!(
                    "{} Focused: {} — {}",
                    status_success(),
                    colorize(&todo.id, TOOL),
                    todo.title
                );
            }
            Ok(())
        } else {
            println!("{} Task not found or focus slots full", status_error());
            Ok(())
        }
    } else {
        // Show focus board
        let focused = store.focused().await?;

        if focused.is_empty() {
            println!("No tasks in focus.");
            println!();
            println!("Focus on a task: klyntbot todo focus <id>");
            return Ok(());
        }

        println!("{}", colorize("Focus Board", BRAND));
        println!();

        for todo in focused {
            show_focused_task(&todo, config.todo.focus.deadline_hours);
            println!();
        }

        Ok(())
    }
}

pub async fn handle_unfocus(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let unfocused = store.unfocus(id).await?;

    if unfocused {
        println!("{} Unfocused: {}", status_success(), colorize(id, TOOL));
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}

/// Show focused task in focus board
fn show_focused_task(todo: &Todo, deadline_hours: u64) {
    println!(
        "{}",
        colorize(
            "┌─ ● Focused ──────────────────────────────────────────┐",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );
    println!(
        "{}  {}  {}{}",
        colorize("│", BRAND),
        colorize(&todo.id, TOOL),
        colorize(&todo.title, BOLD),
        colorize("  │", BRAND)
    );

    // Priority, tags, due
    print!("{}  ", colorize("│", BRAND));
    if let Some(pri) = todo.priority {
        print!("{} ", render_priority(pri));
    }
    if !todo.tags.is_empty() {
        print!(" · {} ", colorize(&todo.tags.join(", "), DIM));
    }
    if let Some(due) = &todo.due_date {
        print!(
            " · Due: {}",
            colorize(&due.format("%b %d").to_string(), DIM)
        );
    }
    println!("{}", colorize("  │", BRAND));

    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );

    // Focus timer
    if let (Some(focused_at), Some(deadline)) = (&todo.focused_at, &todo.focus_deadline) {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(*focused_at);
        let remaining = deadline.signed_duration_since(now);

        let total_secs = deadline_hours * 3600;
        let elapsed_secs = elapsed.num_seconds();
        let progress = (elapsed_secs as f32 / total_secs as f32).min(1.0);

        let bar_width = 20;
        let filled = (progress * bar_width as f32) as usize;
        let empty = bar_width - filled;

        let color = if remaining.num_hours() > 6 {
            BRAND
        } else if remaining.num_hours() > 1 {
            WARNING
        } else {
            ERROR
        };

        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        println!(
            "{}  Time: {} {}{}",
            colorize("│", BRAND),
            colorize(&bar, color),
            if remaining.num_seconds() > 0 {
                format!(
                    "{}h {}m remaining",
                    remaining.num_hours(),
                    remaining.num_minutes() % 60
                )
            } else {
                colorize("EXPIRED", ERROR)
            },
            colorize("  │", BRAND)
        );
    }

    // Description
    if let Some(desc) = &todo.description {
        println!(
            "{}",
            colorize(
                "│                                                       │",
                BRAND
            )
        );
        println!(
            "{}  Description:{}",
            colorize("│", BRAND),
            colorize("  │", BRAND)
        );
        println!(
            "{}  {}{}",
            colorize("│", BRAND),
            desc,
            colorize("  │", BRAND)
        );
    }

    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "└───────────────────────────────────────────────────────┘",
            BRAND
        )
    );
}
