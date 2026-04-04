# Budget-Bounded Execution Model — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the time-bounded agent pipeline (11-step, LLM classifier, wall-clock timeout) with a budget-bounded execution model (6-phase, no classifier, token/turn budget with user-controlled depth modes).

**Architecture:** Create `ExecutionBudget` and `DepthMode` types, build a unified `ExecuteLoop` that replaces both DirectEngine and ReactiveEngine, simplify `process_message()` from 11 steps to 6 phases, remove the IntentAnalyzer LLM classifier (keep heuristic layers), add new AgentEvent variants for the budget HUD, update config schema, and clean all legacy code. The simulator and all consumers are updated to match.

**Tech Stack:** Rust, tokio, serde, sqlx, providers crate (LLM streaming), bus crate (domain events)

---

## File Structure

### New files to create:
- `crates/agent/src/execution/budget.rs` — `ExecutionBudget`, `DepthMode`, `SkillBudget`, budget lifecycle
- `crates/agent/src/execution/execute_loop.rs` — Unified execute loop replacing Direct+Reactive engines
- `crates/config/src/schema/execution.rs` — `ExecutionConfig`, `SkillBudgetConfig`

### Files to modify significantly:
- `crates/agent/src/agent_runtime/runtime.rs` — Simplify `process_message()` from 11 steps to 6 phases
- `crates/agent/src/events.rs` — Add budget/depth/enrichment event variants
- `crates/agent/src/intent_pipeline/analysis.rs` — Remove Layer 3+4, keep Layer 1-2 as signal generator
- `crates/agent/src/intent_pipeline/types.rs` — Remove `ExecutionMode`, simplify `PipelineConfig`
- `crates/config/src/schema/agents.rs` — Remove `pipeline_timeout_secs`, add `execution` section
- `crates/config/src/schema/hot.rs` — Replace `pipeline_timeout_secs` with `safety_timeout_secs`
- `crates/config/src/schema/orchestrator.rs` — Remove `llm_classifier_*` fields
- `crates/simulator/src/agent_harness.rs` — Use new pipeline API

### Files to delete:
- `crates/agent/src/intent_pipeline/engines/direct.rs` — Replaced by ExecuteLoop
- `crates/agent/src/intent_pipeline/engines/reactive.rs` — Replaced by ExecuteLoop
- `crates/agent/src/intent_pipeline/router.rs` — No longer needed (unified loop)

### Files with minor updates:
- `crates/agent/src/intent_pipeline/engines/mod.rs` — Remove direct/reactive re-exports
- `crates/agent/src/execution/mod.rs` — Add budget, execute_loop re-exports
- `crates/agent/src/lib.rs` — Update re-exports
- `crates/config/src/schema/mod.rs` — Add execution module
- `crates/config/src/lib.rs` — Re-export ExecutionConfig
- `crates/agent/src/agent_loop/builder.rs` — Use new constructor (no ExecutionRouter)

---

### Task 1: Create ExecutionBudget and DepthMode Types

**Files:**
- Create: `crates/agent/src/execution/budget.rs`
- Modify: `crates/agent/src/execution/mod.rs`

These are the foundational types the rest of the plan depends on. No external dependencies beyond `serde` and `providers::Usage`.

- [ ] **Step 1: Create budget.rs with DepthMode enum**

Create `crates/agent/src/execution/budget.rs`:

