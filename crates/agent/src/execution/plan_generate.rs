//! PlanGenerateEngine — generates a plan from the user's request via LLM, persists
//! it to the database, executes each step, and returns the accumulated output.
//!
//! Used by [`EngineDispatch`] to handle [`ExecutionStrategy::AutonomousTask`] when a
//! [`storage::PlanRepo`] is available.  Falls back to `ReactPlusEngine` when no repo
//! is configured so that existing tests that don't wire up the DB continue to pass.

use std::sync::Arc;

use chrono::Utc;
use common::Result;
use context_engine::ExecutionStrategy;
use plan::{PlanStatus, StepStatus};
use providers::{DynProvider, Message, Usage};
use tools::RoutingContext;
use tracing::{info, warn};
use uuid::Uuid;

use crate::execution::dispatch::DispatchResult;
use crate::execution::types::ExecutionParams;
use crate::execution::{ExecutionCore, ReactOutcome, ReactPlusEngine, ReflectionMode};
use crate::plan_step_generator::{drafts_to_plan_steps, generate_plan_steps};
use plan::conversions::save_plan;

/// Engine that decomposes an autonomous task into a plan via LLM, persists it, and
/// executes each step using [`crate::plan_executor::run_step`].
pub struct PlanGenerateEngine {
    pub core: Arc<ExecutionCore>,
    pub plan_repo: storage::PlanRepo,
    pub provider: DynProvider,
    pub model: String,
}

impl PlanGenerateEngine {
    pub fn new(
        core: Arc<ExecutionCore>,
        plan_repo: storage::PlanRepo,
        provider: DynProvider,
        model: String,
    ) -> Self {
        Self {
            core,
            plan_repo,
            provider,
            model,
        }
    }

