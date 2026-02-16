//! Daily planning CLI handler.
//!
//! Generates and displays daily task plans with user confirmation workflow.

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::todo::TodoTool;
use tools::todo_store::TodoStore;
use tools::Tool;

/// Handle the daily planning command.
///
/// Generates and displays a plan of the top 3 most impactful tasks for today.
pub async fn handle_plan() -> Result<()> {
    let config = config::load().await?;

    // Check if daily planning is enabled
    if !config.todo.daily_planning.enabled {
        anyhow::bail!(
            "Daily planning is disabled in config. Enable it by setting todo.dailyPlanning.enabled=true in ~/.klyntbot/config.json"
        );
    }

    let store_path = config.todo_store_path();
    let store = Arc::new(RwLock::new(TodoStore::new(store_path)));

    // Create TodoTool
    let tool = TodoTool::new(
        store.clone(),
        config.todo.focus.max_slots,
        config.todo.focus.deadline_hours,
        config.timezone.clone(),
    );

    // Call the plan action
    let ctx = tools::RoutingContext::new(
        common::ChannelName::new("cli"),
        common::ChatId::new("default"),
    );
    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await?;

    // Display the result
    println!("{}", result);

    // NOTE: Interactive response handling (accept, skip, swap, defer) is deferred to Sprint 7+.
    // The response parser (tools::plan_response) is ready, but CLI integration with the
    // agent's task focus system requires additional coordination.
    // For now, users can manually act on the plan via `klyntbot todo focus <id>`.

    Ok(())
}
