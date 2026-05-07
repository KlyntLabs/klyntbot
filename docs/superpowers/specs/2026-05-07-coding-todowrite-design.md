# Coding TodoWrite — design

**Date:** 2026-05-07
**Status:** Spec (brainstorming complete; awaiting user review before plan)
**Companion docs:**
- Comparative analysis: [`docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`](../notes/2026-05-07-long-running-task-comparative-analysis.md) (§7 Phase 2.1)
- Plan-mode foundation (Phase 2.2): TBD — this spec depends on `CodingApprovalPolicy::PlanMode` variant being added there
- Permission gate: [`docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md`](2026-05-05-unified-permission-gate-design.md)
- Compaction architecture: existing — `MidLoopCompressor`, `TieredHistoryCompressor`, `LiveContextRefresher`, `ContextUpdateQueue`
- Mirror engine: `crates/cognitive/src/mirror/` (six existing signal sources; this adds the seventh)

---

## 1. Motivation

Klynt's coding mode currently has no first-class affordance for tracking multi-step work across iterations. The agent loses progress visibility on long horizons; the user has no at-a-glance answer to "what is the agent doing now?"; the cognitive layer has no signal for *what was planned vs what was done* — a category of pattern only Klynt can mine because no comparable agent has the cognitive subsystem to ingest it.

Comparative analysis (companion doc §3–§4) showed three reference points:

- **kimi-cli `SetTodoList`** — single tool with three modes (write/query/clear), per-session JSON, three statuses, colocated anti-abuse prose. Best feature: the verbatim "do not track too-small steps" guidance with worked examples. Worst flaw: subagents have isolated state files but the tool is excluded from subagent allowlists, making the isolation infrastructure dead code.
- **codex `update_plan`** — single tool, full-replacement, in-memory only. Best feature: dual-surface fan-out (one tool call → TUI cell + terminal title counter + WebSocket broadcast). Worst flaw: zero persistence; all state lost on resume or compaction.
- **opencode** — no TodoWrite at all. Substitutes via `summary_message_id` compaction anchors and sub-agent task sessions keyed by `toolCallID`. Best feature: discipline of not adding more state. Worst flaw: long-horizon tasks lose structured progress at compaction.

This spec combines kimi's anti-abuse prose discipline with codex's fan-out broadcast pattern and adds Klynt-specific cognitive integration that none of the comparators can reach.

---

## 2. Goals & non-goals

### Goals

1. Give the LLM externalized memory for multi-step coding tasks that survives iterations and compaction.
2. Surface live progress to the user across four UI layers: sidebar count, inline conversation strip, status bar, on-demand panel.
3. Publish typed events to `DomainEventBus` so the cognitive layer (mirror, reforge, FSRS) can mine patterns: friction (blocked reasons), intent-vs-execution (plan ratification), profile attribution (which subagent profile completes which item types fastest), concurrency-class accuracy.
4. Enforce concurrency-safety invariants at the gate so parallel subagents cannot conflict.
5. Compose cleanly with plan mode (Phase 2.2): plan mode seeds proposed items, user ratifies on exit, items become active.

### Non-goals

- Cross-thread human productivity tracking (that is `feature-tasks`, separate concern).
- Human-first editing affordances *outside* plan mode (TodoWrite is LLM-first; the only place the human writes the list is during plan-mode review).
- Cancelled-as-explicit-status (cancellation is computed via diff between writes; no status pollution).
- Background-task progress (covered by Phase 2.3 background bash).
- Anything that requires changes to provider adapters (the schema is Anthropic-native and degrades gracefully on OpenAI strict-mode providers).

---

## 3. Architecture overview

```
LLM → coding_todo({items}) 
    ↓
CodingTodoTool::execute  (in feature-coding-todo crate)
    · validate (concurrency rules, ≤1 in_progress, blocked_by graph, plan-mode constraints)
    · diff prior list → compute TodoCancelled events for dropped items
    · if PlanMode active → coerce status=Pending; tag proposed_in_plan_session
    · persist row (thread_id, agent_id) to coding_todos table
    · publish TodoEvent variants to DomainEventBus
    · auto-coerce Blocked status for items with unmet blocked_by deps
    ↓
DomainEventBus fans out to:
    1. SQLite                     authoritative storage
    2. UI surfaces (4 components) sidebar count, inline card, status bar, panel
    3. TodoSignalSource (mirror)  7th signal source, nightly aggregation → reforge
    4. coding-ingest wire log     replay via klynt vis
```

