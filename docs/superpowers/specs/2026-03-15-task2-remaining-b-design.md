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

All 6 components switch from `import { allStatus }` to `const { statuses } = useStatusWorkflow()`:

| Component | Current | Change |
|-----------|---------|--------|
| `StatusSelector.tsx` | Iterates `allStatus` for dropdown | Use `statuses` from context |
| `SidebarProperties.tsx` | Status picker from `allStatus` | Use `statuses` from context |
| `IssueContextMenu.tsx` | Status submenu from `allStatus` | Use `statuses` from context |
| `IssueBoard.tsx` | Board columns from hardcoded statuses | Columns from `statuses` via context |
| `GroupIssues.tsx` | Group headers from hardcoded statuses | Headers from `statuses` via context |
| `Filter.tsx` | Filter options from hardcoded statuses | Options from `statuses` via context |

#### 1.4 Board/Grouping Graceful Degradation

Tasks whose `statusLabel` doesn't match any column get bucketed into the closest `statusGroup` match (open → first column, active → middle, closed → done column, blocked → last column) rather than disappearing.

#### 1.5 Files Changed

- **New:** `tasks2/contexts/StatusWorkflowContext.tsx`
- **Modified:** `tasks2/lib/status-icons.tsx` (refactor to icon registry + `matchIcon`)
- **Modified:** `tasks2/lib/mappers.ts` (`resolveStatus` simplified — delegates to context)
- **Modified:** 6 consumer components (swap `allStatus` → `useStatusWorkflow()`)
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

Rename and refactor to return real data where available, `null` where not:

```typescript
function buildFocusSession(task: Task): FocusSession | null {
  if (!task.focusedAt) return null;
  return {
    startedAt: task.focusedAt,
    elapsed: /* computed from focusedAt, already works */,
    totalTracked: task.totalTrackedSecs,
    qualityScore: null,      // no backend support yet
    distractionCount: null,  // no backend support yet
    flowState: null,         // no backend support yet
    qualityHistory: null,    // no backend support yet
  };
}
```

#### 2.4 Conditional UI in SidebarWorkState

Update `SidebarWorkState.tsx` to conditionally hide sections when data is null:
- Quality score section: hidden when `qualityScore === null`
- Distraction count: hidden when `distractionCount === null`
- Flow state badge: hidden when `flowState === null`
- Quality sparkline: hidden when `qualityHistory === null`
- Timer + Stop button: always shown when focused

**Net result:** Honest UI — live timer and working Stop button. No fake metrics.

#### 2.5 Files Changed

- **Modified:** `tasks2/hooks/useIssueDetail.ts` — rename `deriveFocusSession` → `buildFocusSession`, return nulls for unsupported fields
- **Modified:** `tasks2/components/detail/SidebarWorkState.tsx` — enable Stop button with mutation, null-guard quality sections, add Pause tooltip
- **Modified:** `FocusSession` type — make quality fields nullable

---

## Feature 3: Activity Tab Enrichment

### Goal

Log status/priority changes as specific timeline entries and distinguish actor types in the activity feed.

### Current State

- **Backend:** `task_update` handler performs updates but emits only generic `taskUpdated` timeline entries. No actor information on entries.
- **Frontend:** `IssueActivityTab.tsx` renders `ActivityEntry[]` with actor avatars (User/Agent/System). `timelineToActivity()` mapper sets all actors to `"system"`. Rendering is complete — just needs richer data.

### Design

#### 3.1 Backend — Change-Specific Timeline Entries

In `crates/app-core/src/handlers/tasks/crud.rs`, when `task_update` is called:

1. **Diff old vs new** for key fields: `status`, `priority`, `title`, `description`, `project_id`, `parent_id`
2. **Emit specific entries** for each changed field:
   - `entry_type`: `"taskStatusChanged"`, `"taskPriorityChanged"`, `"taskFieldUpdated"`
   - `title`: human-readable, e.g. `"Status changed from Todo to In Progress"`
   - `metadata` (JSON): `{ "field": "status", "from": "todo", "to": "in_progress" }` — structured data for rich frontend rendering
   - `source`: `"task"`
   - `entity_id`: the task ID
3. The generic `taskUpdated` entry is replaced by these specific entries (not duplicated alongside)

#### 3.2 Backend — Actor Type

Add an `actor` field to timeline entries.

**Approach:** Add an optional `actor: Option<String>` parameter to the `task_update` handler. Call sites determine the value:

| Call site | Actor value |
|-----------|-------------|
| Tauri command (user clicked in UI) | `"user"` |
| Agent pipeline (AI-driven update) | `"agent"` |
| Automated/cron/system triggers | `"system"` (default when `None`) |

This keeps changes minimal — no new `ActorContext` struct, just an optional string threaded through.

**Storage:** The timeline entry table likely has a `metadata` JSON column. Actor can be stored there, or as a new column if the schema allows direct addition (pre-release, no migration concerns per CLAUDE.md).

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
- **Modified:** `crates/app-core/src/handlers/tasks/crud.rs` — diff fields, emit specific timeline entries, accept `actor` param
- **Modified:** Timeline entry creation (wherever `TimelineEntry` is constructed) — include actor in metadata or new column
- **Possibly modified:** `crates/storage/` — add `actor` column to timeline table if not using metadata JSON

**Frontend:**
- **Modified:** `tasks2/lib/mappers.ts` — enrich `timelineToActivity()` with new entry types + actor mapping

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
