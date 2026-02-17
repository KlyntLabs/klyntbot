//! PlanExecutor — ReAct loop, backtracking, and context windowing for plans.

use common::Result;
use plan::{Plan, PlanStep};
use providers::DynProvider;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::{registry::ToolRegistry, RoutingContext};

/// Result of executing a single plan step.
/// Returned by execute_step() to inform the orchestration loop of the outcome.
#[derive(Debug)]
pub struct StepExecutionResult {
    /// Whether the step completed successfully
    pub success: bool,
    /// Captured output from tool execution or LLM response
    pub output: String,
    /// Reason for failure if success is false
    pub failure_reason: Option<String>,
}

/// PlanExecutor handles step-by-step plan execution with backtracking.
pub struct PlanExecutor {
    // Phase 4: Fields added for step execution
    // These are passed as parameters to execute_step() to avoid storing
    // duplicates (AgentLoop already owns these Arc references)
}

impl PlanExecutor {
    /// Create a new PlanExecutor.
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a single plan step using the LLM + tool registry.
    ///
    /// Generates tool calls from the step description, executes them via the
    /// tool registry, and captures results. Called by run_plan_execution() in
    /// agent_loop.rs for each step.
    ///
    /// # Arguments
    /// - `step`: The plan step to execute
    /// - `plan_context`: Formatted context string from build_step_context()
    /// - `provider`: LLM provider for generating tool calls
    /// - `tool_registry`: Registry of available tools
    /// - `routing_ctx`: Channel/chat routing context
    ///
    /// # Implementation (Phase 4A - dev-3)
    /// TODO: Implement full ReAct loop:
    /// 1. Build system prompt with plan context + step description
    /// 2. Call provider.chat() to get tool calls
    /// 3. Execute tools via tool_registry.execute()
    /// 4. Capture and return results
    pub async fn execute_step(
        &self,
        step: &PlanStep,
        _plan_context: &str,
        _provider: &DynProvider,
        _tool_registry: &Arc<RwLock<ToolRegistry>>,
        _routing_ctx: &RoutingContext,
    ) -> Result<StepExecutionResult> {
        // Stub implementation — dev-3 will implement the full ReAct loop.
        // Returns success with a placeholder output to allow orchestration
        // logic in run_plan_execution() to be built and tested independently.
        Ok(StepExecutionResult {
            success: true,
            output: format!(
                "[Stub] Step '{}' acknowledged. Full execution coming in Phase 4A.",
                step.description
            ),
            failure_reason: None,
        })
    }

    /// Build context window: current step + next 3 steps.
    /// Returns formatted context string for LLM.
    pub fn build_step_context(&self, plan: &Plan, current_index: usize) -> String {
        let window_end = (current_index + 4).min(plan.steps.len());

        if current_index >= plan.steps.len() {
            return "Plan completed - no active step".to_string();
        }

        let steps_window = &plan.steps[current_index..window_end];

        let mut ctx = format!("## Active Plan: {}\n", plan.title);
        ctx.push_str(&format!(
            "Progress: step {}/{}\n\n",
            current_index + 1,
            plan.steps.len()
        ));

        for (i, step) in steps_window.iter().enumerate() {
            let marker = if i == 0 {
                ">>> CURRENT"
            } else {
                match i {
                    1 => "    NEXT 1",
                    2 => "    NEXT 2",
                    3 => "    NEXT 3",
                    _ => "    NEXT",
                }
            };
            ctx.push_str(&format!(
                "{}: {}\n  Reasoning: {}\n",
                marker, step.description, step.reasoning
            ));
        }
        ctx
    }
}

