//! Plan execution engine — drives step-by-step plan execution.
//!
//! Extracted from `agent_loop.rs` for module cohesion.
//! Implements `AgentLoop::run_plan_execution()` which drives the
//! Approved → Executing → (Completed | Failed) state machine.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use common::Result;
use plan::{conversions, PlanStatus, StepStatus};
use tools::RoutingContext;
use tracing::{debug, info, warn};

use super::agent_loop::AgentLoop;

impl AgentLoop {
    /// Execute an approved plan sequentially, step by step.
    ///
    /// State machine: Approved → Executing → (Completed | Failed)
    ///
    /// For each step:
    /// 1. Marks step as Executing
    /// 2. Runs multi-cycle execution via `ExecutionCore` (up to 5 LLM cycles per step)
    /// 3. Updates step status (Completed or Failed)
    /// 4. Advances current_step_index
    /// 5. Persists plan after each step
    ///
    /// On completion, sets plan.completed_at and transitions to Completed.
    /// If any step fails beyond max_attempts, triggers backtracking via `regenerate_from`.
    pub async fn run_plan_execution(
        &self,
        plan_id: &uuid::Uuid,
        routing_ctx: &RoutingContext,
    ) -> Result<String> {
        let (core, repo) = match (&self.plan_execution_core, &self.plan_repo) {
            (Some(c), Some(r)) => (c, r.clone()),
            _ => {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::ExecutionFailed(
                        "Plan execution not configured (missing plan_repo or execution_core)"
                            .into(),
                    ),
                ))
            }
        };

        // Load and validate, then transition to Executing.
        let mut plan = conversions::load_plan(&repo, plan_id)
            .await?
            .ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;

        // Validate: must be in Approved state
        PlanStatus::validate_transition(&plan.status, &PlanStatus::Executing)?;

        let plan_title = plan.title.clone();
        let mut step_idx = plan.current_step_index;

        // Transition to Executing
        plan.status = PlanStatus::Executing;
        plan.updated_at = Utc::now();
        conversions::save_plan(&repo, &plan).await?;

        // RAII guard: clears the flag on ALL exit paths — normal return, break, and ? propagation.
        struct PlanExecutingGuard(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for PlanExecutingGuard {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _flag_guard = PlanExecutingGuard(Arc::clone(&self.plan_executing));

        // Set executing flag AFTER guard is installed
        self.plan_executing.store(true, Ordering::SeqCst);

        let mut backtrack_count: usize = 0;
        let mut step_count = plan.steps.len();

        info!(
            "Starting plan execution: '{}' ({} steps)",
            plan_title, step_count
        );

        let result: Result<(String, bool)> = loop {
            if step_idx >= step_count {
                break Ok((
                    format!(
                        "Plan '{}' completed: all {} steps executed.",
                        plan_title, step_count
                    ),
                    true,
                ));
            }

            // Reload plan for each step iteration to get latest state
            plan = conversions::load_plan(&repo, plan_id)
                .await?
                .ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;

            let plan_context = super::plan_executor::build_step_context(&plan, step_idx);
            let step_description = plan.steps[step_idx].description.clone();
            let step_snapshot = plan.steps[step_idx].clone();

            // Mark Executing
            plan.steps[step_idx].status = StepStatus::Executing;
            plan.steps[step_idx].started_at = Some(Utc::now());
            plan.steps[step_idx].attempt_count += 1;
            let step_attempt_count = plan.steps[step_idx].attempt_count;
            let step_max_attempts = plan.steps[step_idx].max_attempts;
            plan.updated_at = Utc::now();
            conversions::save_plan(&repo, &plan).await?;

            debug!(
                "Executing step {}/{}: {}",
                step_idx + 1,
                step_count,
                step_description
            );

            // Execute the step — no lock held during the long-running LLM call
            let step_start = Instant::now();
            let step_result = super::plan_executor::run_step(
                core,
                &step_snapshot,
                &plan_context,
                routing_ctx,
                self.confidence_evaluator.as_ref(),
            )
            .await;
            let step_duration_ms = step_start.elapsed().as_millis() as u64;

            // Record plan step outcome for the learning system (best-effort)
            if let Some(recorder) = &self.outcome_recorder {
                let (success, error_cat, step_confidence, recorded_tool_name) = match &step_result {
                    Ok(r) => (
                        r.success,
                        None,
                        r.confidence.as_ref(),
                        r.tool_name
                            .clone()
                            .unwrap_or_else(|| step_description.clone()),
                    ),
                    Err(_) => (
                        false,
                        Some("execution_error"),
                        None,
                        step_description.clone(),
                    ),
                };
                let session_key = format!("{}:{}", routing_ctx.channel, routing_ctx.chat_id);
                recorder
                    .record_tool_outcome(
                        &recorded_tool_name,
                        success,
                        error_cat,
                        step_duration_ms,
                        step_confidence,
                        crate::learning::ExecutionMode::PlanStep {
                            plan_id: plan_id.to_string(),
                            step_index: step_idx,
                        },
                        &session_key,
                    )
                    .await;
            }

            // Reload plan to get latest state before applying results
            plan = conversions::load_plan(&repo, plan_id)
                .await?
                .ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;

            match step_result {
                Ok(result) if result.success => {
                    info!(
                        "Step {}/{} completed successfully",
                        step_idx + 1,
                        step_count
                    );

                    plan.steps[step_idx].status = StepStatus::Completed;
                    plan.steps[step_idx].result = Some(result.output);
                    plan.steps[step_idx].completed_at = Some(Utc::now());
                    step_idx += 1;
                    plan.current_step_index = step_idx;
                    plan.updated_at = Utc::now();
                    conversions::save_plan(&repo, &plan).await?;
                }
                Ok(result) => {
                    let failure_reason = result.failure_reason.unwrap_or(result.output);

                    warn!(
                        "Step {}/{} failed (attempt {}/{}): {}",
                        step_idx + 1,
                        step_count,
                        step_attempt_count,
                        step_max_attempts,
                        failure_reason
                    );

                    // Record backtrack entry
                    plan.backtrack_history.push(plan::BacktrackEntry {
                        step_index: step_idx,
                        attempt: step_attempt_count,
                        failure_reason: failure_reason.clone(),
                        timestamp: Utc::now(),
                    });
                    plan.updated_at = Utc::now();
                    conversions::save_plan(&repo, &plan).await?;

                    if step_attempt_count < step_max_attempts {
                        let backoff_secs = 2u64.pow((step_attempt_count as u32).saturating_sub(1));
                        debug!(
                            "Retrying step {} in {}s (attempt {}/{})",
                            step_idx + 1,
                            backoff_secs,
                            step_attempt_count + 1,
                            step_max_attempts
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    } else {
                        backtrack_count += 1;
                        if backtrack_count >= super::plan_executor::MAX_BACKTRACK_ATTEMPTS {
                            plan.steps[step_idx].status = StepStatus::Failed;
                            plan.updated_at = Utc::now();
                            conversions::save_plan(&repo, &plan).await?;
                            break Err(common::KlyntbotError::Plan(
                                common::PlanError::BacktrackLimitReached(
                                    super::plan_executor::MAX_BACKTRACK_ATTEMPTS,
                                ),
                            ));
                        }

                        warn!(
                            "Regenerating steps from index {} after {} attempts (backtrack {}/{})",
                            step_idx,
                            step_attempt_count,
                            backtrack_count,
                            super::plan_executor::MAX_BACKTRACK_ATTEMPTS
                        );

                        // Mark step as Failed before regeneration
                        plan.steps[step_idx].status = StepStatus::Failed;
                        plan.updated_at = Utc::now();
                        conversions::save_plan(&repo, &plan).await?;

                        let new_steps = super::plan_executor::regenerate_from(
                            &plan,
                            step_idx,
                            &failure_reason,
                            &self.provider,
                        )
                        .await?;

                        // Reload and replace steps from step_idx forward
                        plan = conversions::load_plan(&repo, plan_id)
                            .await?
                            .ok_or_else(|| {
                                common::KlyntbotError::Plan(common::PlanError::NotFound(
                                    plan_id.to_string(),
                                ))
                            })?;
                        plan.steps.truncate(step_idx);
                        plan.steps.extend(new_steps);
                        step_count = plan.steps.len();
                        plan.updated_at = Utc::now();
                        conversions::save_plan(&repo, &plan).await?;

                        info!(
                            "Regenerated {} steps from index {}",
                            step_count - step_idx,
                            step_idx
                        );
                    }
                }
                Err(e) => {
                    plan.steps[step_idx].status = StepStatus::Failed;
                    plan.updated_at = Utc::now();
                    conversions::save_plan(&repo, &plan).await?;
                    break Err(e);
                }
            }

            // Guard against exceeding iteration limit
            let iter_limit = plan.iteration_limit;
            if backtrack_count >= iter_limit {
                plan.steps[step_idx].status = StepStatus::Failed;
                plan.updated_at = Utc::now();
                conversions::save_plan(&repo, &plan).await?;
                break Ok((
                    format!(
                        "Plan '{}' halted: iteration limit ({}) reached.",
                        plan_title, iter_limit
                    ),
                    false,
                ));
            }
        };

        // Finalize plan status
        plan = conversions::load_plan(&repo, plan_id)
            .await?
            .ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;

        let plan_goal_id = plan.goal_id;
        let (summary, plan_succeeded) = match &result {
            Ok((msg, true)) => {
                plan.status = PlanStatus::Completed;
                plan.completed_at = Some(Utc::now());
                plan.updated_at = Utc::now();
                ((*msg).clone(), true)
            }
            Ok((msg, _)) => {
                plan.status = PlanStatus::Failed;
                plan.updated_at = Utc::now();
                ((*msg).clone(), false)
            }
            Err(_) => {
                plan.status = PlanStatus::Failed;
                plan.updated_at = Utc::now();
                (
                    format!("Plan '{}' failed with an error.", plan_title),
                    false,
                )
            }
        };
        conversions::save_plan(&repo, &plan).await?;

        // Notify the completion handler (best-effort; updates linked goal metrics)
        if let Some(handler) = &self.plan_completion_handler {
            if let Err(e) = handler
                .on_plan_completed(plan_id, plan_goal_id, plan_succeeded, &summary)
                .await
            {
                warn!("PlanCompletionHandler failed (non-fatal): {}", e);
            }
        }

        info!("{}", summary);
        result.map(|_| summary.clone())
    }
}