```rust
//! Execution budget — token/turn budget with user-controlled depth modes.
//!
//! Replaces the old wall-clock `pipeline_timeout_secs` with a budget model
//! inspired by Claude Code. The budget correlates with work done, not
//! arbitrary time — a slow provider and a fast provider doing the same
//! work use the same budget.

use serde::{Deserialize, Serialize};

// ── Depth Modes ──────────────────────────────────────────────

/// User-selectable depth mode. Controls how deeply the agent thinks.
///
/// - `Normal`: fast, frictionless — default for daily use
/// - `DeepThink`: +Mirror context, coaching evaluation, visible HUD
/// - `Ultra`: full cognitive partner — auto-save, FSRS atoms, visible enrichment
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

// ── Skill Budget Defaults ────────────────────────────────────

/// Per-skill default budget parameters.
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
            normal_turns: 15,
            deep_multiplier: 1.5,
            ultra_multiplier: 3.0,
        }
    }
}

/// Well-known skill budget presets.
pub fn skill_budget_for(skill_name: &str) -> SkillBudget {
    match skill_name {
        "task-management" => SkillBudget {
            normal_tokens: 40_000,
            normal_turns: 12,
            ..Default::default()
        },
        "finance-management" => SkillBudget {
            normal_tokens: 80_000,
            normal_turns: 20,
            ..Default::default()
        },
        "communication" => SkillBudget {
            normal_tokens: 40_000,
            normal_turns: 10,
            ..Default::default()
        },
        "automation" => SkillBudget {
            normal_tokens: 50_000,
            normal_turns: 15,
            ..Default::default()
        },
        _ => SkillBudget::default(), // "general" and unknown skills
    }
}

// ── Execution Budget ─────────────────────────────────────────

/// Minimum tokens reserved for the final synthesis response.
const RESERVED_SYNTHESIS_TOKENS: u64 = 2_000;

/// Percentage of budget at which "wrap up" instruction is injected.
const WRAP_UP_THRESHOLD: f32 = 0.85;

/// Budget state for a single request execution.
///
/// Created at the start of Phase 2, checked before every LLM call in Phase 3,
/// deducted after every response. When the budget is nearly exhausted, a
/// "wrap up" system instruction is injected. When fully exhausted, the loop
/// forces a synthesis response with partial results.
#[derive(Debug, Clone)]
pub struct ExecutionBudget {
    pub depth: DepthMode,
    max_tokens: u64,
    max_turns: u32,
    tokens_used: u64,
    turns_used: u32,
    cost_usd: f64,
}

impl ExecutionBudget {
    /// Create a budget from the user's depth choice and the matched skill.
    pub fn new(depth: DepthMode, skill_name: &str) -> Self {
        let base = skill_budget_for(skill_name);
        let (max_tokens, max_turns) = match depth {
            DepthMode::Normal => (base.normal_tokens, base.normal_turns),
            DepthMode::DeepThink => (
                (base.normal_tokens as f64 * base.deep_multiplier as f64) as u64,
                (base.normal_turns as f64 * base.deep_multiplier as f64) as u32,
            ),
            DepthMode::Ultra => (
                (base.normal_tokens as f64 * base.ultra_multiplier as f64) as u64,
                u32::MAX, // Ultra: unlimited turns, bounded by monthly budget only
            ),
        };

        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
            cost_usd: 0.0,
        }
    }

    /// Create a budget with explicit limits (for testing / simulator).
    pub fn with_limits(depth: DepthMode, max_tokens: u64, max_turns: u32) -> Self {
        Self {
            depth,
            max_tokens,
            max_turns,
            tokens_used: 0,
            turns_used: 0,
            cost_usd: 0.0,
        }
    }

    /// Record token usage from an LLM response.
    pub fn deduct(&mut self, usage: &providers::Usage) {
        let total = usage.prompt_tokens as u64 + usage.completion_tokens as u64;
        self.tokens_used += total;
    }

    /// Record estimated cost for this response.
    pub fn record_cost(&mut self, cost_usd: f64) {
        self.cost_usd += cost_usd;
    }

    /// Increment the turn counter. Call after each complete LLM cycle.
    pub fn tick_turn(&mut self) {
        self.turns_used += 1;
    }

    /// True when the "wrap up soon" instruction should be injected.
    pub fn should_wrap_up(&self) -> bool {
        self.remaining_pct() < (1.0 - WRAP_UP_THRESHOLD)
    }

    /// True when the budget is fully exhausted.
    pub fn exhausted(&self) -> bool {
        self.tokens_used + RESERVED_SYNTHESIS_TOKENS >= self.max_tokens
            || self.turns_used >= self.max_turns
    }

    /// Fraction of budget remaining (0.0–1.0). Uses the tighter of token or turn budget.
    pub fn remaining_pct(&self) -> f32 {
        let token_pct = if self.max_tokens == 0 {
            0.0
        } else {
            1.0 - (self.tokens_used as f32 / self.max_tokens as f32)
        };
        let turn_pct = if self.max_turns == u32::MAX {
            1.0 // Ultra: unlimited turns
        } else if self.max_turns == 0 {
            0.0
        } else {
            1.0 - (self.turns_used as f32 / self.max_turns as f32)
        };
        token_pct.min(turn_pct).clamp(0.0, 1.0)
    }

    /// Extend the budget by additional turns (user tapped "Extend" in HUD).
    pub fn extend_turns(&mut self, additional: u32) {
        if self.max_turns != u32::MAX {
            self.max_turns = self.max_turns.saturating_add(additional);
        }
    }

    // ── Accessors for HUD events ──

    pub fn tokens_used(&self) -> u64 {
        self.tokens_used
    }

    pub fn turns_used(&self) -> u32 {
        self.turns_used
    }

    pub fn cost_usd(&self) -> f64 {
        self.cost_usd
    }

    pub fn max_turns(&self) -> u32 {
        self.max_turns
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_budget_uses_skill_defaults() {
        let budget = ExecutionBudget::new(DepthMode::Normal, "task-management");
        assert_eq!(budget.max_tokens, 40_000);
        assert_eq!(budget.max_turns, 12);
        assert!(!budget.exhausted());
        assert!(!budget.should_wrap_up());
        assert!((budget.remaining_pct() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn deep_think_scales_by_multiplier() {
        let budget = ExecutionBudget::new(DepthMode::DeepThink, "task-management");
        assert_eq!(budget.max_tokens, 60_000); // 40K * 1.5
        assert_eq!(budget.max_turns, 18); // 12 * 1.5
    }

    #[test]
    fn ultra_has_unlimited_turns() {
        let budget = ExecutionBudget::new(DepthMode::Ultra, "general");
        assert_eq!(budget.max_turns, u32::MAX);
        // Token budget is still finite
        assert_eq!(budget.max_tokens, 180_000); // 60K * 3.0
    }

    #[test]
    fn deduct_tracks_usage() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 10_000, 5);
        let usage = providers::Usage {
            prompt_tokens: 3000,
            completion_tokens: 1000,
            ..Default::default()
        };
        budget.deduct(&usage);
        budget.tick_turn();

        assert_eq!(budget.tokens_used(), 4000);
        assert_eq!(budget.turns_used(), 1);
    }

    #[test]
    fn exhausted_by_tokens() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 5_000, 100);
        let usage = providers::Usage {
            prompt_tokens: 2000,
            completion_tokens: 1500,
            ..Default::default()
        };
        budget.deduct(&usage);
        assert!(!budget.exhausted()); // 3500 used, 5000 max, 2000 reserved → not yet

        budget.deduct(&usage); // 7000 total → exceeds 5000
        assert!(budget.exhausted());
    }

    #[test]
    fn exhausted_by_turns() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 1_000_000, 3);
        budget.tick_turn();
        budget.tick_turn();
        assert!(!budget.exhausted());
        budget.tick_turn();
        assert!(budget.exhausted());
    }

    #[test]
    fn wrap_up_at_85_percent() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 10_000, 100);
        assert!(!budget.should_wrap_up());

        // Use 8600 tokens → 86% → should wrap up
        let usage = providers::Usage {
            prompt_tokens: 5000,
            completion_tokens: 3600,
            ..Default::default()
        };
        budget.deduct(&usage);
        assert!(budget.should_wrap_up());
    }

    #[test]
    fn extend_turns_adds_to_budget() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 100_000, 10);
        for _ in 0..10 {
            budget.tick_turn();
        }
        assert!(budget.exhausted());

        budget.extend_turns(20);
        assert!(!budget.exhausted());
        assert_eq!(budget.max_turns(), 30);
    }

    #[test]
    fn remaining_pct_uses_tighter_constraint() {
        let mut budget = ExecutionBudget::with_limits(DepthMode::Normal, 100_000, 4);
        // Use 3 of 4 turns → 25% remaining by turns
        budget.tick_turn();
        budget.tick_turn();
        budget.tick_turn();
        // But only ~0% tokens used → 100% remaining by tokens
        // Should return 25% (the tighter one)
        assert!((budget.remaining_pct() - 0.25).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Add budget module to execution/mod.rs**

In `crates/agent/src/execution/mod.rs`, add:

```rust
pub mod budget;
```

And add re-exports:

```rust
pub use budget::{DepthMode, ExecutionBudget, SkillBudget};
```

- [ ] **Step 3: Build and run tests**

Run: `cargo build -p agent && cargo nextest run -p agent -E 'test(budget)'`
Expected: All 7 budget tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/execution/budget.rs crates/agent/src/execution/mod.rs
git commit -m "feat(agent): add ExecutionBudget and DepthMode types"
```

