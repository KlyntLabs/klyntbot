# Model Self-Stop Termination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace klynt's coercive token+turn budget termination with OpenCode-style model self-stop, keeping a silent safety cap as a backstop and adding per-tool timeout classes for long-running commands.

**Architecture:** The execute loop becomes a bare `loop {}` that exits on (1) cancellation, (2) `CycleOutcome::FinalResponse` (model self-stop), (3) `LoopDetector::HardStop`, (4) safety-turn-cap hit, (5) safety-token-cap hit, (6) provider error. The wrap-up message, forced final synthesis pass, coercion system prompt, and `SYNTHESIS_FALLBACK` constant are deleted. `ExecutionBudget` is renamed `SafetyCap` and stripped of `should_wrap_up`/`remaining_pct`/`extend_turns`. A new typed `FinishReason` enum replaces stringly-typed reasons. Long-running shell tools opt into a 600s timeout via the existing `Tool::custom_timeout()` hook.

**Tech Stack:** Rust 1.93, tokio, sqlx, tauri 2, vitest, cargo-nextest. Spec: `docs/superpowers/specs/2026-05-05-model-self-stop-termination-design.md`.

---

## Pre-flight

- [ ] **Step 0.1: Confirm baseline tests pass on main**

```bash
cargo nextest run -p agent --no-fail-fast 2>&1 | tail -20
```

Expected: all green. Capture pass count for regression comparison after each task.

- [ ] **Step 0.2: Create feature branch**

```bash
git switch -c feat/model-self-stop-termination
```

---

## Task 1: Introduce typed `FinishReason` enum

**Files:**
- Modify: `crates/agent/src/execution/types.rs`
- Test: `crates/agent/src/execution/types.rs` (inline `#[cfg(test)]`)

Today `ExecuteLoopResult.finish_reason` is `String` ("completed" / "budget_exhausted" / "cancelled" / "error"). We replace with a typed enum so downstream code can match exhaustively. Add the enum first; switch the field type in Task 4 once `execute_loop` knows about it.

- [ ] **Step 1.1: Add `FinishReason` enum to `types.rs`**

Append at the end of `crates/agent/src/execution/types.rs`, just before the `#[cfg(test)] mod tests` block:

```rust
/// Why an `execute_loop` invocation terminated.
///
/// `Completed` is the success path — the model returned a final response with no tool calls.
/// `Cancelled` indicates user-initiated abort via `CancellationToken`.
/// `SafetyTurnLimit` / `TokenLimit` are silent backstops; hitting one is logged at `error!` for
/// user-facing agents and at `warn!` for subagents (where caps are intentional capability tiers).
/// `LoopDetected` comes from `LoopDetector::HardStop` — repeated identical tool signatures.
/// `Error` propagates an upstream provider error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Completed,
    Cancelled,
    SafetyTurnLimit,
    TokenLimit,
    LoopDetected,
    Error,
}

impl FinishReason {
    /// Stable wire string. Kept stable for telemetry consumers (analytics, mirror).
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::SafetyTurnLimit => "safety_turn_limit_reached",
            Self::TokenLimit => "token_limit_reached",
            Self::LoopDetected => "loop_detected",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_wire_str())
    }
}
```

- [ ] **Step 1.2: Add unit test for wire stability**

Append inside `crates/agent/src/execution/types.rs` `mod tests`:

```rust
#[test]
fn finish_reason_wire_strings_are_stable() {
    assert_eq!(FinishReason::Completed.as_wire_str(), "completed");
    assert_eq!(FinishReason::Cancelled.as_wire_str(), "cancelled");
    assert_eq!(
        FinishReason::SafetyTurnLimit.as_wire_str(),
        "safety_turn_limit_reached"
    );
    assert_eq!(FinishReason::TokenLimit.as_wire_str(), "token_limit_reached");
    assert_eq!(FinishReason::LoopDetected.as_wire_str(), "loop_detected");
    assert_eq!(FinishReason::Error.as_wire_str(), "error");
}

#[test]
fn finish_reason_serializes_snake_case() {
    let json = serde_json::to_string(&FinishReason::SafetyTurnLimit).unwrap();
    assert_eq!(json, "\"safety_turn_limit\"");
}
```

- [ ] **Step 1.3: Run tests**

```bash
cargo nextest run -p agent -E 'test(finish_reason)'
```

Expected: 2 passed.

- [ ] **Step 1.4: Commit**

