//! End-to-end integration tests for the Adaptive Orchestrator pipeline.
//!
//! Smoke-tests the happy paths through the full pipeline:
//! Orchestrator → ContextEngine → EngineDispatch → ResponseValidator → CostTracker

mod mock_provider;

use std::sync::Arc;

use klyntbot::agent::execution::{EngineDispatch, ExecutionCore};
use klyntbot::agent::orchestrator::Orchestrator;
use klyntbot::agent::output::cost_tracker::CostTracker;
use klyntbot::agent::pipeline::{AgentPipeline, PipelineConfig};
use klyntbot::providers::types::*;
use klyntbot::tools::registry::ToolRegistry;
use mock_provider::MockProvider;
use tokio::sync::RwLock;

fn routing_ctx() -> klyntbot::tools::RoutingContext {
    klyntbot::tools::RoutingContext::new("e2e-test".into(), "e2e-test".into())
}

/// Build a pipeline backed by the given provider.
fn make_pipeline(provider: Arc<dyn LlmProvider>) -> (AgentPipeline, tempfile::TempDir) {
    let registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let core = Arc::new(ExecutionCore::new(provider.clone(), registry));
    let orchestrator = Arc::new(Orchestrator::new(provider.clone(), "mock"));
    let dispatch = Arc::new(EngineDispatch::new(core));

    let dir = tempfile::tempdir().unwrap();
    let cost_tracker = Arc::new(CostTracker::new(dir.path().to_path_buf()));

    let pipeline = AgentPipeline::new(
        orchestrator,
        dispatch,
        cost_tracker,
        PipelineConfig::default(),
    );
    (pipeline, dir)
}

// ── Test 1: Direct Response happy path ──

#[tokio::test]
async fn test_e2e_direct_response_path() {
    // "hello" → heuristic classifies as DirectResponse → single LLM call → response
    let provider = Arc::new(MockProvider::new("Hello! How can I help you?"));
    let (pipeline, _dir) = make_pipeline(provider);

    let result = pipeline
        .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
        .await
        .unwrap();

    assert_eq!(result.content, "Hello! How can I help you?");
    assert!(
        result.strategy_used.contains("DirectResponse"),
        "Expected DirectResponse, got: {}",
        result.strategy_used
    );
    assert_eq!(result.escalations, 0);
    assert!(result.validation.is_valid);
}

// ── Test 2: Tool-assisted happy path ──

#[tokio::test]
async fn test_e2e_tool_assisted_path() {
    // "show my tasks" → heuristic classifies as ToolAssisted → ReAct+ engine
    // Mock provider returns a text response (no actual tool calls needed)
    let provider = Arc::new(MockProvider::new("Here are your 3 active tasks: ..."));
    let (pipeline, _dir) = make_pipeline(provider);

    let result = pipeline
        .process_message(
            "show my tasks",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.content, "Here are your 3 active tasks: ...");
    assert!(
        result.strategy_used.contains("ToolAssisted"),
        "Expected ToolAssisted, got: {}",
        result.strategy_used
    );
    assert_eq!(result.escalations, 0);
}

// ── Test 3: Autonomous task path ──

#[tokio::test]
async fn test_e2e_autonomous_task_path() {
    // "write me a script that processes CSV files" → AutonomousTask via heuristic
    let provider = Arc::new(MockProvider::new(
        "Here's a Python script that processes CSV files:\n```python\nimport csv\n```",
    ));
    let (pipeline, _dir) = make_pipeline(provider);

    let result = pipeline
        .process_message(
            "write me a script that processes CSV files",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!result.content.is_empty());
    // AutonomousTask uses PlanExecute engine, which the mock may resolve as ToolAssisted
    // after escalation — just verify we get a result
    assert!(result.validation.is_valid);
}

// ── Test 4: Usage tracking records to disk ──

#[tokio::test]
async fn test_e2e_pipeline_records_usage() {
    let provider = Arc::new(MockProvider::new("tracked response"));
    let (pipeline, dir) = make_pipeline(provider);

    pipeline
        .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
        .await
        .unwrap();

    // Verify usage.jsonl was written
    let usage_path = dir.path().join("usage.jsonl");
    assert!(
        usage_path.exists(),
        "usage.jsonl should be created after pipeline run"
    );
    let content = tokio::fs::read_to_string(&usage_path).await.unwrap();
    assert!(
        !content.trim().is_empty(),
        "usage.jsonl should contain at least one record"
    );

    // Verify it's valid JSON
    let record: serde_json::Value = serde_json::from_str(content.trim().lines().next().unwrap())
        .expect("usage record should be valid JSON");
    assert!(record.get("model").is_some());
    assert!(record.get("estimated_cost_usd").is_some());
}

// ── Test 5: Context budget is respected (large history doesn't crash) ──

#[tokio::test]
async fn test_e2e_context_budget_respected() {
    let provider = Arc::new(MockProvider::new("Budget response"));
    let (pipeline, _dir) = make_pipeline(provider);

    // Create a history with many messages to test budget allocation
    let mut history = Vec::new();
    for i in 0..50 {
        history.push(Message::user(format!(
            "This is user message number {} with some extra text to add token count",
            i
        )));
        history.push(Message::assistant(format!("Response to message {}", i)));
    }

    let result = pipeline
        .process_message(
            "what was our conversation about?",
            history,
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    // Should succeed without panicking, even with large history
    assert!(!result.content.is_empty());
}
