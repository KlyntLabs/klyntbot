# Subagent Persistence and Resume

**Date:** 2026-05-12
**Status:** Design — pending review
**Related:**
- Current subagent impl: `crates/agent/src/subagent.rs`
- Kimi-CLI reference: `/Users/jayden/Projects/Klynt/kimi-cli/src/kimi_cli/subagents/`
- Opencode reference: `/Users/jayden/Projects/Klynt/opencode/internal/db/migrations/20250424200609_initial.sql`

## Motivation

Today's subagent runtime has three problems that compound:

1. **Hard 120k token cap** in `subagent.rs:728` and `review_handler.rs:178`. Real coding-style subagent work exceeds this routinely, especially when nested tool use accumulates context.
2. **Silent-drop bug** at `subagent.rs:789`: `run_subagent` returns `Ok(("ok", result.content))` regardless of `result.safety_cap_hit`. The parent agent has no signal that the subagent was truncated — it just sees a happy result with whatever partial text accumulated.
3. **No persistence or resume.** Subagents are one-shot. When a budget is exhausted, the conversation history is lost; the parent cannot pick up where the subagent left off.

Kimi-CLI's solution is instructive:
- No subagent token cap; rely on existing in-flight context compaction (`MidLoopCompressor` in klyntbot, `compact_context` in Kimi).
- One global turn ceiling (Kimi: 500 / turn).
- `MaxStepsReached → ToolError` cleanly surfaces cap-hit to the parent with a "split the task" hint.
- A `SubagentStore` persists each subagent as a durable instance addressable by `agent_id`; resume picks up the same conversation history.

Opencode's solution is also instructive for the UX side: subagents are just sessions with a `parent_session_id`, which means the existing thread UI handles drill-in / navigate-back for free.

This design combines both: Kimi's runtime semantics (resume, clean failure) with opencode's storage model (subagent sessions live in the existing `sessions` table) and a thin metadata layer on top.

## Goals

- Drop the 120k token cap on subagents; replace cumulative-token enforcement with in-flight context compaction.
- Raise the turn cap to a generous default (500), matching Kimi.
- Persist each subagent as a queryable instance with a stable `agent_id`.
- Add `spawn` / `resume` / `list` / `kill` tool actions on the existing `agent` tool.
- Fix the silent-drop bug: surface cap-hit as a structured `is_error: true` payload with `agent_id`, partial summary, and a remediation hint.
- Let the user drill into any subagent thread in the existing chat UI (opencode-style) and navigate back to the parent.
- Recover gracefully from crashes: rows still marked `running` after the heartbeat window flip to `failed`.

## Non-goals

- Remote subagents or distributed execution. All subagents run in-process.
- A separate "subagent tray" / dedicated panel (out of scope for this spec — subagents reuse the chat UI surface).
- Changing the assistant / coding mode subagent profile system in ways unrelated to this work.
- Reforge / mirror / cron integration with subagents (those continue to bypass `agent_runtime`).
- Mid-flight intervention via "steer" — subagents are spawned with a prompt, run to completion or cap, then resumed with a new prompt. No mid-turn steering.

## Profile system: removed

The current `SubagentProfile` enum (`ReadOnly` / `ReadWrite` / `Full`) and its three per-profile tool registries are removed. Rationale:

- The profile-based tool gating duplicates work the `ApprovalGate` already does (per `CLAUDE.md`: every tool call is classified Safe / Sensitive / Destructive / Admin and checked before execution). A subagent calling `bash` still goes through the same approval channel as the parent — the profile system was layered safety, not isolated capability.
- Kimi gets the same outcome with one default ("coder") and zero profile machinery.
- The parent agent decides scope through the subagent's `description` and `prompt`; tool selection is a prompting concern, not a runtime tier.

All subagents inherit the **full tool registry of the parent**, minus the `agent` tool itself (preserves the existing "no nested subagents" rule).

## Architecture

