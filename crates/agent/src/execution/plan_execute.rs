//! Redesigned PlanExecute Engine — drives multi-step plan execution with real
//! parameter generation, rich step context, and periodic reflection checkpoints.
//!
//! Key improvements over the legacy plan_executor.rs:
//! - Real parameter generation via step description in LLM prompts
//! - Rich step context (accumulated results, remaining steps)
//! - Reflection checkpoint every N steps ("are we on track?")
//! - Returns structured PlanExecuteOutcome

use std::sync::Arc;

use tokio::sync::RwLock;

use chrono::Utc;
use common::Result;
use plan::PlanStore;
use providers::{DynProvider, Message};
use tools::{registry::ToolRegistry, RoutingContext};
use uuid::Uuid;

use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams};

/// Reflection checkpoint interval — ask LLM to reflect every N steps.
const REFLECTION_INTERVAL: usize = 5;

/// Maximum LLM cycles per step to prevent infinite tool loops.
const MAX_CYCLES_PER_STEP: usize = 5;

/// Redesigned plan execution engine.
///
/// Wraps an `ExecutionCore` and a `PlanStore`, driving step-by-step execution
/// with rich context injection and periodic reflection.
pub struct PlanExecuteEngine {
    core: ExecutionCore,
    plan_store: Arc<RwLock<PlanStore>>,
}

/// Outcome of a plan execution run.
#[derive(Debug)]
pub enum PlanExecuteOutcome {
    /// All steps completed successfully.
    Completed {
        plan_id: String,
        steps_executed: u32,
    },
    /// A step failed and execution stopped.
    Failed {
        plan_id: String,
        step: u32,
        reason: String,
    },
    /// The plan needs more information to continue.
    NeedsMoreInfo { plan_id: String, question: String },
}

impl PlanExecuteEngine {
    pub fn new(
        provider: DynProvider,
        tool_registry: Arc<RwLock<ToolRegistry>>,
        plan_store: Arc<RwLock<PlanStore>>,
    ) -> Self {
        Self {
            core: ExecutionCore::new(provider, tool_registry),
            plan_store,
        }
    }