The composition is: **codex's fan-out pattern, on top of Klynt's bus, with the cognitive layer as a fifth subscriber** — that fifth subscriber is the differentiator.

### Reuses existing infrastructure

- `tools-core-macros` for `#[derive(Tool)]` + `ToolParams`
- `FeaturePackage` trait + migration system
- `DomainEventBus` for typed event publishing
- `MirrorSignalSource` trait pattern (six existing sources; we add a seventh)
- `ContextUpdateQueue` for compaction-aware state injection
- `coding-ingest` wire bus for replay
- `useThreadEvents` reducer for UI subscriptions
- `klynt_command` Tauri-command macro
- Soul-file hot-reload for anti-abuse prose

### Net-new code

- `feature-coding-todo` crate (tool, types, validation, diff, migration)
- `coding_todos` SQLite table + repo
- `TodoSignalSource` impl in `crates/cognitive/src/mirror/sources/`
- 5 React components in `desktop-ui/src/features/coding/components/todos/`
- 4 Tauri commands in `crates/desktop/src/commands/coding_todo.rs`
- App-core handler in `crates/app-core/src/coding/todo_handler.rs`
- Anti-abuse prose addition in `~/.klyntbot/KLYNTBOT-coding.md`
- `CodingApprovalPolicy::PlanMode` variant (depends on Phase 2.2 plan-mode workstream)

---

## 4. Data model

### SQLite schema

```sql
CREATE TABLE coding_todos (
    thread_id  TEXT NOT NULL,
    agent_id   TEXT NOT NULL,           -- "root" or subagent UUID
    items_json TEXT NOT NULL,           -- JSON array of TodoItem
    proposed_in_plan_session TEXT,      -- nullable; set during plan mode
    updated_at TEXT NOT NULL,           -- jiff::Timestamp ISO 8601
    PRIMARY KEY (thread_id, agent_id)
);

CREATE INDEX idx_coding_todos_thread ON coding_todos(thread_id);
```

One row per agent. The full item list lives as a JSON blob — atomic write per call, no row-level concurrency, simple read.

### Item shape

```rust
pub struct TodoItem {
    pub id: String,                          // ULID; LLM-supplied or auto-generated
    pub title: String,                       // imperative form, ~80 char soft cap
    pub status: TodoStatus,                  // 4-state enum
    pub concurrency: ConcurrencyClass,       // 3-class enum
    pub blocked_reason: Option<String>,      // required iff status == Blocked
    pub blocked_by: Vec<String>,             // item IDs this depends on
    pub delegated_to: Option<AgentId>,       // claim by subagent
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
    Blocked,    // requires blocked_reason
}

pub enum ConcurrencyClass {
    Safe,        // read-only or non-conflicting; multi-parallel OK
    Sequential,  // writes; one at a time, no order requirement
    Exclusive,   // writes; must run alone across whole agent tree
}
```

`id` is optional on input (handler assigns ULID if absent) and required on storage. ULIDs are time-ordered, preserving creation order without an explicit index.

### Item provenance fields

- `delegated_to: Option<AgentId>` — when a parent item is delegated to a subagent, the parent records the claim. Used for traceability and cross-agent UI rendering.
- `proposed_in_plan_session: Option<String>` — set during plan mode (on the row, not the item; all items in a plan-mode write share one session). Cleared on plan ratification.

---

## 5. Tool surface

### Registration

```rust
#[derive(Tool)]
#[tool(
    name = "coding_todo",
    approval_class = "Safe",                // no codebase mutation
    allowed_channels = "coding",            // assistant-mode LLM never sees it
    concurrency_safety = "Sequential",      // one TodoWrite call per agent at a time
)]
pub struct CodingTodoTool { /* deps */ }

#[derive(ToolParams, Deserialize)]
pub struct CodingTodoParams {
    pub items: Vec<TodoItemInput>,           // empty list = clear
}

#[derive(Deserialize, ToolParams)]
pub struct TodoItemInput {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    pub status: TodoStatus,                  // proper enum (Anthropic-native)
    pub concurrency: ConcurrencyClass,       // proper enum
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub delegated_to: Option<String>,
}
```