```
┌─ Parent agent (assistant or coding session) ──────────────────────┐
│                                                                   │
│  subagents.spawn(description, prompt, model?, max_turns?)         │
│       └─► creates session(mode='subagent', parent_session_id)     │
│           creates subagent_instances row (status=running)         │
│           runs execute_loop                                       │
│           ◄── ToolOk{agent_id, session_id, status, summary}       │
│           ◄── ToolError{agent_id, session_id,                     │
│                         status:'stopped_turn', partial_summary,   │
│                         hint}                                     │
│                                                                   │
│  subagents.resume(agent_id, prompt)                               │
│       └─► loads session messages, runs execute_loop again         │
│           same return shape as spawn                              │
│                                                                   │
│  subagents.list(parent_agent_id?, status?)                        │
│       └─► returns instance rows                                   │
│                                                                   │
│  subagents.kill(agent_id)                                         │
│       └─► fires cancel_token, marks instance killed               │
└───────────────────────────────────────────────────────────────────┘
                       │
                       ▼
   sessions table:        mode='subagent', parent_session_id, ...
   subagent_instances:    agent_id, session_id, parent_agent_id,
                          description, status, model, workspace_path,
                          turn_cap, turns_used, turns_used_total,
                          partial_summary, last_cap_hit_at,
                          created_at, updated_at
```

Subagent sessions emit `agent:thread_event` events with the same envelope as coding threads, so the existing `ThreadEventBuffer.applyEvent` in the desktop UI picks them up without any new event types. The user navigates into a subagent thread the same way they navigate into a coding thread.

## Data model

### `sessions` — additive change

The project is pre-release (per `CLAUDE.md` "Pre-release — no user data to migrate"), so this change consolidates into `001_initial.sql` rather than a new incremental migration:

```sql
ALTER TABLE sessions ADD COLUMN parent_session_id TEXT REFERENCES sessions(id);
-- mode CHECK constraint widens:
--   mode IN ('assistant', 'coding', 'subagent')
CREATE INDEX idx_sessions_parent ON sessions(parent_session_id);
```

The `mode='subagent'` value is added to the enum on the Rust side (`SessionMode::Subagent`).

### `subagent_instances` — new table

```sql
CREATE TABLE subagent_instances (
    agent_id          TEXT PRIMARY KEY,
    session_id        TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    parent_agent_id   TEXT REFERENCES subagent_instances(agent_id),  -- null = top-level chat is parent
    description       TEXT NOT NULL,
    status            TEXT NOT NULL,                 -- see lifecycle below
    model             TEXT,                          -- null = inherit parent's effective model
    workspace_path    TEXT NOT NULL,
    turn_cap          INTEGER NOT NULL,              -- per-call ceiling; default 500
    turns_used        INTEGER NOT NULL DEFAULT 0,    -- of the current call; reset on each resume
    turns_used_total  INTEGER NOT NULL DEFAULT 0,    -- lifetime across calls (observability only)
    partial_summary   TEXT,                          -- last assistant text on cap-hit
    last_cap_hit_at   INTEGER,                       -- ms timestamp
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL                -- refreshed on every iteration boundary
);
CREATE INDEX idx_subagent_instances_session ON subagent_instances(session_id);
CREATE INDEX idx_subagent_instances_parent  ON subagent_instances(parent_agent_id);
CREATE INDEX idx_subagent_instances_status  ON subagent_instances(status);
```

`turns_used` resets on each spawn/resume so the existing `SafetyCap::turn_cap_hit()` math is unchanged. `turns_used_total` is observability only and exists so the UI can show "47 turns across 3 resumes."

### Lifecycle states

```
running        → currently executing (loop is live)
idle           → completed a call cleanly, may be resumed
stopped_turn   → hit the turn cap; partial_summary populated
failed         → LLM/provider error or internal failure
killed         → user-cancelled or LLM called agent.kill
completed      → terminal: parent or user marked it done; not resumable
```

Allowed transitions (anything else returns `ToolError`):

