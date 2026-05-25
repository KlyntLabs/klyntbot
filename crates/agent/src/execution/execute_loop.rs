//! Unified execute loop — model self-stop termination.
//!
//! The loop makes LLM calls and executes tools until one of:
//! - The user cancels (partial results returned, FinishReason::Cancelled)
//! - The model returns a final response without tool calls (FinishReason::Completed)
//! - LoopDetector::HardStop fires (FinishReason::LoopDetected)
//! - An explicit safety cap is hit (FinishReason::SafetyTurnLimit or ::TokenLimit) —
//!   only fires for callers that pass a real cap via `SafetyCap::with_limits`
//!   (subagents, coding review). The main agent has no turn cap.
//! - A provider error propagates (FinishReason::Error)
//!
//! There is **no wrap-up message**, **no forced synthesis pass**, and
//! **no coercion system prompt**. The model decides when to stop.

use std::collections::HashSet;

use tracing::{debug, error, warn};

use common::Result;
use providers::Usage;
use tools::RoutingContext;

use super::budget::SafetyCap;
use super::core::ExecutionCore;
use super::live_context_refresher::LiveContextRefresher;
use super::loop_detector::{LoopDetector, LoopStatus};
use super::mid_loop_compressor::MidLoopCompressor;
use super::types::{accumulate_usage, CycleOutcome, ExecutionParams, LoopFinishReason};
use crate::events::AgentEvent;

/// Result of the unified execute loop.
#[derive(Clone)]
pub struct ExecuteLoopResult {
    /// Final response content (may be empty if cap hit before any text was produced).
    pub content: String,
    /// Accumulated token usage across all turns.
    pub usage: Usage,
    /// Total turns executed.
    pub turns: u32,
    /// True iff the loop was stopped by a safety cap (turn or token).
    pub safety_cap_hit: bool,
    /// All tool calls made during execution (by name).
    pub tool_calls: Vec<String>,
    /// Why the loop terminated.
    pub finish_reason: LoopFinishReason,
}