### JSON Schema (auto-generated by `tools-core-macros`)

`status` and `concurrency` emit as proper JSON Schema enums (Anthropic validates at decode), not codex's `string` + description fallback. The LLM physically cannot generate non-enum values.

### Return value (success)

```
Updated coding_todo for agent <agent_id> in thread <thread_id>.
  6 items: 3 pending, 1 in_progress, 1 done, 1 blocked
  Diff vs prior: +2 added, +1 status_changed, +1 cancelled
  Auto-coercions: 1 item moved to Blocked (blocked_by=task_3 not done)
```

The LLM reads this and knows exactly what landed; no need for a query mode.

### Errors

```rust
pub enum CodingTodoError {
    BlockedItemMissingReason { item_id: String },
    MultipleInProgressInAgent { agent_id: String, item_ids: Vec<String> },
    ConcurrencyViolation {
        item_id: String,
        class: ConcurrencyClass,
        conflicts_with: Vec<(AgentId, ItemId)>,
    },
    CycleInBlockedBy { chain: Vec<String> },
    BlockedByUnknownItem { item_id: String, missing_dep: String },
    PlanModeNonPendingStatus { item_id: String, status: TodoStatus },
    DelegatedToUnknownAgent { item_id: String, agent_id: String },
    CrossAgentMutationAttempt { caller: AgentId, target: AgentId },
    BlockedItemMissingUserMessage { item_ids: Vec<String> }, // hard-reject after one prior soft warning
}
```

Each variant renders to a one-line LLM-facing message that names the offending item and prescribes the corrective action. Example:

> Error: item `task_4` has `concurrency=Exclusive` but agent `subagent_a3f2` already has in_progress item `task_2` with overlapping class. Wait for `task_2` to complete or change `task_4` to `Sequential`.

---

## 6. State machine & invariants

### Allowed transitions

```
Pending     ↔ Blocked            (via blocked_by auto-coercion)
Pending     → InProgress
InProgress  → Done
InProgress  → Blocked            (with blocked_reason)
Blocked     → Pending            (when all blocked_by items become Done)
Blocked     → InProgress         (after re-evaluation; blocked_reason cleared)
*           → (dropped from list)  (computed as TodoCancelled event)
```

### Invariants enforced at the gate

| Invariant | Enforcement |
|---|---|
| ≤1 `InProgress` per agent | Per-row count on incoming list |
| Exclusive lock across agent tree | If transitioning to InProgress with `Exclusive`, no other InProgress item exists in any sibling row for this thread |
| Sequential lock | If `Sequential` → InProgress, no other Sequential or Exclusive InProgress item in the thread |
| Safe always allowed | No cross-agent check |
| `blocked_by` auto-transition | Unmet deps → `status` coerced to Blocked with synthetic `blocked_reason` |
| Cycle detection | Reject lists with cycles in `blocked_by` |
| Blocked requires reason | Manually-set Blocked without `blocked_reason` is rejected |
| Profile auto-class | Items written by `explore` profile default to `Safe` regardless of LLM declaration |
| Anti-passivity (Blocked) | Blocked items without same-turn user-facing message → first occurrence emits a soft warning to the wire log + injects a `<system-reminder>` on the next iteration. If the *next* `coding_todo` call still has any Blocked item without a paired user-facing message in that iteration, the call is hard-rejected with `BlockedItemMissingUserMessage`. "Consecutive" means: two successive `coding_todo` calls within the same agent thread, both violating the invariant. |
| Plan-mode pending-only | In plan mode, all items must be Pending |
| Cross-agent mutation | Writing to a row whose `agent_id` differs from caller's identity is rejected |

---

## 7. Concurrency safety

### Why an item-level concurrency class

Klynt has 3 subagent profiles running concurrently (semaphore=3). Without explicit safety classification, two parallel subagents could double-claim items or perform overlapping writes. We borrow Klynt's existing per-tool concurrency vocabulary (`ConcurrencySafety::{Safe, Sequential, Exclusive}`, mentioned in CLAUDE.md cross-cutting improvements) and apply it at the *item* level. Reusing the vocabulary means reforge can cross-reference patterns from a single ontology.

### Class semantics