```
            ┌─► idle ──────► running (via resume) ──┐
            │                                       │
running ────┼─► stopped_turn ──► running (resume) ──┤
            │                                       │
            ├─► failed         (terminal)           │
            │                                       │
            ├─► killed         (terminal)           │
            │                                       │
            └─► completed      (terminal)           │
                                                    │
                                                    ▼
                                          (same set of exits)
```

`completed` is terminal — set when the parent explicitly marks an instance done (or when the LLM calls a future `agent.complete` action, out of scope here). `idle` and `stopped_turn` are resumable; `failed` / `killed` / `completed` are not.

## Tool surface

The existing single-action `SpawnTool` (`crates/tools/src/domain/spawn.rs`, registered as `"spawn"`) is renamed to **`SubagentsTool`** with the canonical multi-action name **`subagents`** — matching the project's "plural for multi-action tools" convention (per `CLAUDE.md`: tasks not task, notes not note). The old `"spawn"` registry name is removed cleanly (pre-release, no compat shims per `CLAUDE.md`). The new tool uses the existing `#[tool_actions]` macro pattern (same as `TasksTool` in `feature-tasks`):

| Action | Params | Success | Failure / cap-hit |
|---|---|---|---|
| `spawn` | `description`, `prompt`, `model?`, `max_turns?` | `ToolOk { agent_id, session_id, status, summary }` | `ToolError { agent_id, session_id, status, turns_used, partial_summary, hint }` |
| `resume` | `agent_id`, `prompt` | same | same |
| `list` | `parent_agent_id?`, `status?` | `ToolOk { instances: [...] }` | n/a |
| `kill` | `agent_id` | `ToolOk { agent_id, status: "killed" }` | `ToolError` if `agent_id` unknown |

### Cap-hit payload (the `is_error: true` shape)

```json
{
  "is_error": true,
  "content": {
    "agent_id": "ag3f7a92c1",
    "session_id": "sess-...",
    "status": "stopped_turn",
    "turns_used": 500,
    "partial_summary": "I found 12 references to `deprecatedFn` in src/. Last working on …",
    "hint": "The subagent stopped at its turn cap. You can call subagents.resume(agent_id=ag3f7a92c1, prompt=...) to continue, or split the remaining work into smaller subtasks."
  }
}
```

Anthropic's tool-use guidance recommends putting remediation text in `is_error: true` payloads — LLMs follow it reliably and the parent agent can re-plan accordingly.

### `partial_summary` derivation

No extra LLM call (would compound the cost of a failure path). Algorithm:

```
1. Scan messages in reverse for the last assistant text content.
2. If found and non-empty, store as partial_summary.
3. If the loop never produced assistant text (cap hit mid-tool-call), store
   "Stopped at turn N during {last_tool_name}. No assistant summary produced."
```

Mirrors Kimi's behavior of using the last assistant message.

### Concurrency rule

`resume` against an instance with `status = 'running'` returns `ToolError("agent {id} is currently running; cannot resume concurrently")`. Same as Kimi.

### Default turn cap

500 per spawn / resume call. Caller can override via `max_turns` on `spawn` (e.g. ultrareview-style narrow checks could pin it to 50). `resume` always uses the cap recorded at spawn time. No token cap.

## Data flow

### Spawn

```
parent agent ──► agent.spawn(description, prompt)
                       │
                       ▼
              subagent::spawn_instance
                       │
                       ├─► sessions: INSERT (mode='subagent', parent_session_id, workspace_id)
                       ├─► subagent_instances: INSERT (status='running', turn_cap=500)
                       ├─► register cancel_token in ActiveSubagentRegistry
                       ├─► emit agent:thread_event { kind:'turn_started', thread_id=session_id }
                       │
                       ▼
              execute_loop
                  SafetyCap::with_limits(Normal, 0 /* no token cap */, 500)
                  tool registry = parent's full kit minus `agent`
                  cancel_token from ActiveSubagentRegistry
                       │
                       │  (on every iteration boundary:
                       │     subagent_instances.updated_at = now()
                       │     turns_used += 1
                       │     turns_used_total += 1)
                       │
                       ▼
              loop exits ──┬─► clean: status='idle',  emit turn_completed{Completed}
                          ├─► cap:  status='stopped_turn', persist partial_summary,
                          │         emit turn_completed{SafetyTurnLimit}
                          ├─► cancelled: status='killed', emit turn_completed{Cancelled}
                          └─► error: status='failed',  emit turn_completed{Error}
                       │
                       ▼
              unregister cancel_token, persist final messages, return result
```

