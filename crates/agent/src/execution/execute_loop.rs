//! Unified execute loop — replaces DirectEngine, ReactiveEngine, and ExecutionRouter.
//!
//! The loop makes LLM calls and executes tools until:
//! - The model returns no tool calls (natural completion)
//! - The budget is exhausted (graceful synthesis)
//! - The user cancels (partial results)
//! - A safety timeout fires (emergency stop — indicates a bug)

use std::collections::HashSet;

use tracing::{debug, warn};

use common::Result;
use providers::Usage;
use tools::RoutingContext;

use super::budget::ExecutionBudget;
use super::core::ExecutionCore;
use super::types::{accumulate_usage, CycleOutcome, ExecutionParams};
use crate::events::AgentEvent;

/// Result of the unified execute loop.
pub struct ExecuteLoopResult {
    /// Final response content.
    pub content: String,
    /// Accumulated token usage across all turns.
    pub usage: Usage,
    /// Total turns executed.
    pub turns: u32,
    /// Whether the loop was stopped by budget exhaustion (vs natural completion).
    pub budget_exhausted: bool,
    /// All tool calls made during execution.
    pub tool_calls: Vec<String>,
}

/// Run the unified execute loop.
///
/// This replaces `ExecutionRouter::execute()` which dispatched to DirectEngine
/// or ReactiveEngine based on a pre-classified `ExecutionMode`. Now the loop
/// always starts and the model self-selects: if it returns text only on the
/// first call, it's equivalent to the old Direct mode. If it returns tool
/// calls, the loop continues (equivalent to old Reactive mode).
pub async fn execute_loop(
    core: &ExecutionCore,
    mut messages: Vec<providers::types::Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    budget: &mut ExecutionBudget,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<ExecuteLoopResult> {
    let mut accumulated_usage = Usage::default();
    let mut all_tool_calls: Vec<String> = Vec::new();
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut last_content = String::new();
    let mut wrap_up_injected = false;

    loop {
        // ── Budget gate ──────────────────────────────────────
        if budget.exhausted() {
            debug!(
                turns = budget.turns_used(),
                tokens = budget.tokens_used(),
                "Budget exhausted — forcing synthesis"
            );
            // If we have content from a previous turn, return it
            if !last_content.is_empty() {
                return Ok(ExecuteLoopResult {
                    content: last_content,
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: true,
                    tool_calls: all_tool_calls,
                });
            }
            // No content yet — inject wrap-up and do one final LLM call
            messages.push(providers::types::Message::system(
                "Your budget is exhausted. Provide a concise final response \
                 summarizing any results you have so far.",
            ));
        }

        if budget.should_wrap_up() && !budget.exhausted() && !wrap_up_injected {
            wrap_up_injected = true;
            messages.push(providers::types::Message::system(
                "You are approaching your budget limit. Please wrap up \
                 and provide your response with the results you have.",
            ));
        }

        // ── Cancellation check ───────────────────────────────
        if let Some(ref token) = params.cancel_token {
            if token.is_cancelled() {
                return Ok(ExecuteLoopResult {
                    content: last_content,
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: false,
                    tool_calls: all_tool_calls,
                });
            }
        }

        // ── Emit turn start ──────────────────────────────────
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::IterationStart {
                    iteration: budget.turns_used() as usize + 1,
                    max: budget.max_turns() as usize,
                })
                .await;
        }

        // ── LLM call (streaming) ─────────────────────────────
        let (outcome, cycle_usage) = core
            .run_cycle(
                &mut messages,
                tools,
                params,
                ctx,
                event_tx.as_ref(),
                Some(&mut seen_tool_calls),
            )
            .await?;

        accumulate_usage(&mut accumulated_usage, &cycle_usage);
        budget.deduct(&cycle_usage);
        budget.tick_turn();

        // ── Handle outcome ───────────────────────────────────
        match outcome {
            CycleOutcome::FinalResponse { content }
            | CycleOutcome::FabricatedResponse { content } => {
                // Model decided it's done — no tool calls, return response
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(AgentEvent::TurnComplete {
                            turn: budget.turns_used(),
                            budget_remaining_pct: budget.remaining_pct(),
                        })
                        .await;
                }
                return Ok(ExecuteLoopResult {
                    content,
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: false,
                    tool_calls: all_tool_calls,
                });
            }

            CycleOutcome::ToolsExecuted { results } => {
                // Record tool calls
                for r in &results {
                    all_tool_calls.push(r.tool_name.clone());
                }
                last_content = String::new(); // Reset — we'll get new content next turn
            }

            CycleOutcome::EmptyResponse => {
                warn!("Execute loop: empty response from LLM");
                // Treat as completion with empty content
                return Ok(ExecuteLoopResult {
                    content: String::new(),
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: false,
                    tool_calls: all_tool_calls,
                });
            }
        }

        // ── Mid-loop compression ─────────────────────────────
        // The caller (process_message) handles compression via MidLoopCompressor
        // by wrapping this loop. We emit the event for transparency.

        // ── Emit budget update ───────────────────────────────
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::BudgetUpdate {
                    tokens_remaining_pct: budget.remaining_pct(),
                    turns_used: budget.turns_used(),
                    max_turns: budget.max_turns(),
                    cost_usd: budget.cost_usd(),
                    depth: budget.depth.to_string(),
                })
                .await;
            let _ = tx
                .send(AgentEvent::TurnComplete {
                    turn: budget.turns_used(),
                    budget_remaining_pct: budget.remaining_pct(),
                })
                .await;
        }

        // ── Budget exhausted after this turn ─────────────────
        if budget.exhausted() {
            // One more LLM call to synthesize results
            debug!(
                "Budget exhausted after turn {} — one more call for synthesis",
                budget.turns_used()
            );
            continue; // The budget gate at the top will inject the wrap-up instruction
        }
    }
}