- **Safe** — read-only or non-conflicting work (e.g., "list all tests for X module", "read schema.sql"). Multiple Safe items can be InProgress concurrently across any agents. The `explore` subagent profile auto-defaults to Safe.
- **Sequential** — writes that may conflict with other Sequential items. One at a time across the thread, but order is unconstrained. Most coding work falls here (typical edits to disjoint files).
- **Exclusive** — writes that demand isolation (migrations, schema changes, mass-rename refactors). Only one Exclusive item across the entire agent tree at a time, blocks all Sequential items too.

### Validation flow

When `coding_todo` is called with new items:

1. **Parse** the items, assign ULIDs to any without IDs.
2. **Build** the merged dependency graph from `blocked_by`. Reject on cycles.
3. **Auto-coerce** Blocked status for any item with unmet `blocked_by`.
4. **Auto-class** items written by `explore`-profile agents to `Safe`.
5. **Per-agent invariants** — ≤1 InProgress, Blocked has reason.
6. **Cross-agent invariants** — query other agents' rows for this thread; check Exclusive/Sequential conflicts.
7. **Plan-mode invariants** — if active, all status must be Pending.
8. **Diff** against prior row; emit `TodoCancelled` events for dropped items.
9. **Persist** via SQLite transaction.
10. **Emit** `TodoStateChanged` events to `DomainEventBus`.

If any validation fails, the entire write is rejected with a structured error. The LLM self-corrects on the next iteration.

---

## 8. Plan mode integration

### Approval policy variant

```rust
pub enum CodingApprovalPolicy {
    Default,
    PlanMode {
        plan_session_id: String,
        plan_file_slug: String,
    },
    YoloMode { until: jiff::Timestamp },
}
```

### Plan-mode tool behavior

