# Model Self-Stop Termination — Design

**Date:** 2026-05-05
**Status:** Draft — pending implementation plan
**Owner:** Jayden
**Related:** `crates/agent/src/execution/execute_loop.rs`, `crates/agent/src/execution/budget.rs`, `crates/agent/src/subagent.rs`, `crates/agent/src/autotuner/`, `crates/desktop-ui/src/features/threads/`

## 1. Problem

Klynt's ReAct loop currently uses a **token + turn budget** (`ExecutionBudget`) as its primary termination model. When the budget runs low it:

1. Injects a `wrap_up` system message coercing the model to finish.
2. On exhaustion, runs a **forced final synthesis pass** with `tools=&[]` and a "Your budget is exhausted. You MUST respond..." coercion prompt.
3. Falls back to a hard-coded `SYNTHESIS_FALLBACK` string if the synthesis pass returns empty.

OpenCode (`../opencode/internal/llm/agent/agent.go:276-310`) takes the opposite approach: a bare `for {}` that exits only when the assistant message returns `FinishReason != ToolUse` (i.e., the model self-stops with `end_turn`), the context is canceled, or an error occurs.

After comparing the two models we want **OpenCode-style model self-stop** as Klynt's primary termination, with the budget repurposed as a silent **safety circuit-breaker** rather than a soft target the model is coerced toward.

### Why change

- **Token waste.** The wrap-up message + forced synthesis pass costs ~15–25% extra tokens per run on long conversations.
- **Truncated outputs.** Forced synthesis with `tools=&[]` produces inferior, summary-shaped responses when the model would otherwise have continued the task naturally.
- **Coercion artifacts.** "Your budget is exhausted" leaks into the conversation transcript, occasionally surfacing in user-visible content via downstream summarization.
- **Misaligned incentives.** A turn cap of 10 trains us to design tools that minimize round-trips at the expense of clarity. Modern frontier models stop themselves correctly on `end_turn` in ≥99% of normal flows.

### Why not pure OpenCode parity (unbounded)

Klynt runs **cron jobs**, **nightly reforge cycles**, and **autotuner benchmarks** where a runaway model is a real cost concern. We keep a high silent ceiling as a backstop.

## 2. Goals & non-goals

### Goals

- Primary termination = model self-stop on `end_turn` / no-tool-call response.
- Remove wrap-up message injection.
- Remove forced final synthesis pass + coercion prompt + `SYNTHESIS_FALLBACK`.
- Preserve the public contract surface: `SubagentProfile` tiers, autotuner single-shot, `AgentEvent::ExecutionStarted { max_iterations }`.
- Add wall-time protection: per-tool timeout policy that handles long Rust builds without firing prematurely.
- Keep `LoopDetector` (repeat tool-signature HardStop) — orthogonal concern, no equivalent in OpenCode but valuable.

### Non-goals

- Removing `ExecutionBudget` entirely. Subagents and autotuner still need turn caps as capability/measurement constraints.
- Changing the cancellation model. `CancellationToken` already mirrors OpenCode's `ctx.Done()`.
- Restructuring `MidLoopCompressor` (token-window compression is independent of termination).
- Removing the desktop UI's iteration-progress display. We update its semantics, not its existence.

## 3. Design

### 3.1 Termination matrix

The execute loop terminates on exactly one of the following, in priority order:

| # | Trigger | `finish_reason` | Behavior |
|---|---------|-----------------|----------|
| 1 | `cancel_token.is_cancelled()` | `cancelled` | Return partial output with whatever turns completed. |
| 2 | LLM call returns no tool calls (natural stop) | `completed` | Return the model's final text. **Primary path.** |
| 3 | `LoopDetector::HardStop` (repeat tool signature) | `loop_detected` | Abort with diagnostic; do not synthesize. |
| 4 | Per-tool timeout fires | (tool error in history; loop continues) | Inject `Tool::Error("timeout after Ns")` so the model can react. |
| 5 | Turn count ≥ safety cap | `safety_turn_limit_reached` | Abort with error event; **no** synthesis pass. |
| 6 | Token usage ≥ token cap | `token_limit_reached` | Abort with error event; **no** synthesis pass. |
| 7 | Provider error | `error` | Surface upstream. |

**Removed** vs today: the "budget approaching → wrap-up" path, the "budget exhausted → forced synthesis" path, and the `SYNTHESIS_FALLBACK` constant.

### 3.2 Safety caps

| Caller | Turn cap | Token cap | Source |
|--------|----------|-----------|--------|
| User-facing agent (default) | **100** | model context window × 0.95 | new constant `DEFAULT_SAFETY_TURN_CAP` |
| `SubagentProfile::ReadOnly` | 5 | unchanged | `subagent.rs` |
| `SubagentProfile::ReadWrite` | 10 | unchanged | `subagent.rs` |
| `SubagentProfile::Full` | 15 | unchanged | `subagent.rs` |
| Autotuner single-shot | 1 | unchanged | `autotuner/metric_collector.rs` |

Hitting a subagent cap is a normal, expected signal — the parent agent can re-dispatch with a higher tier. Hitting the user-facing 100-turn cap is treated as a bug indicator (logged at `error!`).

### 3.3 Tool-level timeout policy

Today: `params.tool_timeout = 30s` default; only `ask_user` gets `INTERACTIVE_TOOL_TIMEOUT = 600s`. This breaks `cargo build` / `cargo test`.

Proposed: introduce three timeout tiers, selectable per tool via a new optional `Tool::timeout_class()` method (default = `Standard`):

| Class | Timeout | Examples |
|-------|---------|----------|
| `Quick` | 30s | most read-only tools, memory queries, note search |
| `Standard` | 120s | DB-touching tools, file edits, web fetch |
| `LongRunning` | **600s** | shell commands, `cargo build`, `cargo test`, `cargo clippy` |
| `Interactive` | 600s | `ask_user` (existing) |

