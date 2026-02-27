//! PlannedEngine — generates a plan from the user's request, persists it,
//! and executes each step with retry + backtracking support.
//!
//! Unifies `execution/plan_generate.rs` and `plan_runner.rs` into a single
//! engine that implements `ExecutionEngine`. Supports escalation takeover
//! via `execute_with_prior_work()`.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use plan::{conversions::save_plan, PlanStatus, PlanVisibility, StepStatus};
use providers::{DynProvider, Message, Usage};
use tools::RoutingContext;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{EngineResult, ExecutionEngine};
use crate::execution::{ExecutionCore, ExecutionParams};
use crate::intent_pipeline::escalation::EscalationContext;
use crate::plan_executor;
use crate::execution::plan_generate::{extract_last_user_message, truncate};
use crate::plan_step_generator::{drafts_to_plan_steps, generate_plan_steps};

/// Engine that decomposes a task into a plan via LLM, persists it, and executes
/// each step with retry + backtracking. Implements `ExecutionEngine` for the
/// intent pipeline.
pub struct PlannedEngine {
    core: Arc<ExecutionCore>,
    plan_repo: storage::PlanRepo,
    provider: DynProvider,
    model: String,
    default_visibility: PlanVisibility,
}

impl PlannedEngine {
    pub fn new(
        core: Arc<ExecutionCore>,
        plan_repo: storage::PlanRepo,
        provider: DynProvider,
        model: String,
        default_visibility: PlanVisibility,
    ) -> Self {
        Self {
            core,
            plan_repo,
            provider,
            model,
            default_visibility,
        }
    }

    /// Escalation takeover — accepts prior work as pre-filled completed steps.
    ///
    /// Generates remaining plan steps with LLM awareness of what's already done,
    /// pre-fills the completed work as completed steps, then executes the rest.
    pub async fn execute_with_prior_work(
        &self,
        escalation: EscalationContext,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        let description = if escalation.original_message.is_empty() {
            extract_last_user_message(&escalation.messages)
        } else {
            escalation.original_message.clone()
        };

        // Build context from completed work so LLM knows what's done
        let completed_context: Vec<Message> = escalation
            .completed_work
            .iter()
            .map(|step| {
                Message::assistant(format!(
                    "Already completed: {} → {}",
                    step.description, step.result
                ))
            })
            .collect();

        let tool_names = self.extract_tool_names(tools);
        let tool_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        let drafts = generate_plan_steps(
            &self.provider,
            &self.model,
            &description,
            &completed_context,
            &tool_refs,
        )
        .await
        .unwrap_or_default();

        if drafts.is_empty() {
            warn!("PlannedEngine: no steps generated for escalation, falling back to reactive");
            return self
                .reactive_fallback(escalation.messages, tools, params, ctx, event_tx)
                .await;
        }

        // Build completed steps from prior work
        let completed_steps: Vec<plan::PlanStep> = escalation
            .completed_work
            .iter()
            .enumerate()
            .map(|(i, step)| plan::PlanStep {
                id: Uuid::new_v4(),
                index: i,
                description: step.description.clone(),
                reasoning: format!("Completed during reactive execution: {}", step.tool_name),
                expected_tools: vec![step.tool_name.clone()],
                status: StepStatus::Completed,
                attempt_count: 1,
                max_attempts: 3,
                result: Some(step.result.clone()),
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
            })
            .collect();

        let completed_count = completed_steps.len();
        let new_steps = drafts_to_plan_steps(&drafts, completed_count);

        // Combine completed + new steps
        let mut all_steps = completed_steps;
        all_steps.extend(new_steps);

        let plan = self.build_plan(&description, all_steps, ctx);
        let mut plan = self.save_and_start_plan(plan).await?;
        plan.current_step_index = completed_count; // Skip already-completed steps

        let content = self.run_plan_steps(&mut plan, ctx).await?;

        Ok(EngineResult::Complete {
            content,
            usage: Usage::default(),
            iterations: plan.steps.len() as u32,
            traces: vec![],
            tool_name: None,
        })
    }

    // ── Private helpers ──────────────────────────────────────────

