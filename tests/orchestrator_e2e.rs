//! End-to-end integration tests for the Intent Pipeline.
//!
//! Smoke-tests the happy paths through the full pipeline:
//! IntentAnalyzer → ContextEngine → ExecutionRouter → ResponseValidator → CostTracker

mod common;

use std::sync::Arc;

use common::MockProvider;
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
use tokio::sync::RwLock;

fn routing_ctx() -> klyntbot::tools::RoutingContext {
    common::test_routing_ctx()
}

/// Build a pipeline backed by the given provider.
async fn make_pipeline(provider: Arc<dyn LlmProvider>) -> IntentPipeline {
    let registry = Arc::new(RwLock::new(ToolRegistry::new()));
    let core = Arc::new(ExecutionCore::new(provider.clone(), registry));

    let direct = DirectEngine::new(Arc::clone(&core));
    let reactive = ReactiveEngine::new(Arc::clone(&core), 10);
    let router = ExecutionRouter::new(direct, reactive, None, 3);

    let analyzer = IntentAnalyzer::new(provider, "mock", &OrchestratorConfig::default());

    let pool = common::test_pool().await;
    let usage_repo = klyntbot::storage::UsageRepo::new(pool.inner().clone());
    let cost_tracker = Arc::new(CostTracker::from_repo(usage_repo));

    IntentPipeline::new(
        analyzer,
        Arc::new(ContextEngine::new()),
        router,
        cost_tracker,
        PipelineConfig::default(),
    )
}

// ── Test 1: Direct Response happy path ──

#[tokio::test]
async fn test_e2e_direct_response_path() {
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
    assert!(result.validation.is_valid);
}

// ── Test 5: Context budget is respected (large history doesn't crash) ──

#[tokio::test]
async fn test_e2e_context_budget_respected() {
    let provider = Arc::new(MockProvider::new("Budget response"));
    let pipeline = make_pipeline(provider).await;

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

    assert!(!result.content.is_empty());
}

// ── Test 6: Greeting is routed via Direct mode ──

#[tokio::test]
async fn intent_pipeline_routes_greeting_directly() {
    let provider = Arc::new(MockProvider::new("Hi! How can I help you today?"));
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message("hey there", vec![], &[], &[], &routing_ctx(), None, None)
        .await
        .unwrap();

    assert_eq!(result.mode_used, "direct");
    assert_eq!(result.escalations, 0);
    assert!(result.validation.is_valid);
    assert_eq!(result.content, "Hi! How can I help you today?");
}

// ── Test 7: Search query is routed via Reactive mode ──

#[tokio::test]
async fn intent_pipeline_routes_search_as_reactive() {
    let provider = Arc::new(MockProvider::new("Found 5 tasks about databases."));
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message(
            "search for tasks about databases",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.mode_used, "reactive");
    assert_eq!(result.escalations, 0);
    assert!(result.validation.is_valid);
    assert_eq!(result.content, "Found 5 tasks about databases.");
}

// ── Test 8: Plan keywords trigger Planned classification ──

#[tokio::test]
async fn intent_pipeline_auto_plans_complex_request() {
    let provider = Arc::new(MockProvider::new(
        "I'll create a plan for reorganizing the codebase.",
    ));
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message(
            "create a plan to reorganize the entire codebase",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    assert!(
        format!("{:?}", result.classification.mode).contains("Planned"),
        "Expected Planned classification, got: {:?}",
        result.classification.mode
    );
    assert_eq!(result.mode_used, "planned");
    assert!(result.validation.is_valid);
}

// ── Test 9: Reactive engine escalates when overwhelmed ──

#[tokio::test]
async fn intent_pipeline_escalates_reactive_to_planned() {
    let provider = Arc::new(MockProvider::with_tool_call(
        "web_search",
        serde_json::json!({"query": "test"}),
    ));
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message(
            "search for tasks about databases",
            vec![],
            &[],
            &[],
            &routing_ctx(),
            None,
            None,
        )
        .await
        .unwrap();

    assert!(
        result.escalations >= 1,
        "Expected escalation, got {} escalations",
        result.escalations
    );
    assert!(
        result.content.contains("planning")
            || result.content.contains("not configured")
            || result.content.contains("exceeded"),
        "Expected escalation message, got: {}",
        result.content
    );
}