```bash
git add crates/agent/src/execution/types.rs
git commit -m "$(cat <<'EOF'
feat(agent): add typed FinishReason enum

Replaces stringly-typed finish_reason on ExecuteLoopResult in a follow-up
commit. Wire format kept stable so analytics/mirror consumers are unaffected.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Rename `ExecutionBudget` → `SafetyCap` and prune coercion API

**Files:**
- Modify: `crates/agent/src/execution/budget.rs`
- Modify: `crates/agent/src/execution/mod.rs` (public re-exports)
- Modify (call sites, mechanical): `crates/agent/src/execution/execute_loop.rs`, `crates/agent/src/agent_runtime/runtime.rs`, `crates/agent/src/subagent.rs`

The struct is renamed to clarify its new role. Methods that only existed to drive coercion are removed: `should_wrap_up`, `remaining_pct`, `extend_turns`, the `RESERVED_SYNTHESIS_TOKENS` and `WRAP_UP_THRESHOLD` constants, and `record_cost`/`cost_usd` (orphaned — not used in the loop). `DepthMode` and `SkillBudget` stay (they're consumed by the HUD and config). The semantics of `exhausted()` change: it now returns true on hard cap hit only (no synthesis reserve subtraction).

- [ ] **Step 2.1: Rewrite `budget.rs`**

Replace the entirety of `crates/agent/src/execution/budget.rs` with:

```rust
//! Safety cap — silent backstop on token/turn exhaustion.
//!
//! Klynt's primary termination model is **model self-stop**: the loop exits
//! when the LLM returns a final response with no tool calls (OpenCode-style).
//! `SafetyCap` is a hard backstop for runaway models — cron jobs and nightly
//! reforge cycles where an unbounded loop would be a real cost concern.
//!
//! Hitting a cap is **not** a graceful path. The loop aborts with
//! `FinishReason::SafetyTurnLimit` or `FinishReason::TokenLimit` and surfaces
//! whatever partial content was accumulated. There is no wrap-up message,
//! no forced synthesis pass, no coercion prompt.

use serde::{Deserialize, Serialize};

// ── Depth Modes ──────────────────────────────────────────────

/// User-selectable depth mode. Controls how deeply the agent thinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthMode {
    #[default]
    Normal,
    DeepThink,
    Ultra,
}

impl std::fmt::Display for DepthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::DeepThink => write!(f, "deep_think"),
            Self::Ultra => write!(f, "ultra"),
        }
    }
}

// ── Default safety caps ──────────────────────────────────────

/// Default user-facing safety cap on turns. Hitting this is treated as a bug
/// indicator. Subagents override this with tier-specific caps (5/10/15).
pub const DEFAULT_SAFETY_TURN_CAP: u32 = 100;

/// Default budget parameters used by HUD and config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBudget {
    pub normal_tokens: u64,
    pub normal_turns: u32,
    pub deep_multiplier: f32,
    pub ultra_multiplier: f32,
}

impl Default for SkillBudget {
    fn default() -> Self {
        Self {
            normal_tokens: 60_000,
            normal_turns: DEFAULT_SAFETY_TURN_CAP,
            deep_multiplier: 1.5,
            ultra_multiplier: 3.0,
        }
    }
}

// ── Safety cap ───────────────────────────────────────────────

/// Hard upper bound on tokens and turns for a single execute_loop run.
#[derive(Debug, Clone)]
pub struct SafetyCap {
    pub depth: DepthMode,
    max_tokens: u64,
    max_turns: u32,
    tokens_used: u64,
    turns_used: u32,
}