    /// Generate steps from scratch, build a plan, persist it, and execute.
    async fn execute_fresh(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        let description = extract_last_user_message(&messages);

        let tool_names = self.extract_tool_names(tools);
        let tool_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        let drafts =
            generate_plan_steps(&self.provider, &self.model, &description, &[], &tool_refs)
                .await
                .unwrap_or_default();

        if drafts.is_empty() {
            warn!(
                "PlannedEngine: no steps generated for '{}', falling back to reactive",
                description
            );
            return self
                .reactive_fallback(messages, tools, params, ctx, event_tx)
                .await;
        }

        let steps = drafts_to_plan_steps(&drafts, 0);
        let plan = self.build_plan(&description, steps, ctx);
        let mut plan = self.save_and_start_plan(plan).await?;

        let content = self.run_plan_steps(&mut plan, ctx).await?;

        Ok(EngineResult::Complete {
            content,
            usage: Usage::default(),
            iterations: plan.steps.len() as u32,
            traces: vec![],
            tool_name: None,
        })
    }

    /// Build a Plan domain object in Approved state.
    fn build_plan(
        &self,
        description: &str,
        steps: Vec<plan::PlanStep>,
        ctx: &RoutingContext,
    ) -> plan::Plan {
        let now = Utc::now();
        plan::Plan {
            id: Uuid::new_v4(),
            session_key: format!("{}:{}", ctx.channel, ctx.chat_id),
            goal_id: None,
            title: truncate(description, 100),
            description: description.to_string(),
            status: PlanStatus::Approved,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            visibility: self.default_visibility.clone(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    /// Persist the plan and transition to Executing.
    async fn save_and_start_plan(&self, mut plan: plan::Plan) -> Result<plan::Plan> {
        if let Err(e) = save_plan(&self.plan_repo, &plan).await {
            warn!("PlannedEngine: failed to persist plan: {}", e);
            return Err(e);
        }

        info!(
            "PlannedEngine: created plan '{}' ({} steps) [{}]",
            plan.title,
            plan.steps.len(),
            plan.id
        );

        plan.status = PlanStatus::Executing;
        plan.updated_at = Utc::now();
        let _ = save_plan(&self.plan_repo, &plan).await;

        Ok(plan)
    }

    /// Execute plan steps sequentially with retry + backtracking.
    async fn run_plan_steps(
        &self,
        plan: &mut plan::Plan,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let step_count = plan.steps.len();
        let mut outputs: Vec<String> = Vec::with_capacity(step_count);
        let mut step_idx = plan.current_step_index;
        let mut backtrack_count: usize = 0;

        while step_idx < plan.steps.len() {
            // Skip already-completed steps (from escalation context)
            if plan.steps[step_idx].status == StepStatus::Completed {
                if let Some(ref result) = plan.steps[step_idx].result {
                    outputs.push(result.clone());
                }
                step_idx += 1;
                continue;
            }

            let plan_context = plan_executor::build_step_context(plan, step_idx);
            let step_snap = plan.steps[step_idx].clone();

            plan.steps[step_idx].status = StepStatus::Executing;
            plan.steps[step_idx].started_at = Some(Utc::now());
            plan.steps[step_idx].attempt_count += 1;
            let attempt_count = plan.steps[step_idx].attempt_count;
            let max_attempts = plan.steps[step_idx].max_attempts;
            plan.updated_at = Utc::now();
            let _ = save_plan(&self.plan_repo, plan).await;

            debug!(
                "PlannedEngine: executing step {}/{}: {}",
                step_idx + 1,
                plan.steps.len(),
                step_snap.description
            );

            let step_result =
                plan_executor::run_step(&self.core, &step_snap, &plan_context, ctx, None).await;

            match step_result {
                Ok(r) if r.success => {
                    plan.steps[step_idx].status = StepStatus::Completed;
                    plan.steps[step_idx].result = Some(r.output.clone());
                    plan.steps[step_idx].completed_at = Some(Utc::now());
                    step_idx += 1;
                    plan.current_step_index = step_idx;
                    outputs.push(r.output);
                }
                Ok(r) => {
                    let reason = r.failure_reason.unwrap_or(r.output);
                    warn!(
                        "PlannedEngine: step {}/{} failed (attempt {}/{}): {}",
                        step_idx + 1,
                        plan.steps.len(),
                        attempt_count,
                        max_attempts,
                        reason
                    );

                    plan.backtrack_history.push(plan::BacktrackEntry {
                        step_index: step_idx,
                        attempt: attempt_count,
                        failure_reason: reason.clone(),
                        timestamp: Utc::now(),
                    });

                    if attempt_count < max_attempts {
                        // Retry the step
                        continue;
                    }

                    // Max attempts reached — try backtracking
                    backtrack_count += 1;
                    if backtrack_count >= plan_executor::MAX_BACKTRACK_ATTEMPTS {
                        plan.steps[step_idx].status = StepStatus::Failed;
                        plan.status = PlanStatus::Failed;
                        plan.updated_at = Utc::now();
                        let _ = save_plan(&self.plan_repo, plan).await;
                        outputs.push(format!("Step {} failed: {}", step_idx + 1, reason));
                        break;
                    }

                    plan.steps[step_idx].status = StepStatus::Failed;
                    plan.updated_at = Utc::now();
                    let _ = save_plan(&self.plan_repo, plan).await;

                    let new_steps = plan_executor::regenerate_from(
                        plan,
                        step_idx,
                        &reason,
                        &self.provider,
                    )
                    .await?;

                    plan.steps.truncate(step_idx);
                    plan.steps.extend(new_steps);
                    plan.updated_at = Utc::now();
                    let _ = save_plan(&self.plan_repo, plan).await;
                }
                Err(e) => {
                    plan.steps[step_idx].status = StepStatus::Failed;
                    outputs.push(format!("Step {} error: {}", step_idx + 1, e));
                    break;
                }
            }

            plan.updated_at = Utc::now();
            let _ = save_plan(&self.plan_repo, plan).await;
        }

        // Finalize plan status if not already failed
        if plan.status != PlanStatus::Failed {
            plan.status = PlanStatus::Completed;
            plan.completed_at = Some(Utc::now());
            plan.updated_at = Utc::now();
            let _ = save_plan(&self.plan_repo, plan).await;
        }

        Ok(outputs.join("\n"))
    }

    /// Fall back to ReactiveEngine when plan generation fails.
    async fn reactive_fallback(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        let engine =
            super::reactive::ReactiveEngine::new(self.core.clone(), 50);
        engine.execute(messages, tools, params, ctx, event_tx).await
    }

    /// Extract tool names from JSON tool definitions.
    fn extract_tool_names(&self, tools: &[serde_json::Value]) -> Vec<String> {
        tools
            .iter()
            .filter_map(|t| {
                t.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
            })
            .collect()
    }
}

#[async_trait]
impl ExecutionEngine for PlannedEngine {
    async fn execute(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<EngineResult> {
        self.execute_fresh(messages, tools, params, ctx, event_tx)
            .await
    }

    fn mode(&self) -> &str {
        "planned"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::intent_pipeline::escalation::CompletedStep;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    // ── Mock provider that returns step JSON then text responses ──

    struct PlanningProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl PlanningProvider {
        /// First call returns plan steps JSON, subsequent calls return text.
        fn with_steps(step_json: &str) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![
                    // Plan generation call
                    LlmResponse {
                        content: Some(step_json.to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                    // Step execution calls — return text responses
                    LlmResponse {
                        content: Some("Step 1 done".to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                    LlmResponse {
                        content: Some("Step 2 done".to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                    LlmResponse {
                        content: Some("Step 3 done".to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                ]),
            })
        }

        /// Returns empty plan (no steps) — triggers reactive fallback.
        fn empty_plan() -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(vec![
                    LlmResponse {
                        content: Some("I cannot generate steps.".to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                    // Fallback reactive response
                    LlmResponse {
                        content: Some("Handled reactively".to_string()),
                        tool_calls: vec![],
                        finish_reason: "stop".to_string(),
                        usage: Usage::default(),
                        reasoning_content: None,
                    },
                ]),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for PlanningProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(LlmResponse {
                    content: Some("fallback".to_string()),
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

    // ── Helpers ──────────────────────────────────────────────────

    fn make_registry() -> Arc<RwLock<ToolRegistry>> {
        Arc::new(RwLock::new(ToolRegistry::new()))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    fn default_params() -> ExecutionParams {
        ExecutionParams::new("mock")
    }

    fn step_json() -> &'static str {
        r#"[
            {"description": "Gather information", "reasoning": "Need context", "expectedTools": []},
            {"description": "Process data", "reasoning": "Transform inputs", "expectedTools": []}
        ]"#
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn planned_generates_and_executes() {
        let provider = PlanningProvider::with_steps(step_json());
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let plan_repo = storage::PlanRepo::new(pool.inner().clone());

        let core = Arc::new(ExecutionCore::new(
            provider.clone() as DynProvider,
            make_registry(),
        ));
        let engine = PlannedEngine::new(
            core,
            plan_repo,
            provider as DynProvider,
            "mock".to_string(),
            PlanVisibility::Transparent,
        );

        let result = engine
            .execute(
                vec![Message::user("build a REST API")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete { content, .. } => {
                assert!(!content.is_empty(), "should have output from step execution");
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[tokio::test]
    async fn planned_accepts_escalation_context() {
        let provider = PlanningProvider::with_steps(step_json());
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let plan_repo = storage::PlanRepo::new(pool.inner().clone());

        let core = Arc::new(ExecutionCore::new(
            provider.clone() as DynProvider,
            make_registry(),
        ));
        let engine = PlannedEngine::new(
            core,
            plan_repo,
            provider as DynProvider,
            "mock".to_string(),
            PlanVisibility::Silent,
        );

        let prior_work = vec![CompletedStep {
            description: "Searched flights".into(),
            tool_name: "web_search".into(),
            result: "Found 5 results".into(),
        }];
        let escalation = EscalationContext {
            messages: vec![Message::user("book cheapest flight")],
            completed_work: prior_work,
            original_message: "book cheapest flight".into(),
        };

        let result = engine
            .execute_with_prior_work(
                escalation,
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        match result {
            EngineResult::Complete {
                content,
                iterations,
                ..
            } => {
                assert!(!content.is_empty());
                // Should have prior work (1) + generated steps (2) = 3 total
                assert!(iterations >= 3, "should include prior work + new steps");
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[tokio::test]
    async fn planned_falls_back_on_empty_steps() {
        let provider = PlanningProvider::empty_plan();
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let plan_repo = storage::PlanRepo::new(pool.inner().clone());

        let core = Arc::new(ExecutionCore::new(
            provider.clone() as DynProvider,
            make_registry(),
        ));
        let engine = PlannedEngine::new(
            core,
            plan_repo,
            provider as DynProvider,
            "mock".to_string(),
            PlanVisibility::Transparent,
        );

        let result = engine
            .execute(
                vec![Message::user("do something")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        // Should fall back to reactive and complete
        match result {
            EngineResult::Complete { content, .. } => {
                assert!(
                    content.contains("Handled reactively"),
                    "should fall back to reactive"
                );
            }
            EngineResult::Escalate { .. } => {
                // Reactive fallback may also escalate — that's acceptable
            }
        }
    }

    #[test]
    fn mode_returns_planned() {
        let provider: DynProvider = PlanningProvider::with_steps("[]");
        let registry = make_registry();
        let core = Arc::new(ExecutionCore::new(provider.clone(), registry));
        // We can't create PlanRepo without a pool in a sync test,
        // but we can test mode() since it doesn't touch the repo
        // Use a dummy by constructing manually
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(async {
            let pool = storage::StoragePool::connect_in_memory().await.unwrap();
            PlannedEngine::new(
                core,
                storage::PlanRepo::new(pool.inner().clone()),
                provider,
                "mock".to_string(),
                PlanVisibility::default(),
            )
        });
        assert_eq!(engine.mode(), "planned");
    }
}
