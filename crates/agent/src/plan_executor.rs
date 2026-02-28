//! Plan execution — multi-cycle step execution, backtracking, context windowing,
//! and plan-completion goal tracking.
//!
//! Public API:
//! - [`run_step`] — execute a single plan step with multi-cycle LLM-tool loops
//! - [`build_step_context`] — build a context window for the current step
//! - [`regenerate_from`] — regenerate plan steps from a failure point
//! - [`PlanCompletionHandlerImpl`] — records plan outcomes on linked goals

use async_trait::async_trait;
use common::{utils::truncate_at_boundary, Result};
use domain::{Plan, PlanStep, StepStatus, DEFAULT_MAX_STEP_ATTEMPTS};
use providers::{
    types::{ChatParams, Message},
    DynProvider,
};
use tools::plan_tool::PlanCompletionHandler;
use tools::RoutingContext;
use tracing::warn;
use uuid::Uuid;

use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams};
use crate::plan_step_generator::{drafts_to_plan_steps, parse_step_drafts};

/// Maximum number of full backtracking events before the plan is marked Failed.
/// Per-step retries (attempt_count) are separate from this limit.
pub const MAX_BACKTRACK_ATTEMPTS: usize = 3;

/// Maximum LLM cycles per step to prevent infinite tool loops (multi-cycle execution).
pub const MAX_CYCLES_PER_STEP: usize = 5;

/// Result of executing a single plan step.
/// Returned by run_step() to inform the orchestration loop of the outcome.
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

/// Execute a single plan step using multi-cycle LLM-tool execution.
///
/// Runs up to [`MAX_CYCLES_PER_STEP`] cycles through the `ExecutionCore`,
/// accumulating tool results across cycles. Each cycle calls the LLM, which
/// may return tool calls (executed and looped) or a final text response.
pub async fn run_step(
    core: &ExecutionCore,
    step: &PlanStep,
    plan_context: &str,
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
        "{plan_context}\n\n\
         ## Current Step\n\
         {desc}\n\n\
         Reasoning: {reason}\n\
         Suggested tools: {expected}\n\n\
         Execute this step NOW by calling the appropriate tool(s) with the correct arguments. \
         If this step requires modifying data, use the update/write action — do NOT just list or read. \
         Use IDs, values, and context from previous step results as arguments.",
        desc = step.description,
        reason = step.reasoning,
    );

    // 2. Get tool definitions
    let tool_defs = {
        let registry = core.tool_registry.read().await;
        registry.get_definitions()
    };

    // 3. Build messages
    let mut messages = vec![
        Message::system(
            "You are autonomously executing a single step of a multi-step plan. \
             Call the appropriate tools with the correct arguments to accomplish the step.\n\n\
             CRITICAL RULES:\n\
             - You are executing AUTONOMOUSLY. Do NOT call ask_user or request user input. \
               Make reasonable decisions yourself based on context and priorities.\n\
             - If the step says to UPDATE, MODIFY, ADD, or SET a field, you MUST call the tool \
               with the write/update action and the relevant parameters. Do NOT just read or list.\n\
             - Use IDs from previous step results as arguments.\n\
             - For the todo tool: use action=\"update\" with id and the field to change \
               (e.g., due_date in YYYY-MM-DD format, priority, status). \
               Use action=\"add\" to create new tasks.\n\
             - When setting due dates, use reasonable dates based on task priority: \
               P1 = today/tomorrow, P2 = this week, P3 = next week, P4/P5 = 2+ weeks out.\n\
             - After making changes, do NOT re-list to verify — trust the tool output.\n\
             - When done, provide a concise summary of what you changed.",
        ),
        Message::user(prompt),
    ];

    let params = ExecutionParams::new(core.provider.default_model());

    // 4. Multi-cycle execution loop
    let mut first_tool_name: Option<String> = None;
    let mut accumulated_output = Vec::new();

    for _cycle in 0..MAX_CYCLES_PER_STEP {
        let (outcome, _usage) = core
            .run_cycle(&mut messages, &tool_defs, &params, routing_ctx, None, None)
            .await?;

        match outcome {
            CycleOutcome::ToolsExecuted { results } => {
                if first_tool_name.is_none() {
                    if let Some(first) = results.first() {
                        first_tool_name = Some(first.tool_name.clone());
                    }
                }
                for r in &results {
                    if !r.success {
                        return Ok(StepExecutionResult {
                            success: false,
                            output: accumulated_output.join("\n"),
                            failure_reason: Some(format!(
                                "Tool '{}' failed: {}",
                                r.tool_name, r.result
                            )),
                            confidence: None,
                            tool_name: first_tool_name,
                        });
                    }
                    accumulated_output.push(format!("{}: {}", r.tool_name, r.result));
                }
                // Continue to next cycle — tool results already in messages via run_cycle
            }
            CycleOutcome::FinalResponse { content }
            | CycleOutcome::FabricatedResponse { content } => {
                let confidence = confidence_evaluator.and_then(|ev| ev.parse_assessment(&content));
                let output = if accumulated_output.is_empty() {
                    content
                } else {
                    accumulated_output.push(content);
                    accumulated_output.join("\n")
                };
                return Ok(StepExecutionResult {
                    success: true,
                    output,
                    failure_reason: None,
                    confidence,
                    tool_name: first_tool_name,
                });
            }
            CycleOutcome::EmptyResponse => {
                return Ok(StepExecutionResult {
                    success: true,
                    output: if accumulated_output.is_empty() {
                        format!("Step '{}' completed", step.description)
                    } else {
                        accumulated_output.join("\n")
                    },
                    failure_reason: None,
                    confidence: None,
                    tool_name: first_tool_name,
                });
            }
        }
    }

    // Max cycles reached — return what we have
    Ok(StepExecutionResult {
        success: true,
        output: if accumulated_output.is_empty() {
            "Step completed (max cycles reached)".to_string()
        } else {
            accumulated_output.join("\n")
        },
        failure_reason: None,
        confidence: None,
        tool_name: first_tool_name,
    })
}