When a tool times out, the result becomes a `Tool::Error("timeout after Ns")` injected into history. The loop continues — the model can decide to retry, decompose, or give up. **No global execution timeout** in this iteration; cron jobs already carry their own outer timeout, and we want to evaluate the per-tool policy in isolation before stacking another guard.

### 3.4 Public contract preservation

- `AgentEvent::ExecutionStarted { max_iterations }` continues to fire. Value = the safety cap (100 / 5 / 10 / 15 / 1). Semantic shifts from "soft target" → "hard safety cap"; documented at the field site.
- `LoopOutcome.budget_exhausted` is **renamed** to `safety_cap_hit` and only set true on triggers #5 / #6 above. Existing `finish_reason` enum gains `safety_turn_limit_reached`, `token_limit_reached`, `loop_detected`; loses `budget_exhausted`.
- `SubagentProfile` tier caps stay numerically identical — they continue to gate capability, just via the new safety-cap mechanism.
- Autotuner with `max_iterations: 1` continues to run a single LLM call. If the model returns tool calls, the loop aborts with `safety_turn_limit_reached`, which the autotuner records as a metric (this matches today's behavior — autotuner already treats `budget_exhausted` as a measurable outcome).

### 3.5 Desktop UI impact

- Progress component updates label from `"Turn 7 / 10"` to `"Turn 7"` (no denominator) for user-facing chats; subagent progress keeps the denominator (it's meaningful there).
- New error toast types: `safety_cap_exceeded`, `loop_detected`, `tool_timeout`. Each maps to a short user-facing message and a "View details" action that surfaces the full `LoopOutcome`.
- Telemetry: emit `ExecutionTerminated { reason, turns, tokens }` so we can observe how often we hit each terminator post-launch.

## 4. Implementation outline

Order matters — each step must compile and test green before the next.

1. **`execute_loop.rs` — strip coercion paths.** Remove the wrap-up injection block, the forced synthesis branch, and the `SYNTHESIS_FALLBACK` constant. Replace budget-exhausted return with `safety_turn_limit_reached` / `token_limit_reached` errors carrying the partial state. Loop now terminates on the matrix in §3.1. Tests: update `refactor_tests.rs` to assert no synthesis pass on cap hit.
2. **`budget.rs` — rename + simplify.** `ExecutionBudget` → `SafetyCap`. Drop `should_wrap_up()`, `remaining_pct()`. Keep `tick_turn()`, `deduct()`, `exhausted()`, `turns_used()`. Update all callers.
3. **`types.rs` — `LoopOutcome` enum migration.** Replace `budget_exhausted: bool` + string `finish_reason` with a typed `enum FinishReason { Completed, Cancelled, SafetyTurnLimit, TokenLimit, LoopDetected, Error }`. Update event serialization.
4. **`Tool::timeout_class()` — new trait method.** Default impl returns `Standard`. Wire in `core.rs:680` to pick the class. Mark `cargo`-running tools as `LongRunning` in `feature-coding`. Tests: `test_tool_timeout` becomes parameterized over class.
5. **Subagent + autotuner audit.** Verify `subagent.rs:674` and `autotuner/metric_collector.rs:258,285` still compile with renamed types. Confirm autotuner's metric semantics unchanged.
6. **Desktop UI updates.** Frontend rebind for `ExecutionStarted` semantics; new error toast variants; telemetry event handler.
7. **Bindings regeneration.** `cargo tauri dev` once to refresh `desktop-ui/src/bindings.ts`.

Estimated diff: ~250 LOC removed, ~120 LOC added, net **−130 LOC**.

## 5. Test plan

- **Unit (`refactor_tests.rs`):** model returns `end_turn` on turn 3 → `Completed`, no synthesis call recorded. Model emits tool calls every turn → hits cap 100, `SafetyTurnLimit`, no synthesis call. Cancellation mid-turn → `Cancelled`. Repeat tool signature 3× → `LoopDetected`.
- **Unit (`core.rs`):** `LongRunning` tool sleeping 200s under default 120s `Standard` class times out; under `LongRunning` 600s, completes.
- **Integration (`tests/e2e/`):** existing agent-loop e2e suite runs unchanged — these never hit the budget and should pass without modification. Add one test that asserts no `SYNTHESIS_FALLBACK` string appears in any e2e response.
- **Subagent (`subagent.rs` tests):** ReadOnly subagent forced to keep calling tools → terminates at 5 turns with `SafetyTurnLimit`, parent agent sees the typed reason.
- **Autotuner (`autotuner/metric_collector.rs` tests):** `max_iterations: 1` benchmark with a tool-calling model still records the iteration cap as the measurement.

## 6. Open questions

None blocking. Two follow-ups for after landing:

1. Should we expose `safety_turn_cap` as a per-channel config knob (e.g., cron jobs at 50, interactive at 100)? Defer until we observe real cap-hit rates.
2. Do we want a global wall-time timeout (e.g., 30 min for any single run)? Defer pending tool-class telemetry.

## 7. Migration

- Pre-release codebase — no on-disk schema changes, no migration scripts.
- Telemetry consumers reading the old `finish_reason: "budget_exhausted"` string need updates: search for usages in `crates/analytics/`, `crates/cognitive/src/mirror/`, and the desktop event handlers. Captured in implementation step 6.

## 8. Rollback

Single-commit revert. The change is localized to `execute_loop.rs`, `budget.rs`, `types.rs`, three subagent/autotuner call sites, and the desktop UI bindings. No data writes, no external surface beyond the typed `FinishReason` enum.