    /// Execute an autonomous task by:
    /// 1. Extracting the description from the last user message
    /// 2. Generating plan steps via LLM
    /// 3. Persisting the plan and steps to the database
    /// 4. Running each step through [`crate::plan_executor::run_step`]
    pub async fn execute(
        &self,
        messages: Arc<Vec<Message>>,
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<DispatchResult> {
        let description = extract_last_user_message(&messages);

        // Generate plan steps — fall back to ReactPlus on empty/error
        let drafts = generate_plan_steps(&self.provider, &self.model, &description, &[], &[])
            .await
            .unwrap_or_default();

        if drafts.is_empty() {
            warn!(
                "PlanGenerateEngine: no steps generated for '{}', falling back to ReactPlus",
                description
            );
            return self
                .react_plus_fallback(messages, params, ctx, event_tx)
                .await;
        }

        // Build the Plan domain object (Approved so we can execute it)
        let now = Utc::now();
        let plan_id = Uuid::new_v4();
        let session_key = format!("{}:{}", ctx.channel, ctx.chat_id);
        let steps = drafts_to_plan_steps(&drafts, 0);

        let plan = plan::Plan {
            id: plan_id,
            session_key,
            goal_id: None,
            title: truncate(&description, 100),
            description: description.clone(),
            status: PlanStatus::Approved,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        if let Err(e) = save_plan(&self.plan_repo, &plan).await {
            warn!(
                "PlanGenerateEngine: failed to persist plan: {}; falling back to ReactPlus",
                e
            );
            return self
                .react_plus_fallback(messages, params, ctx, event_tx)
                .await;
        }

        info!(
            "PlanGenerateEngine: created plan '{}' ({} steps) [{}]",
            plan.title,
            plan.steps.len(),
            plan_id
        );

        // Execute the plan steps
        let content = self.run_plan_steps(plan, ctx).await?;

        Ok(DispatchResult {
            content,
            final_strategy: ExecutionStrategy::AutonomousTask { max_iterations: 50 },
            escalation_count: 0,
            usage: Usage::default(),
        })
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Run each plan step sequentially using [`crate::plan_executor::run_step`].
    /// Persists step state after every step.  Returns the accumulated output.
    async fn run_plan_steps(&self, mut plan: plan::Plan, ctx: &RoutingContext) -> Result<String> {
        let step_count = plan.steps.len();
        let mut outputs: Vec<String> = Vec::with_capacity(step_count);

        // Transition to Executing
        plan.status = PlanStatus::Executing;
        plan.updated_at = Utc::now();
        let _ = save_plan(&self.plan_repo, &plan).await;

        for step_idx in 0..step_count {
            let plan_context = crate::plan_executor::build_step_context(&plan, step_idx);
            let step_snap = plan.steps[step_idx].clone();

            plan.steps[step_idx].status = StepStatus::Executing;
            plan.steps[step_idx].started_at = Some(Utc::now());
            plan.steps[step_idx].attempt_count += 1;
            plan.updated_at = Utc::now();
            let _ = save_plan(&self.plan_repo, &plan).await;

            let step_result =
                crate::plan_executor::run_step(&self.core, &step_snap, &plan_context, ctx, None)
                    .await;

            match step_result {
                Ok(r) if r.success => {
                    plan.steps[step_idx].status = StepStatus::Completed;
                    plan.steps[step_idx].result = Some(r.output.clone());
                    plan.steps[step_idx].completed_at = Some(Utc::now());
                    plan.current_step_index = step_idx + 1;
                    outputs.push(r.output);
                }
                Ok(r) => {
                    let reason = r.failure_reason.unwrap_or(r.output);
                    warn!(
                        "PlanGenerateEngine: step {}/{} failed: {}",
                        step_idx + 1,
                        step_count,
                        reason
                    );
                    plan.steps[step_idx].status = StepStatus::Failed;
                    outputs.push(format!("Step {} failed: {}", step_idx + 1, reason));
                }
                Err(e) => {
                    plan.steps[step_idx].status = StepStatus::Failed;
                    outputs.push(format!("Step {} error: {}", step_idx + 1, e));
                }
            }

            plan.updated_at = Utc::now();
            let _ = save_plan(&self.plan_repo, &plan).await;
        }

        // Finalize plan status
        plan.status = PlanStatus::Completed;
        plan.completed_at = Some(Utc::now());
        plan.updated_at = Utc::now();
        let _ = save_plan(&self.plan_repo, &plan).await;

        Ok(outputs.join("\n"))
    }

    /// Fall back to ReactPlus with high iteration count when plan generation fails.
    async fn react_plus_fallback(
        &self,
        messages: Arc<Vec<Message>>,
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<DispatchResult> {
        let tools = {
            let reg = self.core.tool_registry.read().await;
            reg.get_definitions()
        };
        let engine = ReactPlusEngine::new(self.core.clone())
            .with_max_iterations(50)
            .with_reflection_mode(ReflectionMode::EveryN(5));

        let outcome = engine
            .execute(messages, &tools, params, ctx, event_tx.cloned())
            .await?;

        let (content, usage) = match outcome {
            ReactOutcome::Response { content, usage, .. } => (content, usage),
            ReactOutcome::EscalateToAutonomous { reason, usage } => {
                (format!("Task complexity exceeded capacity: {}", reason), usage)
            }
            ReactOutcome::MaxIterationsReached {
                partial_content,
                usage,
            } => (
                partial_content.unwrap_or_else(|| "Max iterations reached".to_string()),
                usage,
            ),
        };

        Ok(DispatchResult {
            content,
            final_strategy: ExecutionStrategy::AutonomousTask { max_iterations: 50 },
            escalation_count: 0,
            usage,
        })
    }
}

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

/// Truncate a string at a character boundary so it fits in `max_chars`.
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_last_user_message_returns_last() {
        let msgs = vec![
            Message::user("first"),
            Message::assistant("response"),
            Message::user("second"),
        ];
        assert_eq!(extract_last_user_message(&msgs), "second");
    }

    #[test]
    fn test_extract_last_user_message_empty_history() {
        assert_eq!(
            extract_last_user_message(&[]),
            "Complete the task"
        );
    }

    #[test]
    fn test_truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let s = "a".repeat(200);
        let result = truncate(&s, 100);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 103); // 100 chars + "..."
    }
}