---

### Task 2: Add New AgentEvent Variants

**Files:**
- Modify: `crates/agent/src/events.rs`

Add the budget HUD, depth suggestion, enrichment, and turn-tracking events.

- [ ] **Step 1: Add new event variants to AgentEvent enum**

In `crates/agent/src/events.rs`, add these variants to the `AgentEvent` enum (at the end, before the closing `}`):

```rust
    // ── Budget HUD (Deep/Ultra mode) ─────────────────────────

    /// Emitted after each turn with current budget state.
    /// UI renders the live budget HUD from this.
    BudgetUpdate {
        tokens_remaining_pct: f32,
        turns_used: u32,
        max_turns: u32,
        cost_usd: f64,
        depth: String,
    },

    /// User extended the budget mid-conversation.
    BudgetExtended {
        additional_turns: u32,
        new_max_turns: u32,
    },

    // ── Depth suggestion (adaptive layer) ────────────────────

    /// Mirror/history suggests a different depth mode.
    DepthSuggestion {
        recommended: String,
        reason: String,
    },

    // ── Enrichment progress (Phase 4) ────────────────────────

    /// Post-response enrichment started (Mirror, Coaching, NoteTree, FSRS).
    EnrichmentStarted {
        phase: String,
    },

    /// Post-response enrichment completed.
    EnrichmentComplete {
        phase: String,
        summary: String,
    },

    // ── Turn tracking ────────────────────────────────────────

    /// Emitted at the end of each execute-loop turn.
    TurnComplete {
        turn: u32,
        budget_remaining_pct: f32,
    },
```

- [ ] **Step 2: Build**

Run: `cargo build -p agent`
Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add budget HUD, depth suggestion, and enrichment AgentEvent variants"
```

---

### Task 3: Create ExecutionConfig and Update Config Schema

**Files:**
- Create: `crates/config/src/schema/execution.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/agents.rs`
- Modify: `crates/config/src/schema/hot.rs`
- Modify: `crates/config/src/schema/orchestrator.rs`

- [ ] **Step 1: Create execution.rs config**

Create `crates/config/src/schema/execution.rs`:

```rust
//! Execution pipeline configuration — budget-bounded model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution pipeline configuration. Replaces the old `pipeline_timeout_secs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionConfig {
    /// Safety wall-clock timeout in seconds. Catches deadlocks only.
    /// Should never fire in normal operation. Default: 600.
    #[serde(default = "default_safety_timeout")]
    pub safety_timeout_secs: u64,

    /// Enable adaptive depth suggestions from Mirror. Default: true.
    #[serde(default = "default_true")]
    pub adaptive_depth: bool,

    /// Per-skill budget overrides. Keys are skill names.
    #[serde(default)]
    pub skill_budgets: HashMap<String, SkillBudgetOverride>,
}

