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
use plan::{PlanStatus, StepStatus};
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
        let (core, store_arc) = match (&self.plan_execution_core, &self.plan_store) {
            (Some(c), Some(s)) => (c, Arc::clone(s)),
            _ => {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::ExecutionFailed(
                        "Plan execution not configured (missing plan_store or execution_core)"
                            .into(),
                    ),
                ))
            }
        };

        // Load and validate, then transition to Executing in-place (zero-copy).
        let (plan_title, initial_step_idx) = {
            let mut store = store_arc.write().await;
            let plan = store.get(plan_id).await?.ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;

            // Validate: must be in Approved state
            PlanStatus::validate_transition(&plan.status, &PlanStatus::Executing)?;

            let title = plan.title.clone();
            let step_idx = plan.current_step_index;

            // Transition to Executing in-place — no Plan clone
            {
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;
                p.status = PlanStatus::Executing;
                p.updated_at = Utc::now();
            }
            store.persist_latest(plan_id).await?;

            (title, step_idx)
        };

        // RAII guard: clears the flag on ALL exit paths — normal return, break, and ? propagation.
        // Without this guard, any ? operator inside the loop leaks the flag as true permanently.
        struct PlanExecutingGuard(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for PlanExecutingGuard {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _flag_guard = PlanExecutingGuard(Arc::clone(&self.plan_executing));

        // Set executing flag AFTER guard is installed so any ? between here and
        // the loop cannot leak the flag (guard clears it on all exit paths).
        self.plan_executing.store(true, Ordering::SeqCst);

        let mut step_idx = initial_step_idx;
        // Tracks how many full plan regenerations have occurred (distinct from per-step retries)
        let mut backtrack_count: usize = 0;
        // step_count is re-read after regeneration
        let mut step_count = {
            let mut store = store_arc.write().await;
            store
                .get_mut(plan_id)
                .await?
                .map(|p| p.steps.len())
                .unwrap_or(0)
        };

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

            // Under one lock: build step context, clone just the PlanStep (not whole Plan),
            // mark step Executing in-place, then persist. Lock released before LLM call.
            // Also capture attempt_count/max_attempts here to avoid a second lock on failure path (I1).
            let (
                plan_context,
                step_snapshot,
                step_description,
                step_attempt_count,
                step_max_attempts,
            ) = {
                let mut store = store_arc.write().await;
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;

                let ctx = super::plan_executor::build_step_context(p, step_idx);
                let step_desc = p.steps[step_idx].description.clone();
                // Clone just the step for run_step — far cheaper than cloning Plan
                let step_snap = p.steps[step_idx].clone();

                // Mark Executing in-place
                p.steps[step_idx].status = StepStatus::Executing;
                p.steps[step_idx].started_at = Some(Utc::now());
                p.steps[step_idx].attempt_count += 1;
                // Snapshot attempt/max after incrementing — used in failure path below
                let attempt = p.steps[step_idx].attempt_count;
                let max_attempts = p.steps[step_idx].max_attempts;
                p.updated_at = Utc::now();
                // NLL releases the borrow of store through p here (last use of p)
                store.persist_latest(plan_id).await?;

                (ctx, step_snap, step_desc, attempt, max_attempts)
            };

            debug!(
                "Executing step {}/{}: {}",
                step_idx + 1,
                step_count,
                step_description
            );

            // Execute the step — no store lock held during the long-running LLM call
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

            match step_result {
                Ok(result) if result.success => {
                    info!(
                        "Step {}/{} completed successfully",
                        step_idx + 1,
                        step_count
                    );

                    // Mark step Completed in-place — no Plan clone
                    let mut store = store_arc.write().await;
                    let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                        common::KlyntbotError::Plan(common::PlanError::NotFound(
                            plan_id.to_string(),
                        ))
                    })?;
                    p.steps[step_idx].status = StepStatus::Completed;
                    p.steps[step_idx].result = Some(result.output);
                    p.steps[step_idx].completed_at = Some(Utc::now());
                    step_idx += 1;
                    p.current_step_index = step_idx;
                    p.updated_at = Utc::now();
                    store.persist_latest(plan_id).await?;
                }
                Ok(result) => {
                    // Step failed — use attempt info captured in the Executing lock above (I1).
                    let failure_reason = result.failure_reason.unwrap_or(result.output);

                    warn!(
                        "Step {}/{} failed (attempt {}/{}): {}",
                        step_idx + 1,
                        step_count,
                        step_attempt_count,
                        step_max_attempts,
                        failure_reason
                    );

                    // Single lock: record backtrack entry in-place
                    {
                        let mut store = store_arc.write().await;
                        let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                            common::KlyntbotError::Plan(common::PlanError::NotFound(
                                plan_id.to_string(),
                            ))
                        })?;
                        p.backtrack_history.push(plan::BacktrackEntry {
                            step_index: step_idx,
                            attempt: step_attempt_count,
                            failure_reason: failure_reason.clone(),
                            timestamp: Utc::now(),
                        });
                        p.updated_at = Utc::now();
                        store.persist_latest(plan_id).await?;
                    }

                    if step_attempt_count < step_max_attempts {
                        // Exponential backoff before retry: 2^(attempt-1) seconds
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
                        // Max per-step attempts exhausted — check backtrack limit
                        backtrack_count += 1;
                        if backtrack_count >= super::plan_executor::MAX_BACKTRACK_ATTEMPTS {
                            // Backtrack limit reached — fail the plan in-place
                            let mut store = store_arc.write().await;
                            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                                common::KlyntbotError::Plan(common::PlanError::NotFound(
                                    plan_id.to_string(),
                                ))
                            })?;
                            p.steps[step_idx].status = StepStatus::Failed;
                            p.updated_at = Utc::now();
                            store.persist_latest(plan_id).await?;
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

                        // Regeneration requires a Plan snapshot for the LLM call (one-time clone).
                        // Persist the Failed status BEFORE the LLM call so a crash doesn't
                        // leave the step stuck in Executing (I2).
                        let plan_snapshot = {
                            let mut store = store_arc.write().await;
                            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                                common::KlyntbotError::Plan(common::PlanError::NotFound(
                                    plan_id.to_string(),
                                ))
                            })?;
                            p.steps[step_idx].status = StepStatus::Failed;
                            p.updated_at = Utc::now();
                            // NLL: last use of p ends borrow so store can be used below
                            let snapshot = p.clone();
                            store.persist_latest(plan_id).await?;
                            snapshot
                        };

                        let new_steps = super::plan_executor::regenerate_from(
                            &plan_snapshot,
                            step_idx,
                            &failure_reason,
                            &self.provider,
                        )
                        .await?;

                        // Replace steps from step_idx forward in-place
                        let mut store = store_arc.write().await;
                        let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                            common::KlyntbotError::Plan(common::PlanError::NotFound(
                                plan_id.to_string(),
                            ))
                        })?;
                        p.steps.truncate(step_idx);
                        p.steps.extend(new_steps);
                        step_count = p.steps.len();
                        p.updated_at = Utc::now();
                        store.persist_latest(plan_id).await?;

                        // step_idx stays the same — now points to first regenerated step
                        info!(
                            "Regenerated {} steps from index {}",
                            step_count - step_idx,
                            step_idx
                        );
                    }
                }
                Err(e) => {
                    // Hard error — mark step Failed in-place, let finalizer own plan status
                    let mut store = store_arc.write().await;
                    let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                        common::KlyntbotError::Plan(common::PlanError::NotFound(
                            plan_id.to_string(),
                        ))
                    })?;
                    p.steps[step_idx].status = StepStatus::Failed;
                    p.updated_at = Utc::now();
                    store.persist_latest(plan_id).await?;
                    break Err(e);
                }
            }

            // Guard against exceeding iteration limit.
            // Compare backtrack_count (plan regenerations) — NOT backtrack_history.len()
            // (which counts every per-step retry and inflates the check prematurely).
            // Single lock: read iteration_limit and conditionally mark Failed + break (I6).
            {
                let mut store = store_arc.write().await;
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;
                let iter_limit = p.iteration_limit;
                if backtrack_count >= iter_limit {
                    p.steps[step_idx].status = StepStatus::Failed;
                    p.updated_at = Utc::now();
                    store.persist_latest(plan_id).await?;
                    break Ok((
                        format!(
                            "Plan '{}' halted: iteration limit ({}) reached.",
                            plan_title, iter_limit
                        ),
                        false,
                    ));
                }
            }
        };

        // _flag_guard (PlanExecutingGuard) clears plan_executing on drop.

        // Finalize plan status in-place — no Plan clone
        let (summary, plan_goal_id, plan_succeeded) = {
            let mut store = store_arc.write().await;
            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;
            let goal_id = p.goal_id;
            let (msg, succeeded) = match &result {
                Ok((msg, true)) => {
                    p.status = PlanStatus::Completed;
                    p.completed_at = Some(Utc::now());
                    p.updated_at = Utc::now();
                    ((*msg).clone(), true)
                }
                Ok((msg, _)) => {
                    p.status = PlanStatus::Failed;
                    p.updated_at = Utc::now();
                    ((*msg).clone(), false)
                }
                Err(_) => {
                    p.status = PlanStatus::Failed;
                    p.updated_at = Utc::now();
                    (
                        format!("Plan '{}' failed with an error.", plan_title),
                        false,
                    )
                }
            };
            store.persist_latest(plan_id).await?;
            (msg, goal_id, succeeded)
        };

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
