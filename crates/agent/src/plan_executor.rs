//! PlanExecutor — ReAct loop, backtracking, and context windowing for plans.

use common::{error::PlanError, Result};
use plan::{Plan, PlanStep, StepStatus};
use providers::{
    types::{ChatParams, Message},
    DynProvider,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::{registry::ToolRegistry, RoutingContext};
use uuid::Uuid;

/// Maximum number of full backtracking events before the plan is marked Failed.
/// Per-step retries (attempt_count) are separate from this limit.
pub const MAX_BACKTRACK_ATTEMPTS: usize = 3;

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
    /// Confidence assessment from the LLM response, if available
    pub confidence: Option<crate::confidence::ConfidenceAssessment>,
    /// The actual tool name executed (first tool call), for outcome recording.
    /// None when the step completed via LLM text response (no tool calls).
    pub tool_name: Option<String>,
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
    /// # Known limitation
    /// This is a single-cycle implementation: one LLM call per step.
    /// A full ReAct loop (multi-cycle with reflection) is available via
    /// `PlanExecuteEngine` in `execution/plan_execute.rs`.
    pub async fn execute_step(
        &self,
        step: &PlanStep,
        plan_context: &str,
        provider: &DynProvider,
        tool_registry: &Arc<RwLock<ToolRegistry>>,
        routing_ctx: &RoutingContext,
        confidence_evaluator: Option<&crate::confidence::ConfidenceEvaluator>,
    ) -> Result<StepExecutionResult> {
        // 1. Build prompt from plan context + step details
        let expected = if step.expected_tools.is_empty() {
            "none specified".to_string()
        } else {
            step.expected_tools.join(", ")
        };
        let prompt = format!(
            "{plan_context}\n\nCurrent step: {desc}\nReasoning: {reason}\nExpected tools: {expected}",
            desc = step.description,
            reason = step.reasoning,
        );

        // 2. Get tool definitions (read lock only, then release)
        let tool_defs = {
            let registry = tool_registry.read().await;
            registry.get_definitions()
        };

        // 3. Build messages and call the LLM provider
        let messages = vec![
            Message::system(
                "You are executing a single step of a multi-step plan. \
                 Use the available tools to complete the step, then stop.",
            ),
            Message::user(prompt),
        ];
        let params = ChatParams::new(provider.default_model());
        let tool_slice = if tool_defs.is_empty() {
            None
        } else {
            Some(tool_defs.as_slice())
        };
        let response = provider.chat(&messages, tool_slice, &params).await?;

        // Parse confidence assessment from LLM response content (best-effort)
        let confidence = confidence_evaluator
            .and_then(|ev| ev.parse_assessment(response.content.as_deref().unwrap_or("")));

        // 4. If the LLM returned tool calls, execute each one via the registry
        if !response.tool_calls.is_empty() {
            // Capture the first tool name for outcome recording
            let first_tool_name = response.tool_calls[0].name.clone();
            let mut results = Vec::new();
            for tool_call in &response.tool_calls {
                // Clone the Arc<dyn Tool> out while holding the lock, then
                // release the lock before the async execute() call.
                let tool = {
                    let registry = tool_registry.read().await;
                    registry.get(&tool_call.name)
                };
                match tool {
                    Some(t) => match t.execute(tool_call.arguments.clone(), routing_ctx).await {
                        Ok(out) => results.push(format!("{}: {}", tool_call.name, out)),
                        Err(e) => {
                            return Ok(StepExecutionResult {
                                success: false,
                                output: String::new(),
                                failure_reason: Some(format!(
                                    "Tool '{}' execution failed: {e}",
                                    tool_call.name
                                )),
                                confidence,
                                tool_name: Some(first_tool_name),
                            });
                        }
                    },
                    None => {
                        return Ok(StepExecutionResult {
                            success: false,
                            output: String::new(),
                            failure_reason: Some(format!(
                                "Tool '{}' not found in registry",
                                tool_call.name
                            )),
                            confidence,
                            tool_name: Some(first_tool_name),
                        });
                    }
                }
            }
            return Ok(StepExecutionResult {
                success: true,
                output: results.join("\n"),
                failure_reason: None,
                confidence,
                tool_name: Some(first_tool_name),
            });
        }

        // 5. No tool calls — use the text response as the step output
        let output = response
            .content
            .unwrap_or_else(|| format!("Step '{}' completed", step.description));
        Ok(StepExecutionResult {
            success: true,
            output,
            failure_reason: None,
            confidence,
            tool_name: None,
        })
    }

    /// Regenerate plan steps from a failure point using the LLM.
    ///
    /// Called when a step exceeds its `max_attempts` retry limit. Uses the plan's
    /// context (title, description, completed steps) to prompt the LLM for new
    /// steps from `failure_index` forward.
    ///
    /// Returns the new `Vec<PlanStep>` to be appended from `failure_index`.
    /// The caller is responsible for:
    /// - Truncating `plan.steps` at `failure_index`
    /// - Extending with the returned steps
    /// - Enforcing `MAX_BACKTRACK_ATTEMPTS` (tracked in the calling loop)
    ///
    /// # Arguments
    /// - `plan`: The plan being executed (read-only context)
    /// - `failure_index`: Step index at which execution failed
    /// - `failure_reason`: Human-readable explanation of the failure
    /// - `provider`: LLM provider for generating new steps
    pub async fn regenerate_from(
        &self,
        plan: &Plan,
        failure_index: usize,
        failure_reason: &str,
        provider: &DynProvider,
    ) -> Result<Vec<PlanStep>> {
        // Summarize completed steps to give context to the LLM
        let completed_summary = self.summarize_completed_steps(plan, failure_index);

        let failed_step_desc = plan
            .steps
            .get(failure_index)
            .map(|s| s.description.as_str())
            .unwrap_or("unknown step");

        // Build regeneration prompt
        let prompt = format!(
            "A multi-step plan failed partway through and needs replanning.\n\
             \n\
             Plan: {title}\n\
             Goal: {desc}\n\
             \n\
             Completed steps (do NOT redo these):\n{completed}\n\
             \n\
             Failed at step {n}: \"{failed}\"\n\
             Failure reason: {reason}\n\
             \n\
             Generate ONLY the remaining steps needed to complete the plan from this point.\n\
             Respond with a JSON array (no other text):\n\
             [{{\"description\": \"...\", \"reasoning\": \"...\", \"expectedTools\": []}}]",
            title = plan.title,
            desc = plan.description,
            completed = completed_summary,
            n = failure_index + 1,
            failed = failed_step_desc,
            reason = failure_reason,
        );

        let messages = vec![
            Message::system(
                "You are a planning agent. Respond ONLY with a valid JSON array of steps. \
                 No markdown, no explanation — just the JSON array.",
            ),
            Message::user(prompt),
        ];
        let params = ChatParams::new(provider.default_model());
        let response = provider.chat(&messages, None, &params).await?;

        let content = response.content.unwrap_or_default();

        // Attempt to parse LLM response; fall back to a single retry step on failure
        let steps = self
            .parse_steps_from_json(&content, failure_index)
            .unwrap_or_default();

        if steps.is_empty() {
            // LLM returned empty or unparseable output — create a minimal retry step
            let failed_desc = plan
                .steps
                .get(failure_index)
                .map(|s| s.description.as_str())
                .unwrap_or("failed step");
            return Ok(vec![PlanStep {
                id: Uuid::new_v4(),
                index: failure_index,
                description: format!("Retry: {failed_desc}"),
                reasoning: format!("Previous attempt failed: {failure_reason}"),
                expected_tools: vec![],
                status: StepStatus::Pending,
                attempt_count: 0,
                max_attempts: 3,
                result: None,
                started_at: None,
                completed_at: None,
            }]);
        }

        Ok(steps)
    }

    /// Summarize completed steps as a numbered list for the regeneration prompt.
    fn summarize_completed_steps(&self, plan: &Plan, up_to: usize) -> String {
        let end = up_to.min(plan.steps.len());
        if end == 0 {
            return "(none)".to_string();
        }
        plan.steps[..end]
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Parse a JSON array of step objects returned by the LLM into PlanStep values.
    fn parse_steps_from_json(&self, json_str: &str, start_index: usize) -> Result<Vec<PlanStep>> {
        // Tolerate LLMs wrapping the array in markdown fences or prose
        let trimmed = self.extract_json_array(json_str);

        let raw: Vec<serde_json::Value> = serde_json::from_str(trimmed).map_err(|e| {
            PlanError::GenerationFailed(format!(
                "Regeneration response was not valid JSON: {e}\nResponse: {json_str}"
            ))
        })?;

        let steps = raw
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let description = v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let reasoning = v
                    .get("reasoning")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                let expected_tools = v
                    .get("expectedTools")
                    .and_then(|t| t.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                PlanStep {
                    id: Uuid::new_v4(),
                    index: start_index + i,
                    description,
                    reasoning,
                    expected_tools,
                    status: StepStatus::Pending,
                    attempt_count: 0,
                    max_attempts: 3,
                    result: None,
                    started_at: None,
                    completed_at: None,
                }
            })
            .collect();

        Ok(steps)
    }

    /// Extract a JSON array substring from LLM output that may contain prose or markdown.
    fn extract_json_array<'a>(&self, s: &'a str) -> &'a str {
        if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
            if start < end {
                return &s[start..=end];
            }
        }
        s
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
            confidence: None,
            tool_name: Some("echo_tool".to_string()),
        };
        assert!(success_result.success);
        assert_eq!(success_result.output, "Step completed with output");
        assert!(success_result.failure_reason.is_none());

        let failure_result = StepExecutionResult {
            success: false,
            output: String::new(),
            failure_reason: Some("Tool execution failed".to_string()),
            confidence: None,
            tool_name: None,
        };
        assert!(!failure_result.success);
        assert!(failure_result.failure_reason.is_some());
        assert_eq!(
            failure_result.failure_reason.unwrap(),
            "Tool execution failed"
        );
    }

    // --- Tests 18-19: regenerate_from() backtracking tests (Phase 4B - dev-1) ---

    struct MockLlmRegen {
        /// JSON string to return as content (simulates LLM generating new steps)
        json_response: String,
    }

    #[async_trait]
    impl LlmProvider for MockLlmRegen {
        async fn chat(
            &self,
            _messages: &[providers::types::Message],
            _tools: Option<&[JsonValue]>,
            _params: &ChatParams,
        ) -> common::Result<providers::types::LlmResponse> {
            Ok(providers::types::LlmResponse {
                content: Some(self.json_response.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: providers::types::Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock_regen"
        }
    }

    // Test 18: regenerate_from() preserves completed steps and creates new steps
    #[tokio::test]
    async fn test_regenerate_from_replaces_steps_from_index() {
        // Given: a plan with 3 steps, step 0 completed, step 1 failed
        // When: regenerate_from() is called with from_index=1
        // Then:
        //   - Returns new steps (not containing the failed step data)
        //   - New steps have Pending status and attempt_count=0
        //   - Count of new steps matches what LLM returned

        let executor = PlanExecutor::new();
        let mut plan = test_plan_with_steps(3, "test-session");

        // Mark step 0 as completed
        plan.steps[0].status = plan::StepStatus::Completed;
        plan.steps[0].result = Some("Step 0 done".to_string());

        // Mark step 1 as failed
        plan.steps[1].status = plan::StepStatus::Failed;
        plan.steps[1].attempt_count = 3;

        // LLM returns 2 replacement steps
        let json = r#"[
            {"description": "Regen step A", "reasoning": "Alternative approach", "expected_tools": ["echo_tool"]},
            {"description": "Regen step B", "reasoning": "Follow-up", "expected_tools": []}
        ]"#;
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: json.to_string(),
        });

        let new_steps = executor
            .regenerate_from(&plan, 1, "Tool not found", &provider)
            .await
            .expect("regenerate_from should not Err");

        assert_eq!(new_steps.len(), 2, "should return 2 replacement steps");
        assert_eq!(new_steps[0].description, "Regen step A");
        assert_eq!(new_steps[1].description, "Regen step B");

        // All new steps start fresh
        for step in &new_steps {
            assert_eq!(step.status, plan::StepStatus::Pending);
            assert_eq!(step.attempt_count, 0);
            assert!(step.result.is_none());
        }
    }

    // Test 19: regenerate_from() falls back to a single retry step when LLM returns empty
    #[tokio::test]
    async fn test_regenerate_from_fallback_on_empty_llm_response() {
        // Given: a plan with steps
        // When: regenerate_from() is called and the LLM returns no valid JSON steps
        // Then:
        //   - Returns a single fallback step (not an Err)
        //   - Fallback step contains the failed step's description
        //   - Fallback step has Pending status

        let executor = PlanExecutor::new();
        let plan = test_plan_with_steps(2, "test-session");

        // LLM returns empty or invalid JSON
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: "I cannot generate replacement steps.".to_string(),
        });

        let new_steps = executor
            .regenerate_from(&plan, 0, "something went wrong", &provider)
            .await
            .expect("should not Err even on bad LLM output");

        assert_eq!(new_steps.len(), 1, "should have exactly 1 fallback step");
        let step = &new_steps[0];
        assert_eq!(step.status, plan::StepStatus::Pending);
        assert_eq!(step.attempt_count, 0);
        assert!(
            step.description.contains("Retry") || step.description.contains("Step 0"),
            "fallback step should reference the failed step: {}",
            step.description
        );
    }

    // --- Tests 23-26: execute_step() behavioural tests (Phase 4A - dev-3) ---

    use async_trait::async_trait;
    use providers::types::{
        ChatParams, LlmProvider, LlmResponse, ToolCall as ProviderToolCall, Usage,
    };
    use serde_json::Value as JsonValue;
    use tools::{registry::ToolRegistry, RoutingContext, Tool};

    struct MockLlmToolCall {
        tool_name: String,
    }

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
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    struct MockLlmText {
        text: String,
    }

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
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo_tool"
        }
        fn description(&self) -> &str {
            "Echoes back"
        }
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
            .execute_step(&step, "plan ctx", &provider, &reg_with_echo(), &ctx, None)
            .await
            .expect("execute_step should not return Err");

        assert!(
            res.success,
            "expected success; failure: {:?}",
            res.failure_reason
        );
        assert!(
            res.output.contains("echo result"),
            "expected 'echo result' in output, got: {}",
            res.output
        );
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
            .execute_step(&step, "plan ctx", &provider, &empty_reg, &ctx, None)
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
            .execute_step(&step, "plan ctx", &provider, &empty_reg, &ctx, None)
            .await
            .expect("execute_step must return Ok even on tool-not-found");

        assert!(!res.success, "should fail when tool is missing");
        let reason = res.failure_reason.expect("failure_reason should be set");
        assert!(
            reason.to_lowercase().contains("ghost_tool")
                || reason.to_lowercase().contains("not found"),
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
            .execute_step(&step, "ctx", &provider, &empty_reg, &ctx, None)
            .await
            .expect("should not Err");

        assert!(res.success, "empty response should still succeed");
    }

    // Test 27: Verify MAX_BACKTRACK_ATTEMPTS constant is correctly set
    #[test]
    fn test_max_backtrack_attempts_constant() {
        // The backtrack limit is 3: checked in run_plan_execution() before calling
        // regenerate_from(). After 3 full backtrack events the plan is marked Failed.
        assert_eq!(
            MAX_BACKTRACK_ATTEMPTS, 3,
            "MAX_BACKTRACK_ATTEMPTS should be 3 (architecture decision)"
        );
    }

    // Test 28: regenerate_from() returns new steps with correct starting index
    #[tokio::test]
    async fn test_regenerate_from_step_indices_start_at_failure_point() {
        let executor = PlanExecutor::new();
        let plan = test_plan_with_steps(5, "test-session");

        let json = r#"[
            {"description": "New step X", "reasoning": "reason X", "expected_tools": []},
            {"description": "New step Y", "reasoning": "reason Y", "expected_tools": []}
        ]"#;
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: json.to_string(),
        });

        // Failing at step index 3 — new steps should start indexing from 3
        let new_steps = executor
            .regenerate_from(&plan, 3, "timeout", &provider)
            .await
            .expect("should succeed");

        assert_eq!(new_steps.len(), 2);
        assert_eq!(new_steps[0].index, 3);
        assert_eq!(new_steps[1].index, 4);
        assert_eq!(new_steps[0].description, "New step X");
        assert_eq!(new_steps[1].description, "New step Y");
    }
}