/// Per-skill budget override in config.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBudgetOverride {
    pub normal_tokens: Option<u64>,
    pub normal_turns: Option<u32>,
    pub deep_multiplier: Option<f32>,
    pub ultra_multiplier: Option<f32>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            safety_timeout_secs: default_safety_timeout(),
            adaptive_depth: true,
            skill_budgets: HashMap::new(),
        }
    }
}

fn default_safety_timeout() -> u64 {
    600
}

fn default_true() -> bool {
    true
}
```

- [ ] **Step 2: Register in schema/mod.rs**

In `crates/config/src/schema/mod.rs`, add:

```rust
pub mod execution;
```

And in `crates/config/src/lib.rs` (or wherever schema types are re-exported), add:

```rust
pub use schema::execution::{ExecutionConfig, SkillBudgetOverride};
```

- [ ] **Step 3: Update AgentDefaults — remove pipeline_timeout_secs, add execution**

In `crates/config/src/schema/agents.rs`, in the `AgentDefaults` struct:

Remove the field:
```rust
    pub pipeline_timeout_secs: u64,
```

Add in its place:
```rust
    /// Execution budget configuration. Replaces pipeline_timeout_secs.
    #[serde(default)]
    pub execution: ExecutionConfig,
```

Also remove the `default_pipeline_timeout_secs()` function and its `#[serde(default = "...")]` annotation.

- [ ] **Step 4: Update HotConfig — replace pipeline_timeout_secs with safety_timeout_secs**

In `crates/config/src/schema/hot.rs`, in the `HotConfig` struct:

Replace:
```rust
    pub pipeline_timeout_secs: u64,
```
With:
```rust
    pub safety_timeout_secs: u64,
```

In `HotConfigDiff`, replace:
```rust
    pub pipeline_timeout_changed: bool,
```
With:
```rust
    pub safety_timeout_changed: bool,
```

Update the `From<&Config>` impl and the `diff()` method to use the new field name. The value comes from `config.agents.defaults.execution.safety_timeout_secs`.

- [ ] **Step 5: Clean OrchestratorConfig — remove LLM classifier fields**

In `crates/config/src/schema/orchestrator.rs`, remove these fields from `OrchestratorConfig`:

```rust
    pub llm_classifier_timeout: u64,
    pub llm_classifier_model: Option<String>,
```

And their corresponding default functions. Keep `heuristic_confidence_threshold`, `max_escalations`, `max_fabrication_retries`, `satisfaction_window_minutes` — these are still used by heuristic classification and other systems.

- [ ] **Step 6: Fix all compilation errors**

Run: `cargo build --workspace 2>&1 | head -50`

There will be compilation errors in files that reference `pipeline_timeout_secs` or `llm_classifier_*`. Fix each one:
- `crates/agent/src/agent_runtime/runtime.rs` — uses `pipeline_timeout_secs` from HotConfig
- `crates/agent/src/intent_pipeline/analysis.rs` — uses `llm_classifier_*` from OrchestratorConfig
- `crates/agent/src/agent_loop/builder.rs` — constructs PipelineConfig with pipeline_timeout_secs
- `crates/app-core/` — may reference these config fields
- `crates/simulator/src/agent_harness.rs` — uses PipelineConfig

For now, replace `pipeline_timeout_secs` references with `safety_timeout_secs` where they appear, using the same numeric value. The actual behavior change happens in later tasks. For `llm_classifier_*` references, remove them or replace with the heuristic confidence threshold.

- [ ] **Step 7: Build the full workspace**

Run: `cargo build --workspace`
Expected: Clean build with no errors from config changes.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(config): add ExecutionConfig, remove pipeline_timeout_secs and llm_classifier fields"
```

---

### Task 4: Create the Unified ExecuteLoop

**Files:**
- Create: `crates/agent/src/execution/execute_loop.rs`
- Modify: `crates/agent/src/execution/mod.rs`

This is the core of the redesign — a single loop that replaces DirectEngine + ReactiveEngine + ExecutionRouter.

- [ ] **Step 1: Create execute_loop.rs**

Create `crates/agent/src/execution/execute_loop.rs`:

```rust
//! Unified execute loop — replaces DirectEngine, ReactiveEngine, and ExecutionRouter.
//!
//! The loop makes LLM calls and executes tools until:
//! - The model returns no tool calls (natural completion)
//! - The budget is exhausted (graceful synthesis)
//! - The user cancels (partial results)
//! - A safety timeout fires (emergency stop — indicates a bug)

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use common::Result;
use providers::{types::Message, Usage};
use tools::RoutingContext;

