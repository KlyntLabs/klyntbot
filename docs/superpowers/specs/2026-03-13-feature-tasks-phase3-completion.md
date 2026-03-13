# feature-tasks Phase 3 Completion: Cron, Auto-Apply, Focus Tracking

**Date:** 2026-03-13
**Status:** Approved for implementation

## Context

Phase 3 of feature-tasks is ~95% implemented. All handler traits, LLM implementations, DB tables, DomainEvent variants, and prompt templates are in place. This spec covers the 3 remaining wiring gaps needed to complete Phase 3.

**Out of scope (explicit deferral):** Subagent spawning in `LlmTaskExecutionHandler` — marked TODO, awaits `SpawnHandler` integration. FSRS recurring task generation is a separate future initiative.

---

## Gap 1 — Proactive Scan Cron Job

### Problem
`LlmProactiveHandler` is fully implemented but only evaluates `TaskOverdue` internally. Three additional triggers (`TaskStale`, `WipLimitExceeded`, `BlockedChainStale`) are defined but never evaluated. No cron job triggers any of this automatically.

### Solution

**Expand `suggest()` to cover all 4 triggers**, then wire a cron job that calls it.

**`suggest()` expansion** (in `crates/agent/src/handlers/proactive.rs`):
The current `suggest()` only fetches tasks with `status = "todo"` and evaluates `TaskOverdue`. Expand it to also cover three new triggers. This requires broadening the query and adding a second DB call:

- **Query change**: Remove the `status: Some("todo")` filter (or replace with a multi-status filter covering `["todo", "doing"]`) so stale and WIP tasks are included.
- **TaskStale**: tasks with `status = todo` or `status = doing` where `updated_at < now - stale_task_days` (from `TasksConfig`)
- **WipLimitExceeded**: issue a **separate DB call** to count tasks with `status = doing`. If count exceeds `wip_limit` (from `TasksConfig`), fetch the oldest in-progress tasks and call `evaluate_task()` with `SuggestionTrigger::WipLimitExceeded`. Do not derive this from the main task list — the count query must be independent.
- **BlockedChainStale**: tasks with blockers where the blocker's `updated_at < now - stale_task_days`

All four evaluations call the existing `evaluate_task()` method with the appropriate `SuggestionTrigger` variant.

**Cron wiring** in `crates/app-core/src/init/cron.rs`:
Follow the existing `register_cron_callbacks` + `ensure_cron_jobs` + named-constant pattern.

**Schedule:** `CronSchedule::Cron { expr: "0 */4 * * *".to_string(), tz: None }` (every 4 hours)

**Flow for `run_proactive_scan()`:**
1. Call `LlmProactiveHandler::suggest(SuggestionScope::default())` — returns `Vec<SuggestionCandidate>`
2. For each candidate: persist to `task_suggestions` (insert row with status `pending`), obtaining the DB row ID
3. Emit `ProactiveSuggestionCreated` domain event with the persisted row ID as `suggestion_id`
4. Auto-apply check runs inline (see Gap 2)

**Avoiding double event emission:** `LlmProactiveHandler::evaluate_task()` currently emits `ProactiveSuggestionCreated` internally. To avoid emitting the event twice (once inside `evaluate_task`, once from `run_proactive_scan` after persist), refactor `evaluate_task` to **return** candidates without emitting events. Event emission becomes the caller's responsibility. The trait method signature on `ProactiveHandler` (L4) does not change — only the internal LLM implementation changes. Direct callers of `evaluate_task()` that currently rely on the event being emitted internally must emit it themselves after calling the method.

**Config guard:** Read `TasksConfig.proactive_suggestions` (the existing field in `crates/feature-tasks/src/config.rs`). `TasksConfig` is NOT part of the global `config::Config` struct and there is no `config.tasks` field. To access it, pass a `tasks_config: TasksConfig` parameter into `init_cron` / `register_cron_callbacks` alongside the existing `config: &config::Config`. The call site in `AppCore` initialization constructs `TasksConfig::default()` (or deserializes from stored user config if available) and passes it through. Only register the cron job if `tasks_config.proactive_suggestions == true`.

**Implementation files:**
- `crates/app-core/src/init/cron.rs` — add `JOB_PROACTIVE_SCAN` constant, register callback, add to `ensure_cron_jobs` guarded by `TasksConfig.proactive_suggestions`
- `crates/app-core/src/handlers/tasks/proactive.rs` (new or existing) — `run_proactive_scan()` function
- `crates/agent/src/handlers/proactive.rs` — expand `suggest()` with 3 new trigger evaluations; refactor `evaluate_task()` to return candidates without emitting events internally