### Resume

```
parent agent ──► agent.resume(agent_id, prompt)
                       │
                       ▼
              subagent::resume_instance(agent_id)
                       │
                       ├─► load subagent_instances row
                       │     status == 'running'              → ToolError(already running)
                       │     status in {killed, completed, failed} → ToolError(not resumable)
                       │     status in {idle, stopped_turn}   → proceed
                       │
                       ├─► load messages from existing messages table by session_id
                       ├─► append the new user prompt as the next user message
                       ├─► subagent_instances: UPDATE (status='running', turns_used=0)
                       ├─► register cancel_token; emit turn_started
                       │
                       ▼
              execute_loop with fresh SafetyCap (same 500 cap)
                       │
                       └─► same exit branches as spawn
```

The key invariant: **resume reuses the session row and its messages table** — no separate snapshot/restore. This is what makes the storage model pay off: the existing message-load path handles everything.

### Kill (both LLM and chat-cancel)

```
       LLM:  agent.kill(agent_id)  ──┐
                                     │
   user cancels via chat UI          │
       (chat-cancel button):  ───────┤
                                     ▼
                       subagent::kill_instance(agent_id)
                                     │
                                     ├─► look up cancel_token in ActiveSubagentRegistry
                                     │     not present → no-op cancel; still update DB
                                     │     present     → token.cancel()
                                     │
                                     ├─► subagent_instances: UPDATE status='killed'
                                     ├─► emit agent:thread_event { kind:'turn_completed',
                                     │       finish_reason:'cancelled' }
                                     └─► return { agent_id, status: 'killed' }
```

Cancellation observes at iteration boundary (per `CLAUDE.md`: `execute_loop.rs:113`). A `kill` call may take up to one in-flight tool's timeout to actually stop — same behavior as coding-thread cancellation today.

The chat-cancel button path calls the same `subagent::kill_instance` handler. When the user opens a subagent thread and clicks the cancel/interrupt button, the existing `chat_cancel` Tauri command routes by session mode — for `mode='subagent'` sessions it forwards to `kill_instance(agent_id)` rather than the coding-thread cancel path.

## Frontend navigation

The opencode-style drill-down is built on the existing thread UI — no parallel infrastructure:

1. **Tool result chip:** when the parent agent's stream renders a subagent tool call, the result block includes a clickable chip:

   ```
   subagents.spawn(...)
   ────────────────────────────────────
   ✓ spawned subagent

   ↳ ag3f7a92c1 — Search deprecated refs
   ```

   The chip is part of the tool-call result component (alongside the existing tool body), not a separate inline element. It's clickable; clicking deep-links into the subagent thread.

2. **Routing on click:** `activeThreadId` is set to the subagent's `session_id`. The existing thread routing (chat view, right-panel todos/jobs, virtualized messages) re-renders against that thread.

3. **Breadcrumb in the thread header:** when `parent_session_id IS NOT NULL`, the header shows `<parent name> › <subagent description>`. Clicking the parent name navigates back to the parent thread.

4. **Sidebar grouping:** subagent sessions appear as children of their parent under the same project, indented one level. Same expand/collapse UX as the existing project → threads grouping.

5. **A new Tauri command** `subagent_list_for_session(session_id)` powers the breadcrumb and grouping. No new event channel; it's a simple query against `subagent_instances` joined with `sessions`.

## Heartbeat and crash recovery

