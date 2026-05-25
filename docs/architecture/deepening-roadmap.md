# Architecture-deepening roadmap

Durable replacement for the `/improve-codebase-architecture` HTML report (which is
written to `$TMPDIR` and reaped). This is the source of truth for the deepening
loop: each row is a candidate that turns a shallow module into a deep one, worked
in order. Vocabulary follows `LANGUAGE.md` (module / interface / depth / seam /
adapter / locality) and `CONTEXT.md` for the domain.

**Candidates #3–#5 below are reconstructed from memory** (the report file was gone
by the time this doc was created). Re-validate each candidate's scope at its
EXPLORE step before acting — do not trust the reconstructed problem statement over
what the code at HEAD actually shows. If a #6 existed, it was not recovered.

| # | Candidate | Strength | Status |
|---|-----------|----------|--------|
| 1 | Narrow `RoutingContext` into per-tool context views | Strong | ✅ Done |
| 2 | Replace dead `ExecutionEngine` with a real, mockable seam | Strong | 🔧 In progress (Option B chosen) |
| 3 | Generate the dev-dispatch arm from `#[klynt_command]` | (reconstructed) | ⬜ Pending |
| 4 | Decompose the 34-wide `Repos` aggregate | (reconstructed) | ⬜ Pending |
| 5 | Push `AppCorePlugin` impls into feature crates | (reconstructed) | ⬜ Pending |

---

## #1 — Narrow `RoutingContext` into per-tool context views ✅

**Done.** Tools no longer receive the full ~23-field `RoutingContext`; they declare
the slice they need via `type Ctx<'a>: FromRoutingContext<'a>` and the `#[derive(Tool)]`
bridge projects it. Ladder: `() ⊂ HookCtx ⊂ IoCtx ⊂ FullCtx`.

- ADR: `docs/adr/0002-tool-context-projection.md`
- Deep-dive: `docs/architecture/deep-dive-02-tool-context-projection.md`
- Commits: `633bc4d46` (Phase A machinery), `d9f9e890f` (Phase B/C narrowing),
  `5eafe654c` (Phase E — `#[tool_actions]` ctx views)
- Recorded floor: the untyped `Tool::execute(args, &RoutingContext)` boundary and
  MCP tools are a **deliberate** boundary, not a gap (see ADR-0002).

---

## #2 — The dead `ExecutionEngine` seam ⛔

**Original candidate (from the report):** "Revive the `ExecutionEngine` seam."
A single user message crosses ~5 concrete structs (`AgentLoop` → `AgentRuntime` →
`execute_loop` → `ExecutionCore` → provider/tools) with no unifying interface;
`AgentRuntime` is shallow (~31 fields, mostly fire-and-forget telemetry spawns);
the `ExecutionEngine` trait exists with zero implementors — an abandoned seam.
Proposed fix: implement the trait for the real path + a mock; route runtime and
subagent through it.

**EXPLORE finding (2026-05-25, against HEAD `5eafe654c`) — the premise changed:**

- `crates/agent/src/engines/mod.rs` is **entirely dead code.** `ExecutionEngine`
  *and* `EngineResult` have zero references in the workspace. Deleting the module
  concentrates no complexity (pure pass-through to nowhere → deletion test says
  "delete," not "deepen").
- The trait is **stale**, not merely dormant: its return type
  `EngineResult { Complete | Escalate }` encodes a Direct→Reactive *escalation*
  model the module's own doc comment says was already unified away into
  `execute_loop`. Its signature doesn't match the real path (no
  `cap: &mut SafetyCap`; wrong return type). "Reviving" it would resurrect an
  abstraction the codebase deliberately abandoned.
- The **real shared contract already exists** as a free function:
  `execute_loop(core, messages, tools, params, cap, ctx, event_tx) -> ExecuteLoopResult`
  (`crates/agent/src/execution/execute_loop.rs`). The exact two callers the
  candidate named already route through it: `subagent.rs:728`, `subagent.rs:863`,
  `agent_runtime/runtime.rs:570`. Two adapters → this is a *real* seam already.

**Decision needed (loop paused here):** the candidate splits into independent moves —
  - (A) Delete the dead `engines/mod.rs` module (deepening-by-deletion; safe, off
    the hot path).
  - (B) Also trait-ify the real seam (`execute_loop` / `ExecutionCore`) so runtime
    and subagent share a *mockable* interface — touches the perf-gated hot path.
  - (C) Treat #2 as obsolete (the unifying interface already exists) and move on.

See conversation for the surfaced options. Resolve before implementing.

---

## #3 — Generate the dev-dispatch arm from `#[klynt_command]` (reconstructed) ⬜

Reconstructed premise: the dev HTTP server (`crates/desktop/src/dev_server/`)
hand-maintains a dispatch table that mirrors the Tauri command registration in
`specta_builder.rs` / `klynt_collect_commands!`. The two drift; a new
`#[klynt_command]` must be added in two places. Proposed: generate the dev-dispatch
arm from the same macro source of truth. **Re-validate at EXPLORE.**

## #4 — Decompose the 34-wide `Repos` aggregate (reconstructed) ⬜

Reconstructed premise: `Repos::from_pool(&pool)` exposes ~34 repository accessors
on one struct — a wide aggregate every caller depends on wholesale, defeating
locality and leak-prevention. Proposed: group into cohesive sub-aggregates behind
narrower seams. **Re-validate at EXPLORE.**

## #5 — Push `AppCorePlugin` impls into feature crates (reconstructed) ⬜

Reconstructed premise: `AppCorePlugin` implementations live centrally rather than
beside the feature crates they belong to, so feature wiring isn't local to the
feature. Proposed: relocate impls into their feature crates. **Re-validate at
EXPLORE.**