/// Run the unified execute loop.
pub async fn execute_loop(
    core: &ExecutionCore,
    mut messages: Vec<providers::types::Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    cap: &mut SafetyCap,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<ExecuteLoopResult> {
    let mut accumulated_usage = Usage::default();
    let mut all_tool_calls: Vec<String> = Vec::new();
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut last_content = String::new();
    let mut loop_detector = LoopDetector::new();
    let compressor = MidLoopCompressor::new(core.token_counter().clone(), params.context_window);
    let refresher = params.context_update_queue.as_ref().map(|queue| {
        LiveContextRefresher::new(
            core.token_counter().clone(),
            queue.clone(),
            params.injector_registry.clone(),
        )
    });

    loop {
        // ── Cancellation check ───────────────────────────────
        if let Some(ref token) = params.cancel_token {
            if token.is_cancelled() {
                return Ok(ExecuteLoopResult {
                    content: last_content,
                    usage: accumulated_usage,
                    turns: cap.turns_used(),
                    safety_cap_hit: false,
                    tool_calls: all_tool_calls,
                    finish_reason: LoopFinishReason::Cancelled,
                });
            }
        }

        // ── Safety cap gate (hard stop, no coercion) ─────────
        if cap.turn_cap_hit() {
            error!(
                turns = cap.turns_used(),
                max_turns = cap.max_turns(),
                "Safety turn cap hit — aborting execute_loop"
            );
            return Ok(ExecuteLoopResult {
                content: last_content,
                usage: accumulated_usage,
                turns: cap.turns_used(),
                safety_cap_hit: true,
                tool_calls: all_tool_calls,
                finish_reason: LoopFinishReason::SafetyTurnLimit,
            });
        }
        if cap.token_cap_hit() {
            error!(
                tokens = cap.tokens_used(),
                max_tokens = cap.max_tokens(),
                "Safety token cap hit — aborting execute_loop"
            );
            return Ok(ExecuteLoopResult {
                content: last_content,
                usage: accumulated_usage,
                turns: cap.turns_used(),
                safety_cap_hit: true,
                tool_calls: all_tool_calls,
                finish_reason: LoopFinishReason::TokenLimit,
            });
        }

        // ── Emit turn start ──────────────────────────────────
        crate::execution::core::fan_out_event(
            event_tx.as_ref(),
            core.domain_event_bus.as_ref(),
            AgentEvent::IterationStart {
                iteration: cap.turns_used() as usize + 1,
                max: cap.max_turns() as usize,
            },
        )
        .await;

        // ── LLM call (streaming) — always pass full tool list ─
        let cache_bps: Vec<providers::CacheBreakpoint> = if params.cache_enabled {
            crate::execution::cache_policy::compression_aware_default(
                &messages,
                Some(tools),
                &compressor,
            )
        } else {
            Vec::new()
        };
        tracing::debug!(
            turn = cap.turns_used() + 1,
            "execute_loop: calling core.run_cycle (LLM)"
        );
        let (outcome, cycle_usage) = core
            .run_cycle(
                &mut messages,
                tools,
                params,
                ctx,
                event_tx.as_ref(),
                Some(&mut seen_tool_calls),
                &cache_bps,
            )
            .await?;
        tracing::debug!(
            turn = cap.turns_used() + 1,
            "execute_loop: core.run_cycle returned"
        );

        accumulate_usage(&mut accumulated_usage, &cycle_usage);
        cap.deduct(&cycle_usage);
        cap.tick_turn();
        if let Some(ref cb) = params.on_iteration {
            cb();
        }

        // ── Handle outcome ───────────────────────────────────
        match outcome {
            CycleOutcome::FinalResponse { content }
            | CycleOutcome::FabricatedResponse { content } => {
                crate::execution::core::fan_out_event(
                    event_tx.as_ref(),
                    core.domain_event_bus.as_ref(),
                    AgentEvent::TurnComplete {
                        turn: cap.turns_used(),
                        // Kept for HUD compatibility; computed locally now.
                        budget_remaining_pct: remaining_pct(cap),
                    },
                )
                .await;
                return Ok(ExecuteLoopResult {
                    content,
                    usage: accumulated_usage,
                    turns: cap.turns_used(),
                    safety_cap_hit: false,
                    tool_calls: all_tool_calls,
                    finish_reason: LoopFinishReason::Completed,
                });
            }

            CycleOutcome::ToolsExecuted { results } => {
                for r in &results {
                    all_tool_calls.push(r.tool_name.clone());
                }

                let iteration_calls: Vec<(String, serde_json::Value)> = results
                    .iter()
                    .map(|r| (r.tool_name.clone(), r.arguments.clone()))
                    .collect();
                match loop_detector.record_iteration(cap.turns_used() as usize, &iteration_calls) {
                    LoopStatus::Warning {
                        count,
                        tools_summary,
                        ..
                    } => {
                        warn!(
                            iteration = cap.turns_used(),
                            count,
                            tools = %tools_summary,
                            "LoopDetector: repeating tool-call pattern detected"
                        );
                        let suggestion = format!(
                            "Iteration {} repeats the same tool calls {} times. \
                             Consider trying a different approach.",
                            cap.turns_used(),
                            count
                        );
                        crate::execution::core::fan_out_event(
                            event_tx.as_ref(),
                            core.domain_event_bus.as_ref(),
                            AgentEvent::LoopDetected {
                                iteration: cap.turns_used() as usize,
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
                            iteration = cap.turns_used(),
                            count,
                            tools = %tools_summary,
                            "LoopDetector: hard-stop threshold reached — aborting execution"
                        );
                        crate::execution::core::fan_out_event(
                            event_tx.as_ref(),
                            core.domain_event_bus.as_ref(),
                            AgentEvent::LoopHardStop {
                                iteration: cap.turns_used() as usize,
                                tools_summary: tools_summary.clone(),
                            },
                        )
                        .await;
                        return Ok(ExecuteLoopResult {
                            content: last_content,
                            usage: accumulated_usage,
                            turns: cap.turns_used(),
                            safety_cap_hit: false,
                            tool_calls: all_tool_calls,
                            finish_reason: LoopFinishReason::LoopDetected,
                        });
                    }
                    LoopStatus::NoLoop => {}
                }

                last_content = String::new();
            }

            CycleOutcome::EmptyResponse => {
                // Model returned no text and no tool calls — treat as self-stop.
                // Emit TurnComplete so downstream bridges (coding turn_handler,
                // assistant streaming) finalise the UI; the FinalResponse /
                // FabricatedResponse branches do the same.
                debug!(
                    turn = cap.turns_used(),
                    "Empty response from LLM — treating as model self-stop"
                );
                crate::execution::core::fan_out_event(
                    event_tx.as_ref(),
                    core.domain_event_bus.as_ref(),
                    AgentEvent::TurnComplete {
                        turn: cap.turns_used(),
                        budget_remaining_pct: remaining_pct(cap),
                    },
                )
                .await;
                return Ok(ExecuteLoopResult {
                    content: std::mem::take(&mut last_content),
                    usage: accumulated_usage,
                    turns: cap.turns_used(),
                    safety_cap_hit: false,
                    tool_calls: all_tool_calls,
                    finish_reason: LoopFinishReason::Completed,
                });
            }

            CycleOutcome::Cancelled {
                partial_content, ..
            } => {
                let content = if partial_content.is_empty() {
                    std::mem::take(&mut last_content)
                } else {
                    partial_content
                };
                return Ok(ExecuteLoopResult {
                    content,
                    usage: accumulated_usage,
                    turns: cap.turns_used(),
                    safety_cap_hit: false,
                    tool_calls: all_tool_calls,
                    finish_reason: LoopFinishReason::Cancelled,
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
                    iteration: cap.turns_used() as usize,
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
                let updates =
                    refresher.inject_pending_with_ctx(&mut messages, params.context_window, ctx);
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

        // ── Emit budget update (HUD compatibility) ───────────
        crate::execution::core::fan_out_event(
            event_tx.as_ref(),
            core.domain_event_bus.as_ref(),
            AgentEvent::BudgetUpdate {
                tokens_remaining_pct: remaining_pct(cap),
                turns_used: cap.turns_used(),
                max_turns: cap.max_turns(),
                cost_usd: 0.0, // cost tracked externally via CostTracker
                depth: cap.depth.as_str().to_string(),
                cache_read_tokens: accumulated_usage.cache_read_tokens,
                cache_write_tokens: accumulated_usage.cache_write_tokens,
            },
        )
        .await;
    }
}

/// HUD-only helper. Computes a 0..1 fill fraction from the safety cap.
/// Not exposed on `SafetyCap` because it's not a budget knob anymore — it's
/// only used to populate the existing `TurnComplete.budget_remaining_pct`
/// event field, which the HUD consumes.
fn remaining_pct(cap: &SafetyCap) -> f32 {
    let token_pct = if cap.max_tokens() == 0 {
        1.0
    } else {
        1.0 - (cap.tokens_used() as f32 / cap.max_tokens() as f32)
    };
    let turn_pct = if cap.max_turns() == u32::MAX {
        1.0
    } else if cap.max_turns() == 0 {
        0.0
    } else {
        1.0 - (cap.turns_used() as f32 / cap.max_turns() as f32)
    };
    token_pct.min(turn_pct).clamp(0.0, 1.0)
}