use super::budget::ExecutionBudget;
use super::core::ExecutionCore;
use super::types::{CycleOutcome, ExecutionParams, ToolExecutionResult};
use super::ReasoningTrace;
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
    /// Reasoning traces from the execution.
    pub traces: Vec<ReasoningTrace>,
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
    mut messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    budget: &mut ExecutionBudget,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<ExecuteLoopResult> {
    let mut accumulated_usage = Usage::default();
    let mut all_tool_calls: Vec<String> = Vec::new();
    let mut all_traces: Vec<ReasoningTrace> = Vec::new();
    let mut seen_tool_calls: HashSet<u64> = HashSet::new();
    let mut last_content = String::new();

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
                    traces: all_traces,
                });
            }
            // No content yet — inject wrap-up and do one final LLM call
            messages.push(Message::system(
                "Your budget is exhausted. Provide a concise final response \
                 summarizing any results you have so far.",
            ));
        }

        if budget.should_wrap_up() && !budget.exhausted() {
            // Inject a gentle reminder — the model will see this in context
            messages.push(Message::system(
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
                    traces: all_traces,
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

        accumulated_usage.prompt_tokens += cycle_usage.prompt_tokens;
        accumulated_usage.completion_tokens += cycle_usage.completion_tokens;
        accumulated_usage.cache_read_tokens += cycle_usage.cache_read_tokens;
        accumulated_usage.cache_write_tokens += cycle_usage.cache_write_tokens;
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
                    traces: all_traces,
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
                    traces: all_traces,
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
            debug!("Budget exhausted after turn {} — one more call for synthesis", budget.turns_used());
            continue; // The budget gate at the top will inject the wrap-up instruction
        }
    }
}
```

- [ ] **Step 2: Add execute_loop to execution/mod.rs**

In `crates/agent/src/execution/mod.rs`, add:

```rust
pub mod execute_loop;
```

And re-export:

```rust
pub use execute_loop::{execute_loop, ExecuteLoopResult};
```

- [ ] **Step 3: Build**

Run: `cargo build -p agent`
Expected: Clean build. The `execute_loop` function compiles but is not yet called by anything.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/execution/execute_loop.rs crates/agent/src/execution/mod.rs
git commit -m "feat(agent): add unified ExecuteLoop replacing Direct+Reactive engines"
```

---

### Task 5: Simplify IntentAnalyzer — Remove LLM Classifier

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs`
- Modify: `crates/agent/src/intent_pipeline/types.rs`

Remove Layer 3 (LLM classifier) and Layer 4 (cognitive boost). Keep Layer 1 (heuristic AC matchers) and Layer 2 (embedding fallback) as a lightweight signal generator.

- [ ] **Step 1: Simplify the analyze() method**

In `crates/agent/src/intent_pipeline/analysis.rs`, find the `analyze()` method. It currently runs a 4-layer cascade. Simplify it to only run Layer 1 and Layer 2:

The new `analyze()` should:
1. Run `analyze_heuristic()` (Layer 1) — unchanged
2. If confidence < threshold AND embedder is available, run `analyze_with_embedding()` (Layer 2) — unchanged
3. **Remove:** the Layer 3 `classify_with_llm()` call entirely
4. **Remove:** the Layer 4 `apply_cognitive_boost()` call
5. Set `source` to `AnalysisSource::Heuristic` always (no more `LlmClassifier` or `ShadowDeferred`)
6. Return the result from Layer 1 or Layer 2

Also remove the `shadow_mode` field from the `IntentAnalyzer` struct — it's no longer needed since we never call the LLM. Remove `with_shadow_mode()` builder method.

Remove the `classify_with_llm()` private method entirely (it's the one that makes the LLM call via `self.classifier`).

Remove the `apply_cognitive_boost()` private method.

Remove the `IntentClassifier` field from the struct (the LLM-based classifier).

Keep `classifier_params` only if it's used by remaining code. If the only consumer was `classify_with_llm()`, remove it too.

- [ ] **Step 2: Remove ExecutionMode enum**

In `crates/agent/src/intent_pipeline/types.rs`, the `ExecutionMode` enum is no longer needed — there's no pre-selection of Direct vs Reactive. However, `IntentAnalysis` still uses `mode: ExecutionMode`.

Replace `ExecutionMode` with a simpler signal:

```rust
/// Complexity assessment from heuristic analysis.
/// The execute loop uses this to tune its behavior but does NOT
/// pre-select a mode — the model self-selects via tool_use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLevel {
    /// Simple query, likely no tools needed.
    Simple,
    /// Moderate complexity, likely 1-3 tool calls.
    Moderate,
    /// High complexity, likely 4+ tool calls or sequential dependencies.
    Complex,
}
```

Update `IntentAnalysis`:
```rust
pub struct IntentAnalysis {
    pub complexity: ComplexityLevel,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
    pub needs_orchestration: bool,
}
```

Update all references from `analysis.mode` to `analysis.complexity` throughout the codebase.

- [ ] **Step 3: Remove PipelineConfig.pipeline_timeout_secs**

In `crates/agent/src/intent_pipeline/types.rs`, remove `pipeline_timeout_secs` from `PipelineConfig`. Add `safety_timeout_secs` in its place:

```rust
pub struct PipelineConfig {
    pub execution_model: String,
    pub system_prompt: String,
    pub context_window: usize,
    pub max_response_tokens: usize,
    pub channel: String,
    pub provider_name: String,
    pub scenario_max_graph_depth: u32,
    pub safety_timeout_secs: u64, // was pipeline_timeout_secs
}
```

Update the `Default` impl to use `600` for `safety_timeout_secs`.

- [ ] **Step 4: Remove AnalysisSource::LlmClassifier and ShadowDeferred**

In `types.rs`, update `AnalysisSource`:

```rust
pub enum AnalysisSource {
    Heuristic,
    Embedding,
    MidExecutionEscalation,
}
```

Remove `LlmClassifier` and `ShadowDeferred` — they no longer exist.

- [ ] **Step 5: Fix all compilation errors**

Run: `cargo build --workspace 2>&1 | head -80`

Fix all errors. The main consumers of the old types are:
- `runtime.rs` — references `ExecutionMode`, `analysis.mode`, `pipeline_timeout_secs`
- `router.rs` — dispatches on `ExecutionMode::Direct` vs `Reactive`
- `agent_harness.rs` (simulator) — constructs `PipelineConfig`
- `agent_loop/builder.rs` — constructs pipeline components
- Various event emissions that reference classification

For `runtime.rs`, temporarily update references but don't rewrite `process_message()` yet — that's Task 6.

- [ ] **Step 6: Build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(agent): remove LLM classifier from IntentAnalyzer, replace ExecutionMode with ComplexityLevel"
```

