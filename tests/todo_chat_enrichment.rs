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

use klyntbot::storage::{StoragePool, TodoRepo};
use klyntbot::tools::todo::TodoTool;
use klyntbot::tools::{RoutingContext, Tool};
use tempfile::TempDir;

/// Helper to create a test TodoTool.
///
/// Uses a lazy (non-connected) pool — queries that hit Postgres will fail at runtime.
/// These tests exercise the tool's add/complete flow, which requires Postgres.
/// They are `#[ignore]`d unless DATABASE_URL is set.
fn create_test_tool() -> (TodoTool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let pool = StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default());
    (tool, temp_dir)
}

fn ctx() -> RoutingContext {
    RoutingContext::new(
        klyntbot::common::ChannelName::new("telegram"),
        klyntbot::common::ChatId::new("test-chat"),
    )
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_add_task_returns_id() {
    let (tool, _dir) = create_test_tool();

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
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_complete_task_returns_clean_output() {
    let (tool, _dir) = create_test_tool();

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