/// Build context window: completed step results + current step + next 3 steps.
///
/// Includes results from the last 3 completed steps so the LLM has context
/// to generate proper tool arguments for the current step.
/// Results are truncated to 500 characters to keep context manageable.
pub fn build_step_context(plan: &Plan, current_index: usize) -> String {
    let window_end = (current_index + 4).min(plan.steps.len());

    if current_index >= plan.steps.len() {
        return "Plan completed - no active step".to_string();
    }

    let steps_window = &plan.steps[current_index..window_end];

    let mut ctx = format!("## Active Plan: {}\n", plan.title);
    ctx.push_str(&format!("Goal: {}\n", plan.description));
    ctx.push_str(&format!(
        "Progress: step {}/{}\n",
        current_index + 1,
        plan.steps.len()
    ));

    // Include results from the last 3 completed steps for context
    let completed_start = current_index.saturating_sub(3);
    let completed_steps: Vec<_> = plan.steps[completed_start..current_index]
        .iter()
        .filter(|s| s.result.is_some())
        .collect();
    if !completed_steps.is_empty() {
        ctx.push_str("\n## Previous Results\n");
        for step in &completed_steps {
            if let Some(ref result) = step.result {
                let truncated = if result.len() > 500 {
                    format!("{}...(truncated)", truncate_at_boundary(result, 500))
                } else {
                    result.clone()
                };
                ctx.push_str(&format!(
                    "- Step {}: {}\n  Result: {}\n",
                    step.index + 1,
                    step.description,
                    truncated
                ));
            }
        }
    }

    ctx.push('\n');
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
pub async fn regenerate_from(
    plan: &Plan,
    failure_index: usize,
    failure_reason: &str,
    provider: &DynProvider,
) -> Result<Vec<PlanStep>> {
    let completed_summary = summarize_completed_steps(plan, failure_index);

    let failed_step_desc = plan
        .steps
        .get(failure_index)
        .map(|s| s.description.as_str())
        .unwrap_or("unknown step");

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

    let steps = drafts_to_plan_steps(&parse_step_drafts(&content), failure_index);

    if steps.is_empty() {
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
            max_attempts: DEFAULT_MAX_STEP_ATTEMPTS,
            result: None,
            started_at: None,
            completed_at: None,
        }]);
    }

    Ok(steps)
}