---

## Gap 2 — Suggestion Auto-Application

### Problem
All suggestions persist as `pending` regardless of confidence. The spec requires suggestions at confidence ≥ 0.85 to be auto-applied without user review.

### Solution
After persisting each suggestion in `run_proactive_scan()`, check confidence and call `SuggestionApplier::apply()` for high-confidence ones.

**Design decisions:**
- Threshold: `TasksConfig.suggestion_auto_apply_threshold` (default `0.85`, existing field in `crates/feature-tasks/src/config.rs`)
- `LlmProactiveHandler` gains an `Option<Arc<dyn SuggestionApplier>>` field
- `app-core` can construct `LlmProactiveHandler` with `TaskSuggestionApplier` — this is already valid since `app-core` depends on `agent` (confirmed by existing `agent::NotificationDispatcher` import in `crates/app-core/src/init/cron.rs`)
- Injected at construction in `crates/app-core/src/init/cron.rs` — `None` disables auto-apply (keeps handler unit-testable without a full applier)
- If confidence ≥ threshold:
  1. Call `SuggestionApplier::apply(&suggestion_id, Some(&task_id), &action)` — `task_id` is `Option<&str>` per the trait signature
  2. On success: update suggestion status to `AutoApplied`
  3. On failure: log error at `warn` level, leave status as `pending` — do not propagate error
- If confidence < threshold: leave status as `pending`
- Log at `info` level on auto-apply: `"Auto-applied suggestion {id} ({type}) for task {task_id} (confidence {:.2})"`

**Implementation files:**
- `crates/agent/src/handlers/proactive.rs` — add `Option<Arc<dyn SuggestionApplier>>` field, update `LlmProactiveHandler::new()`, add auto-apply logic
- `crates/app-core/src/init/cron.rs` — inject `TaskSuggestionApplier` when constructing handler

---

## Gap 3 — Focus Time Tracking (FE + BE)

### Problem
`TaskFocusStarted` and `TaskFocusEnded` DomainEvents are defined and the cognitive pipeline handles them, but nothing emits them. No UI exists to start/end a focus session.

### Backend

**Active session state:**
```rust
pub struct ActiveTaskFocus {
    pub task_id: String,
    pub started_at: DateTime<Utc>,
    pub energy_level: Option<EnergyLevel>,  // copied from task.energy_level at start time
}
```
Placed in `crates/feature-tasks/src/types/` (inline in `entity.rs` or a new `active_focus.rs`).
Held in `AppCore` as `active_task_focus: Arc<Mutex<Option<ActiveTaskFocus>>>`.
Field named `active_task_focus` (not `active_focus`) to be clearly distinct from the existing `focus_manager: Option<Arc<FocusManager>>` field (which belongs to `feature_productivity`).

**New Tauri commands:**
- `start_focus(task_id: String)` → starts focus session
- `end_focus(task_id: String)` → ends active focus session

**`start_focus` flow:**
1. Read task from DB — if not found, return `Err(ApiError::not_found("task", &task_id))`
2. Check for existing active focus session — if one is active on a different task, auto-end it first (run end_focus flow for previous task, emit `TaskFocusEnded` and entity update for previous task)
3. Update `tasks.focused_at = now()` (existing column on `Task`)
4. Store `ActiveTaskFocus { task_id, started_at: now(), energy_level: task.energy_level }` in `AppCore`
5. Emit `TaskFocusStarted { task_id, energy_level }`
6. Call `emit_updates(&app, &[EntityUpdate::Task(task_id)])` so the UI reflects the focused state

**`end_focus` flow:**
1. Retrieve active `ActiveTaskFocus` from `AppCore` — if `None`, return `Ok(None)` (no-op, not an error)
2. **Read the full `Task` from the DB** using `task_id` from the session (needed for `estimated_minutes` in step 7 — do this before clearing any state)
3. Calculate `duration_secs = now - started_at`
4. Clear `tasks.focused_at = NULL`
5. Clear `active_task_focus` from `AppCore`
6. Emit `TaskFocusEnded { task_id, duration_secs }`
7. If `task.estimated_minutes` is set: compute `deviation_pct = (duration_secs/60 - estimated_minutes) / estimated_minutes * 100`, emit `EstimationRecorded { task_id, estimated_mins, actual_mins: duration_secs/60, deviation_pct }`
8. Call `emit_updates(&app, &[EntityUpdate::Task(task_id)])` so the UI removes the focused indicator