    /// Execute a plan by ID, driving each step through the ExecutionCore.
    ///
    /// For each step:
    /// 1. Build a rich system prompt with step description, previous results, remaining steps
    /// 2. Run `ExecutionCore::run_cycle()` in a loop until a final response or max cycles
    /// 3. Record the step result
    /// 4. Every `REFLECTION_INTERVAL` steps, ask the LLM for a reflection checkpoint
    pub async fn execute_plan(
        &self,
        plan_id: &str,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<PlanExecuteOutcome> {
        let plan_uuid = plan_id.parse::<Uuid>().map_err(|e| {
            common::error::KlyntbotError::Plan(common::error::PlanError::NotFound(format!(
                "Invalid plan ID: {}",
                e
            )))
        })?;

        // Load the plan
        let plan = {
            let mut store = self.plan_store.write().await;
            store.get(&plan_uuid).await?
        };

        let plan = match plan {
            Some(p) => p,
            None => {
                return Ok(PlanExecuteOutcome::Failed {
                    plan_id: plan_id.to_string(),
                    step: 0,
                    reason: "Plan not found".to_string(),
                });
            }
        };

        let steps: Vec<(usize, String, Vec<String>)> = plan
            .steps
            .iter()
            .map(|s| (s.index, s.description.clone(), s.expected_tools.clone()))
            .collect();

        if steps.is_empty() {
            return Ok(PlanExecuteOutcome::Completed {
                plan_id: plan_id.to_string(),
                steps_executed: 0,
            });
        }

        let mut accumulated_results: Vec<String> = Vec::new();
        let mut steps_executed: u32 = 0;

        for (i, (step_index, description, expected_tools)) in steps.iter().enumerate() {
            // Update step status to Executing
            {
                let mut store = self.plan_store.write().await;
                if let Some(plan) = store.get_mut(&plan_uuid).await? {
                    if let Some(step) = plan.steps.get_mut(*step_index) {
                        step.status = plan::StepStatus::Executing;
                        step.started_at = Some(Utc::now());
                        step.attempt_count += 1;
                    }
                    store.persist_latest(&plan_uuid).await?;
                }
            }

            // Build the step prompt
            let system_prompt = build_step_prompt(
                description,
                expected_tools,
                &accumulated_results,
                &steps[i + 1..],
            );

            let mut messages = vec![
                Message::system(system_prompt),
                Message::user(format!("Execute step {}: {}", step_index + 1, description)),
            ];

            // Run cycles for this step
            let step_result = self
                .run_step_cycles(&mut messages, tools, params, ctx)
                .await;

            match step_result {
                Ok(content) => {
                    accumulated_results.push(format!(
                        "Step {} result: {}",
                        step_index + 1,
                        content
                    ));

                    // Mark step completed
                    {
                        let mut store = self.plan_store.write().await;
                        if let Some(plan) = store.get_mut(&plan_uuid).await? {
                            if let Some(step) = plan.steps.get_mut(*step_index) {
                                step.status = plan::StepStatus::Completed;
                                step.completed_at = Some(Utc::now());
                                step.result = Some(content);
                            }
                            store.persist_latest(&plan_uuid).await?;
                        }
                    }

                    steps_executed += 1;

                    // Reflection checkpoint
                    if steps_executed as usize % REFLECTION_INTERVAL == 0 && i + 1 < steps.len() {
                        let reflection = self
                            .run_reflection(&accumulated_results, &steps[i + 1..], params)
                            .await;
                        if let Ok(ref text) = reflection {
                            accumulated_results.push(format!("Reflection: {}", text));
                        }
                    }
                }
                Err(e) => {
                    let reason = format!("{}", e);

                    // Mark step failed
                    {
                        let mut store = self.plan_store.write().await;
                        if let Some(plan) = store.get_mut(&plan_uuid).await? {
                            if let Some(step) = plan.steps.get_mut(*step_index) {
                                step.status = plan::StepStatus::Failed;
                                step.completed_at = Some(Utc::now());
                                step.result = Some(reason.clone());
                            }
                            store.persist_latest(&plan_uuid).await?;
                        }
                    }

                    return Ok(PlanExecuteOutcome::Failed {
                        plan_id: plan_id.to_string(),
                        step: *step_index as u32,
                        reason,
                    });
                }
            }
        }

        Ok(PlanExecuteOutcome::Completed {
            plan_id: plan_id.to_string(),
            steps_executed,
        })
    }

    /// Run LLM-tool cycles for a single step until we get a final response.
    async fn run_step_cycles(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<String> {
        for _cycle in 0..MAX_CYCLES_PER_STEP {
            // Usage is tracked at the provider level; cycle usage intentionally
            // not accumulated here as PlanExecuteEngine operates outside dispatch.
            let (outcome, _cycle_usage) = self.core.run_cycle(messages, tools, params, ctx).await?;

            match outcome {
                CycleOutcome::FinalResponse { content } => {
                    return Ok(content);
                }
                CycleOutcome::ToolsExecuted { .. } => {
                    // Continue looping — tool results are appended to messages by run_cycle
                }
                CycleOutcome::EmptyResponse => {
                    return Ok("Step completed (no output)".to_string());
                }
            }
        }

        Ok("Step completed (max cycles reached)".to_string())
    }

    /// Ask the LLM for a reflection on progress so far.
    async fn run_reflection(
        &self,
        accumulated_results: &[String],
        remaining_steps: &[(usize, String, Vec<String>)],
        params: &ExecutionParams,
    ) -> Result<String> {
        let results_summary = accumulated_results.join("\n");
        let remaining: Vec<String> = remaining_steps
            .iter()
            .map(|(idx, desc, _)| format!("  Step {}: {}", idx + 1, desc))
            .collect();

        let prompt = format!(
            "Reflect on the plan execution progress.\n\n\
             Results so far:\n{}\n\n\
             Remaining steps:\n{}\n\n\
             Are we on track? Should any remaining steps be adjusted? \
             Respond concisely.",
            results_summary,
            remaining.join("\n")
        );

        let messages = vec![Message::user(prompt)];
        let response = self
            .core
            .provider
            .chat(&messages, None, &params.chat_params)
            .await?;

        Ok(response.content.unwrap_or_default())
    }
}

/// Build a rich system prompt for a step execution.
fn build_step_prompt(
    description: &str,
    expected_tools: &[String],
    accumulated_results: &[String],
    remaining_steps: &[(usize, String, Vec<String>)],
) -> String {
    let mut prompt = format!(
        "You are executing a step in a multi-step plan.\n\n\
         ## Current Step\n{}\n",
        description
    );

    if !expected_tools.is_empty() {
        prompt.push_str(&format!(
            "\n## Suggested Tools\n{}\n",
            expected_tools.join(", ")
        ));
    }

    if !accumulated_results.is_empty() {
        prompt.push_str("\n## Previous Results\n");
        // Include last 3 results to stay within context limits
        let start = accumulated_results.len().saturating_sub(3);
        for result in &accumulated_results[start..] {
            prompt.push_str(&format!("- {}\n", result));
        }
    }

    if !remaining_steps.is_empty() {
        prompt.push_str("\n## Remaining Steps\n");
        for (idx, desc, _) in remaining_steps.iter().take(3) {
            prompt.push_str(&format!("- Step {}: {}\n", idx + 1, desc));
        }
        if remaining_steps.len() > 3 {
            prompt.push_str(&format!(
                "  ... and {} more steps\n",
                remaining_steps.len() - 3
            ));
        }
    }

    prompt.push_str(
        "\nUse the available tools to accomplish this step. \
         When done, provide a concise summary of what was accomplished.",
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use plan::{Plan, PlanStatus, PlanStep, StepStatus};
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // ── Mock Provider ──────────────────────────────────────────

    struct SequencePlanProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl SequencePlanProvider {
        fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for SequencePlanProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(LlmResponse {
                    content: Some("done".to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                })
            } else {
                Ok(responses.remove(0))
            }
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Provider that returns an error on a specific call index.
    struct FailOnStepProvider {
        fail_on_call: usize,
        call_count: Mutex<usize>,
    }

    impl FailOnStepProvider {
        fn new(fail_on: usize) -> Arc<Self> {
            Arc::new(Self {
                fail_on_call: fail_on,
                call_count: Mutex::new(0),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for FailOnStepProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut count = self.call_count.lock().unwrap();
            let current = *count;
            *count += 1;

            if current == self.fail_on_call {
                Err(common::error::KlyntbotError::Provider(
                    common::error::ProviderError::Http("LLM call failed".to_string()),
                ))
            } else {
                Ok(LlmResponse {
                    content: Some(format!("Step {} done", current + 1)),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                })
            }
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn make_step(index: usize, description: &str) -> PlanStep {
        PlanStep {
            id: Uuid::new_v4(),
            index,
            description: description.to_string(),
            reasoning: "test reasoning".to_string(),
            expected_tools: vec!["test_tool".to_string()],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        }
    }

    fn make_plan(steps: Vec<PlanStep>) -> Plan {
        let now = Utc::now();
        Plan {
            id: Uuid::new_v4(),
            session_key: "test-session".to_string(),
            goal_id: None,
            title: "Test Plan".to_string(),
            description: "A test plan".to_string(),
            status: PlanStatus::Executing,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    /// Holds the tempdir alongside the plan store to keep it alive for the test duration.
    struct TestPlanStore {
        store: Arc<RwLock<PlanStore>>,
        _dir: tempfile::TempDir,
    }

    async fn make_plan_store_with(plan: &Plan) -> TestPlanStore {
        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let mut store = PlanStore::new(path);
        store.upsert(plan.clone()).await.unwrap();
        TestPlanStore {
            store: Arc::new(RwLock::new(store)),
            _dir: dir,
        }
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    // ── Tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_plan_executes_all_steps() {
        let plan = make_plan(vec![
            make_step(0, "Analyze the codebase"),
            make_step(1, "Write the implementation"),
            make_step(2, "Run the tests"),
        ]);
        let plan_id = plan.id.to_string();

        let provider = SequencePlanProvider::new(vec![
            LlmResponse {
                content: Some("Analyzed codebase".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            LlmResponse {
                content: Some("Implementation written".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            LlmResponse {
                content: Some("Tests passed".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
        ]);

        let test_store = make_plan_store_with(&plan).await;
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry, test_store.store.clone());

        let result = engine
            .execute_plan(&plan_id, &[], &ExecutionParams::new("mock"), &routing_ctx())
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Completed { steps_executed, .. } => {
                assert_eq!(steps_executed, 3);
            }
            other => panic!("Expected Completed, got {:?}", other),
        }

        // Verify step statuses in store
        let mut store = test_store.store.write().await;
        let updated_plan = store.get(&plan.id).await.unwrap().unwrap();
        assert_eq!(updated_plan.steps[0].status, StepStatus::Completed);
        assert_eq!(updated_plan.steps[1].status, StepStatus::Completed);
        assert_eq!(updated_plan.steps[2].status, StepStatus::Completed);
        assert!(updated_plan.steps[0].result.is_some());
    }

    #[tokio::test]
    async fn test_failed_step_returns_failed_outcome() {
        let plan = make_plan(vec![
            make_step(0, "Step one"),
            make_step(1, "Step two (will fail)"),
            make_step(2, "Step three (should not run)"),
        ]);
        let plan_id = plan.id.to_string();

        // Fail on the second call (step 2, index 1)
        let provider = FailOnStepProvider::new(1);
        let test_store = make_plan_store_with(&plan).await;
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry, test_store.store.clone());

        let result = engine
            .execute_plan(&plan_id, &[], &ExecutionParams::new("mock"), &routing_ctx())
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Failed { step, reason, .. } => {
                assert_eq!(step, 1);
                assert!(reason.contains("LLM call failed"));
            }
            other => panic!("Expected Failed, got {:?}", other),
        }

        // Step 0 should be completed, step 1 failed, step 2 still pending
        let mut store = test_store.store.write().await;
        let updated_plan = store.get(&plan.id).await.unwrap().unwrap();
        assert_eq!(updated_plan.steps[0].status, StepStatus::Completed);
        assert_eq!(updated_plan.steps[1].status, StepStatus::Failed);
        assert_eq!(updated_plan.steps[2].status, StepStatus::Pending);
    }

    #[tokio::test]
    async fn test_parameter_generation_in_prompt() {
        // Verify that build_step_prompt includes step description, expected tools,
        // and that the engine passes this to the LLM by executing successfully
        let plan = make_plan(vec![make_step(
            0,
            "Read the auth.rs file and analyze the authentication flow",
        )]);
        let plan_id = plan.id.to_string();

        let provider = SequencePlanProvider::new(vec![LlmResponse {
            content: Some("Analyzed auth flow".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }]);
        let test_store = make_plan_store_with(&plan).await;
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry, test_store.store.clone());

        let result = engine
            .execute_plan(&plan_id, &[], &ExecutionParams::new("mock"), &routing_ctx())
            .await
            .unwrap();

        // Execution should succeed
        assert!(matches!(result, PlanExecuteOutcome::Completed { .. }));

        // Verify the prompt builder includes the step description
        let prompt = build_step_prompt(
            "Read the auth.rs file and analyze the authentication flow",
            &["read_file".to_string()],
            &[],
            &[],
        );
        assert!(prompt.contains("auth.rs"));
        assert!(prompt.contains("authentication flow"));
        assert!(prompt.contains("read_file"));
    }

    #[tokio::test]
    async fn test_empty_plan_completes_immediately() {
        let plan = make_plan(vec![]);
        let plan_id = plan.id.to_string();

        let provider = SequencePlanProvider::new(vec![]);
        let test_store = make_plan_store_with(&plan).await;
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry, test_store.store.clone());

        let result = engine
            .execute_plan(&plan_id, &[], &ExecutionParams::new("mock"), &routing_ctx())
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Completed { steps_executed, .. } => {
                assert_eq!(steps_executed, 0);
            }
            other => panic!("Expected Completed with 0 steps, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_nonexistent_plan_returns_failed() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let store = Arc::new(RwLock::new(PlanStore::new(path)));

        let provider = SequencePlanProvider::new(vec![]);
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry, store);

        let fake_id = Uuid::new_v4().to_string();
        let result = engine
            .execute_plan(&fake_id, &[], &ExecutionParams::new("mock"), &routing_ctx())
            .await
            .unwrap();

        match result {
            PlanExecuteOutcome::Failed { reason, .. } => {
                assert!(reason.contains("not found"));
            }
            other => panic!("Expected Failed for missing plan, got {:?}", other),
        }
    }

    #[test]
    fn test_build_step_prompt_includes_description() {
        let prompt =
            build_step_prompt("Read the config file", &["read_file".to_string()], &[], &[]);

        assert!(prompt.contains("Read the config file"));
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("Current Step"));
    }

    #[test]
    fn test_build_step_prompt_includes_accumulated_results() {
        let results = vec![
            "Step 1 result: Found 3 files".to_string(),
            "Step 2 result: Modified auth.rs".to_string(),
        ];
        let prompt = build_step_prompt("Step 3", &[], &results, &[]);

        assert!(prompt.contains("Previous Results"));
        assert!(prompt.contains("Found 3 files"));
        assert!(prompt.contains("Modified auth.rs"));
    }

    #[test]
    fn test_build_step_prompt_includes_remaining_steps() {
        let remaining = vec![
            (2, "Write tests".to_string(), vec![]),
            (3, "Run linter".to_string(), vec![]),
        ];
        let prompt = build_step_prompt("Current step", &[], &[], &remaining);

        assert!(prompt.contains("Remaining Steps"));
        assert!(prompt.contains("Write tests"));
        assert!(prompt.contains("Run linter"));
    }

    #[test]
    fn test_build_step_prompt_caps_remaining_at_3() {
        let remaining: Vec<(usize, String, Vec<String>)> =
            (0..6).map(|i| (i, format!("Step {}", i), vec![])).collect();
        let prompt = build_step_prompt("Current", &[], &[], &remaining);

        assert!(prompt.contains("and 3 more steps"));
    }
}