impl Default for PlanExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use plan::{PlanStatus, PlanStep, StepStatus};
    use uuid::Uuid;

    /// Helper to create a test Plan with N steps
    fn test_plan_with_steps(step_count: usize, session_key: &str) -> Plan {
        let now = Utc::now();
        Plan {
            id: Uuid::new_v4(),
            session_key: session_key.to_string(),
            goal_id: None,
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            status: PlanStatus::Approved,
            steps: (0..step_count)
                .map(|i| PlanStep {
                    id: Uuid::new_v4(),
                    index: i,
                    description: format!("Step {}", i),
                    reasoning: format!("Reasoning {}", i),
                    expected_tools: vec![],
                    status: StepStatus::Pending,
                    attempt_count: 0,
                    max_attempts: 3,
                    result: None,
                    started_at: None,
                    completed_at: None,
                })
                .collect(),
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    #[test]
    fn test_step_context_window_builds_correctly() {
        // Test 17
        // Given: a Plan with 10 steps, current_step_index = 2
        // When: build_step_context() is called
        // Then:
        //   - context includes steps 2, 3, 4, 5 (current + next 3)
        //   - current step is marked ">>> CURRENT"
        //   - next steps are marked "    NEXT 1", "    NEXT 2", "    NEXT 3"
        // Maps to: US-3 (AC-3.2)

        let mut plan = test_plan_with_steps(10, "test-session");
        plan.current_step_index = 2;

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(context.contains("    NEXT 1: Step 3"));
        assert!(context.contains("    NEXT 2: Step 4"));
        assert!(context.contains("    NEXT 3: Step 5"));
        assert!(!context.contains("Step 6")); // Outside window
        assert!(!context.contains("Step 1")); // Before window
    }

    #[test]
    fn test_step_context_window_at_end() {
        // Edge case: current step is near the end
        let mut plan = test_plan_with_steps(5, "test-session");
        plan.current_step_index = 3; // Only 2 steps left (3 and 4)

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 3);

        assert!(context.contains(">>> CURRENT: Step 3"));
        assert!(context.contains("    NEXT 1: Step 4"));
        assert!(!context.contains("    NEXT 2")); // No step 5
        assert!(!context.contains("    NEXT 3")); // No step 6
    }

    #[test]
    fn test_step_context_window_at_last_step() {
        // Edge case: current step is the last step
        let mut plan = test_plan_with_steps(3, "test-session");
        plan.current_step_index = 2; // Last step

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(!context.contains("    NEXT 1")); // No next steps
    }

    #[test]
    fn test_step_context_window_completed() {
        // Edge case: plan completed (index beyond steps)
        let plan = test_plan_with_steps(3, "test-session");

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 5); // Beyond steps

        assert_eq!(context, "Plan completed - no active step");
    }

    // NOTE: Tests 18-19 (backtracking) will be implemented in Phase 4 (Agent Integration)
    // as they require full agent loop and tool execution context.

    // NOTE: Test 20 (iteration limit switching) belongs in agent_loop.rs tests,
    // not in plan_executor.rs.

    #[test]
    fn test_step_execution_result_fields() {
        // Test 22: StepExecutionResult has the correct fields
        // Validates the interface contract for dev-3's implementation

        let success_result = StepExecutionResult {
            success: true,
            output: "Step completed with output".to_string(),
            failure_reason: None,
        };
        assert!(success_result.success);
        assert_eq!(success_result.output, "Step completed with output");
        assert!(success_result.failure_reason.is_none());

        let failure_result = StepExecutionResult {
            success: false,
            output: String::new(),
            failure_reason: Some("Tool execution failed".to_string()),
        };
        assert!(!failure_result.success);
        assert!(failure_result.failure_reason.is_some());
        assert_eq!(
            failure_result.failure_reason.unwrap(),
            "Tool execution failed"
        );
    }

    // --- Tests 23-26: execute_step() behavioural tests (Phase 4A - dev-3) ---

    use async_trait::async_trait;
    use providers::types::{ChatParams, LlmProvider, LlmResponse, ToolCall as ProviderToolCall, Usage};
    use serde_json::Value as JsonValue;
    use tools::{registry::ToolRegistry, RoutingContext, Tool};

    struct MockLlmToolCall { tool_name: String }

    #[async_trait]
    impl LlmProvider for MockLlmToolCall {
        async fn chat(
            &self,
            _messages: &[providers::types::Message],
            _tools: Option<&[JsonValue]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            Ok(LlmResponse {
                content: None,
                tool_calls: vec![ProviderToolCall {
                    id: "call_1".to_string(),
                    name: self.tool_name.clone(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
    }

    struct MockLlmText { text: String }

    #[async_trait]
    impl LlmProvider for MockLlmText {
        async fn chat(
            &self,
            _messages: &[providers::types::Message],
            _tools: Option<&[JsonValue]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.text.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
    }

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str { "echo_tool" }
        fn description(&self) -> &str { "Echoes back" }
        fn parameters(&self) -> JsonValue {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: JsonValue, _ctx: &RoutingContext) -> common::Result<String> {
            Ok("echo result".to_string())
        }
    }

    fn reg_with_echo() -> Arc<RwLock<ToolRegistry>> {
        let mut r = ToolRegistry::new();
        r.register(EchoTool);
        Arc::new(RwLock::new(r))
    }

    fn test_step(tools: Vec<String>) -> PlanStep {
        PlanStep {
            id: Uuid::new_v4(),
            index: 0,
            description: "Run the thing".to_string(),
            reasoning: "because".to_string(),
            expected_tools: tools,
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        }
    }

    // Test 23: provider returns tool call → tool found in registry → success + output
    #[tokio::test]
    async fn test_execute_step_runs_tool_and_captures_output() {
        let executor = PlanExecutor::new();
        let provider: DynProvider = Arc::new(MockLlmToolCall {
            tool_name: "echo_tool".to_string(),
        });
        let step = test_step(vec!["echo_tool".to_string()]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let res = executor
            .execute_step(&step, "plan ctx", &provider, &reg_with_echo(), &ctx)
            .await
            .expect("execute_step should not return Err");

        assert!(res.success, "expected success; failure: {:?}", res.failure_reason);
        assert!(res.output.contains("echo result"),
            "expected 'echo result' in output, got: {}", res.output);
    }

    // Test 24: provider returns text-only → text becomes output → success
    #[tokio::test]
    async fn test_execute_step_uses_provider_text_when_no_tool_calls() {
        let executor = PlanExecutor::new();
        let provider: DynProvider = Arc::new(MockLlmText {
            text: "Completed by reasoning.".to_string(),
        });
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));
        let step = test_step(vec![]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let res = executor
            .execute_step(&step, "plan ctx", &provider, &empty_reg, &ctx)
            .await
            .expect("should not Err");

        assert!(res.success);
        assert_eq!(res.output, "Completed by reasoning.");
    }

    // Test 25: provider requests tool not in registry → failure result (not Err)
    #[tokio::test]
    async fn test_execute_step_graceful_failure_on_missing_tool() {
        let executor = PlanExecutor::new();
        let provider: DynProvider = Arc::new(MockLlmToolCall {
            tool_name: "ghost_tool".to_string(),
        });
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));
        let step = test_step(vec!["ghost_tool".to_string()]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let res = executor
            .execute_step(&step, "plan ctx", &provider, &empty_reg, &ctx)
            .await
            .expect("execute_step must return Ok even on tool-not-found");

        assert!(!res.success, "should fail when tool is missing");
        let reason = res.failure_reason.expect("failure_reason should be set");
        assert!(
            reason.to_lowercase().contains("ghost_tool") || reason.to_lowercase().contains("not found"),
            "failure reason should mention the missing tool: {reason}"
        );
    }

    // Test 26: empty provider response (no text, no tools) → step still succeeds
    #[tokio::test]
    async fn test_execute_step_handles_empty_provider_response() {
        let executor = PlanExecutor::new();
        let provider: DynProvider = Arc::new(MockLlmText {
            text: String::new(),
        });
        let step = test_step(vec![]);
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));

        let res = executor
            .execute_step(&step, "ctx", &provider, &empty_reg, &ctx)
            .await
            .expect("should not Err");

        assert!(res.success, "empty response should still succeed");
    }
}
