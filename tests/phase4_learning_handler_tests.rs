//! Integration tests for I5: LearningHandler Trait + LearningTool
//!
//! AC-I5.1: LearningHandler trait is defined in tools crate with get_status() and analyze_now()
//! AC-I5.2: LearningTool implements Tool trait with name "learning", action enum in parameters
//! AC-I5.3: LearningHandlerImpl returns None from get_status() when no strategy records exist,
//!          Some(LearningStatus) when strategy records are present
//! AC-I5.4: LearningTool.execute() routes "status" and "analyze" actions correctly through handler
//! AC-I5.5: get_threshold_history(limit) returns last N ThresholdEntry records; "history" action
//!          in LearningTool routes correctly; empty Vec returned when no changes exist
//!
//! Run: `cargo nextest run --test phase4_learning_handler_tests`

use async_trait::async_trait;
use chrono::Utc;
use common::{ChannelName, ChatId, Result};
use klyntbot::agent::learning::adaptive::AdaptiveThresholds;
use klyntbot::agent::learning_handler::LearningHandlerImpl;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::learning_tool::{LearningHandler, LearningStatus, LearningTool, ThresholdEntry};
use tools::{RoutingContext, Tool};

// ─────────────────────────────────────────────────────────────
// Mock Handler (used for AC-I5.1 and AC-I5.4 tests)
// ─────────────────────────────────────────────────────────────

struct MockLearningHandler {
    status: Option<LearningStatus>,
    history: Vec<ThresholdEntry>,
}

#[async_trait]
impl LearningHandler for MockLearningHandler {
    async fn get_status(&self) -> Result<Option<LearningStatus>> {
        Ok(self.status.clone())
    }

    async fn analyze_now(&self) -> Result<LearningStatus> {
        Ok(self.status.clone().unwrap_or_else(|| LearningStatus {
            current_threshold: 0.7,
            total_strategy_records: 0,
            strategy_accuracy: 0.0,
            avg_response_time_ms: 0,
            avg_satisfaction: None,
            suggested_threshold: 0.7,
            per_tool: HashMap::new(),
        }))
    }

    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>> {
        let entries = &self.history;
        let start = entries.len().saturating_sub(limit);
        Ok(entries[start..].to_vec())
    }
}

fn make_routing_ctx() -> RoutingContext {
    RoutingContext::new(ChannelName::new("cli"), ChatId::new("test-chat"))
}

// ─────────────────────────────────────────────────────────────
// AC-I5.1: LearningHandler trait is defined with correct methods
// ─────────────────────────────────────────────────────────────

/// AC-I5.1: A mock implementation of LearningHandler compiles and works.
#[tokio::test]
async fn test_learning_handler_trait_get_status_none() {
    let mock = MockLearningHandler {
        status: None,
        history: vec![],
    };
    let status = mock.get_status().await.unwrap();
    assert!(
        status.is_none(),
        "Expected None when handler has no status data"
    );
}

/// AC-I5.1: get_status() returns Some when status is set.
#[tokio::test]
async fn test_learning_handler_trait_get_status_some() {
    let mock = MockLearningHandler {
        history: vec![],
        status: Some(LearningStatus {
            current_threshold: 0.75,
            total_strategy_records: 10,
            strategy_accuracy: 0.9,
            avg_response_time_ms: 200,
            avg_satisfaction: Some(0.8),
            suggested_threshold: 0.72,
            per_tool: HashMap::new(),
        }),
    };
    let status = mock.get_status().await.unwrap();
    assert!(status.is_some());
    let s = status.unwrap();
    assert_eq!(s.total_strategy_records, 10);
}

/// AC-I5.1: analyze_now() returns a LearningStatus even with no data.
#[tokio::test]
async fn test_learning_handler_trait_analyze_now() {
    let mock = MockLearningHandler {
        status: None,
        history: vec![],
    };
    let status = mock.analyze_now().await.unwrap();
    assert_eq!(status.total_strategy_records, 0);
    assert!((status.current_threshold - 0.7).abs() < f32::EPSILON);
}

// ─────────────────────────────────────────────────────────────
// AC-I5.2: LearningTool implements Tool trait
// ─────────────────────────────────────────────────────────────