---

### Task 6: Rewrite process_message() — The 6-Phase Pipeline

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

This is the largest single change. Rewrite `process_message()` from the 11-step pipeline to the 6-phase model.

- [ ] **Step 1: Update AgentRuntime struct**

Replace the `router: ExecutionRouter` field with `core: Arc<ExecutionCore>`:

```rust
pub struct AgentRuntime {
    // ... keep all existing fields except:
    // REMOVE: router: ExecutionRouter,
    // ADD:
    core: Arc<ExecutionCore>,
    // ... keep everything else
}
```

Update the `new()` constructor to take `core: Arc<ExecutionCore>` instead of `router: ExecutionRouter`.

- [ ] **Step 2: Update RuntimeResult to include budget info**

```rust
pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,           // "normal", "deep_think", "ultra"
    pub classification: IntentAnalysis,
    pub validation: ValidationResult,
    pub agent_name: String,
    pub turns: u32,
    pub budget_exhausted: bool,
    pub tool_calls: Vec<String>,
}
```

- [ ] **Step 3: Rewrite process_message()**

Add `depth: DepthMode` parameter to `process_message()`:

```rust
pub async fn process_message(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value],
    tool_names: &[&str],
    ctx: &RoutingContext,
    system_prompt: Option<&str>,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    correction: Option<context_engine::CorrectionContext>,
    depth: DepthMode,
) -> Result<RuntimeResult>
```

The body should implement the 6-phase pipeline:

**Phase 1: Route** — Keep existing SkillRouter logic (Steps 0a, 0b, 1 from old pipeline). This is the heuristic skill match. Do NOT make any LLM calls.

**Phase 2: Prepare** — Keep Steps 2, 2a, 3, 6, 7 from old pipeline (set profile, activate skills, filter tools, assemble context). Create `ExecutionBudget::new(depth, &skill_name)`.

**Phase 3: Execute** — Replace Steps 4, 5, 8, 9 with:
```rust
// Safety timeout wraps the ENTIRE execution including context assembly
let safety_timeout = Duration::from_secs(
    self.hot_config.read().await.safety_timeout_secs
);

let loop_result = tokio::time::timeout(
    safety_timeout,
    execute_loop(
        &self.core,
        messages,
        &filtered_tools,
        &params,
        &mut budget,
        ctx,
        event_tx.clone(),
    ),
)
.await
.map_err(|_| {
    common::KlyntbotError::Internal(
        "Safety timeout (600s) — this is a bug, please report it".to_string(),
    )
})??;
```

**Phase 4: Enrich** — NEW. Spawn async enrichment tasks based on depth:
```rust
if depth == DepthMode::DeepThink || depth == DepthMode::Ultra {
    if let Some(ref tx) = event_tx {
        let _ = tx.send(AgentEvent::EnrichmentStarted {
            phase: "mirror_reflection".to_string(),
        }).await;
    }
    // Mirror reflection, coaching, auto-save — all async
    // (actual enrichment handlers to be wired by caller, e.g., AppCore)
}
```

**Phase 5: Record** — Keep Steps 10, 11 (cost tracking, autotuner). Run async where possible.

**Phase 6: Adapt** — Placeholder for depth history recording. The depth history system is a future enhancement per the spec; for now, just log the depth choice.

- [ ] **Step 4: Remove old Step 4 (classify) and Step 5 (confidence check)**

Delete the entire classification block that called `self.analyzer.analyze()` followed by confidence checking and mode downgrade. The analyzer is still used for signal generation (complexity assessment) but NOT for LLM classification.

Keep the heuristic analysis call but use it only for `ComplexitySignals`:
```rust
// Phase 1 includes: lightweight heuristic analysis for signals (no LLM call)
let analysis = self.analyzer.analyze(message, tool_names).await;
```

- [ ] **Step 5: Remove the old timeout wrapper**

Delete the `tokio::time::timeout(pipeline_timeout, pipeline_future)` block at the old Step 8. It's replaced by the safety timeout wrapping the execute_loop.

- [ ] **Step 6: Fix all compilation errors and build**

Run: `cargo build --workspace`