| Tool | Plan-mode behavior |
|---|---|
| `coding_todo` | Allowed. Status forced to Pending. **Row** tagged with `proposed_in_plan_session = plan_session_id` (not the items — all items in the row share the row's tag). Emits `TodoPlanProposed`. |
| `Edit`, `Write` | Restricted to `plan_file_slug.md` only. Other paths → `<system-reminder>` injection. |
| `Read`, `Grep`, `Glob`, `LSP` | Allowed (read-only). |
| Other write tools | Rejected with prose explaining plan mode. |

### Two flows for entering plan mode

| Flow | Trigger | Mechanism |
|---|---|---|
| **Interactive plan mode** | User asks "let's plan this" or invokes `/plan` slash command | Enter `PlanMode` policy → LLM calls `coding_todo` (forced pending, tagged) → user reviews in sidebar → ratify on exit |
| **Plan file import** | User says "execute @docs/superpowers/plans/X.md" | LLM uses existing `Read` tool → derives items → calls `coding_todo` directly with Pending status. Cognitive layer correlates Read→TodoWrite sequence as implicit provenance. No plan mode needed. |

### Ratification flow

1. User clicks **"Ratify & Execute"** in sidebar `PlanModeBanner`.
2. `coding_plan_ratify` Tauri command fires.
3. Backend reads the row's items, strips `proposed_in_plan_session`, transitions policy to `Default`.
4. Emits `TodoPlanRatified { ratified_count, user_edited_count, user_removed_count }`.
5. LLM's next iteration sees `<system-reminder>Plan ratified by user. {N} items active. Begin execution.</system-reminder>`.
6. Execution begins; LLM picks first Pending item, marks InProgress, works through.

### User edits during plan mode

The sidebar exposes edit and remove affordances *only* during plan mode. Edits trigger `coding_plan_user_edit` Tauri command, which writes the modified list with `source = User` tagging. These are the only LLM-first-violation-by-design moments — scoped explicitly to the plan-review window.

Outside plan mode, the sidebar is read-only.

---

## 9. Cognitive integration

### Domain events

```rust
pub enum TodoEvent {
    TodoStateChanged {
        thread_id, agent_id, agent_profile,
        item_id, from: TodoStatus, to: TodoStatus,
        concurrency: ConcurrencyClass,
        reason: Option<String>,
        timestamp,
    },
    TodoCancelled {
        thread_id, agent_id, agent_profile,
        item_id, prior_status,
        was_blocked_by: Vec<String>,
        timestamp,
    },
    TodoPlanProposed {
        thread_id, plan_session_id, item_ids, timestamp,
    },
    TodoPlanRatified {
        thread_id, plan_session_id,
        ratified_count: usize,
        user_edited_count: usize,
        user_removed_count: usize,
        timestamp,
    },
}
```

Published to `DomainEventBus` from inside `CodingTodoTool::execute` after persistence.

### TodoSignalSource — the 7th MirrorSignalSource

Implements existing `MirrorSignalSource` trait at `crates/cognitive/src/mirror/sources/coding_todo.rs`. Subscribes to `TodoEvent` stream, aggregates windowed observations, emits typed `Signal`s to `MirrorFacade` for nightly reforge synthesis.

### Day-one aggregators (each ~10 LOC)

1. **Plan ratification rate per task type** — `TodoPlanProposed` count vs `TodoPlanRatified.ratified_count`, grouped by item title clustering.
2. **Blocked-reason clustering** — k-means over `blocked_reason` strings; surface top friction patterns to mirror narrative.
3. **Profile-time correlation** — `TodoStateChanged` durations from InProgress→Done, grouped by `agent_profile`.
4. **Cancellation patterns** — `TodoCancelled` count grouped by item title patterns; reforge-actionable when one cluster dominates.
5. **Concurrency-class accuracy** — declared `Exclusive` vs actual conflicts observed; over-declaration → reforge suggests relaxation.
6. **`blocked_by` graph utility** — completion-time correlation: items with declared deps vs without.

These aren't speculative — each is a literal one-method aggregator over the event stream. Mirror's nightly cron (03:00 local) does the heavy lifting via `reforge`.

### Wire log integration

Each `TodoEvent` is also published to `coding-ingest`'s pipeline (per the existing 2026-04-29 spec §12 — desktop process is the runtime, emission always in-process). Replay via `klynt vis` shows todo evolution alongside conversation events.

---

## 10. Compaction-aware re-injection

### The staleness problem

`MidLoopCompressor` triggers at 70% context fill, replacing older `Message::Tool` results with extractive summaries. If the most recent `coding_todo` call is among the compressed messages, the LLM loses its current state.

### Solution

Before `MidLoopCompressor` compresses, it checks: is there a `coding_todo` tool result in the eviction window? If yes, publish `ContextUpdate::TodoStateRefresh { thread_id, agent_id }` to `ContextUpdateQueue`. `LiveContextRefresher` drains this between iterations, injecting:

```
<system-reminder>
Current coding todo list (auto-injected after compaction):
- [in_progress] task_2 · Add migration file · concurrency=Sequential · delegated_to=root
- [pending]     task_3 · Update tests · blocked_by=task_2
- [blocked]     task_4 · Backfill rows · reason: "waiting on user clarification"
- [done]        task_1 · Read existing schema
</system-reminder>
```

Token budget: the high-priority lane (90% context budget) handles this. Items beyond the budget get extractive-summarized via the same compression algorithm, not dropped.

### Subagent compaction

Subagents experience the same staleness. Their `LiveContextRefresher` queues both:

- `ContextUpdate::TodoStateRefresh` — for the subagent's own writable list
- `ContextUpdate::ParentTodoStateRefresh { parent_thread_id, parent_agent_id }` — for the read-only parent context

Two distinct system-reminders, clear visual separation.

---

## 11. UI components

### Component map

| Component | Location | Responsibility |
|---|---|---|
| `TodoSidebarBadge` | `desktop-ui/src/features/coding/components/todos/TodoSidebarBadge.tsx` | Per-thread chip in `ThreadListItem`: `{pending}/{total} · {blocked_count}` |
| `TodoInlineCard` | `desktop-ui/src/features/coding/components/todos/TodoInlineCard.tsx` | Collapsed strip per `coding_todo` call: "Plan updated · 1/4 done" + click-expand |
| `TodoStatusBar` | `desktop-ui/src/features/coding/components/todos/TodoStatusBar.tsx` | Sticky bottom of `MessagePane`: current InProgress item title (truncated), conflicts, blocked count |
| `TodoPanel` | `desktop-ui/src/features/coding/components/todos/TodoPanel.tsx` | On-demand drawer: hierarchical tree (root + subagents); SVG `blocked_by` connectors; status icons; tooltip for `blocked_reason` |
| `PlanModeBanner` | `desktop-ui/src/features/coding/components/todos/PlanModeBanner.tsx` | Top of `MessagePane` when `PlanMode` active: "Reviewing plan — N items proposed [Ratify & Execute] [Edit] [Cancel]" |

### Subscription model

All five subscribe to `coding:todos_updated` events through the existing `useThreadEvents` reducer at `desktop-ui/src/features/coding/hooks/useThreadEvents.ts`. Backend emits one event per `coding_todo` call (and on every `TodoPlanRatified`). Reducer caches per-`(thread_id, agent_id)` state; components select via memoized hooks.

### Initial hydration

On thread open, components call `coding_todo_get(thread_id)` Tauri command which returns the full `CodingTodoView { agents: HashMap<AgentId, Vec<TodoItem>>, plan_mode_state: Option<PlanModeView> }`. After hydration, live updates flow via the event stream.

### Styling

Plain CSS in `desktop-ui/src/styles/coding-todo.css`, imported via `index.css`. BEM-ish class names: `coding-todo`, `coding-todo__sidebar-badge`, `coding-todo__inline-card`, `coding-todo__panel-tree`, `coding-todo__plan-banner`. Status-color tokens:

| Status | Token | Visual |
|---|---|---|
| Pending | `var(--color-fg-muted)` | dimmed bullet `○` |
| InProgress | `var(--color-accent-warm)` | filled triangle `▶` (animated dot when streaming) |
| Blocked | `var(--color-fg-warning)` | filled square `■` with tooltip |
| Done | `var(--color-success)` | check `✓` + strikethrough |

Typography: `--fs-xs` for compact surfaces (badge, inline strip), `--fs-sm` for body (panel, banner).

---

## 12. Anti-abuse prose

Added to `~/.klyntbot/KLYNTBOT-coding.md` (live-reload via `SoulContextSource`). Combines kimi-cli's verbatim guidance with Klynt-specific extensions for plan mode and concurrency declaration.

```markdown
## coding_todo — when to use it (and when not)

The `coding_todo` tool exists for tasks that take more than 4–5 distinct
steps and where you need to track progress across iterations. Abusing
this tool by tracking too-small steps wastes tokens and makes the
conversation messy.

### Do NOT use coding_todo when:

- The user asks a single question. ("What language is this project
  written in?", "What's the best practice for X?")
- The task takes only a few tool calls. ("Fix the unit test
  `test_xxx`", "Refactor function `foo` to be cleaner.")
- The user prompt is specific and you can follow it directly.
  ("Replace X with Y in file Z", "Create file X with content Y.")
- You are exploring read-only context for a single answer.

### DO use coding_todo when:

- The task spans multiple files or multiple distinct concerns.
- Plan mode is active — write proposed items as `pending` for user review.
- A subagent has been delegated several discrete subtasks.
- You catch yourself losing track of completed steps in a long turn.

### Anti-churn rule

Do not call `coding_todo` repeatedly without making real progress on at
least one task between calls. If you cannot advance any task with
available tools, **emit a user-facing message explaining the block**
instead of replanning. Repeatedly updating the todo list without doing
work is counterproductive.

If any item has status `blocked`, you MUST emit a user-facing message
in the same turn explaining what's needed to unblock. Otherwise the
system will warn on the first occurrence and reject the call on the
second consecutive turn.

### Concurrency declaration

Every item declares a `concurrency` class:

- **Safe** — read-only or non-conflicting (multiple can run in parallel
  across subagents). Default for items in the `explore` subagent.
- **Sequential** — writes that may conflict with other Sequential items
  (one at a time, but order doesn't strictly matter).
- **Exclusive** — writes that conflict with everything (must run alone
  across the whole agent tree). Reserve for migrations, schema changes,
  or work that demands isolation.

When in doubt, prefer `Sequential`. `Exclusive` is heavy; the system
will block other items until it completes.

### Plan mode discipline

In plan mode, every item must have `status=pending`. Use plan mode to
propose decomposition; the user ratifies or edits before execution
begins. Do not mark items in_progress in plan mode — the system rejects
non-pending status during plan mode.
```

---

## 13. Subagent context injection

### Two paths

Subagents read parent state (read-only) and write their own (writable). Two separate system-reminders maintain visual clarity.

#### Path 1: `SubagentBuilder::with_parent_todos(parent_state)`

At spawn time, the subagent's initial context includes:

```
<system-reminder>
Parent agent's current plan (read-only context — your task is delegated
from this list). You cannot modify it; you maintain your own coding_todo
list.

- [in_progress] task_2 · Add migration file · concurrency=Sequential · delegated_to=<your_agent_id>
- [pending]     task_3 · Update tests · blocked_by=task_2
- [done]        task_1 · Read existing schema
</system-reminder>
```

#### Path 2: subagent-side compaction refresh

Subagent's `LiveContextRefresher` queues:
- `ContextUpdate::TodoStateRefresh` — for the subagent's own writable list
- `ContextUpdate::ParentTodoStateRefresh { parent_thread_id, parent_agent_id }` — for read-only parent visibility

Both inject after subagent compaction events.

### Enforcement

Cross-agent mutation is rejected at the gate. If a subagent calls `coding_todo` and any item's effective `agent_id` differs from caller's identity, the handler returns `CodingTodoError::CrossAgentMutationAttempt`. The kimi pattern (build isolation infrastructure, then forbid its use via missing allowlist entries) is avoided: the schema and the enforcement are the same mechanism.

---

## 14. Crate placement

| Code | Location |
|---|---|
| `feature-coding-todo` crate (new at L4) | `crates/feature-coding-todo/` |
| TodoTool, TodoItem types | `feature-coding-todo/src/{tool,types}.rs` |
| TodoRepo (per `*Repo` pattern) | `crates/storage/src/repos/coding_todo.rs` |
| Migration | `feature-coding-todo/src/migrations.rs` (`FeatureMigration` impl) |
| Validation logic | `feature-coding-todo/src/validation.rs` |
| Diff-and-emit-events | `feature-coding-todo/src/diff.rs` |
| `TodoSignalSource` | `crates/cognitive/src/mirror/sources/coding_todo.rs` |
| App-core handler | `crates/app-core/src/coding/todo_handler.rs` (instrumented per CLAUDE.md) |
| Tauri commands | `crates/desktop/src/commands/coding_todo.rs` (`#[klynt_command]`) |
| Frontend components | `desktop-ui/src/features/coding/components/todos/` |
| CSS | `desktop-ui/src/styles/coding-todo.css` |
| Anti-abuse prose | `~/.klyntbot/KLYNTBOT-coding.md` (soul file) |
| Plan-mode policy variant | `crates/approval/src/lib.rs` (depends on Phase 2.2) |
| Provider registration | `crates/app-core/src/init/coding_subscribers.rs` |
| Domain events | `crates/bus/src/domain_events.rs` |

---

## 15. Tauri commands

Per CLAUDE.md, `#[klynt_command]` is the happy path: `pub async fn` with no `state` parameter (the macro injects `AppCore` access internally) and a bare `T` return (the macro wraps errors).

```rust
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> () { ... }

#[klynt_command]
pub async fn coding_plan_user_edit(
    thread_id: String,
    plan_session_id: String,
    items: Vec<TodoItemInput>,
) -> CodingTodoView { ... }

#[klynt_command]
pub async fn coding_plan_user_remove(
    thread_id: String,
    plan_session_id: String,
    item_ids: Vec<String>,
) -> CodingTodoView { ... }
```

Each delegates to a corresponding `AppCore` handler instrumented with `#[tracing::instrument(skip(self), err)]`. Each command's path must be added to `desktop_macros::klynt_collect_commands![...]` in `specta_builder.rs` (the `registration_drift` test enforces this). After adding, run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts` (the `bindings_are_current` test enforces this).

---

## 16. Testing strategy

### Unit tests (in `feature-coding-todo`)

- Validation: each `CodingTodoError` variant has a positive and negative test case.
- Diff: dropped-item detection emits `TodoCancelled` events with correct `prior_status`.
- Auto-coercion: `blocked_by` with unmet deps coerces to Blocked.
- Cycle detection: cyclic `blocked_by` is rejected.
- ULID assignment: items without `id` get a ULID; existing ULIDs preserved.

### Integration tests (in `app-core/tests/coding_todo_handler.rs`)

- End-to-end: LLM call → handler → SQLite → DomainEventBus subscriber receives event.
- Concurrency: two parallel subagents both writing — second is rejected with `ConcurrencyViolation`.
- Plan mode: writes during PlanMode are tagged `proposed_in_plan_session`; `coding_plan_ratify` clears the tag.
- Cross-agent mutation rejection: subagent attempting to write to root's row is rejected.
- Compaction injection: `MidLoopCompressor` enqueues `TodoStateRefresh` when relevant.

### Frontend tests

- `TodoSidebarBadge.test.tsx` — renders correct counts from event stream.
- `TodoInlineCard.test.tsx` — collapsed and expanded states.
- `TodoStatusBar.test.tsx` — InProgress display, blocked count, click → opens panel.
- `TodoPanel.test.tsx` — hierarchical tree with subagents, blocked_by connectors.
- `PlanModeBanner.test.tsx` — only shown during PlanMode; ratify/edit buttons fire correct Tauri commands.

### KCA gates

The KCA validation script (`./scripts/run_kca_validation.sh`) must pass before merge. New gates this design adds:
- TodoSignalSource emits ≥1 signal per session with TodoWrite usage.
- Concurrency violations correctly rejected at gate.
- Plan ratification events present in wire log when plan mode is exited.

---

## 17. Dependencies & sequencing

| Dependency | Status | Impact |
|---|---|---|
| Phase 0.1 — approval handler | ✅ Done (commit `bb664ce8c`) | Provides the `ApprovalGate` foundation; TodoTool `approval_class = Safe` so no gate prompt fires, but the gate is the integration point. |
| Phase 0.2 — auto-title | ✅ Done | Independent. |
| Phase 0.3 — mid-stream cancel | 🚧 In progress | Independent; TodoWrite handler doesn't depend. |
| Phase 0.4 — ThreadEventBuffer | ✅ Done | UI components subscribe via the buffer's reducer. |
| Phase 1 — KlyntTracingProvider | 🚧 In progress | TodoEvent flows through coding-ingest once Phase 1 lands. Until then, events still publish to DomainEventBus and SQLite — the wire-log surface comes online when Phase 1 does. |
| Phase 2.2 — plan mode | ⏳ Future | The `CodingApprovalPolicy::PlanMode` variant lives in this workstream. TodoWrite plan-mode integration is *defined* in this spec but *enabled* by Phase 2.2 landing. Plan-mode UI affordances stay hidden until then. |

### Sequencing

1. **Land core TodoWrite without plan-mode integration.** Sections 4–7, 9–11, 13–15. Plan-mode hooks stub to `Default` policy.
2. **Once Phase 2.2 lands**, enable plan-mode integration: Section 8. UI affordances unhide.
3. **Once Phase 1 lands**, wire-log integration is automatic — no additional work.

---

## 18. Open questions

1. **ULID library choice** — `ulid` crate vs `rusty-ulid` vs Klynt-internal helper. (Probably `ulid` — most popular, `Send + Sync`, no_std-friendly.)
2. **Token budget for parent-todo-injection in subagents** — how big can the parent list grow before extractive-summary kicks in? Default to 5% of context; revisit after telemetry.
3. **Plan-mode UI for editing items** — inline-edit fields vs modal? Defer to Phase 2.2 design; spec only mandates "user can edit during plan mode."
4. **`/btw` integration** — should `/btw` (Phase 2.5) have a read-only view of current todos? Probably yes; defer to Phase 2.5 spec.
5. **Mobile/web client view** — Klynt's MCP server exposes some tools to external clients. Should `coding_todo_get` also be exposed to MCP? Probably yes for read-only; defer to MCP spec update.
6. **Reforge feedback loop** — how does reforge's nightly suggestion *feed back* into TodoWrite behavior? Via `KLYNTBOT-coding.md` updates? Per-thread overrides? Defer to a follow-up spec on reforge integration.

---

## 19. Companion documents

- **Comparative analysis (source of truth for design rationale):** `docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md`
- **Permission gate (foundation):** `docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md`
- **Compaction architecture:** code references — `crates/agent/src/execution/{mid_loop_compressor,live_context_refresher}.rs`, `crates/context_engine/src/history_compressor/tiered.rs`, `crates/bus/src/context_updates.rs`
- **Mirror engine:** code reference — `crates/cognitive/src/mirror/`, six existing signal sources
- **Implementation plan:** to be created via `superpowers:writing-plans` skill once this spec is approved.