/// AC-I5.2: LearningTool.name() returns "learning".
#[test]
fn test_learning_tool_name() {
    let tool = LearningTool::new(None);
    assert_eq!(tool.name(), "learning");
}

/// AC-I5.2: LearningTool.description() is non-empty.
#[test]
fn test_learning_tool_description_nonempty() {
    let tool = LearningTool::new(None);
    assert!(!tool.description().is_empty());
}

/// AC-I5.2: LearningTool.parameters() includes action enum with "status" and "analyze".
#[test]
fn test_learning_tool_parameters_action_enum() {
    let tool = LearningTool::new(None);
    let params = tool.parameters();

    let action_enum = &params["properties"]["action"]["enum"];
    let variants: Vec<&str> = action_enum
        .as_array()
        .expect("action.enum should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        variants.contains(&"status"),
        "Parameters schema must include 'status' action, got: {:?}",
        variants
    );
    assert!(
        variants.contains(&"analyze"),
        "Parameters schema must include 'analyze' action, got: {:?}",
        variants
    );
}

/// AC-I5.2: LearningTool.parameters() marks "action" as required.
#[test]
fn test_learning_tool_parameters_action_required() {
    let tool = LearningTool::new(None);
    let params = tool.parameters();

    let required: Vec<&str> = params["required"]
        .as_array()
        .expect("required should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        required.contains(&"action"),
        "'action' must be in required fields, got: {:?}",
        required
    );
}

/// AC-I5.2: LearningTool.to_schema() returns a valid function schema.
#[test]
fn test_learning_tool_to_schema() {
    let tool = LearningTool::new(None);
    let schema = tool.to_schema();
    assert_eq!(schema["type"], "function");
    assert_eq!(schema["function"]["name"], "learning");
}

// ─────────────────────────────────────────────────────────────
// AC-I5.3: LearningHandlerImpl with real StrategyRepo
// ─────────────────────────────────────────────────────────────

async fn make_handler() -> (LearningHandlerImpl, storage::StrategyRepo) {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let strategy_repo = storage::StrategyRepo::new(pool.inner().clone());
    let adaptive = Arc::new(RwLock::new(AdaptiveThresholds::new_in_memory(
        0.7, 0.4, 0.9, 50,
    )));
    let handler = LearningHandlerImpl::new(strategy_repo.clone(), adaptive);
    (handler, strategy_repo)
}

/// AC-I5.3: get_status() returns None when StrategyRepo is empty.
#[tokio::test]
async fn test_learning_handler_impl_get_status_empty_store() {
    let (handler, _repo) = make_handler().await;

    let status = handler.get_status().await.unwrap();
    assert!(
        status.is_none(),
        "Expected None when no strategy records have been recorded"
    );
}

/// AC-I5.3: get_status() returns Some(LearningStatus) when strategy records exist.
#[tokio::test]
async fn test_learning_handler_impl_get_status_with_outcomes() {
    let (handler, repo) = make_handler().await;

    // Record a strategy record with tool info
    let row = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "req-1".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "DirectResponse".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 1,
        success: true,
        user_satisfaction: Some(1.0),
        response_time_ms: 50,
        chat_id: None,
        tool_name: Some("todo".to_string()),
        tool_success: Some(true),
        tool_duration_ms: Some(25),
    };
    repo.create(&row).await.unwrap();

    let status = handler.get_status().await.unwrap();
    assert!(
        status.is_some(),
        "Expected Some when strategy records exist"
    );

    let s = status.unwrap();
    assert_eq!(s.total_strategy_records, 1);
    assert!(
        s.per_tool.contains_key("todo"),
        "Expected 'todo' in per_tool stats"
    );
    assert_eq!(s.per_tool["todo"].total_calls, 1);
    assert_eq!(s.per_tool["todo"].success_count, 1);
}

/// AC-I5.3: analyze_now() always returns a LearningStatus (even with empty repo).
#[tokio::test]
async fn test_learning_handler_impl_analyze_now_empty() {
    let (handler, _repo) = make_handler().await;

    let status = handler.analyze_now().await.unwrap();
    assert_eq!(status.total_strategy_records, 0);
    // threshold should still be the initial 0.7
    assert!(
        (status.current_threshold - 0.7).abs() < 0.001,
        "Expected initial threshold of 0.7, got {}",
        status.current_threshold
    );
}