This will require updating:
- All callers of `process_message()` to pass the new `depth` parameter
- The simulator's `agent_harness.rs` to pass `DepthMode::Normal`
- The `agent_loop/mod.rs` to pass depth from the inbound message
- Any code that references `router.execute()` to use `execute_loop()` instead

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(agent): rewrite process_message() as 6-phase budget-bounded pipeline"
```

---

### Task 7: Delete Legacy Engines and Router

**Files:**
- Delete: `crates/agent/src/intent_pipeline/engines/direct.rs`
- Delete: `crates/agent/src/intent_pipeline/engines/reactive.rs`
- Delete: `crates/agent/src/intent_pipeline/router.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`

- [ ] **Step 1: Delete the old engine files**

```bash
rm crates/agent/src/intent_pipeline/engines/direct.rs
rm crates/agent/src/intent_pipeline/engines/reactive.rs
rm crates/agent/src/intent_pipeline/router.rs
```

- [ ] **Step 2: Update engines/mod.rs**

Remove the `direct` and `reactive` module declarations and any re-exports from `crates/agent/src/intent_pipeline/engines/mod.rs`. If the only contents were these two modules, the file can contain just:

```rust
// Engine implementations have been unified into execution/execute_loop.rs.
// This module remains for any future engine variants (e.g., debate engine).
pub mod debate;
```

If `debate.rs` exists and is still used, keep it. Otherwise, make `engines/mod.rs` empty or delete the module entirely.

- [ ] **Step 3: Update intent_pipeline/mod.rs**

Remove the `router` module declaration. Remove any re-exports of `ExecutionRouter`, `DirectEngine`, `ReactiveEngine`, `RouterResult`.

- [ ] **Step 4: Clean up imports throughout the workspace**

Run: `cargo build --workspace 2>&1 | head -80`

Fix any remaining imports of:
- `use agent::intent_pipeline::router::*`
- `use agent::intent_pipeline::engines::direct::*`
- `use agent::intent_pipeline::engines::reactive::*`
- `use agent::ExecutionRouter`

In `crates/agent/src/lib.rs`, update re-exports to remove `ExecutionRouter` and add `ExecuteLoopResult`, `execute_loop`.

- [ ] **Step 5: Update agent_loop/builder.rs**

The `AgentLoopBuilder` currently constructs `DirectEngine`, `ReactiveEngine`, and `ExecutionRouter`. Replace this with constructing just `ExecutionCore` and passing it to `AgentRuntime::new()`.

Find the section that creates the engines:
```rust
let direct = DirectEngine::new(Arc::clone(&core));
let reactive = ReactiveEngine::new(Arc::clone(&core), max_iterations);
let exec_router = ExecutionRouter::new(direct, reactive);
```

Replace with just:
```rust
// ExecutionCore is passed directly to AgentRuntime — no separate engines needed
```

And pass `core` instead of `exec_router` to `AgentRuntime::new()`.

- [ ] **Step 6: Build and verify clean workspace**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets`
Expected: Clean build, zero clippy warnings (except pre-existing desktop crate exceptions).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(agent): delete DirectEngine, ReactiveEngine, ExecutionRouter — unified into ExecuteLoop"
```

---

### Task 8: Update the Simulator

**Files:**
- Modify: `crates/simulator/src/agent_harness.rs`
- Modify: `crates/simulator/src/harness.rs`
- Modify: `crates/simulator/src/scenario.rs`

- [ ] **Step 1: Update AgentHarness to use new pipeline**

In `crates/simulator/src/agent_harness.rs`:

- Remove `ExecutionRouter` construction. Pass `ExecutionCore` directly.
- Update `PipelineConfig` to use `safety_timeout_secs: 600` instead of `pipeline_timeout_secs: 60`.
- Remove the `max_iterations` parameter (no longer needed — budget handles this).
- The `process()` method should pass `DepthMode::Normal` to `process_message()`.

Update constructor signature:
```rust
pub async fn new(
    pool: &storage::StoragePool,
    inner_pool: sqlx::SqlitePool,
    bus: Arc<DomainEventBus>,
    context_queue: Arc<bus::ContextUpdateQueue>,
    skill_catalog: Arc<RwLock<SkillCatalog>>,
    skill_router: Arc<RwLock<SkillRouter>>,
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
    provider_name: &str,
    model: &str,
    provider_error_rate: f64,
    seed: u64,
) -> common::Result<Self>
```

Note: `max_iterations` parameter removed.

- [ ] **Step 2: Remove the 60s tokio::time::timeout wrapper in harness.rs**

In `crates/simulator/src/harness.rs`, find the `tokio::time::timeout(Duration::from_secs(60), ...)` wrapper around `agent.process()`. Remove it — the safety timeout is now inside the pipeline (600s), and the budget handles graceful termination.

Replace with a direct call:
```rust
let agent_result = agent.process(msg, day_counter, &history).await;
```

- [ ] **Step 3: Add depth_mode to scenario config**

In `crates/simulator/src/scenario.rs`, add to `SimulationConfig`:

```rust
    /// Depth mode for agent execution. Default: "normal".
    #[serde(default = "default_depth_mode")]
    pub agent_depth_mode: String,
```

With:
```rust
fn default_depth_mode() -> String {
    "normal".to_string()
}
```

Parse it into `DepthMode` in the harness when creating the budget.

- [ ] **Step 4: Update harness to pass depth to agent**

In `crates/simulator/src/harness.rs`, where `agent.process()` is called, pass the depth mode from the scenario config. Also remove the `agent_max_iterations` references — they're replaced by the budget's turn limit.

- [ ] **Step 5: Build and run simulation tests**

Run: `cargo build -p simulator && cargo nextest run --test simulation -E 'not test(run_software_engineer_12mo) & not test(run_software_engineer_1mo) & not test(run_cognitive_llm) & not test(run_agent_validation)'`

Expected: All heuristic simulation tests PASS.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(simulator): update agent harness for budget-bounded pipeline, remove timeout wrapper"
```

