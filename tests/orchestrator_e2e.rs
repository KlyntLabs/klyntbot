//! End-to-end integration tests for the Intent Pipeline.
//!
//! Smoke-tests the happy paths through the full pipeline:
//! IntentAnalyzer → ContextEngine → ExecutionRouter → ResponseValidator → CostTracker

mod mock_provider;

use std::sync::Arc;

use klyntbot::agent::execution::ExecutionCore;
use klyntbot::agent::intent_pipeline::analyzer::IntentAnalyzer;
use klyntbot::agent::intent_pipeline::engines::direct::DirectEngine;
use klyntbot::agent::intent_pipeline::engines::reactive::ReactiveEngine;
use klyntbot::agent::intent_pipeline::pipeline::{IntentPipeline, PipelineConfig};
use klyntbot::agent::intent_pipeline::router::ExecutionRouter;
use klyntbot::agent::output::cost_tracker::CostTracker;
use klyntbot::config::OrchestratorConfig;
use klyntbot::context_engine::ContextEngine;
use klyntbot::providers::types::*;
use klyntbot::tools::registry::ToolRegistry;
use mock_provider::MockProvider;
use tokio::sync::RwLock;

fn routing_ctx() -> klyntbot::tools::RoutingContext {
    klyntbot::tools::RoutingContext::new("e2e-test".into(), "e2e-test".into())
}

/// Build a pipeline backed by the given provider.
async fn make_pipeline(provider: Arc<dyn LlmProvider>) -> IntentPipeline {
    let registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let core = Arc::new(ExecutionCore::new(provider.clone(), registry));

    let direct = DirectEngine::new(Arc::clone(&core));
    let reactive = ReactiveEngine::new(Arc::clone(&core), 10);
    let router = ExecutionRouter::new(direct, reactive, None, 3);

    let analyzer = IntentAnalyzer::new(provider, "mock", &OrchestratorConfig::default());

    let pool = sqlx::SqlitePool::connect(":memory:")
        .await
        .expect("in-memory SQLite pool");
    let usage_repo = klyntbot::storage::UsageRepo::new(pool);
    let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));

    IntentPipeline::new(
        analyzer,
        ContextEngine::new(),
        router,
        cost_tracker,
        PipelineConfig::default(),
    )
}

// ── Test 1: Direct Response happy path ──

#[tokio::test]
async fn test_e2e_direct_response_path() {
    // "hello" → heuristic classifies as Direct → single LLM call → response
    let provider = Arc::new(MockProvider::new("Hello! How can I help you?"));
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message("hello", vec![], &[], &[], &routing_ctx(), None, None)
        .await
        .unwrap();

    assert_eq!(result.content, "Hello! How can I help you?");
    assert!(
        result.mode_used.contains("direct"),
        "Expected direct, got: {}",
        result.mode_used
    );
    assert_eq!(result.escalations, 0);
    assert!(result.validation.is_valid);
}

// ── Test 2: Tool-assisted happy path ──

#[tokio::test]
async fn test_e2e_tool_assisted_path() {
    // "show my tasks" → heuristic classifies as Reactive → ReactiveEngine
    // Mock provider returns a text response (no actual tool calls needed)
    let provider = Arc::new(MockProvider::new("Here are your 3 active tasks: ..."));
    let pipeline = make_pipeline(provider).await;

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
        result.mode_used.contains("reactive"),
        "Expected reactive, got: {}",
        result.mode_used
    );
    assert_eq!(result.escalations, 0);
}

// ── Test 3: Autonomous task path ──

#[tokio::test]
async fn test_e2e_autonomous_task_path() {
    // "write me a script that processes CSV files" → Reactive via heuristic
    let provider = Arc::new(MockProvider::new(
        "Here's a Python script that processes CSV files:\n```python\nimport csv\n```",
    ));
    let pipeline = make_pipeline(provider).await;

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
    // Mock resolves as reactive (text response, no tool calls)
    assert!(result.validation.is_valid);
}

// Test 4 (usage.jsonl disk check) removed — CostTracker now records to SQL, not JSONL.

// ── Test 5: Context budget is respected (large history doesn't crash) ──

#[tokio::test]
async fn test_e2e_context_budget_respected() {
    let provider = Arc::new(MockProvider::new("Budget response"));
    let pipeline = make_pipeline(provider).await;

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
