//! Regression tests for agent_loop.rs bugs C1 and C2.
//!
//! C1: plan_executing flag leaked true when run_plan_execution errors via ?
//! C2: iteration limit compared backtrack_history.len() (all failures) instead of
//!     backtrack_count (plan regenerations only), causing premature halts.

#[path = "mock_provider.rs"]
mod mock_provider;
use mock_provider::MockProvider;

use async_trait::async_trait;
use chrono::Utc;
use klyntbot::plan::{BacktrackEntry, Plan, PlanStatus, PlanStep, PlanStore, StepStatus};
use klyntbot::providers::types::{ChatParams, LlmProvider, LlmResponse, Message, Usage};
use klyntbot::tools::RoutingContext;
use klyntbot::{AgentLoop, MessageBus};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::sync::RwLock;
use uuid::Uuid;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_config(temp_dir: &TempDir) -> klyntbot::Config {
    let mut config = klyntbot::Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();
    config
}

fn approved_plan_with_step(step_desc: &str, max_attempts: u8) -> Plan {
    let now = Utc::now();
    Plan {
        id: Uuid::new_v4(),
        session_key: "cli:test".to_string(),
        goal_id: None,
        title: "Test Plan".to_string(),
        description: "Regression test plan".to_string(),
        status: PlanStatus::Approved,
        steps: vec![PlanStep {
            id: Uuid::new_v4(),
            index: 0,
            description: step_desc.to_string(),
            reasoning: "test".to_string(),
            expected_tools: vec![],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts,
            result: None,
            started_at: None,
            completed_at: None,
        }],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

async fn agent_with_plan_store(
    plan_store: Arc<RwLock<PlanStore>>,
    provider: impl LlmProvider + 'static,
    temp_dir: &TempDir,
) -> AgentLoop {
    let config = make_config(temp_dir);
    let bus = Arc::new(MessageBus::new(10));
    let todo_store = Arc::new(RwLock::new(klyntbot::tools::todo_store::TodoStore::new(
        config.todo_store_path(),
    )));
    let goal_store = Some(Arc::new(RwLock::new(klyntbot::goal::GoalStore::new(
        config.goal_store_path(),
    ))));
    AgentLoop::new_with_cron(
        bus,
        Arc::new(provider),
        config,
        None,
        todo_store,
        goal_store,
        Some(plan_store),
        None,
    )
    .await
    .unwrap()
}

// ─── C1: plan_executing flag leak ─────────────────────────────────────────────

/// Mock provider that returns a "tool-not-found" failure on the first call
/// and an error on the second call (simulating a regenerate_from failure).
///
/// Call 1 (execute_step): tool call to non-existent tool → step fails gracefully (success=false).
/// Call 2 (regenerate_from): returns Err → ? propagates, bypassing flag reset.
struct ToolCallThenErrorProvider {
    call_count: Arc<Mutex<usize>>,
}

impl ToolCallThenErrorProvider {
    fn new() -> Self {
        Self {
            call_count: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl LlmProvider for ToolCallThenErrorProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
    ) -> klyntbot::common::error::Result<LlmResponse> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let call = *count;
        drop(count);

        if call == 1 {
            // Call 1: return a tool call to a non-existent tool (step fails gracefully)
            Ok(LlmResponse {
                content: None,
                tool_calls: vec![klyntbot::providers::types::ToolCall {
                    id: "call_1".to_string(),
                    name: "nonexistent_tool_c1".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        } else {
            // Call 2+: error during regenerate_from — this is the leak trigger
            Err(klyntbot::common::error::KlyntbotError::Provider(
                klyntbot::common::error::ProviderError::InvalidResponse(
                    "simulated regeneration error".to_string(),
                ),
            ))
        }
    }

    fn default_model(&self) -> &str {
        "mock"
    }

    fn name(&self) -> &str {
        "tool-call-then-error"
    }
}

/// C1 regression: plan_executing flag must be false after run_plan_execution errors.
///
/// Given: an approved plan with 1 step (max_attempts=1), using a provider that
///   fails the step gracefully then errors on the regeneration call
/// When: run_plan_execution() returns Err (via ? from regenerate_from)
/// Then: is_plan_executing() is false — the flag was not leaked
#[tokio::test]
async fn test_plan_executing_flag_cleared_after_error() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let plan_store = Arc::new(RwLock::new(PlanStore::new(
        temp_dir.path().join(".klyntbot/data/plans.jsonl"),
    )));

    // Approved plan with 1 step, max_attempts=1 (single failure triggers regeneration)
    let plan = approved_plan_with_step("step that fails", 1);
    let plan_id = plan.id;
    {
        let mut store = plan_store.write().await;
        store.upsert(plan).await.unwrap();
    }

    let provider = ToolCallThenErrorProvider::new();
    let agent = agent_with_plan_store(Arc::clone(&plan_store), provider, &temp_dir).await;

    assert!(
        !agent.is_plan_executing(),
        "flag should be false before execution"
    );

    let routing_ctx = RoutingContext::new("cli".into(), "test".into());
    let result = agent.run_plan_execution(&plan_id, &routing_ctx).await;

    // The provider errors on the regenerate_from call — execution must fail
    assert!(result.is_err(), "run_plan_execution should return Err");

    // C1 regression: flag must be cleared even on error
    assert!(
        !agent.is_plan_executing(),
        "plan_executing flag must be false after error — it leaked (C1 bug)"
    );
}

// ─── C2: iteration limit uses wrong variable ───────────────────────────────────

/// C2 regression: iteration_limit must compare backtrack_count (plan regenerations),
/// not backtrack_history.len() (all step failures including per-step retries).
///
/// Setup:
///   - Plan with iteration_limit=3 and 2 pre-existing backtrack_history entries
///     (simulating 2 prior retries that did NOT trigger a regeneration).
///   - 1 step with max_attempts=1 so a single failure immediately triggers regeneration.
///
/// Execution flow:
///   1. Step fails (tool not found) → backtrack_history.len() becomes 3
///      - Buggy check: 3 >= iteration_limit(3) → HALT (plan never completes)
///      - Fixed check: backtrack_count(1) >= iteration_limit(3) → continue
///   2. regenerate_from() returns 1 new step (via mock provider JSON response)
///   3. New step executes successfully → plan completes
///
/// Then: plan reports "completed: all N steps executed."
#[tokio::test]
async fn test_iteration_limit_uses_regeneration_count_not_history_len() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let plan_store = Arc::new(RwLock::new(PlanStore::new(
        temp_dir.path().join(".klyntbot/data/plans.jsonl"),
    )));

    // Plan with 2 pre-existing history entries (NOT regenerations — just retry records)
    let mut plan = approved_plan_with_step("failing step", 1);
    plan.iteration_limit = 3; // With the bug: 2 + 1 new failure = 3 >= 3 → HALT
    plan.backtrack_history = vec![
        BacktrackEntry {
            step_index: 0,
            attempt: 1,
            failure_reason: "prior retry 1".to_string(),
            timestamp: Utc::now(),
        },
        BacktrackEntry {
            step_index: 0,
            attempt: 1,
            failure_reason: "prior retry 2".to_string(),
            timestamp: Utc::now(),
        },
    ];
    let plan_id = plan.id;
    {
        let mut store = plan_store.write().await;
        store.upsert(plan).await.unwrap();
    }

    // Provider sequence:
    // Call 1 (execute step): tool call to non-existent tool → step fails (success=false)
    // Call 2 (regenerate_from): valid JSON with 1 new step
    // Call 3 (execute new step): text response → step succeeds
    let regen_json = r#"[{"description":"recovered step","reasoning":"retry with new approach","expectedTools":[]}]"#;
    let provider = MockProvider::with_responses(vec![
        // Call 1: tool call to nonexistent tool → step fails gracefully
        LlmResponse {
            content: None,
            tool_calls: vec![klyntbot::providers::types::ToolCall {
                id: "call_1".to_string(),
                name: "nonexistent_tool_c2".to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
        // Call 2: valid regeneration JSON
        LlmResponse {
            content: Some(regen_json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
        // Call 3: text response for the new regenerated step
        LlmResponse {
            content: Some("Recovered step completed.".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
    ]);

    let agent = agent_with_plan_store(Arc::clone(&plan_store), provider, &temp_dir).await;
    let routing_ctx = RoutingContext::new("cli".into(), "test".into());

    let result = agent.run_plan_execution(&plan_id, &routing_ctx).await;

    // C2 regression: plan must complete, not be halted by the iteration limit
    assert!(
        result.is_ok(),
        "plan should complete successfully, not be halted (C2 bug). Error: {:?}",
        result.err()
    );
    let summary = result.unwrap();
    assert!(
        summary.contains("completed: all"),
        "expected completion message, got: {summary}"
    );
}