**Implementation files:**
- `crates/feature-tasks/src/types/` — add `ActiveTaskFocus` struct
- `crates/app-core/src/state.rs` — add `active_task_focus: Arc<Mutex<Option<ActiveTaskFocus>>>` to `AppCore`
- `crates/app-core/src/handlers/tasks/focus.rs` (new, inside `handlers/tasks/`) — `start_focus()` and `end_focus()` logic
- `crates/desktop/src/commands/tasks.rs` — thin Tauri command adapters delegating to `AppCore`

### Frontend

**State sync:** `useFocusSession` subscribes to task entity updates via the existing entity subscription mechanism. When the backend calls `emit_updates` for a task, the hook detects `focused_at != null` → session active; `focused_at == null` → session ended. This covers the auto-end case: when `start_focus` for a second task auto-ends the previous, both tasks receive `emit_updates`, and the hook transitions cleanly.

**Components:**
- `FocusBanner` — persistent indicator shown when a session is active: task title + elapsed timer + "End Focus" button. Must use `position: fixed` (or be rendered via a portal) to avoid clipping by any ancestor `overflow` constraints (see CLAUDE.md CSS gotchas).
- "Start Focus" button on task cards and task detail view

**Timer behavior:**
- Updates every second via `setInterval`
- Format: `mm:ss` up to 59:59, then `hh:mm:ss`
- Derives start time from task's `focused_at` field (not local state) so the timer survives tab re-focus and page refreshes

**Behavior:**
- Starting focus on a second task: frontend calls `start_focus(newTaskId)`, backend auto-ends previous and emits entity updates for both tasks — `useFocusSession` reacts to both
- On task completion during focus: call `end_focus` automatically

**Implementation files:**
- `desktop-ui/src/features/tasks/components/FocusBanner.tsx` (new)
- `desktop-ui/src/features/tasks/hooks/useFocusSession.ts` (new)
- `desktop-ui/src/features/tasks/components/TaskCard.tsx` — add Start Focus button
- `desktop-ui/src/features/tasks/pages/TaskDetail.tsx` — add Start Focus button

---

## Implementation Order

1. **Gap 1 + Gap 2** (pure Rust wiring, no UI) — implement and test together
2. **Gap 3 BE** (Tauri commands + AppCore state)
3. **Gap 3 FE** (React components + hooks)

---

## Testing

**Gap 1:**
- Unit test: cron job registered with `CronSchedule::Cron { expr: "0 */4 * * *", tz: None }`
- Unit test: `run_proactive_scan()` persists suggestions to `task_suggestions` (in-memory SQLite)
- Unit test: job skips registration when `TasksConfig.proactive_suggestions == false`
- Unit test: `evaluate_task()` returns candidates without emitting `ProactiveSuggestionCreated` internally
- Unit test: `suggest()` evaluates `TaskStale` trigger for tasks not updated within `stale_task_days`
- Unit test: `suggest()` evaluates `WipLimitExceeded` when in-progress count exceeds `wip_limit`

**Gap 2:**
- Unit test: suggestion with confidence 0.90 → `SuggestionApplier::apply()` called with `Some(&task_id)`, status = `AutoApplied`
- Unit test: suggestion with confidence 0.70 → `apply()` not called, status = `pending`
- Unit test: `apply()` returns error → status stays `pending`, no panic
- Unit test: `SuggestionApplier` = `None` → auto-apply silently skipped

**Gap 3 BE:**
- Unit test: `start_focus` with non-existent `task_id` → returns `Err(ApiError::not_found(...))`
- Unit test: `start_focus` sets `focused_at`, stores `ActiveTaskFocus`, emits `TaskFocusStarted`
- Unit test: `start_focus` while session active → previous task auto-ended (emits `TaskFocusEnded` + entity update for previous)
- Unit test: `end_focus` with no active session → returns `Ok(None)`, no panic
- Unit test: `EstimationRecorded` emitted only when `estimated_minutes` is set
- Unit test: task read from DB before state is cleared (verify correct `deviation_pct`)
- All tests use `StoragePool::connect_in_memory()`

**Gap 3 FE:**
- Vitest for `useFocusSession`: timer increments each second
- Vitest: entity update with `focused_at = null` → session cleared
- Vitest: entity update for new focused task while session active → transitions to new task
- Vitest: timer derived from `focused_at` field — survives re-render without reset