Each iteration of `execute_loop` refreshes `subagent_instances.updated_at = now()` immediately after `cap.tick_turn()`. This serves as the heartbeat.

**Crash sweep at app startup:**

```sql
UPDATE subagent_instances
SET status = 'failed',
    partial_summary = COALESCE(partial_summary, 'Process crashed before completion'),
    updated_at = strftime('%s','now') * 1000
WHERE status = 'running'
  AND updated_at < (strftime('%s','now') * 1000) - 300000;  -- 5 minutes
```

5 minutes is the threshold. The iteration-boundary refresh keeps healthy long-running subagents alive even when an individual tool call takes minutes (e.g. a slow bash command), as long as iterations keep advancing.

Concurrent process safety: the sweep runs once during `app-core` init before subagent runtime is started, so we won't race with active runs.

## Error handling matrix

| Trigger | New status | Emits | Visible to parent? |
|---|---|---|---|
| Loop returns clean | `idle` | `turn_completed{Completed}` | `ToolOk` |
| Safety turn cap hit | `stopped_turn` | `turn_completed{SafetyTurnLimit}` | `ToolError` (cap-hit payload) |
| Cancel token fires (via `kill`) | `killed` | `turn_completed{Cancelled}` | `ToolError` (when LLM called) or `ToolOk{killed}` (when user cancelled) |
| LLM / provider error | `failed` | `turn_completed{Error}` + error item | `ToolError` with error text |
| Process crash mid-run | `running` → `failed` (sweep) | n/a at crash time; `agent.list` reflects new status after sweep | next `agent.list` call sees `failed` |

## Testing

### Unit tests

- `SubagentStore`: round-trip create → load → update each lifecycle transition. Assert illegal transitions return errors.
- `partial_summary` derivation: covers (a) last assistant text present, (b) cap hit mid-tool-call with no assistant text, (c) empty conversation.
- `SafetyCap::with_limits(_, 0, 500)` no-ops on token check; turn check fires at 500.

### Integration tests (workspace facade)

- `agent.spawn` returns `agent_id`, instance row exists with `status='idle'` after clean run.
- `agent.spawn` → forced cap-hit at small `max_turns=3` → `ToolError` with `partial_summary` populated.
- `agent.resume` against a `stopped_turn` instance — verify `turns_used` resets, `turns_used_total` accumulates, conversation continues from the persisted messages.
- `agent.resume` against a `running` instance returns `ToolError("currently running")`.
- `agent.resume` against a `failed`/`killed`/`completed` instance returns `ToolError("not resumable")`.
- `agent.kill` during an active loop transitions to `killed` within one iteration boundary; cancel_token observed.
- `agent.list` filters by `parent_agent_id` and `status`.

### Frontend tests (vitest)

- Click on tool-result chip routes to subagent `session_id` and updates `activeThreadId`.
- Breadcrumb renders when `parent_session_id` is set and is hidden otherwise.
- Sidebar groups subagent sessions under their parent within the same project.

### Crash recovery test

- Insert a `subagent_instances` row with `status='running'` and `updated_at` 10 min in the past.
- Run startup sweep.
- Assert status flips to `failed`, `partial_summary` is set if it was previously null.

### E2E soak

- Spawn a subagent with `max_turns=5` doing repetitive list-then-read work. Trigger cap-hit; resume 3 times; verify final summary is coherent and no orphan rows.

## Migration / rollout

- Pre-release per `CLAUDE.md` — schema changes consolidate into `001_initial.sql`. No incremental migration file.
- Tool rename (`SpawnTool` / `"spawn"` → `SubagentsTool` / `"subagents"`) is a breaking change to the LLM-facing tool schema:
  - Existing internal callers of the old `run_subagent(..., profile, ...)` (`crates/agent/src/subagent.rs:630`, callers at `:456` and `:971`) drop the `profile` argument.
  - `crates/app-core/src/coding/review_handler.rs:178` keeps its explicit `SafetyCap::with_limits` call but drops the `120_000` token cap (passes `0`).
  - The MCP-exposed tool's parameter schema regenerates from the `#[derive(Tool)]` macro; Claude Code / external MCP clients pick up the new shape on next restart.
  - `crates/config/src/schema/mcp.rs:EXPLICIT_TOOL_ALLOWLIST` does *not* need to change — the existing `"agent"` entry is the natural-language dispatcher (separate tool) and is unrelated; we are renaming `"spawn"` → `"subagents"` but `"spawn"` was not in the allowlist.
  - Persona/skill references to the `spawn` tool name in `skills/**/*.md` (if any) need to be updated to `subagents`.