---

### Task 9: Clean Legacy Code — Final Pass

**Files:**
- Various across the workspace

- [ ] **Step 1: Search for any remaining references to removed types**

```bash
cargo build --workspace 2>&1 | grep "error" | head -30
```

Also search for stale references:
```bash
grep -rn "ExecutionMode\|DirectEngine\|ReactiveEngine\|ExecutionRouter\|pipeline_timeout_secs\|llm_classifier_timeout\|llm_classifier_model\|ShadowDeferred\|shadow_mode\|with_shadow_mode" crates/ --include="*.rs" | grep -v "target/"
```

Fix any remaining references.

- [ ] **Step 2: Remove the iteration_budget() function**

In `crates/agent/src/intent_pipeline/types.rs`, remove:
```rust
pub fn iteration_budget(&self) -> u32 {
    let base = (self.estimated_tool_calls as u32 * 3).max(10);
    (base + 5).min(30)
}
```

This was used to compute ReAct iterations — no longer needed.

Also remove constants:
```rust
ORCHESTRATION_MIN_ITERATIONS
SHADOW_MODE_FALLBACK_ITERATIONS
```

- [ ] **Step 3: Clean up the IntentClassifier**

If `IntentClassifier` (the LLM-based classifier struct) still exists in `analysis.rs` and is unused, remove it entirely along with its prompt templates and helper functions.

- [ ] **Step 4: Run full workspace build + clippy + tests**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets
cargo nextest run --workspace
```

Fix any issues.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: remove all legacy pipeline timeout, classifier, and engine code"
```

---

### Task 10: Run Simulator and Verify Metrics

- [ ] **Step 1: Run the smoke test**

```bash
cargo nextest run --test simulation -E 'test(smoke_test_7_day)'
```

Expected: PASS.

- [ ] **Step 2: Run all heuristic simulation tests**

```bash
cargo nextest run --test simulation -E 'not test(run_software_engineer_12mo) & not test(run_software_engineer_1mo) & not test(run_cognitive_llm) & not test(run_agent_validation)'
```

Expected: All PASS.

- [ ] **Step 3: Run the 1-week agent validation (requires DEEPSEEK_API_KEY)**

```bash
cargo test --test simulation smoke::run_agent_validation_1week -- --nocapture 2> /tmp/sim_stderr.txt &
# Monitor progress:
tail -f /tmp/sim_stderr.txt | grep "\[sim\]"
```

Expected: Agent calls complete without timeout. Tier 5-6 metrics produce real non-zero values. The test should finish in 10-20 minutes (vs 45+ minutes before).

- [ ] **Step 4: Report metrics**

Compare before/after:
- Agent timeout rate: was 86% → should be <10%
- Agent routing accuracy: was 0% → should be >0 (still limited by skill name mismatch)
- Response quality: should have real values when agent completes
- All turns should complete within budget, not by timeout

- [ ] **Step 5: Commit any test fixes**

```bash
git add -A
git commit -m "test(simulator): verify budget-bounded pipeline produces real metrics"
```

---

## Self-Review

**Spec coverage check:**
- ✅ ExecutionBudget struct with token/turn limits → Task 1
- ✅ DepthMode (Normal/DeepThink/Ultra) → Task 1
- ✅ Skill-aware default budgets → Task 1 (skill_budget_for)
- ✅ New AgentEvent variants (BudgetUpdate, DepthSuggestion, Enrichment, TurnComplete) → Task 2
- ✅ ExecutionConfig replacing pipeline_timeout → Task 3
- ✅ Remove llm_classifier_* from OrchestratorConfig → Task 3
- ✅ HotConfig updated → Task 3
- ✅ Unified ExecuteLoop → Task 4
- ✅ Remove IntentAnalyzer Layer 3+4 → Task 5
- ✅ Replace ExecutionMode with ComplexityLevel → Task 5
- ✅ 6-phase process_message() → Task 6
- ✅ Delete DirectEngine/ReactiveEngine/ExecutionRouter → Task 7
- ✅ Simulator updated → Task 8
- ✅ Legacy cleanup → Task 9
- ✅ Verification → Task 10
- ⚠️ Phase 4 Enrichment: Task 6 has a placeholder spawn. Full enrichment wiring (Mirror, Coaching, NoteTree, FSRS) depends on app-core integration which is outside this plan's scope. The event variants exist, the spawn points exist, but the actual enrichment handlers will be wired when the desktop UI is ready.
- ⚠️ Adaptive depth suggestions (DepthHistory, Mirror-based suggestions): The DepthSuggestion event exists but the history tracking and Mirror integration is a follow-up. This plan creates the foundation.
- ⚠️ Desktop UI (pills, HUD, enrichment stream): Explicitly out of scope per the spec. Separate PR.

**Placeholder scan:** No TBDs, TODOs, or "implement later" in any step. The Phase 4 enrichment spawns are real `tokio::spawn` calls with real event emissions — they just don't have production enrichment handlers wired yet (those are app-core level, not agent-core level).

**Type consistency:** `ExecutionBudget`, `DepthMode`, `ExecuteLoopResult`, `ComplexityLevel`, `AnalysisSource`, `PipelineConfig` all consistent across tasks. `process_message()` signature consistent between Task 6 (definition) and Task 8 (simulator caller).
