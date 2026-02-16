//! Todo task management CLI commands.
//!
//! Implements all todo subcommands including task creation,
//! enrichment flow, focus mode, and dashboard rendering.

mod add;
mod attach;
mod backfill;
mod complete;
mod depend;
mod enrich;
mod focus;
mod list;
mod move_task;
mod recur;
mod report;
mod search;
mod time;
mod tree;
mod update;

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
        TodoCommands::Tree { project, depth } => tree::handle_tree(project, depth).await,
        TodoCommands::Search {
            query,
            include_attachments,
            semantic,
            hybrid,
            threshold,
            limit,
        } => {
            if semantic {
                search::handle_semantic_search(query, threshold, limit).await
            } else if hybrid {
                search::handle_hybrid_search(query, limit).await
            } else {
                search::handle_search(query, include_attachments).await
            }
        }
        TodoCommands::Update {
            id,
            title,
            description,
            priority,
            due,
            tags,
            status,
        } => update::handle_update(id, title, description, priority, due, tags, status).await,
        TodoCommands::Attach {
            id,
            file,
            url,
            note,
            title,
        } => attach::handle_attach(id, file, url, note, title).await,
        TodoCommands::Detach { id, attachment_id } => {
            attach::handle_detach(&id, &attachment_id).await
        }
        TodoCommands::AddSubtask {
            parent_id,
            title,
            description,
            priority,
            due,
            tags,
        } => add::handle_add_subtask(parent_id, title, description, priority, due, tags).await,
        TodoCommands::Move {
            id,
            parent,
            project,
        } => move_task::handle_move(id, parent, project).await,
        TodoCommands::LogTime { id, minutes, note } => {
            time::handle_log_time(id, minutes, note).await
        }
        TodoCommands::Report { period, project } => report::handle_report(period, project).await,
        TodoCommands::Depend { id, on, remove } => depend::handle_depend(id, on, remove).await,
        TodoCommands::Enrich { id } => enrich::handle_enrich(id).await,
        TodoCommands::Recur(cmd) => recur::handle_recur(cmd).await,
        TodoCommands::BackfillEmbeddings => backfill::handle_backfill().await,
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

/// Parse a due date string to DateTime<Utc> using the shared date utility.
/// Interprets non-timezone strings in the configured timezone.
fn parse_due_date(date_str: &str, timezone: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    common::utils::date::parse_datetime(date_str, timezone)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse date: {}", date_str))
}

/// Format a UTC datetime for display in the user's timezone
pub(super) fn format_date_local(
    dt: &chrono::DateTime<chrono::Utc>,
    timezone: &str,
    fmt: &str,
) -> String {
    common::utils::date::format_datetime_local(dt, timezone, fmt)
}

/// Show detailed task view (used by show and list)
fn show_task_details(todo: &Todo, timezone: &str) {
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
        format_date_local(&todo.created_at, timezone, "%b %d"),
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
        let formatted = format_date_local(due, timezone, "%Y-%m-%d");
        let due_str = if *due < now {
            colorize(&format!("{} (OVERDUE)", formatted), ERROR)
        } else {
            colorize(&formatted, DIM)
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