/// AC-I5.3: analyze_now() reflects multiple records with per-tool aggregation.
#[tokio::test]
async fn test_learning_handler_impl_analyze_now_with_outcomes() {
    let (handler, repo) = make_handler().await;

    // Record two strategy records with "todo" tool and one with "shell"
    for i in 0..2 {
        let row = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: format!("req-todo-{}", i),
            predicted_strategy: "ToolAssisted".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 100,
            chat_id: None,
            tool_name: Some("todo".to_string()),
            tool_success: Some(true),
            tool_duration_ms: Some(30),
        };
        repo.create(&row).await.unwrap();
    }
    let shell_row = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "req-shell-0".to_string(),
        predicted_strategy: "ToolAssisted".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 5000,
        chat_id: None,
        tool_name: Some("shell".to_string()),
        tool_success: Some(false),
        tool_duration_ms: Some(5000),
    };
    repo.create(&shell_row).await.unwrap();

    let status = handler.analyze_now().await.unwrap();
    assert_eq!(status.total_strategy_records, 3);
    assert_eq!(status.per_tool["todo"].total_calls, 2);
    assert_eq!(status.per_tool["todo"].success_count, 2);
    assert_eq!(status.per_tool["shell"].total_calls, 1);
    assert_eq!(status.per_tool["shell"].success_count, 0);
}

// ─────────────────────────────────────────────────────────────
// AC-I5.4: LearningTool.execute() routes actions correctly
// ─────────────────────────────────────────────────────────────

/// AC-I5.4: execute("status") returns JSON when handler has data.
#[tokio::test]
async fn test_learning_tool_execute_status_with_data() {
    let mock = Arc::new(MockLearningHandler {
        history: vec![],
        status: Some(LearningStatus {
            current_threshold: 0.75,
            total_strategy_records: 42,
            strategy_accuracy: 0.85,
            avg_response_time_ms: 300,
            avg_satisfaction: Some(0.9),
            suggested_threshold: 0.72,
            per_tool: HashMap::new(),
        }),
    });

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["total_strategy_records"], 42);
    assert!(
        (parsed["current_threshold"].as_f64().unwrap() as f32 - 0.75).abs() < 0.001,
        "Expected current_threshold ~0.75, got {}",
        parsed["current_threshold"]
    );
}

/// AC-I5.4: execute("status") returns no-data message when handler returns None.
#[tokio::test]
async fn test_learning_tool_execute_status_no_data() {
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![],
    });

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    // Must include some indicator of "no data available"
    assert!(
        result.contains("No learning data") || result.contains("message"),
        "Expected no-data message, got: {}",
        result
    );
}

/// AC-I5.4: execute("analyze") returns a LearningStatus JSON.
#[tokio::test]
async fn test_learning_tool_execute_analyze() {
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![],
    });

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "analyze"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    // analyze_now always returns a status
    assert!(
        parsed["total_strategy_records"].is_number(),
        "Expected total_strategy_records in response"
    );
    assert!(
        parsed["current_threshold"].is_number(),
        "Expected current_threshold in response"
    );
}

/// AC-I5.4: execute() with no handler configured returns an error.
#[tokio::test]
async fn test_learning_tool_execute_no_handler_returns_error() {
    let tool = LearningTool::new(None);
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await;
    assert!(
        result.is_err(),
        "Expected error when no handler configured, got: {:?}",
        result
    );
}

