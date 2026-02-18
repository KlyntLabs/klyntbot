//! Calendar CLI commands.
//!
//! Implements calendar subcommands including reconciliation.

use anyhow::Result;
use common::utils::terminal::*;

use crate::commands::CalendarCommands;

/// Handle calendar subcommands.
pub async fn handle_calendar(cmd: CalendarCommands) -> Result<()> {
    match cmd {
        CalendarCommands::Reconcile => handle_reconcile().await,
    }
}

/// Handle calendar reconcile command.
async fn handle_reconcile() -> Result<()> {
    // Load config
    let config = config::load().await?;

    // Check if bidirectional sync is enabled
    if !config.calendar.bidirectional_sync {
        println!("Bidirectional sync is disabled in config.");
        println!(
            "Enable it by setting calendar.bidirectionalSync to true in ~/.klyntbot/config.json"
        );
        return Ok(());
    }

    // Check if any calendar provider is enabled
    if !config.calendar.is_any_enabled() {
        println!("No calendar providers are enabled.");
        println!("Run 'klyntbot init' to configure calendar sync.");
        return Ok(());
    }

    println!("Running calendar reconciliation...\n");

    // Create todo store and calendar adapter
    let todo_path = config.todo_store_path();
    let todo_store = std::sync::Arc::new(tokio::sync::RwLock::new(
        tools::todo_store::TodoStore::new(todo_path),
    ));

    let adapter = agent::CalendarSyncAdapter::new(
        std::sync::Arc::clone(&todo_store),
        &config.calendar,
        config.timezone.clone(),
        None, // No notification dispatcher for CLI
        true, // Force enabled for manual reconcile
    )
    .await?;

    // Fetch events from all providers
    use tools::calendar_tool::CalendarHandler;
    let events = adapter.get_events_for_reconciliation().await?;

    // Run reconciliation
    let report = agent::reconcile_calendar_events(todo_store, events).await?;

    // Display report
    println!("{}", colorize("Calendar Reconciliation Report", BRAND));
    println!("{}", "─".repeat(40));

    let headers = vec!["Metric", "Count"];
    let rows = vec![
        vec!["Checked".to_string(), report.checked.to_string()],
        vec![
            "Due dates updated".to_string(),
            report.due_dates_updated.to_string(),
        ],
        vec![
            "Todos completed".to_string(),
            report.todos_completed.to_string(),
        ],
        vec![
            "Links cleared".to_string(),
            report.links_cleared.to_string(),
        ],
        vec!["Errors".to_string(), report.errors.len().to_string()],
    ];

    println!("{}", draw_table(&headers, &rows));

    if !report.errors.is_empty() {
        println!("\nErrors:");
        for err in &report.errors {
            println!("  - {}", err);
        }
    }

    let changes = report.due_dates_updated + report.todos_completed + report.links_cleared;
    if changes == 0 {
        println!("\nNo changes needed — all calendar-linked todos are in sync.");
    }

    Ok(())
}