impl SafetyCap {
    /// Create a cap from the user's depth choice.
    pub fn new(depth: DepthMode) -> Self {
        let base = SkillBudget::default();
        let (max_tokens, max_turns) = match depth {
            DepthMode::Normal => (base.normal_tokens, base.normal_turns),
            DepthMode::DeepThink => (
                (base.normal_tokens as f64 * base.deep_multiplier as f64) as u64,
                (base.normal_turns as f64 * base.deep_multiplier as f64) as u32,
            ),
            DepthMode::Ultra => (
                (base.normal_tokens as f64 * base.ultra_multiplier as f64) as u64,
                u32::MAX,
            ),
        };
        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
        }
    }

    /// Create a cap with explicit limits (for subagents, autotuner, tests).
    pub fn with_limits(depth: DepthMode, max_tokens: u64, max_turns: u32) -> Self {
        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
        }
    }

    pub fn deduct(&mut self, usage: &providers::Usage) {
        self.tokens_used += usage.prompt_tokens as u64 + usage.completion_tokens as u64;
    }

    pub fn tick_turn(&mut self) {
        self.turns_used += 1;
    }

    /// True when either cap has been hit (no synthesis reserve).
    pub fn turn_cap_hit(&self) -> bool {
        self.turns_used >= self.max_turns
    }

    pub fn token_cap_hit(&self) -> bool {
        self.max_tokens > 0 && self.tokens_used >= self.max_tokens
    }

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used
    }

    pub fn turns_used(&self) -> u32 {
        self.turns_used
    }

    pub fn max_turns(&self) -> u32 {
        self.max_turns
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_uses_defaults() {
        let cap = SafetyCap::new(DepthMode::Normal);
        assert_eq!(cap.max_tokens, 60_000);
        assert_eq!(cap.max_turns, DEFAULT_SAFETY_TURN_CAP);
        assert!(!cap.turn_cap_hit());
        assert!(!cap.token_cap_hit());
    }

    #[test]
    fn deep_think_scales() {
        let cap = SafetyCap::new(DepthMode::DeepThink);
        assert_eq!(cap.max_tokens, 90_000);
        assert_eq!(cap.max_turns, 150);
    }

    #[test]
    fn ultra_unlimited_turns() {
        let cap = SafetyCap::new(DepthMode::Ultra);
        assert_eq!(cap.max_turns, u32::MAX);
        assert_eq!(cap.max_tokens, 180_000);
    }

    #[test]
    fn turn_cap_hit_at_limit() {
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 1_000_000, 3);
        cap.tick_turn();
        cap.tick_turn();
        assert!(!cap.turn_cap_hit());
        cap.tick_turn();
        assert!(cap.turn_cap_hit());
    }

    #[test]
    fn token_cap_hit_at_limit() {
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 10_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 6000,
            completion_tokens: 3000,
            ..Default::default()
        };
        cap.deduct(&usage);
        assert!(!cap.token_cap_hit());
        cap.deduct(&usage);
        assert!(cap.token_cap_hit());
    }

    #[test]
    fn no_synthesis_reserve_subtraction() {
        // Regression: the old budget reserved 2000 tokens for synthesis,
        // exhausting early. SafetyCap exhausts at the literal max.
        let mut cap = SafetyCap::with_limits(DepthMode::Normal, 10_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 5000,
            completion_tokens: 4000,
            ..Default::default()
        };
        cap.deduct(&usage);
        assert!(!cap.token_cap_hit(), "9000 < 10_000 — must not be capped");
    }
}
```

- [ ] **Step 2.2: Update `execution/mod.rs` re-exports**

```bash
sed -n '1,40p' crates/agent/src/execution/mod.rs
```

Find the line re-exporting `ExecutionBudget`. Replace `ExecutionBudget` with `SafetyCap` in that line. Apply the same rename throughout the file.

- [ ] **Step 2.3: Mechanical rename in non-loop call sites**

```bash
rg -l 'ExecutionBudget' crates/agent/src
```

For each file *except* `execute_loop.rs` (handled in Task 3), substitute `ExecutionBudget` → `SafetyCap`. Also rename method calls: `should_wrap_up()` and `remaining_pct()` and `extend_turns(` and `record_cost(` and `cost_usd()` and `exhausted()` will all need attention. After Step 2.1's rewrite, only `tick_turn`, `deduct`, `with_limits`, `new`, `turns_used`, `tokens_used`, `max_turns`, `max_tokens`, `turn_cap_hit`, `token_cap_hit` remain. Any reference to a removed method must be deleted or rewritten.

Apply with:

```bash
for f in $(rg -l 'ExecutionBudget' crates/agent/src --glob '!execute_loop.rs'); do
  sed -i.bak 's/ExecutionBudget/SafetyCap/g' "$f" && rm "$f.bak"
done
```

Then manually inspect each file:

```bash
rg -n 'should_wrap_up|remaining_pct|extend_turns|record_cost|cost_usd|\.exhausted\(\)' crates/agent/src
```

For each hit, decide: delete the line if it's part of the old coercion logic, or rewrite to use `turn_cap_hit()` / `token_cap_hit()`. Most call sites are inside `execute_loop.rs` (handled in Task 3); the others are mostly in `runtime.rs` HUD events and `subagent.rs`.

- [ ] **Step 2.4: Compile-check (will fail in `execute_loop.rs`; that's expected)**

```bash
cargo check -p agent 2>&1 | tail -30
```

Expected: errors limited to `execute_loop.rs`. If errors appear elsewhere, fix them now (likely missed `should_wrap_up`/`remaining_pct` references).

- [ ] **Step 2.5: Commit (loop file still broken; we fix it next task)**

```bash
git add -A
git commit -m "$(cat <<'EOF'
refactor(agent): rename ExecutionBudget -> SafetyCap, drop coercion API

Removes should_wrap_up, remaining_pct, extend_turns, record_cost, cost_usd,
RESERVED_SYNTHESIS_TOKENS, WRAP_UP_THRESHOLD. exhausted() split into
turn_cap_hit / token_cap_hit. execute_loop.rs intentionally still broken;
fixed in next commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Rewrite `execute_loop` — model self-stop primary

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`
- Test: `crates/agent/src/agent_loop/refactor_tests.rs` (existing; we add cases)

This is the core change. The loop's structure becomes:

1. cancel check → `Cancelled`
2. cap check → `SafetyTurnLimit` / `TokenLimit` (no synthesis pass, no fallback string)
3. emit `IterationStart`
4. `run_cycle` (always with full tool list — no `NO_TOOLS` branch)
5. dispatch on `CycleOutcome`:
   - `FinalResponse` / `FabricatedResponse` → `Completed`
   - `ToolsExecuted` → loop-detect → continue
   - `EmptyResponse` → return whatever was accumulated as `Completed` (model self-stopping with empty content is treated as "done")
6. mid-loop compression + live context refresh (unchanged)

- [ ] **Step 3.1: Replace `execute_loop.rs` body**

Replace the contents of `crates/agent/src/execution/execute_loop.rs` from line 1 to EOF with:

```rust
//! Unified execute loop — model self-stop termination.
//!
//! The loop makes LLM calls and executes tools until one of:
//! - The user cancels (partial results returned, FinishReason::Cancelled)
//! - The model returns a final response without tool calls (FinishReason::Completed)
//! - LoopDetector::HardStop fires (FinishReason::LoopDetected)
//! - The safety cap is hit (FinishReason::SafetyTurnLimit or ::TokenLimit)
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
use super::types::{accumulate_usage, CycleOutcome, ExecutionParams, FinishReason};
use crate::events::AgentEvent;

/// Result of the unified execute loop.
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
    pub finish_reason: FinishReason,
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
    let refresher = params
        .context_update_queue
        .as_ref()
        .map(|queue| LiveContextRefresher::new(core.token_counter().clone(), queue.clone()));

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
                    finish_reason: FinishReason::Cancelled,
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
                finish_reason: FinishReason::SafetyTurnLimit,
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
                finish_reason: FinishReason::TokenLimit,
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
        cap.deduct(&cycle_usage);
        cap.tick_turn();

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
                    finish_reason: FinishReason::Completed,
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
                            finish_reason: FinishReason::LoopDetected,
                        });
                    }
                    LoopStatus::NoLoop => {}
                }

                last_content = String::new();
            }

            CycleOutcome::EmptyResponse => {
                // Model returned no text and no tool calls — treat as self-stop.
                debug!(
                    turn = cap.turns_used(),
                    "Empty response from LLM — treating as model self-stop"
                );
                return Ok(ExecuteLoopResult {
                    content: std::mem::take(&mut last_content),
                    usage: accumulated_usage,
                    turns: cap.turns_used(),
                    safety_cap_hit: false,
                    tool_calls: all_tool_calls,
                    finish_reason: FinishReason::Completed,
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
```

> The HUD-only `remaining_pct` is intentionally *local* to this file. It's not a `SafetyCap` method because the cap isn't a budget anymore — exposing it on the type would invite future code to start coercing on it.

- [ ] **Step 3.2: Compile**

```bash
cargo check -p agent 2>&1 | tail -20
```

Expected: clean. If there are unresolved references to removed event fields (the old `SynthesisFailed` event was emitted only on the deleted code paths), make sure no other crate emits it — search:

```bash
rg 'SynthesisFailed' crates/
```

If `SynthesisFailed` is only defined in `events.rs` and only emitted from the old `execute_loop` paths (now deleted), keep the variant — it may be useful — but plan to delete it in Task 9 if no consumer uses it.

- [ ] **Step 3.3: Run existing loop tests**

```bash
cargo nextest run -p agent -E 'test(refactor_tests) | test(execute_loop)'
```

Expected: most pass. Any test asserting `budget_exhausted: true` or `finish_reason: "budget_exhausted"` will fail — those are the tests we update in Step 3.4.

- [ ] **Step 3.4: Update / replace tests in `refactor_tests.rs`**

Open `crates/agent/src/agent_loop/refactor_tests.rs`. For each test:

- Replace `budget_exhausted: true` with `safety_cap_hit: true`.
- Replace `finish_reason: "budget_exhausted"` with `finish_reason: FinishReason::SafetyTurnLimit` (or `::TokenLimit` per the test setup — read the assertion context).
- Replace `finish_reason: "completed"` with `finish_reason: FinishReason::Completed`, etc.
- Delete any test asserting on the wrap-up message ("approaching your budget limit") or coercion message ("budget is exhausted. You MUST respond"). Those code paths are gone.
- Delete any test asserting `SYNTHESIS_FALLBACK` content. The constant is gone; on cap hit we now return whatever `last_content` accumulated (possibly empty).

Add three new tests at the bottom of `refactor_tests.rs`:

```rust
#[tokio::test]
async fn cap_hit_returns_safety_turn_limit_with_no_synthesis_call() {
    // Setup: model that always returns a tool call, max_turns = 3.
    // Expected: 3 LLM calls, then return with FinishReason::SafetyTurnLimit
    //           and safety_cap_hit = true. Crucially: only 3 LLM calls
    //           (no extra "synthesis" call after cap hit).
    let core = test_core_always_tool_calls();
    let mut cap = SafetyCap::with_limits(DepthMode::Normal, u64::MAX, 3);
    let result = execute_loop(
        &core,
        vec![providers::types::Message::user("hi")],
        &[fake_tool_schema()],
        &test_params(),
        &mut cap,
        &test_ctx(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.finish_reason, FinishReason::SafetyTurnLimit);
    assert!(result.safety_cap_hit);
    assert_eq!(result.turns, 3);
    assert_eq!(core.llm_call_count(), 3, "no extra synthesis call expected");
}

#[tokio::test]
async fn natural_self_stop_returns_completed() {
    // Setup: model returns FinalResponse on turn 2.
    let core = test_core_with_responses(vec![tool_call_resp(), final_text_resp("done")]);
    let mut cap = SafetyCap::new(DepthMode::Normal);
    let result = execute_loop(
        &core,
        vec![providers::types::Message::user("hi")],
        &[fake_tool_schema()],
        &test_params(),
        &mut cap,
        &test_ctx(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.finish_reason, FinishReason::Completed);
    assert!(!result.safety_cap_hit);
    assert_eq!(result.content, "done");
    assert_eq!(result.turns, 2);
}

#[tokio::test]
async fn no_wrap_up_or_coercion_messages_are_injected() {
    // Drive the loop near the cap and verify no system messages with the
    // old coercion phrases appear in the message history.
    let core = test_core_recording_messages();
    let mut cap = SafetyCap::with_limits(DepthMode::Normal, u64::MAX, 5);
    let _ = execute_loop(
        &core,
        vec![providers::types::Message::user("hi")],
        &[fake_tool_schema()],
        &test_params(),
        &mut cap,
        &test_ctx(),
        None,
    )
    .await
    .unwrap();
    let recorded = core.recorded_messages();
    let blob = recorded
        .iter()
        .map(|m| format!("{m:?}"))
        .collect::<String>();
    assert!(
        !blob.contains("approaching your budget"),
        "wrap-up message must not appear"
    );
    assert!(
        !blob.contains("budget is exhausted"),
        "coercion message must not appear"
    );
}
```

> The helpers `test_core_always_tool_calls`, `test_core_with_responses`, `test_core_recording_messages`, `test_params`, `test_ctx`, `tool_call_resp`, `final_text_resp`, `fake_tool_schema`, and `core.llm_call_count()` / `core.recorded_messages()` should already exist in `refactor_tests.rs` or its sibling `test_utils.rs` — that file's existing tests use the same harness. If a helper is missing, add it to `crates/agent/src/test_utils.rs` (a small mock `ExecutionCore` that records LLM call count and pushes scripted responses).

- [ ] **Step 3.5: Run updated tests**

```bash
cargo nextest run -p agent -E 'test(refactor_tests)'
```

Expected: green. If `test_core_*` helpers don't exist, see if `crates/agent/src/test_utils.rs` exposes a `MockExecutionCore` — extend that. If still stuck, scope down by deleting the new tests temporarily and verify the rewritten core compiles & old tests pass first; add the new tests in a follow-up commit.

- [ ] **Step 3.6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(agent): model self-stop primary, safety cap as silent backstop

Removes wrap-up message injection, forced final synthesis pass, and the
SYNTHESIS_FALLBACK coercion path from execute_loop. The loop now exits on
cancel / FinalResponse / LoopDetector::HardStop / safety cap / provider
error. Cap hit returns a clean error with whatever partial content
accumulated — no synthesis dance.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Update `ExecuteLoopResult` consumers

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (call sites at `:536` and `:583`)
- Modify: `crates/agent/src/subagent.rs:718`
- Modify: `crates/agent/src/learning/tool_tracking.rs` (consumes `budget_exhausted` from result)

The field renamed `budget_exhausted` → `safety_cap_hit` and `finish_reason: String` → `finish_reason: FinishReason`. Every consumer that pattern-matched on the string needs updating.

- [ ] **Step 4.1: Find all consumers**

```bash
rg -n 'budget_exhausted|\.finish_reason' crates/agent/src
```

Expect ~15-20 hits. For each:
- `result.budget_exhausted` → `result.safety_cap_hit`
- `result.finish_reason == "completed"` → `result.finish_reason == FinishReason::Completed`
- `result.finish_reason == "budget_exhausted"` → `result.safety_cap_hit` (boolean check is clearer)
- Anywhere a wire string is needed (logging, telemetry): `result.finish_reason.as_wire_str()`

- [ ] **Step 4.2: Update `runtime.rs` retry logic**

In `crates/agent/src/agent_runtime/runtime.rs`, the second `execute_loop` invocation around line 583 retries on memory refusal. Confirm its precondition — it likely checks `result.finish_reason` or response content. Adjust the match if it uses string comparison.

```bash
sed -n '500,600p' crates/agent/src/agent_runtime/runtime.rs
```

Read that range, then update conditionals.

- [ ] **Step 4.3: Compile**

```bash
cargo check -p agent 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 4.4: Run agent tests**

```bash
cargo nextest run -p agent
```

Expected: green (pre-existing pass count from Step 0.1, possibly +3 from Task 3 new tests).

- [ ] **Step 4.5: Commit**

```bash
git add -A
git commit -m "refactor(agent): migrate execute_loop consumers to FinishReason / safety_cap_hit

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Storage column rename — `budget_exhausted` → `safety_cap_hit`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`
- Modify: `crates/storage/src/repos/strategy.rs`
- Modify: `crates/storage/src/rows/learning.rs` (where `StrategyRecordRow` is defined)
- Modify: `crates/agent/src/learning/tool_tracking.rs` (writes the row)
- Modify: `crates/agent/src/autotuner/metric_collector.rs:269,296` (test fixtures)
- Modify: `crates/cognitive/src/services/reforge/feedback.rs:169` (SUM query)

Per CLAUDE.md: pre-release, no on-disk users. We rename in-place in `001_initial.sql` and in every `StrategyRecordRow` reference.

- [ ] **Step 5.1: Rename SQL column**

In `crates/storage/migrations/001_initial.sql:225`, change:

```sql
budget_exhausted   INTEGER DEFAULT 0,
```

to:

```sql
safety_cap_hit   INTEGER DEFAULT 0,
```

- [ ] **Step 5.2: Update `StrategyRecordRow`**

```bash
rg -n 'pub budget_exhausted' crates/storage/src
```

Find the field definition and rename `budget_exhausted: bool` → `safety_cap_hit: bool`. Update the corresponding `serde` rename or column-mapping attribute if any.

- [ ] **Step 5.3: Update `repos/strategy.rs` SQL**

```bash
rg -n 'budget_exhausted' crates/storage/src/repos/strategy.rs
```

Rename in the `INSERT` column list and in `.bind()` calls. Apply mechanically:

```bash
sed -i.bak 's/budget_exhausted/safety_cap_hit/g' crates/storage/src/repos/strategy.rs
rm crates/storage/src/repos/strategy.rs.bak
```

- [ ] **Step 5.4: Update `feedback.rs` SUM query**

In `crates/cognitive/src/services/reforge/feedback.rs:169`, change:

```sql
COALESCE(SUM(CASE WHEN budget_exhausted = 1 THEN 1 ELSE 0 END), 0)
```

to:

```sql
COALESCE(SUM(CASE WHEN safety_cap_hit = 1 THEN 1 ELSE 0 END), 0)
```

Also update the corresponding Rust field name in the `StrategyMetrics` struct (or whatever consumes this query result).

- [ ] **Step 5.5: Apply remaining mechanical renames**

```bash
for f in $(rg -l 'budget_exhausted' crates/agent/src crates/storage/src crates/cognitive/src); do
  sed -i.bak 's/budget_exhausted/safety_cap_hit/g' "$f" && rm "$f.bak"
done
rg 'budget_exhausted' crates/  # should be empty now
```

- [ ] **Step 5.6: Compile + test**

```bash
cargo check --workspace 2>&1 | tail -10
cargo nextest run -p agent -p storage -p cognitive
```

Expected: green.

- [ ] **Step 5.7: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
refactor(storage): rename budget_exhausted column to safety_cap_hit

Pre-release in-place rename per CLAUDE.md migration policy. Updates
StrategyRecordRow, strategy repo SQL, reforge feedback SUM query, and
autotuner test fixtures.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Long-running tool timeouts via `Tool::custom_timeout`

**Files:**
- Modify: shell-tool implementations under `crates/feature-coding/src/tools/` (or wherever `cargo`/`bash` tools live — discover via `rg`)
- Modify: `crates/agent/src/execution/core.rs:51` (rename `INTERACTIVE_TOOL_TIMEOUT` to `LONG_RUNNING_TOOL_TIMEOUT`, repurpose it as the value tools opt into)
- Test: each tool's existing inline tests

The `Tool::custom_timeout()` hook already exists. We don't add a new trait method — we just override it on shell-y tools.

- [ ] **Step 6.1: Locate shell / build tools**

```bash
rg -l 'fn name.*"bash"|fn name.*"shell"|fn name.*"cargo"|fn name.*"run_command"' crates/
```

Capture the file paths and tool names. The expected candidates are the bash tool, cargo tool, and any test-runner tool.

- [ ] **Step 6.2: Add a shared constant**

In `crates/agent/src/execution/core.rs:51`, replace:

```rust
const INTERACTIVE_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
```

with:

```rust
/// Timeout for tools that block on user input (`ask_user`).
const INTERACTIVE_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Recommended timeout for shell / build / test tools whose wall-time can
/// legitimately exceed `params.tool_timeout` (default 30s). Tools opt in by
/// returning `Some(LONG_RUNNING_TOOL_TIMEOUT)` from `Tool::custom_timeout()`.
pub const LONG_RUNNING_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
```

- [ ] **Step 6.3: Override `custom_timeout` on each long-running tool**

For each tool found in 6.1, add:

```rust
fn custom_timeout(&self) -> Option<std::time::Duration> {
    Some(crate::execution::core::LONG_RUNNING_TOOL_TIMEOUT)
}
```

> Inside the same crate as `core.rs` (`agent`) the path is `crate::execution::core::LONG_RUNNING_TOOL_TIMEOUT`. From a different crate (e.g., `feature-coding`), import via the `agent` re-export — add a `pub use execution::core::LONG_RUNNING_TOOL_TIMEOUT;` line to `crates/agent/src/lib.rs` if not already exported.

- [ ] **Step 6.4: Add a unit test asserting the override**

For each tool that gained the override, add:

```rust
#[test]
fn cargo_tool_uses_long_running_timeout() {
    let t = CargoTool::new();
    assert_eq!(
        t.custom_timeout(),
        Some(std::time::Duration::from_secs(600))
    );
}
```

(Adapt the constructor to whatever the tool actually exposes.)

- [ ] **Step 6.5: Compile + test**

```bash
cargo nextest run -p agent
# plus whichever feature crate hosts the shell tools
```

Expected: green.

- [ ] **Step 6.6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(tools): long-running tools opt into 600s timeout

Adds LONG_RUNNING_TOOL_TIMEOUT constant; cargo / shell / build tools
override Tool::custom_timeout() to opt in. Default 30s tool_timeout still
applies to read-only tools. ask_user retains its dedicated INTERACTIVE_TOOL_TIMEOUT.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Desktop UI — bindings and progress label

**Files:**
- Modify: `desktop-ui/src/bindings.ts` (regenerated by `cargo tauri dev`)
- Modify: `desktop-ui/src/features/threads/` — locate the iteration-progress component
- Modify: `desktop-ui/src/services/events.ts` (if a Zod / Valibot schema exists for `ExecutionStarted`)

`ExecutionStarted { max_iterations }` still fires; its semantics shift from "soft target" to "hard cap". The progress display drops the denominator for the user-facing main agent (where the cap = 100 is meaningless to show) but keeps it for subagents (cap = 5/10/15 is meaningful tier info).

- [ ] **Step 7.1: Regenerate Specta bindings**

```bash
cd /Users/jayden/Projects/Klynt/bot
cargo tauri dev &  # let it generate, then Ctrl-C
```

After Vite is up, check:

```bash
git diff desktop-ui/src/bindings.ts
```

Expected: type updates around `FinishReason` and `ExecuteLoopResult`. Stop the dev server after the file is regenerated.

- [ ] **Step 7.2: Locate progress component**

```bash
rg -l 'maxIterations|max_iterations' desktop-ui/src
```

For each consumer, decide:
- Subagent contexts (where `engine` field indicates subagent) → keep `Turn N / M` label.
- Main agent → display just `Turn N`.

If a single component handles both, branch on the `engine` field of `ExecutionStarted`.

- [ ] **Step 7.3: Update component**

Pseudocode (adjust to actual JSX):

```tsx
const { iteration, max, engine } = progressState;
const isSubagent = engine?.startsWith("subagent");
return <span>{isSubagent ? `Turn ${iteration} / ${max}` : `Turn ${iteration}`}</span>;
```

- [ ] **Step 7.4: Run UI tests**

```bash
cd desktop-ui && bun run test
```

Expected: green. If there are snapshot tests asserting `Turn N / M` for the main agent, update snapshots.

- [ ] **Step 7.5: Run UI typecheck + lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: clean.

- [ ] **Step 7.6: Commit**

```bash
git add -A
git commit -m "feat(desktop-ui): drop turn denominator for main agent (safety cap is silent)

Subagent progress still shows Turn N / M (tier caps are meaningful to the user).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Spec-coverage tests + cleanup

**Files:**
- Test: `crates/agent/src/agent_loop/refactor_tests.rs` (additional cases)
- Modify: `crates/agent/src/events.rs` — drop `SynthesisFailed` if no consumer remains

- [ ] **Step 8.1: Confirm no e2e regression includes the fallback string**

```bash
cargo nextest run --workspace -E 'kind(test) and binary(e2e)'
```

Then grep the test output JSON / stdout for the old fallback string:

```bash
rg "I was unable to produce a final response within the allowed budget" crates/ tests/
```

Expected: only matches in archived docs/CHANGELOG, not in code.

- [ ] **Step 8.2: Remove `SynthesisFailed` event variant if orphaned**

```bash
rg 'SynthesisFailed' crates/
```

Expected: only the `enum AgentEvent` variant declaration (no emitters). If so, delete the variant from `events.rs` plus any `match` arms that reference it. If a desktop UI consumer references it, keep it for now and mark as `#[deprecated]`.

- [ ] **Step 8.3: Run full agent test suite**

```bash
cargo nextest run -p agent
```

- [ ] **Step 8.4: Commit (only if Step 8.2 produced changes)**

```bash
git add -A
git commit -m "chore(agent): remove orphaned SynthesisFailed event variant

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Full validation gate

- [ ] **Step 9.1: Workspace build**

```bash
cargo build --workspace 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 9.2: Workspace tests**

```bash
cargo nextest run --workspace 2>&1 | tail -20
```

Expected: green. Compare pass count against Step 0.1 baseline. Acceptable delta: +N for added tests in Tasks 1, 3, 6.

- [ ] **Step 9.3: Doctests**

```bash
cargo test --workspace --doc
```

Expected: green.

- [ ] **Step 9.4: Clippy (zero warnings policy)**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20
```

Expected: zero warnings outside the `desktop` crate's pre-existing exceptions.

- [ ] **Step 9.5: Format check**

```bash
cargo fmt --all --check
```

If it complains, run `cargo fmt --all` and amend the last commit (or commit separately as `style: cargo fmt`).

- [ ] **Step 9.6: KCA validation gates**

```bash
./scripts/run_kca_validation.sh
```

Expected: green.

- [ ] **Step 9.7: UI build**

```bash
cd desktop-ui && bun run build && bun run test && bun run lint && bun run typecheck
```

Expected: green across all four.

- [ ] **Step 9.8: Manual smoke test (Tauri dev)**

```bash
cd /Users/jayden/Projects/Klynt/bot
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
# In another terminal: cd desktop-ui && bun run dev
```

Open the desktop app. Verify:
1. A simple chat ("hi") produces a single turn, terminates as `Completed`.
2. A multi-tool task ("list my tasks then summarize") runs tools, terminates as `Completed` without a wrap-up message in the transcript.
3. Iteration HUD reads `Turn N` (no denominator) for the main agent.

- [ ] **Step 9.9: Final commit (if anything was tweaked in 9.x)**

```bash
git status
# If clean, skip. Otherwise:
git add -A
git commit -m "chore: validation gate fixups

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 9.10: Push branch**

```bash
git push -u origin feat/model-self-stop-termination
```

- [ ] **Step 9.11: Open PR** (only on user's explicit request)

```bash
gh pr create --title "feat(agent): model self-stop termination (OpenCode-style)" --body "$(cat <<'EOF'
## Summary
- Replace token+turn budget coercion with model self-stop as primary terminator
- Rename ExecutionBudget → SafetyCap; drop wrap-up, forced synthesis, SYNTHESIS_FALLBACK
- Add typed FinishReason enum (replaces stringly-typed reason)
- Long-running shell/build tools opt into 600s timeout via Tool::custom_timeout()
- Storage column budget_exhausted → safety_cap_hit (pre-release in-place rename)

Spec: docs/superpowers/specs/2026-05-05-model-self-stop-termination-design.md
Plan: docs/superpowers/plans/2026-05-05-model-self-stop-termination.md

## Test plan
- [x] `cargo nextest run --workspace`
- [x] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [x] `./scripts/run_kca_validation.sh` — green
- [x] `cd desktop-ui && bun run build && bun run test && bun run lint && bun run typecheck`
- [x] Manual smoke test in `cargo tauri dev` (single-turn, multi-tool, subagent)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage:**
- §3.1 termination matrix → Tasks 3, 4 (`FinishReason` cases mapped 1:1).
- §3.2 safety caps → Task 2 (`DEFAULT_SAFETY_TURN_CAP = 100`); subagent caps preserved by no-op (Task 4 audit).
- §3.3 tool-level timeout policy → Task 6 (uses existing `custom_timeout`; no new trait method needed — simpler than spec proposed).
- §3.4 public contract preservation → Tasks 4, 5, 7 (`ExecutionStarted` schema unchanged; storage column renamed; `FinishReason` typed).
- §3.5 desktop UI → Task 7.
- §4 implementation outline ordering → Tasks 1–8 follow the same dependency order.
- §5 test plan → Tasks 1.2, 3.4, 6.4, 8.1, 9.2.
- §7 migration → Task 5.
- §8 rollback → single-branch, mostly mechanical commits per task.

**Placeholder scan:** No "TBD" / "implement later" / "appropriate error handling" placeholders. Each step shows the actual code or command. The exception is Task 6.1, which uses `rg` to discover tool file paths because the workspace doesn't have a single canonical home for shell tools — discovery is the genuine work, not a placeholder.

**Type consistency:**
- `SafetyCap` methods used: `new`, `with_limits`, `deduct`, `tick_turn`, `turn_cap_hit`, `token_cap_hit`, `tokens_used`, `turns_used`, `max_turns`, `max_tokens`. All defined in Task 2.1, all used consistently in Task 3.1.
- `FinishReason` variants: `Completed`, `Cancelled`, `SafetyTurnLimit`, `TokenLimit`, `LoopDetected`, `Error`. Consistent across Tasks 1, 3, 4.
- `ExecuteLoopResult` field renames: `budget_exhausted` → `safety_cap_hit`, `finish_reason: String` → `finish_reason: FinishReason`. Applied uniformly Tasks 3–5.

**Scope check:** Single-subsystem (agent execution loop). Self-contained — no other plan needed.

---

## Spec ↔ Plan link

Spec lives at `docs/superpowers/specs/2026-05-05-model-self-stop-termination-design.md`. Any deviation discovered during implementation should update the spec in the same PR.
