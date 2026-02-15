//! Task reporting and analytics command.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{TimeEntrySource, TodoFilter, TodoStatus};

pub async fn handle_report(period: String, project_id: Option<String>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let now = Utc::now();
    let (period_start, period_label) = match period.as_str() {
        "week" => (now - chrono::Duration::days(7), "Last 7 days"),
        "month" => (now - chrono::Duration::days(30), "Last 30 days"),
        other => {
            println!(
                "{} Invalid period '{}'. Use 'week' or 'month'.",
                status_error(),
                other
            );
            return Ok(());
        }
    };

    let filter = TodoFilter {
        project_id: project_id.clone(),
        ..Default::default()
    };

    let todos = store.list(&filter).await?;

    // Calculate statistics
    let total = todos.len();
    let completed_in_period = todos
        .iter()
        .filter(|t| {
            t.status == TodoStatus::Done
                && t.completed_at.map(|dt| dt >= period_start).unwrap_or(false)
        })
        .count();
    let created_in_period = todos
        .iter()
        .filter(|t| t.created_at >= period_start)
        .count();

    let active = todos
        .iter()
        .filter(|t| t.status == TodoStatus::Todo || t.status == TodoStatus::Doing)
        .count();
    let overdue = todos
        .iter()
        .filter(|t| t.due_date.map(|d| d < now).unwrap_or(false) && t.status != TodoStatus::Done)
        .count();

    // Time tracking stats
    let total_time_secs: u64 = todos
        .iter()
        .flat_map(|t| &t.time_entries)
        .filter(|e| e.started_at >= period_start)
        .filter_map(|e| e.duration_secs)
        .sum();

    let focus_entries: Vec<_> = todos
        .iter()
        .flat_map(|t| &t.time_entries)
        .filter(|e| e.source == TimeEntrySource::Focus && e.started_at >= period_start)
        .collect();
    let focus_session_count = focus_entries.len();
    let focus_time_secs: u64 = focus_entries.iter().filter_map(|e| e.duration_secs).sum();

    let manual_time_secs: u64 = todos
        .iter()
        .flat_map(|t| &t.time_entries)
        .filter(|e| e.source == TimeEntrySource::Manual && e.started_at >= period_start)
        .filter_map(|e| e.duration_secs)
        .sum();

    // By priority breakdown
    let mut by_priority = [0usize; 6]; // index 0 unused, 1-5 for priorities
    for todo in &todos {
        let pri = todo.priority.unwrap_or(3) as usize;
        if pri <= 5 {
            by_priority[pri] += 1;
        }
    }

    // Header
    println!(
        "{}",
        colorize(
            "┌─ Todo Report ──────────────────────────────────────┐",
            BRAND
        )
    );
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize(period_label, BOLD),
        colorize("  │", BRAND)
    );
    if let Some(ref pid) = project_id {
        println!(
            "{}  Project: {}{}",
            colorize("│", BRAND),
            colorize(pid, TOOL),
            colorize("  │", BRAND)
        );
    }
    println!(
        "{}",
        colorize(
            "│                                                     │",
            BRAND
        )
    );

    // Task stats
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize("Tasks", BOLD),
        colorize("  │", BRAND)
    );
    println!(
        "{}    Total:     {}{}",
        colorize("│", BRAND),
        total,
        colorize("  │", BRAND)
    );
    println!(
        "{}    Active:    {}{}",
        colorize("│", BRAND),
        active,
        colorize("  │", BRAND)
    );
    println!(
        "{}    Created:   {}{}",
        colorize("│", BRAND),
        created_in_period,
        colorize("  │", BRAND)
    );
    println!(
        "{}    Completed: {}{}",
        colorize("│", BRAND),
        completed_in_period,
        colorize("  │", BRAND)
    );

    if overdue > 0 {
        println!(
            "{}    Overdue:   {}{}",
            colorize("│", BRAND),
            colorize(&overdue.to_string(), ERROR),
            colorize("  │", BRAND)
        );
    }

    // Completion rate progress bar
    if total > 0 {
        let done_total = todos
            .iter()
            .filter(|t| t.status == TodoStatus::Done)
            .count();
        let rate = done_total as f32 / total as f32;
        let bar = render_progress_bar(rate, 20);
        println!("{}{}", colorize("│", BRAND), colorize("  │", BRAND));
        println!(
            "{}    Done:      {} {:.0}%{}",
            colorize("│", BRAND),
            bar,
            rate * 100.0,
            colorize("  │", BRAND)
        );
    }

    println!(
        "{}",
        colorize(
            "│                                                     │",
            BRAND
        )
    );

    // Time tracking
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize("Time Tracking", BOLD),
        colorize("  │", BRAND)
    );
    println!(
        "{}    Total:          {:.1}h{}",
        colorize("│", BRAND),
        total_time_secs as f64 / 3600.0,
        colorize("  │", BRAND)
    );
    println!(
        "{}    Focus sessions: {} ({:.1}h){}",
        colorize("│", BRAND),
        focus_session_count,
        focus_time_secs as f64 / 3600.0,
        colorize("  │", BRAND)
    );
    println!(
        "{}    Manual logged:  {:.1}h{}",
        colorize("│", BRAND),
        manual_time_secs as f64 / 3600.0,
        colorize("  │", BRAND)
    );

    println!(
        "{}",
        colorize(
            "│                                                     │",
            BRAND
        )
    );

    // By priority
    if total > 0 {
        println!(
            "{}  {}{}",
            colorize("│", BRAND),
            colorize("By Priority", BOLD),
            colorize("  │", BRAND)
        );
        for pri in (1..=5).rev() {
            if by_priority[pri] > 0 {
                let bar = render_progress_bar(by_priority[pri] as f32 / total as f32, 15);
                println!(
                    "{}    P{}: {} {}{}",
                    colorize("│", BRAND),
                    pri,
                    bar,
                    by_priority[pri],
                    colorize("  │", BRAND)
                );
            }
        }
        println!(
            "{}",
            colorize(
                "│                                                     │",
                BRAND
            )
        );
    }

    println!(
        "{}",
        colorize(
            "└─────────────────────────────────────────────────────┘",
            BRAND
        )
    );

    Ok(())
}

/// Render a simple progress bar.
fn render_progress_bar(fraction: f32, width: usize) -> String {
    let filled = (fraction * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);

    let color = if fraction >= 0.75 {
        SUCCESS
    } else if fraction >= 0.4 {
        WARNING
    } else {
        DIM
    };

    format!(
        "{}{}",
        colorize(&"█".repeat(filled), color),
        colorize(&"░".repeat(empty), DIM),
    )
}