/// AC-I5.4: execute() with an unknown action returns an error.
#[tokio::test]
async fn test_learning_tool_execute_unknown_action() {
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![],
    });
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "nonexistent"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await;
    assert!(
        result.is_err(),
        "Expected error for unknown action, got: {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────
// AC-I5.5: get_threshold_history() + "history" LearningTool action
// ─────────────────────────────────────────────────────────────

/// AC-I5.5: Mock handler returns empty history when no changes exist.
#[tokio::test]
async fn test_learning_handler_trait_threshold_history_empty() {
    let mock = MockLearningHandler {
        status: None,
        history: vec![],
    };
    let history = mock.get_threshold_history(10).await.unwrap();
    assert!(
        history.is_empty(),
        "Expected empty history with no threshold changes"
    );
}

/// AC-I5.5: Mock handler returns history entries limited by `limit`.
#[tokio::test]
async fn test_learning_handler_trait_threshold_history_respects_limit() {
    use chrono::Utc;
    let mock = MockLearningHandler {
        status: None,
        history: vec![
            ThresholdEntry {
                from: 0.70,
                to: 0.72,
                reason: "r1".to_string(),
                timestamp: Utc::now(),
            },
            ThresholdEntry {
                from: 0.72,
                to: 0.74,
                reason: "r2".to_string(),
                timestamp: Utc::now(),
            },
            ThresholdEntry {
                from: 0.74,
                to: 0.76,
                reason: "r3".to_string(),
                timestamp: Utc::now(),
            },
        ],
    };

    let all = mock.get_threshold_history(10).await.unwrap();
    assert_eq!(all.len(), 3, "Expected all 3 entries with limit=10");

    let limited = mock.get_threshold_history(2).await.unwrap();
    assert_eq!(limited.len(), 2, "Expected last 2 entries with limit=2");
    // Should return the LAST N entries
    assert!(
        (limited[0].from - 0.72).abs() < 0.001,
        "First entry in limited result should be r2"
    );
    assert!(
        (limited[1].from - 0.74).abs() < 0.001,
        "Second entry should be r3"
    );
}

/// AC-I5.5: LearningHandlerImpl.get_threshold_history() returns empty Vec with no changes.
#[tokio::test]
async fn test_learning_handler_impl_threshold_history_empty() {
    let (handler, _repo) = make_handler().await;

    let history = handler.get_threshold_history(10).await.unwrap();
    assert!(
        history.is_empty(),
        "Expected empty threshold history on fresh AdaptiveThresholds"
    );
}

/// AC-I5.5: LearningTool "history" action returns JSON array from handler.
#[tokio::test]
async fn test_learning_tool_execute_history_empty() {
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![],
    });
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        parsed.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Expected empty JSON array, got: {}",
        result
    );
}

/// AC-I5.5: LearningTool "history" action passes through limit parameter (default: 10).
#[tokio::test]
async fn test_learning_tool_execute_history_with_entries() {
    use chrono::Utc;
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![
            ThresholdEntry {
                from: 0.70,
                to: 0.72,
                reason: "initial".to_string(),
                timestamp: Utc::now(),
            },
            ThresholdEntry {
                from: 0.72,
                to: 0.75,
                reason: "increased".to_string(),
                timestamp: Utc::now(),
            },
        ],
    });
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(arr.len(), 2, "Expected 2 history entries");
    assert_eq!(arr[0]["reason"], "initial");
    assert_eq!(arr[1]["reason"], "increased");
}

/// AC-I5.5: LearningTool "history" action respects explicit limit parameter.
#[tokio::test]
async fn test_learning_tool_execute_history_explicit_limit() {
    use chrono::Utc;
    let mock = Arc::new(MockLearningHandler {
        status: None,
        history: vec![
            ThresholdEntry {
                from: 0.70,
                to: 0.72,
                reason: "r1".to_string(),
                timestamp: Utc::now(),
            },
            ThresholdEntry {
                from: 0.72,
                to: 0.74,
                reason: "r2".to_string(),
                timestamp: Utc::now(),
            },
            ThresholdEntry {
                from: 0.74,
                to: 0.76,
                reason: "r3".to_string(),
                timestamp: Utc::now(),
            },
        ],
    });
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history", "limit": 2});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(arr.len(), 2, "Expected 2 entries with limit=2");
}

/// AC-I5.5: "history" action is present in LearningTool parameters enum.
#[test]
fn test_learning_tool_parameters_includes_history_action() {
    let tool = LearningTool::new(None);
    let params = tool.parameters();
    let variants: Vec<&str> = params["properties"]["action"]["enum"]
        .as_array()
        .expect("enum should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        variants.contains(&"history"),
        "Parameters schema must include 'history' action, got: {:?}",
        variants
    );
}
