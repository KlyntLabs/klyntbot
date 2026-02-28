//! PlannedEngine — generates a plan from the user's request, persists it,
//! and executes each step with retry + backtracking support.
//!
//! Unifies `execution/plan_generate.rs` and `plan_runner.rs` into a single
//! engine that implements `ExecutionEngine`. Supports escalation takeover
//! via `execute_with_prior_work()`.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use common::{
    utils::{tool_def_name, truncate_chars},
    Result,
};
use domain::plan::save_plan;
use domain::{PlanStatus, PlanVisibility, StepStatus, DEFAULT_MAX_STEP_ATTEMPTS};
use providers::{DynProvider, Message, Usage};
use tools::RoutingContext;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{EngineResult, ExecutionEngine};
use crate::execution::{ExecutionCore, ExecutionParams};
use crate::intent_pipeline::router::EscalationContext;
use crate::plan_executor;
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

        let tool_refs: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();

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
        let completed_steps: Vec<domain::PlanStep> = escalation
            .completed_work
            .iter()
            .enumerate()
            .map(|(i, step)| domain::PlanStep {
                id: Uuid::new_v4(),
                index: i,
                description: step.description.clone(),
                reasoning: format!("Completed during reactive execution: {}", step.tool_name),
                expected_tools: vec![step.tool_name.clone()],
                status: StepStatus::Completed,
                attempt_count: 1,
                max_attempts: DEFAULT_MAX_STEP_ATTEMPTS,
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

        let plan = self.build_plan(&description, all_steps, ctx, None);
        let mut plan = self.save_and_start_plan(plan).await?;
        plan.current_step_index = completed_count; // Skip already-completed steps

        let raw_output = self.run_plan_steps(&mut plan, ctx).await?;
        let content = self
            .synthesize_response(&plan.description, &raw_output)
            .await;

        Ok(EngineResult::complete(
            content,
            Usage::default(),
            plan.steps.len() as u32,
        ))
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
        visibility_override: Option<PlanVisibility>,
    ) -> Result<EngineResult> {
        let description = extract_last_user_message(&messages);

        let tool_refs: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();

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
        let plan = self.build_plan(&description, steps, ctx, visibility_override);
        let mut plan = self.save_and_start_plan(plan).await?;

        let raw_output = self.run_plan_steps(&mut plan, ctx).await?;
        let content = self
            .synthesize_response(&plan.description, &raw_output)
            .await;

        Ok(EngineResult::complete(
            content,
            Usage::default(),
            plan.steps.len() as u32,
        ))
    }

    /// Build a Plan domain object in Approved state.
    ///
    /// If `visibility_override` is provided, it is used instead of the
    /// engine's default visibility. This lets the classifier/heuristics
    /// decision (e.g. Transparent for user-requested plans) take precedence
    /// over the config default (on_failure for auto-generated plans).
    fn build_plan(
        &self,
        description: &str,
        steps: Vec<domain::PlanStep>,
        ctx: &RoutingContext,
        visibility_override: Option<PlanVisibility>,
    ) -> domain::Plan {
        let now = Utc::now();
        domain::Plan {
            id: Uuid::new_v4(),
            session_key: format!("{}:{}", ctx.channel, ctx.chat_id),
            goal_id: None,
            title: truncate_chars(description, 100, "..."),
            description: description.to_string(),
            status: PlanStatus::Approved,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            visibility: visibility_override.unwrap_or_else(|| self.default_visibility.clone()),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    /// Persist the plan and transition to Executing.
    async fn save_and_start_plan(&self, mut plan: domain::Plan) -> Result<domain::Plan> {
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
        plan: &mut domain::Plan,
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

                    plan.backtrack_history.push(domain::BacktrackEntry {
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

                    let new_steps =
                        plan_executor::regenerate_from(plan, step_idx, &reason, &self.provider)
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

    /// Synthesize a human-readable summary from raw step outputs via LLM.
    ///
    /// Falls back to the raw output if the synthesis call fails.
    async fn synthesize_response(&self, goal: &str, step_outputs: &str) -> String {
        let prompt = format!(
            "You just completed a multi-step plan for the user.\n\n\
             **Goal:** {}\n\n\
             **Raw step results:**\n{}\n\n\
             Provide a clear, concise summary based ONLY on the actual tool outputs above. \
             CRITICAL: Do NOT claim actions were taken unless the raw results show concrete \
             evidence (e.g., \"Updated task X\" or changed field values). If a step was supposed \
             to update data but the raw output only shows listings/reads without confirmation \
             of changes, honestly report that those updates were NOT actually performed. \
             Do NOT repeat raw tool outputs verbatim. Use markdown formatting where helpful.",
            goal, step_outputs
        );

        match self
            .provider
            .chat(
                &[Message::user(prompt)],
                None,
                &providers::ChatParams::new(&self.model),
            )
            .await
        {
            Ok(response) => response
                .content
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| step_outputs.to_string()),
            Err(e) => {
                warn!("PlannedEngine: synthesis call failed, returning raw output: {e}");
                step_outputs.to_string()
            }
        }
    }

    /// Execute with a specific visibility override from the intent classifier.
    ///
    /// Called by the router when the `ExecutionMode::Planned` carries a
    /// visibility set by the classifier/heuristics (e.g. Transparent for
    /// user-requested plans).
    pub async fn execute_with_visibility(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
        visibility: PlanVisibility,
    ) -> Result<EngineResult> {
        self.execute_fresh(messages, tools, params, ctx, event_tx, Some(visibility))
            .await
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
        let engine = super::reactive::ReactiveEngine::new(self.core.clone(), 50);
        engine.execute(messages, tools, params, ctx, event_tx).await
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
        self.execute_fresh(messages, tools, params, ctx, event_tx, None)
            .await
    }

    fn mode(&self) -> &str {
        "planned"
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extract the text of the last user message from a conversation history.
fn extract_last_user_message(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::User { content } => match content {
                providers::UserContent::Text(t) => {
                    if t.trim().is_empty() {
                        None
                    } else {
                        Some(t.clone())
                    }
                }
                providers::UserContent::MultiPart(_) => None,
            },
            _ => None,
        })
        .unwrap_or_else(|| "Complete the task".to_string())
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;
    use crate::intent_pipeline::router::CompletedStep;

    /// Build a mock provider with plan step JSON + text follow-up responses.
    fn planning_provider(step_json: &str) -> Arc<MockSequenceProvider> {
        MockSequenceProvider::new(vec![
            text_response(step_json),
            text_response("Step 1 done"),
            text_response("Step 2 done"),
            text_response("Step 3 done"),
        ])
    }

    /// Build a mock provider that returns invalid plan JSON, triggering reactive fallback.
    fn empty_plan_provider() -> Arc<MockSequenceProvider> {
        MockSequenceProvider::new(vec![
            text_response("I cannot generate steps."),
            text_response("Handled reactively"),
        ])
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
        let provider = planning_provider(step_json());
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
                assert!(
                    !content.is_empty(),
                    "should have output from step execution"
                );
            }
            EngineResult::Escalate { .. } => panic!("Expected Complete, got Escalate"),
        }
    }

    #[tokio::test]
    async fn planned_accepts_escalation_context() {
        let provider = planning_provider(step_json());
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
            .execute_with_prior_work(escalation, &[], &default_params(), &routing_ctx(), None)
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
        let provider = empty_plan_provider();
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
        let provider: DynProvider = MockSequenceProvider::new(vec![]);
        let core = Arc::new(ExecutionCore::new(provider.clone(), make_registry()));
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