/// Summarize completed steps as a numbered list for the regeneration prompt.
fn summarize_completed_steps(plan: &Plan, up_to: usize) -> String {
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

// ===========================================================================
// Plan completion handler — records plan outcomes on linked goals
// ===========================================================================

/// Implements `PlanCompletionHandler` by updating typed goal columns in `GoalRepo`.
///
/// When a plan finishes (success or failure) and has a `goal_id`, this handler
/// updates the goal's plan-completion columns (`plans_completed`, `plans_failed`,
/// `avg_duration_ms`, `last_plan_at`) directly via `GoalRepo` atomic increments.
pub struct PlanCompletionHandlerImpl {
    repo: storage::GoalRepo,
}

impl PlanCompletionHandlerImpl {
    pub fn new(repo: storage::GoalRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl PlanCompletionHandler for PlanCompletionHandlerImpl {
    async fn on_plan_completed(
        &self,
        _plan_id: &Uuid,
        goal_id: Option<Uuid>,
        success: bool,
        _summary: &str,
    ) -> Result<()> {
        let Some(gid) = goal_id else {
            return Ok(());
        };

        if success {
            if let Err(e) = self.repo.increment_completed(gid, 0).await {
                warn!(
                    "Failed to increment plans_completed for goal {}: {}",
                    gid, e
                );
            }
        } else if let Err(e) = self.repo.increment_failed(gid).await {
            warn!("Failed to increment plans_failed for goal {}: {}", gid, e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use domain::{PlanStatus, PlanStep, StepStatus};
    use providers::types::{
        ChatParams, LlmProvider, LlmResponse, ToolCall as ProviderToolCall, Usage,
    };
    use serde_json::Value as JsonValue;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tools::{registry::ToolRegistry, RoutingContext, Tool};
    use uuid::Uuid;

    // ── Helpers ──────────────────────────────────────────────────

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
            visibility: domain::PlanVisibility::default(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
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

    // ── Mock providers ──────────────────────────────────────────

    struct MockLlmRegen {
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

    // ── Constants ────────────────────────────────────────────────

    #[test]
    fn test_max_backtrack_attempts_constant() {
        assert_eq!(MAX_BACKTRACK_ATTEMPTS, 3);
    }

    #[test]
    fn test_max_cycles_per_step_constant() {
        assert_eq!(MAX_CYCLES_PER_STEP, 5);
    }

    // ── StepExecutionResult ─────────────────────────────────────

    #[test]
    fn test_step_execution_result_fields() {
        let success = StepExecutionResult {
            success: true,
            output: "done".to_string(),
            failure_reason: None,
            confidence: None,
            tool_name: Some("echo_tool".to_string()),
        };
        assert!(success.success);
        assert!(success.failure_reason.is_none());

        let failure = StepExecutionResult {
            success: false,
            output: String::new(),
            failure_reason: Some("Tool execution failed".to_string()),
            confidence: None,
            tool_name: None,
        };
        assert!(!failure.success);
        assert_eq!(failure.failure_reason.unwrap(), "Tool execution failed");
    }

    // ── build_step_context ──────────────────────────────────────

    #[test]
    fn test_step_context_window_builds_correctly() {
        let mut plan = test_plan_with_steps(10, "test-session");
        plan.current_step_index = 2;

        let context = build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(context.contains("    NEXT 1: Step 3"));
        assert!(context.contains("    NEXT 2: Step 4"));
        assert!(context.contains("    NEXT 3: Step 5"));
        assert!(!context.contains("Step 6"));
        assert!(!context.contains("Step 1"));
    }

    #[test]
    fn test_step_context_window_at_end() {
        let mut plan = test_plan_with_steps(5, "test-session");
        plan.current_step_index = 3;

        let context = build_step_context(&plan, 3);

        assert!(context.contains(">>> CURRENT: Step 3"));
        assert!(context.contains("    NEXT 1: Step 4"));
        assert!(!context.contains("    NEXT 2"));
    }

    #[test]
    fn test_step_context_window_at_last_step() {
        let mut plan = test_plan_with_steps(3, "test-session");
        plan.current_step_index = 2;

        let context = build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(!context.contains("    NEXT 1"));
    }

    #[test]
    fn test_step_context_window_completed() {
        let plan = test_plan_with_steps(3, "test-session");
        let context = build_step_context(&plan, 5);
        assert_eq!(context, "Plan completed - no active step");
    }

    #[test]
    fn test_step_context_includes_completed_results() {
        let mut plan = test_plan_with_steps(5, "test-session");
        plan.steps[0].status = StepStatus::Completed;
        plan.steps[0].result = Some("Found 3 config files".to_string());
        plan.steps[1].status = StepStatus::Completed;
        plan.steps[1].result = Some("Modified auth.rs".to_string());
        plan.steps[2].status = StepStatus::Completed;
        plan.steps[2].result = Some("Tests pass".to_string());

        let context = build_step_context(&plan, 3);

        assert!(context.contains("Goal: Test"));
        assert!(context.contains("Previous Results"));
        assert!(context.contains("Found 3 config files"));
        assert!(context.contains("Modified auth.rs"));
        assert!(context.contains("Tests pass"));
        assert!(context.contains(">>> CURRENT: Step 3"));
    }

    #[test]
    fn test_step_context_caps_completed_results_at_3() {
        let mut plan = test_plan_with_steps(6, "test-session");
        for i in 0..5 {
            plan.steps[i].status = StepStatus::Completed;
            plan.steps[i].result = Some(format!("Result of step {}", i));
        }

        let context = build_step_context(&plan, 5);

        assert!(context.contains("Result of step 2"));
        assert!(context.contains("Result of step 3"));
        assert!(context.contains("Result of step 4"));
        assert!(!context.contains("Result of step 0"));
        assert!(!context.contains("Result of step 1"));
    }

    // ── regenerate_from ─────────────────────────────────────────

    #[tokio::test]
    async fn test_regenerate_from_replaces_steps_from_index() {
        let mut plan = test_plan_with_steps(3, "test-session");
        plan.steps[0].status = StepStatus::Completed;
        plan.steps[0].result = Some("Step 0 done".to_string());
        plan.steps[1].status = StepStatus::Failed;
        plan.steps[1].attempt_count = 3;

        let json = r#"[
            {"description": "Regen step A", "reasoning": "Alternative approach", "expectedTools": ["echo_tool"]},
            {"description": "Regen step B", "reasoning": "Follow-up", "expectedTools": []}
        ]"#;
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: json.to_string(),
        });

        let new_steps = regenerate_from(&plan, 1, "Tool not found", &provider)
            .await
            .expect("should not Err");

        assert_eq!(new_steps.len(), 2);
        assert_eq!(new_steps[0].description, "Regen step A");
        assert_eq!(new_steps[1].description, "Regen step B");
        for step in &new_steps {
            assert_eq!(step.status, StepStatus::Pending);
            assert_eq!(step.attempt_count, 0);
            assert!(step.result.is_none());
        }
    }

    #[tokio::test]
    async fn test_regenerate_from_fallback_on_empty_llm_response() {
        let plan = test_plan_with_steps(2, "test-session");
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: "I cannot generate replacement steps.".to_string(),
        });

        let new_steps = regenerate_from(&plan, 0, "something went wrong", &provider)
            .await
            .expect("should not Err even on bad LLM output");

        assert_eq!(new_steps.len(), 1);
        let step = &new_steps[0];
        assert_eq!(step.status, StepStatus::Pending);
        assert_eq!(step.attempt_count, 0);
        assert!(
            step.description.contains("Retry") || step.description.contains("Step 0"),
            "fallback step should reference the failed step: {}",
            step.description
        );
    }

    #[tokio::test]
    async fn test_regenerate_from_step_indices_start_at_failure_point() {
        let plan = test_plan_with_steps(5, "test-session");
        let json = r#"[
            {"description": "New step X", "reasoning": "reason X", "expectedTools": []},
            {"description": "New step Y", "reasoning": "reason Y", "expectedTools": []}
        ]"#;
        let provider: DynProvider = Arc::new(MockLlmRegen {
            json_response: json.to_string(),
        });

        let new_steps = regenerate_from(&plan, 3, "timeout", &provider)
            .await
            .expect("should succeed");

        assert_eq!(new_steps.len(), 2);
        assert_eq!(new_steps[0].index, 3);
        assert_eq!(new_steps[1].index, 4);
        assert_eq!(new_steps[0].description, "New step X");
        assert_eq!(new_steps[1].description, "New step Y");
    }

    // ── run_step (multi-cycle execution) ────────────────────────

    #[tokio::test]
    async fn test_run_step_with_text_response() {
        let provider: DynProvider = Arc::new(MockLlmText {
            text: "Step completed via text.".to_string(),
        });
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = ExecutionCore::new(provider, empty_reg);
        let step = test_step(vec![]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let result = run_step(&core, &step, "plan ctx", &ctx, None)
            .await
            .expect("should not Err");

        assert!(result.success);
        assert_eq!(result.output, "Step completed via text.");
        assert!(result.tool_name.is_none());
    }

    #[tokio::test]
    async fn test_run_step_with_tool_execution() {
        let provider: DynProvider = Arc::new(MockLlmToolCall {
            tool_name: "echo_tool".to_string(),
        });
        let core = ExecutionCore::new(provider, reg_with_echo());
        let step = test_step(vec!["echo_tool".to_string()]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let result = run_step(&core, &step, "plan ctx", &ctx, None)
            .await
            .expect("should not Err");

        assert!(result.success);
        assert!(result.output.contains("echo_tool"));
        assert_eq!(result.tool_name, Some("echo_tool".to_string()));
    }

    #[tokio::test]
    async fn test_run_step_missing_tool_fails() {
        let provider: DynProvider = Arc::new(MockLlmToolCall {
            tool_name: "ghost_tool".to_string(),
        });
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = ExecutionCore::new(provider, empty_reg);
        let step = test_step(vec!["ghost_tool".to_string()]);
        let ctx = RoutingContext::new("cli".into(), "test".into());

        let result = run_step(&core, &step, "plan ctx", &ctx, None)
            .await
            .expect("should return Ok(failure), not Err");

        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_run_step_handles_empty_response() {
        let provider: DynProvider = Arc::new(MockLlmText {
            text: String::new(),
        });
        let step = test_step(vec![]);
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let empty_reg = Arc::new(RwLock::new(ToolRegistry::new()));
        let core = ExecutionCore::new(provider, empty_reg);

        let result = run_step(&core, &step, "ctx", &ctx, None)
            .await
            .expect("should not Err");

        assert!(result.success, "empty response should still succeed");
    }
}
