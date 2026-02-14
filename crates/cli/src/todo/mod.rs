//! Todo task management CLI commands.
//!
//! Implements all todo subcommands including task creation,
//! enrichment flow, focus mode, and dashboard rendering.

mod add;
mod complete;
mod focus;
mod list;

use anyhow::Result;
use common::utils::terminal::*;
use tools::todo_types::Todo;

use crate::commands::TodoCommands;

/// Handle todo subcommands.
///
/// Routes to the appropriate handler based on the subcommand variant.
pub async fn handle_todo(cmd: TodoCommands) -> Result<()> {
    match cmd {
        TodoCommands::Add {
            title,
            description,
            priority,
            due,
            tags,
        } => add::handle_add(title, description, priority, due, tags).await,
        TodoCommands::List {
            status,
            tag,
            priority_min,
            limit,
        } => list::handle_list(status, tag, priority_min, limit).await,
        TodoCommands::Show { id } => list::handle_show(&id).await,
        TodoCommands::Complete { id } => complete::handle_complete(&id).await,
        TodoCommands::Delete { id } => complete::handle_delete(&id).await,
        TodoCommands::Focus { id } => focus::handle_focus(id).await,
        TodoCommands::Unfocus { id } => focus::handle_unfocus(&id).await,
        TodoCommands::Summary => list::handle_summary().await,
    }
}

/// Render priority badge (shared across submodules)
fn render_priority(priority: u8) -> String {
    let (color, blocks) = match priority {
        5 => (ERROR, "■■■■■"),
        4 => (ERROR, "■■■■○"),
        3 => (WARNING, "■■■○○"),
        2 => (DIM, "■■○○○"),
        1 => (DIM, "■○○○○"),
        _ => (DIM, "○○○○○"),
    };

    let badge = format!("{} P{}", blocks, priority);
    colorize(&badge, color)
}

/// Parse a due date string (YYYY-MM-DD) to DateTime<Utc>
fn parse_due_date(date_str: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    use chrono::{NaiveDate, Utc};

    let naive = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let naive_datetime = naive
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid date"))?;

    Ok(chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        naive_datetime,
        Utc,
    ))
}

/// Show detailed task view (used by show and list)
fn show_task_details(todo: &Todo) {
    println!(
        "{}",
        colorize(
            "┌─ Task ─────────────────────────────────────────────────┐",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize(&todo.title, BOLD),
        colorize("  │", BRAND)
    );
    println!(
        "{}  {}  ·  Status: {:?}  ·  Created: {}{}",
        colorize("│", BRAND),
        colorize(&todo.id, TOOL),
        todo.status,
        todo.created_at.format("%b %d"),
        colorize("  │", BRAND)
    );
    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );

    // Description
    if let Some(desc) = &todo.description {
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
        println!(
            "{}",
            colorize(
                "│                                                         │",
                BRAND
            )
        );
    }

    // Priority
    if let Some(pri) = todo.priority {
        println!(
            "{}  Priority:    {}{}",
            colorize("│", BRAND),
            render_priority(pri),
            colorize("  │", BRAND)
        );
    }

    // Due date
    if let Some(due) = &todo.due_date {
        let now = chrono::Utc::now();
        let due_str = if *due < now {
            colorize(&format!("{} (OVERDUE)", due.format("%Y-%m-%d")), ERROR)
        } else {
            colorize(&due.format("%Y-%m-%d").to_string(), DIM)
        };
        println!(
            "{}  Due:         {}{}",
            colorize("│", BRAND),
            due_str,
            colorize("  │", BRAND)
        );
    }

    // Tags
    if !todo.tags.is_empty() {
        println!(
            "{}  Tags:        {}{}",
            colorize("│", BRAND),
            colorize(&todo.tags.join(", "), DIM),
            colorize("  │", BRAND)
        );
    }

    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "└─────────────────────────────────────────────────────────┘",
            BRAND
        )
    );
}
