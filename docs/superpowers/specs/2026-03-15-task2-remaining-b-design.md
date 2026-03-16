# Task2 Remaining Work — Option B Design

**Date:** 2026-03-15
**Status:** Draft
**Scope:** Complete three features end-to-end in order: Status Workflow Integration, Focus Session Controls, Activity Tab Enrichment. Approach C (feature-by-feature, full stack).

## Context

Task2 UI is fully wired to real backend data (tasks, projects, areas, timeline, mutations, real-time updates) per the [2026-03-14 integration spec](./2026-03-14-task2-integration-design.md). This spec covers the next phase: replacing remaining stubs and hardcoded values with real data.

### Out of Scope

- AI Suggestions (#4) — needs new backend pipeline, future work
- Task Memory / Cognitive Bridge (#5) — future work
- Project Icons (#6) — cosmetic, future work
- Comment-type activity entries — needs a comments system
- Server-side `entity_id` filtering on timeline — optimization, not needed yet

---

## Feature 1: Status Workflow Integration

### Goal

Replace 6 hardcoded statuses with dynamic workflow labels fetched per-project from the backend.

### Current State

- **Backend:** Fully built. `workflow_get_effective` Tauri command returns `StatusLabel[]` per project. `StatusWorkflow` and `StatusLabel` CRUD in `crates/storage/src/repos/status_workflow.rs`. Default workflow has 6 labels: Backlog, Todo, In Progress, In Review, Done, Blocked.
- **Frontend:** `useEffectiveLabels(projectId)` hook exists in `src/shared/hooks/useWorkflows.ts`. 6 hardcoded statuses in `tasks2/lib/status-icons.tsx`. All 6 consumer components import `allStatus` from there.
- **Mapper:** `resolveStatus()` in `mappers.ts` already handles custom `statusLabel` as fallback.

### Design

#### 1.1 StatusWorkflowProvider Context

A single React context at the tasks2 feature root — not per-component hooks.

**Location:** `desktop-ui/src/features/tasks2/contexts/StatusWorkflowContext.tsx`

**Responsibilities:**
- Takes the currently selected project (from navigation/filters) as input
- Calls `useEffectiveLabels(projectId)` once — if `projectId` is null (all projects view), fetches the default workflow (pass `null` or omit)
- Transforms `StatusLabel[]` → `Status[]` using icon matching (see 1.2)
- Exposes via `useStatusWorkflow()`:
  - `statuses: Status[]` — full list for dropdowns/pickers
  - `resolveStatusById(id: string): Status` — lookup by status label ID
  - `resolveStatusByTask(task: Task): Status` — resolve the best match for a task's current status

#### 1.2 Icon Resolution

A `matchIcon(labelName: string)` function inside the provider's transform logic.

**Algorithm:**
1. Normalize: lowercase, trim, strip hyphens/underscores, collapse spaces
2. Match against known names map:
   - `"backlog"` → BacklogIcon
   - `"todo"`, `"to do"` → TodoIcon
   - `"inprogress"`, `"in progress"` → InProgressIcon
   - `"inreview"`, `"in review"`, `"technical review"`, `"review"` → ReviewIcon
   - `"done"`, `"completed"`, `"complete"` → CompletedIcon
   - `"blocked"`, `"paused"`, `"on hold"` → PausedIcon
3. No match → colored `CircleDot` using `statusLabel.color`

**Location:** `desktop-ui/src/features/tasks2/lib/status-icons.tsx` — refactored to export individual icon components + the `matchIcon` function. The `allStatus` array export is removed.

#### 1.3 Component Updates

All consumer components switch from `import { allStatus }` to `const { statuses } = useStatusWorkflow()`:

| Component | Current | Change |
|-----------|---------|--------|
| `StatusSelector.tsx` | Iterates `allStatus` for dropdown | Use `statuses` from context |
| `SidebarProperties.tsx` | Status picker from `allStatus` | Use `statuses` from context |
| `IssueContextMenu.tsx` | Status submenu from `allStatus` | Use `statuses` from context |
| `IssueBoard.tsx` | Board columns from hardcoded statuses | Columns from `statuses` via context |
| `AllIssues.tsx` | Uses `allStatus` in `handleUpdateStatus` | Use `statuses` from context |
| `CreateIssueModal.tsx` | Status options from `allStatus` | Use `statuses` from context |
| `Filter.tsx` | Filter options from hardcoded statuses | Options from `statuses` via context |

Note: `GroupIssues.tsx` receives `Status` as a prop (does not import `allStatus`), so it needs no import change — it will automatically receive dynamic statuses from its parent.

#### 1.4 Board/Grouping Graceful Degradation

Tasks whose `statusLabel` doesn't match any column get bucketed into the closest `statusGroup` match (open → first column, active → middle, closed → done column, blocked → last column) rather than disappearing. The `Status` interface is extended with an optional `statusGroup: StatusGroup` field (carried from `StatusLabel.statusGroup`) to enable this matching.

#### 1.5 `StatusIcon` Component and `status-utils.tsx`

The existing `StatusIcon` component in `status-icons.tsx` uses a `statusById` lookup keyed by hardcoded IDs. This will break for dynamic status labels. Update `StatusIcon` to use `matchIcon(name)` instead of ID-based lookup. Components like `GroupIssues.tsx` and `SidebarProperties.tsx` that call `renderStatusIcon(status.id)` via `status-utils.tsx` will be updated to pass the status name instead, or use the `Status` object's `icon` property directly.

#### 1.6 `resolveStatus()` Stays Pure

`resolveStatus()` in `mappers.ts` remains a pure function — signature changes from `resolveStatus(task: Task)` to `resolveStatus(task: Task, labels: StatusLabel[])`. The `StatusWorkflowProvider` calls it internally.

**Ripple-through:** Internal mapper functions that call `resolveStatus` also gain the `labels` parameter:
- `taskToIssue(task, projectMap, labels)` — passes `labels` to `resolveStatus`
- `taskToDetailTask(task, projectMap, labels)` — same
- `taskToSubIssue(task, labels)` — same

The `statusByBackend` map (currently built from `allStatusDefs`) is replaced by building it from the `labels` parameter inside these functions.

Callers of these mappers (hooks like `useTasks`, `useIssueDetail`) get `labels` from the `StatusWorkflowProvider` context and pass it through.

#### 1.7 Mutation Payload: `status_label_id`

When the user picks a dynamic status from the dropdown, the mutation must send `status_label_id` (the label's UUID) to the backend — not just the raw `status` string. The backend `TaskUpdateParams` already supports `status_label_id` (see `crud.rs`).

**Replace `statusToBackend()`:** The current helper maps `status.backendStatus` to a raw string. Replace with a helper that returns `{ status: label.statusGroup, statusLabelId: label.id }` for the mutation params. This ensures both the status group (for queries/filtering) and the specific label (for display) are persisted.

Components calling `task_update` mutations (e.g., `StatusSelector`, `IssueContextMenu`, `AllIssues`) update their mutation payloads accordingly.

#### 1.9 Known Limitation: Workflow Label Changes

If workflow labels are edited while the board is open, the UI won't update until the next `useEffectiveLabels` refetch (typically on component remount or window focus via Tanstack Query defaults). No real-time event exists for workflow changes. This is acceptable for now — workflow edits are rare admin operations.

#### 1.10 Files Changed

- **New:** `tasks2/contexts/StatusWorkflowContext.tsx`
- **Modified:** `tasks2/lib/status-icons.tsx` (refactor to icon registry + `matchIcon`, update `StatusIcon` component)
- **Modified:** `tasks2/lib/status-utils.tsx` (update `renderStatusIcon` to use name-based matching)
- **Modified:** `tasks2/lib/mappers.ts` (`resolveStatus` accepts `StatusLabel[]` param instead of using hardcoded list)
- **Modified:** 7 consumer components (swap `allStatus` → `useStatusWorkflow()`)
- **Modified:** tasks2 root layout/provider to wrap with `StatusWorkflowProvider`

---

## Feature 2: Focus Session Controls

### Goal

Wire the Stop button to end focus sessions and remove hardcoded quality metrics.

### Current State

- **Backend:** `task_start_focus` and `task_end_focus` Tauri commands fully implemented in `crates/desktop/src/commands/tasks.rs`. `AppCore::start_focus()` and `AppCore::end_focus()` in `crates/app-core/src/handlers/tasks/focus.rs`. Focus state stored as `task.focusedAt` (ISO timestamp) + `task.totalTrackedSecs`.
- **Frontend:** `SidebarWorkState.tsx` renders timer, quality score, sparkline, Pause/Stop buttons. Both buttons are `disabled`. `deriveFocusSession()` in `useIssueDetail.ts` returns hardcoded values (`qualityScore: 0.7`, `distractionCount: 0`, `flowState: "building"`, `qualityHistory: [0.5, 0.6, 0.7]`).

### Design

#### 2.1 Wire Stop Button

- Add `useMutation("task_end_focus")` in the detail view
- Stop button calls the mutation with `{ id: task.id }`
- On success: `entity:updated` event fires → task refetches → `focusedAt` becomes null → focus panel disappears naturally
- No new backend work needed

#### 2.2 Pause Button — Deferred

No backend concept of "paused focus" exists. Adding one requires new DB fields, a new Tauri command, and timer state management.

- Pause button stays visually present but `disabled`
- Add tooltip: "Coming soon"
- Future iteration can add pause/resume support

#### 2.3 Replace `deriveFocusSession()` with `buildFocusSession(task)`

Rename and refactor to return real data where available, `null` where not. This is an **interface replacement** — the old `FocusSession` shape (`focusMode`, `qualityScore` as required fields) is replaced with a new shape:

```typescript
interface FocusSession {
  startedAt: string;            // ISO timestamp from task.focusedAt
  elapsed: number;              // seconds, computed from startedAt
  totalTracked: number;         // seconds from task.totalTrackedSecs
  qualityScore: number | null;  // no backend support yet
  distractionCount: number | null;
  flowState: string | null;
  qualityHistory: number[] | null;
}

function buildFocusSession(task: Task): FocusSession | null {
  if (!task.focusedAt) return null;
  return {
    startedAt: task.focusedAt,
    elapsed: /* computed from focusedAt, already works */,
    totalTracked: task.totalTrackedSecs ?? 0,
    qualityScore: null,
    distractionCount: null,
    flowState: null,
    qualityHistory: null,
  };
}
```

All consumers of `FocusSession` (primarily `SidebarWorkState.tsx`) must be updated to use the new shape. The old `focusMode` field is dropped — it was a hardcoded string with no backend equivalent.

#### 2.4 Edge Case: `end_focus` In-Memory State

Note: `end_focus` on the backend uses an in-memory `active_task_focus` tracker and ignores the passed `task_id` (parameter is `_task_id`). If the app restarts while a task has `focusedAt` set, the in-memory state is lost and `end_focus` returns `Ok(None)`. The Stop button should handle this gracefully — if the mutation returns no result, fall back to calling `task_update` to clear `focusedAt` directly. This is an edge case worth handling but not blocking.

#### 2.5 Conditional UI in SidebarWorkState

Update `SidebarWorkState.tsx` to conditionally hide sections when data is null:
- Quality score section: hidden when `qualityScore === null`
- Distraction count: hidden when `distractionCount === null`
- Flow state badge: hidden when `flowState === null`
- Quality sparkline: hidden when `qualityHistory === null`
- Timer + Stop button: always shown when focused

**Net result:** Honest UI — live timer and working Stop button. No fake metrics.

#### 2.6 Files Changed

- **Modified:** `tasks2/hooks/useIssueDetail.ts` — rename `deriveFocusSession` → `buildFocusSession`, return nulls for unsupported fields
- **Modified:** `tasks2/components/detail/SidebarWorkState.tsx` — enable Stop button with mutation, null-guard quality sections, add Pause tooltip
- **Modified:** `FocusSession` type — make quality fields nullable

---

## Feature 3: Activity Tab Enrichment

### Goal

Log status/priority changes as specific timeline entries and distinguish actor types in the activity feed.

### Current State

- **Backend:** `task_update` handler in `crud.rs` performs updates and emits `EntityUpdate` events for UI refresh, but does NOT emit timeline entries directly. Timeline entries are generated from `DomainEvent` records via `normalize_domain_event` in `crates/app-core/src/handlers/timeline.rs`. `TimelineEntryType` is a Rust enum in `desktop-shared` with variants like `TaskUpdated`, `TaskCreated`, etc. No actor information on entries.
- **Frontend:** `IssueActivityTab.tsx` renders `ActivityEntry[]` with actor avatars (User/Agent/System). `timelineToActivity()` mapper sets all actors to `"system"`. Rendering is complete — just needs richer data.

### Design

#### 3.1 Backend — Change-Specific Timeline Entries

**Mechanism:** Emit new `DomainEvent` variants via the message bus, then handle them in `normalize_domain_event` to produce timeline entries. This follows the existing pattern — timeline entries are not written directly from handlers.

**Steps:**

1. **Add new `DomainEvent` variants** (in `common` or wherever `DomainEvent` is defined):
   - `TaskStatusChanged { task_id, from, to, actor }`
   - `TaskPriorityChanged { task_id, from, to, actor }`
   - `TaskFieldUpdated { task_id, field, from, to, actor }`

2. **Add new `TimelineEntryType` variants** in `crates/desktop-shared/src/commands/timeline.rs`:
   - `TaskStatusChanged`
   - `TaskPriorityChanged`
   - `TaskFieldUpdated`

3. **In `task_update` handler** (`crud.rs`): before applying the patch, fetch the current task via `repos.tasks.get(&params.id)` to diff old vs new values. For each changed key field (`status`, `priority`, `title`, `description`, `project_id`, `parent_id`), emit the corresponding `DomainEvent` variant via the bus. This adds one extra DB read per update (acceptable tradeoff).

4. **In `normalize_domain_event`** (`timeline.rs`): handle the new event variants to produce `TimelineEntry` records:
   - `entry_type`: the new `TimelineEntryType` variant
   - `title`: human-readable, e.g. `"Status changed from Todo to In Progress"`
   - `metadata` (JSON): `{ "field": "status", "from": "todo", "to": "in_progress", "actor": "user" }`
   - `source`: `"task"`
   - `entity_id`: the task ID

5. **Update `compute_summary()`** in `timeline.rs` to handle the new `TimelineEntryType` variants (otherwise they fall through the catch-all).

6. **Verify deduplication filter** (timeline.rs lines ~137-146) does NOT exclude the new entry types. The current filter removes `TaskCreated`, `TaskCompleted`, etc. because those are also generated by direct pipeline queries. The new variants (`TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated`) are only produced by domain events, so they should pass through naturally — no code change expected, but verify.

7. The generic `TaskUpdated` event continues to fire for non-diffed fields. The new specific events replace it only for fields we explicitly track.

#### 3.2 Backend — Actor Type

Store actor in the `metadata` JSON field of timeline entries (which is `Option<serde_json::Value>` — already exists, no schema change needed).

**Approach:** Add an optional `actor: Option<String>` parameter to the `task_update` handler. Call sites determine the value:

| Call site | Actor value |
|-----------|-------------|
| Tauri command (user clicked in UI) | `"user"` |
| Agent pipeline (AI-driven update) | `"agent"` |
| Automated/cron/system triggers | `"system"` (default when `None`) |

The actor value flows: Tauri command → `AppCore::task_update(params, actor)` → `DomainEvent` variant → `normalize_domain_event` → `metadata.actor` in the timeline entry.

**Tauri command layer:** The `task_update` Tauri command in `desktop/src/commands/tasks.rs` hardcodes `actor: Some("user".into())` when calling `AppCore::task_update()`. The `actor` does NOT go into `TaskUpdateParams` (which is a shared IPC type) — it's a separate parameter on the handler method. This avoids frontend needing to pass `actor` on every call.

#### 3.3 Frontend — Enrich `timelineToActivity()` Mapper

In `tasks2/lib/mappers.ts`:

1. **Map new entry types to actions:**
   - `"taskStatusChanged"` → `"changed status"`
   - `"taskPriorityChanged"` → `"changed priority"`
   - `"taskFieldUpdated"` → `"updated {field}"`

2. **Parse metadata for detail text:**
   - Extract `from`/`to` values → render as `"Todo → In Progress"`
   - Field name for generic updates → `"Updated title"`

3. **Map actor field:**
   - `"user"` → `{ actorType: "user", actorName: "You" }`
   - `"agent"` → `{ actorType: "agent", actorName: "Klyntbot" }`
   - `"system"` or missing → `{ actorType: "system", actorName: "System" }`

The `IssueActivityTab.tsx` component already handles all three actor types with different avatar icons and gradient colors — no changes needed there.

#### 3.4 What We're NOT Doing

- Comment-type entries (user-authored text) — needs a comments system, separate feature
- Agent action logging (suggestions applied, linked issues) — depends on AI Suggestions (#4)
- Server-side `entity_id` filtering — optimization, current client-side filter works fine

#### 3.5 Files Changed

**Backend:**
- **Modified:** `common` (or wherever `DomainEvent` is defined) — add `TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated` variants
- **Modified:** `crates/desktop-shared/src/commands/timeline.rs` — add new `TimelineEntryType` enum variants
- **Modified:** `crates/app-core/src/handlers/tasks/crud.rs` — fetch old task before update, diff fields, emit specific domain events, accept `actor` param
- **Modified:** `crates/app-core/src/handlers/timeline.rs` — handle new events in `normalize_domain_event`, update `compute_summary()`, update deduplication filter
- **Modified:** `crates/desktop/src/commands/tasks.rs` — pass `actor: Some("user".into())` to `task_update` handler (both Tauri command and dev server dispatch)

**Frontend:**
- **Modified:** `tasks2/lib/mappers.ts` — enrich `timelineToActivity()` with new entry types + actor mapping from metadata

---

## Dependency Order

```
Feature 1: Status Workflow  (frontend only, backend exists)
    ↓
Feature 2: Focus Session    (frontend wiring, no backend changes)
    ↓
Feature 3: Activity Tab     (backend + frontend, benefits from #1 being done
                              so status changes can be logged properly)
```

Features 1 and 2 are independent and could be parallelized, but sequential is safer for review. Feature 3 depends conceptually on #1 (status names should be resolved for "from → to" display).