- Frontend `bindings.ts` regenerates on `cargo tauri dev` thanks to the existing specta pipeline.
- `chat_cancel` (`crates/app-core/src/handlers/chat/streaming.rs:334` and `:1477`) gains a mode-discriminating branch: for `mode='subagent'` sessions, route to `kill_instance(agent_id)`; otherwise the existing path.

## Implementation notes

### File-level scope (anticipated edits)

- `crates/storage/migrations/001_initial.sql` — add columns and table.
- `crates/storage/src/repos/subagent_instances.rs` — new repo (CRUD + sweep).
- `crates/storage/src/repos/sessions.rs` — extend `SessionMode` enum and queries.
- `crates/agent/src/subagent.rs` — delete `SubagentProfile` (lines ~25-95), refactor `run_subagent_task` (line 630) into `spawn_instance` / `resume_instance` / `kill_instance`, fix silent-drop bug at line 789, add heartbeat refresh inside `execute_loop`, wire `ActiveSubagentRegistry` (`DashMap<agent_id, CancellationToken>`).
- `crates/tools/src/domain/spawn.rs` — rename `SpawnTool` → `SubagentsTool`, registry name `"spawn"` → `"subagents"`, switch from single `fn name`/`fn execute` to `#[tool_actions]` with four actions.
- `crates/app-core/src/init/` — call the crash-sweep at startup (probably `init/mod.rs` or `init/storage.rs`).
- `crates/app-core/src/handlers/chat/streaming.rs` — `chat_cancel` (lines 334 and 1477) branches on `SessionMode`: `Subagent` routes to `kill_instance`.
- `crates/desktop/src/commands/subagent.rs` — new Tauri command `subagent_list_for_session` (using `#[klynt_command]` per `CLAUDE.md`, registered in `specta_builder.rs`).
- `desktop-ui/src/api/endpoints/subagent.ts` — invoke wrapper for the new command.
- `desktop-ui/src/features/threads/components/*` — breadcrumb in thread header, sidebar grouping under parent, tool-result chip component.
- `desktop-ui/src/features/coding/state/codingEventReducer.ts` — no changes (subagents reuse the existing event envelope).
- `desktop-shared/src/coding/events.rs` — no changes if the existing `thread_id` field works for subagent sessions.

### Approval gate behavior

Subagent tool calls still pass through `ApprovalGate::check` exactly as the parent's would (per `CLAUDE.md`). The `RoutingContext` carried through `execute_loop` already records the agent chain (`routing_ctx.agent_chain`), so approval decisions can be scoped to the subagent if needed. No special-case work here.

### Tracing

New handler methods on `AppCore` (`spawn_subagent`, `resume_subagent`, `kill_subagent`, `list_subagents`) all carry `#[tracing::instrument(skip(self), err)]` per the existing convention. Tauri-command shells in `crates/desktop/src/commands/` remain uninstrumented.

## Open questions

None blocking. Possible follow-ups (out of scope here):
- Mid-flight steer for subagents (currently spawn → run → resume; no in-loop nudge).
- An explicit `agent.complete` action that flips an `idle` instance to terminal `completed`, freeing the parent from "should I resume this or is it done?" ambiguity.
- A dedicated `SubagentTray` panel (Kimi-style). Punted because the in-thread navigation already covers the main use case.
- Cross-session subagent reuse (a subagent created in session A surfaced and resumed from session B). Probably not desirable.
