//! Chat enrichment flow validation tests
//!
//! The old field-completeness confidence heuristic has been removed.
//! Confidence scoring is now LLM-driven (see crates/agent/src/confidence/).
//!
//! These tests previously validated the todo tool's confidence breakdown output.
//! They are replaced by unit tests in:
//! - crates/agent/src/confidence/evaluator.rs (parsing, thresholds)
//! - crates/agent/src/confidence/types.rs (serialization)
//! - crates/agent/src/confidence/log.rs (JSONL persistence)

use klyntbot::tools::todo::TodoTool;
use klyntbot::tools::todo_store::TodoStore;
use klyntbot::tools::{RoutingContext, Tool};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Helper to create a test TodoTool
async fn create_test_tool() -> (TodoTool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());
    (tool, temp_dir)
}

fn ctx() -> RoutingContext {
    RoutingContext::new(
        klyntbot::common::ChannelName::new("telegram"),
        klyntbot::common::ChatId::new("test-chat"),
    )
}

#[tokio::test]
async fn test_add_task_returns_id() {
    let (tool, _dir) = create_test_tool().await;

    let args = serde_json::json!({
        "action": "add",
        "title": "fix bug"
    });

    let result = tool.execute(args, &ctx()).await.unwrap();
    assert!(
        result.contains("Task created"),
        "Should confirm task creation"
    );
    assert!(result.contains("fix bug"), "Should include task title");
    assert!(result.contains("ID:"), "Should include task ID");
    // No confidence display
    assert!(
        !result.contains("confidence:"),
        "Should NOT contain legacy confidence display"
    );
}

#[tokio::test]
async fn test_complete_task_returns_clean_output() {
    let (tool, _dir) = create_test_tool().await;

    let args = serde_json::json!({
        "action": "add",
        "title": "fix the authentication timeout bug",
        "description": "Auth tokens expire during long sessions",
        "priority": 4,
        "due_date": "2026-02-20T12:00:00Z",
        "tags": ["backend", "urgent"]
    });

    let result = tool.execute(args, &ctx()).await.unwrap();
    assert!(result.contains("Task created"), "Should confirm creation");
    assert!(
        !result.contains("Confidence breakdown:"),
        "Should NOT contain old confidence breakdown"
    );
    assert!(
        !result.contains("AI INSTRUCTION"),
        "Should NOT contain AI INSTRUCTION"
    );
}
