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
use super::live_context_refresher::LiveContextRefresher;
use super::loop_detector::{LoopDetector, LoopStatus};
use super::mid_loop_compressor::MidLoopCompressor;
use super::types::{accumulate_usage, CycleOutcome, ExecutionParams};
use crate::events::AgentEvent;

/// Returned to the caller when the synthesis pass produces no usable text.
/// Never let an empty string escape `execute_loop` — users see this directly.
const SYNTHESIS_FALLBACK: &str =
    "I was unable to produce a final response within the allowed budget. \
     Please try again or rephrase your request.";

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
    /// Why the loop terminated: "completed", "budget_exhausted", "cancelled", "error".
    pub finish_reason: String,
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
    let mut synthesis_attempted = false;
    let mut loop_detector = LoopDetector::new();
    let compressor = MidLoopCompressor::new(core.token_counter().clone(), params.context_window);
    let refresher = params
        .context_update_queue
        .as_ref()
        .map(|queue| LiveContextRefresher::new(core.token_counter().clone(), queue.clone()));

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
                    finish_reason: "budget_exhausted".into(),
                });
            }
            // Already tried synthesis once — stop to avoid infinite loop
            // (e.g. if the model keeps returning tool calls instead of text).
            // Return a hardcoded fallback so callers never see an empty string.
            if synthesis_attempted {
                warn!("Budget exhausted and synthesis did not produce text — returning fallback");
                crate::execution::core::fan_out_event(
                    event_tx.as_ref(),
                    core.domain_event_bus.as_ref(),
                    AgentEvent::SynthesisFailed {
                        iteration: budget.turns_used() as usize,
                    },
                )
                .await;
                return Ok(ExecuteLoopResult {
                    content: SYNTHESIS_FALLBACK.to_string(),
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: true,
                    tool_calls: all_tool_calls,
                    finish_reason: "budget_exhausted".into(),
                });
            }
            synthesis_attempted = true;
            // No content yet — inject wrap-up and do one final LLM call.
            // The next iteration's run_cycle will be invoked with tools=&[]
            // (see synthesis-pass branch below) so reasoning models that
            // would otherwise emit yet another tool_call are forced to
            // commit text into `content`. The prompt explicitly forbids
            // empty responses and structured output to deter reasoning
            // models from emitting JSON or tool invocations.
            messages.push(providers::types::Message::system(
                "Your budget is exhausted. You MUST respond with a non-empty \
                 plain-text answer — even if incomplete. Do NOT call any tools. \
                 Do NOT output JSON or tool invocations. Summarise what you have \
                 found so far, or state that you could not complete the task. \
                 An empty response is not acceptable.",
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
                    finish_reason: "cancelled".into(),
                });
            }
        }

        // ── Emit turn start ──────────────────────────────────
        crate::execution::core::fan_out_event(
            event_tx.as_ref(),
            core.domain_event_bus.as_ref(),
            AgentEvent::IterationStart {
                iteration: budget.turns_used() as usize + 1,
                max: budget.max_turns() as usize,
            },
        )
        .await;

        // ── LLM call (streaming) ─────────────────────────────
        // On the synthesis pass (budget exhausted, last attempt before
        // giving up) hide tools so reasoning models can't keep emitting
        // tool_calls and starving content. Forces the model to commit
        // a textual answer.
        const NO_TOOLS: &[serde_json::Value] = &[];
        let cycle_tools: &[serde_json::Value] = if synthesis_attempted && last_content.is_empty() {
            NO_TOOLS
        } else {
            tools
        };
        let cache_bps: Vec<providers::CacheBreakpoint> = if params.cache_enabled {
            crate::execution::cache_policy::compression_aware_default(
                &messages,
                Some(cycle_tools),
                &compressor,
            )
        } else {
            Vec::new()
        };
        let (outcome, cycle_usage) = core
            .run_cycle(
                &mut messages,
                cycle_tools,
                params,
                ctx,
                event_tx.as_ref(),
                Some(&mut seen_tool_calls),
                &cache_bps,
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
                crate::execution::core::fan_out_event(
                    event_tx.as_ref(),
                    core.domain_event_bus.as_ref(),
                    AgentEvent::TurnComplete {
                        turn: budget.turns_used(),
                        budget_remaining_pct: budget.remaining_pct(),
                    },
                )
                .await;
                return Ok(ExecuteLoopResult {
                    content,
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: false,
                    tool_calls: all_tool_calls,
                    finish_reason: "completed".into(),
                });
            }

            CycleOutcome::ToolsExecuted { results } => {
                // Record tool calls
                for r in &results {
                    all_tool_calls.push(r.tool_name.clone());
                }

                // ── Loop detection ───────────────────────────
                // Hash this iteration's (name, args) pairs and check for
                // oscillation. Warning surfaces a hint to the user via
                // event; HardStop aborts to prevent silent budget burn.
                let iteration_calls: Vec<(String, serde_json::Value)> = results
                    .iter()
                    .map(|r| (r.tool_name.clone(), r.arguments.clone()))
                    .collect();
                match loop_detector.record_iteration(budget.turns_used() as usize, &iteration_calls)
                {
                    LoopStatus::Warning {
                        count,
                        tools_summary,
                        ..
                    } => {
                        warn!(
                            iteration = budget.turns_used(),
                            count,
                            tools = %tools_summary,
                            "LoopDetector: repeating tool-call pattern detected"
                        );
                        let suggestion = format!(
                            "Iteration {} repeats the same tool calls {} times. \
                             Consider trying a different approach.",
                            budget.turns_used(),
                            count
                        );
                        crate::execution::core::fan_out_event(
                            event_tx.as_ref(),
                            core.domain_event_bus.as_ref(),
                            AgentEvent::LoopDetected {
                                iteration: budget.turns_used() as usize,
                                tools_summary,
                                suggestion,
                            },
                        )
                        .await;
                    }
                    LoopStatus::HardStop {
                        count,
                        tools_summary,
                    } => {
                        warn!(
                            iteration = budget.turns_used(),
                            count,
                            tools = %tools_summary,
                            "LoopDetector: hard-stop threshold reached — aborting execution"
                        );
                        crate::execution::core::fan_out_event(
                            event_tx.as_ref(),
                            core.domain_event_bus.as_ref(),
                            AgentEvent::LoopHardStop {
                                iteration: budget.turns_used() as usize,
                                tools_summary: tools_summary.clone(),
                            },
                        )
                        .await;
                        return Err(common::KlyntbotError::Tool(
                            common::ToolError::ExecutionFailed(format!(
                                "Agent entered an infinite loop (tool pattern '{}' repeated {} times)",
                                tools_summary, count
                            )),
                        ));
                    }
                    LoopStatus::NoLoop => {}
                }

                last_content = String::new(); // Reset — we'll get new content next turn
            }

            CycleOutcome::EmptyResponse => {
                if !last_content.is_empty() {
                    // Had content from a previous turn — use that rather than
                    // returning nothing.
                    warn!("Execute loop: empty response after prior content, using last content");
                    return Ok(ExecuteLoopResult {
                        content: std::mem::take(&mut last_content),
                        usage: accumulated_usage,
                        turns: budget.turns_used(),
                        budget_exhausted: false,
                        tool_calls: all_tool_calls,
                        finish_reason: "completed".into(),
                    });
                }
                if !budget.exhausted() {
                    // Budget remaining — retry (tick_turn already called above).
                    warn!("Execute loop: empty response from LLM, retrying");
                    continue;
                }
                warn!(
                    "Execute loop: empty response from LLM, no budget for retry — returning fallback"
                );
                crate::execution::core::fan_out_event(
                    event_tx.as_ref(),
                    core.domain_event_bus.as_ref(),
                    AgentEvent::SynthesisFailed {
                        iteration: budget.turns_used() as usize,
                    },
                )
                .await;
                return Ok(ExecuteLoopResult {
                    content: SYNTHESIS_FALLBACK.to_string(),
                    usage: accumulated_usage,
                    turns: budget.turns_used(),
                    budget_exhausted: true,
                    tool_calls: all_tool_calls,
                    finish_reason: "budget_exhausted".into(),
                });
            }
        }

        // ── Mid-loop compression ─────────────────────────────
        if let Some(ref engine) = params.hook_engine {
            let session_key = common::SessionKey::new(&ctx.channel, &ctx.chat_id).to_string();
            let current_tokens = compressor.estimate_tokens(&messages);
            let pre_input = klynt_hooks::events::pre_compact::PreCompactInput {
                session_id: session_key.clone(),
                message_count: messages.len() as u64,
                current_tokens: current_tokens as u64,
                context_window: params.context_window as u64,
                base: Default::default(),
            };
            let _ = engine
                .fire(klynt_hooks::engine::HookFireInput::PreCompact(pre_input))
                .await;
        }
        if let Some((before_tokens, after_tokens, messages_compacted)) =
            compressor.compress_if_needed(&mut messages)
        {
            crate::execution::core::fan_out_event(
                event_tx.as_ref(),
                core.domain_event_bus.as_ref(),
                AgentEvent::ContextCompressed {
                    before_tokens,
                    after_tokens,
                    iteration: budget.turns_used() as usize,
                },
            )
            .await;
            if let Some(ref engine) = params.hook_engine {
                let session_key = common::SessionKey::new(&ctx.channel, &ctx.chat_id).to_string();
                let post_input = klynt_hooks::events::post_compact::PostCompactInput {
                    session_id: session_key,
                    messages_compacted: messages_compacted as u64,
                    tokens_before: before_tokens as u64,
                    tokens_after: after_tokens as u64,
                    base: Default::default(),
                };
                let _ = engine
                    .fire(klynt_hooks::engine::HookFireInput::PostCompact(post_input))
                    .await;
            }
        }

        // ── Live context refresh ─────────────────────────────
        if !params.pause_context_updates {
            if let Some(ref refresher) = refresher {
                let updates = refresher.inject_pending(&mut messages, params.context_window);
                if !updates.is_empty() {
                    let tokens_added: usize = updates.iter().map(|u| u.tokens).sum();
                    crate::execution::core::fan_out_event(
                        event_tx.as_ref(),
                        core.domain_event_bus.as_ref(),
                        AgentEvent::ContextReassembled {
                            updates,
                            tokens_added,
                        },
                    )
                    .await;
                }
            }
        }

        // ── Emit budget update ───────────────────────────────
        crate::execution::core::fan_out_event(
            event_tx.as_ref(),
            core.domain_event_bus.as_ref(),
            AgentEvent::BudgetUpdate {
                tokens_remaining_pct: budget.remaining_pct(),
                turns_used: budget.turns_used(),
                max_turns: budget.max_turns(),
                cost_usd: budget.cost_usd(),
                depth: budget.depth.to_string(),
                cache_read_tokens: accumulated_usage.cache_read_tokens,
                cache_write_tokens: accumulated_usage.cache_write_tokens,
            },
        )
        .await;
        crate::execution::core::fan_out_event(
            event_tx.as_ref(),
            core.domain_event_bus.as_ref(),
            AgentEvent::TurnComplete {
                turn: budget.turns_used(),
                budget_remaining_pct: budget.remaining_pct(),
            },
        )
        .await;

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
