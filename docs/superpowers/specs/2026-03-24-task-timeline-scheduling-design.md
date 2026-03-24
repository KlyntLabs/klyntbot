# Task Timeline Scheduling — Design Spec

**Date:** 2026-03-24
**Status:** Approved
**Scope:** Add scheduled time ranges to tasks and enable drag-and-drop scheduling on the dashboard timeline

## Problem

The dashboard timeline currently shows tasks only as point-in-time markers at their creation time. Tasks with due dates appear as small blocks using `estimated_minutes` for synthetic height. There is no way to schedule a task into a specific time slot, and no drag-and-drop interaction to reposition or resize task blocks.

## Design

### Data Model

Add two new nullable fields to the `Task` entity:

```
scheduled_start: Option<DateTime<Utc>>
scheduled_end: Option<DateTime<Utc>>
```

These represent "when I plan to work on this task" — separate from `due_date` which is the deadline.

**Display logic on the dashboard timeline:**

| Task state | Timeline behavior |
|---|---|
| `scheduled_start` + `scheduled_end` set | Full draggable block positioned at the scheduled time |
| `due_date` only (no scheduled times) | Small chip in the "Due Today" tray at the top of the tasks column |
| Neither `due_date` nor scheduled times | Not shown on the timeline |

### Schema Changes

**Storage** (`crates/storage`):
- Add `scheduled_start TEXT` and `scheduled_end TEXT` nullable columns to the `tasks` table — edit the existing `001_create_tasks.sql` migration in-place and bump the `FeatureMigration` version (pre-release, no backwards-compat migrations needed per CLAUDE.md)
- Add `scheduled_start: Option<DateTime<Utc>>` and `scheduled_end: Option<DateTime<Utc>>` to `TaskRow`
- Add `scheduled_start: Option<Option<DateTime<Utc>>>` and `scheduled_end: Option<Option<DateTime<Utc>>>` to `TaskPatch` (matching the existing double-`Option<DateTime>` pattern used by `due_date`)

**Feature-tasks** (`crates/feature-tasks`):
- Add `scheduled_start: Option<DateTime<Utc>>` and `scheduled_end: Option<DateTime<Utc>>` to `Task` domain entity
- Update `From<TaskRow>` conversion
- Update `FeatureMigration` version number in `lib.rs`

**Desktop-shared DTOs** (`crates/desktop-shared`):
- Add `scheduled_start: Option<Option<String>>` and `scheduled_end: Option<Option<String>>` to `TaskUpdateParams` (double-`Option` pattern: outer `None` = don't change, inner `None` = set to null)
- Add `scheduled_start: Option<String>` and `scheduled_end: Option<String>` to `TaskResponse`

**Frontend types** (`desktop-ui/src/shared/types/tasks.ts`):
- Add `scheduledStart: string | null` and `scheduledEnd: string | null` to `Task` (matching the `dueDate: string | null` pattern)
- Add `scheduledStart?: string | null` and `scheduledEnd?: string | null` to `TaskUpdateParams`

### Update Path (task_update chain)

The existing `task_update` Tauri command handles the full chain — no new commands needed. But the new fields must be threaded through every layer:

1. **`TaskUpdateParams`** (desktop-shared DTO) — add fields (described above)
2. **`AppCore::task_update`** (`crates/app-core/src/handlers/tasks/crud.rs`) — map `TaskUpdateParams.scheduled_start`/`scheduled_end` → `TaskPatch.scheduled_start`/`scheduled_end`, following the existing `due_date` mapping pattern with `parse_optional_datetime()`
3. **`TaskPatch`** (`crates/storage/src/repos/task_repo/mod.rs`) — add fields as `Option<Option<DateTime<Utc>>>`
4. **SQL UPDATE** (`crates/storage/src/repos/task_repo/core.rs`) — add `CASE WHEN` clauses for `scheduled_start` and `scheduled_end`, following the existing `due_date` pattern with boolean flag + value binding
5. **`row_to_task_response`** (`crates/app-core/src/handlers/tasks/converters.rs`) — map `TaskRow.scheduled_start`/`scheduled_end` → `TaskResponse.scheduled_start`/`scheduled_end` (format as RFC3339 strings)
6. **`task_create`** (`crates/app-core/src/handlers/tasks/crud.rs`) — set `scheduled_start: None` and `scheduled_end: None` in the `TaskRow` initializer to avoid compile errors

### Timeline Entry Generation

**SQL query update** (`crates/storage/src/repos/task_repo/time_entries.rs`):
- Update the `tasks_for_timeline` SQL `WHERE` clause to also match tasks whose `scheduled_start`/`scheduled_end` overlaps the queried day: `OR (scheduled_start < ?end_bound AND scheduled_end > ?start_bound)`. Use half-open interval `[start, end)` consistent with existing range queries.

**`normalize_task()` update** (`crates/app-core/src/handlers/timeline.rs`):
- If a task has `scheduled_start` + `scheduled_end`:
  - Generate a `TaskDue` entry with `started_at = scheduled_start`, `ended_at = scheduled_end`, `duration_secs` computed from the range
  - Add `"scheduled": true` and `"taskId": "<id>"` to the entry's metadata
- If a task has only `due_date` (no scheduled times):
  - Generate a `TaskDue` entry as today (same as current behavior)
  - Add `"scheduled": false` and `"taskId": "<id>"` to the entry's metadata

### Frontend Architecture

**Tasks column layout:**

```
┌─────────────── Tasks Column ───────────────┐
│ ┌─ Due Today Tray ───────────────────────┐ │
│ │ [Task A chip] [Task B chip]            │ │  ← due_date only, no scheduled times
│ └────────────────────────────────────────┘ │
│                                            │
│  9 AM ─────────────────────────────────    │
│        ┌──────────────────────┐            │
│        │ Task C (9:00–10:00)  │ ← draggable│
│        │                  ═══ │ ← resize   │
│        └──────────────────────┘            │
│ 10 AM ─────────────────────────────────    │
│        ┌─────────┐┌──────────┐             │
│        │ Task D  ││ Task E   │ ← overlap   │
│        └─────────┘└──────────┘             │
│ 11 AM ─────────────────────────────────    │
└────────────────────────────────────────────┘
```

**Column layout change in `DayColumnsView`:**

The tasks column currently renders as a single `<div className="relative">` containing positioned entries. To support the tray:
- The tasks column becomes a flex column: a fixed-height `DueTodayTray` at the top + a `relative` positioned container for the time grid entries below
- Other columns are unaffected — they keep their existing single `<div className="relative">` structure
- The tray renders inside the column's sticky header row (alongside the column label), so it scrolls with the header and doesn't affect the time grid alignment

**Routing entries to the correct renderer:**

In `DayColumnsView`, when rendering the tasks column:
1. Split `columnEntries.get("tasks")` into two groups based on `entry.metadata.scheduled`:
   - `scheduled === true` → render as `DraggableTaskBlock` (positioned in the time grid)
   - `scheduled === false` → render as chips in `DueTodayTray`
2. The `TaskCreated` and `TaskCompleted` entry types continue using the existing `ColumnEntry` renderer unchanged

**New components:**

1. **`DueTodayTray`** — renders inside the tasks column header area. Shows compact pills for due-date-only tasks using `--timeline-todo` color. Chips are draggable onto the timeline grid.

2. **`DraggableTaskBlock`** — replaces `ColumnEntry` for scheduled tasks. Handles drag-move (body) and resize (bottom edge). Receives overlap layout from `computeOverlapLayout()` via the same mechanism as `ColumnEntry` (layout map computed in `DayColumnsView` and passed as prop).

3. **`useTimelineDrag` hook** — shared drag state machine: converts pixel offsets to minute deltas, snaps to 15-minute grid, renders ghost preview during drag, calls `task_update` mutation on drop. Stores pre-drag `{ scheduledStart, scheduledEnd }` in a ref for rollback on failure.

### Interaction Details

**Drag-move (body):**
- Cursor: `grab` on hover → `grabbing` while dragging
- Semi-transparent ghost follows cursor, snapped to 15-minute grid
- Original block stays at reduced opacity until drop
- On drop: `task_update` with new `scheduled_start` + `scheduled_end` (preserving duration)
- Dragging outside column bounds cancels the operation

**Bottom-edge resize:**
- 6px hit zone at the bottom of each scheduled block
- Cursor: `ns-resize`
- Minimum duration: 15 minutes
- Ghost extends/shrinks in real-time, snapped to 15-min grid
- On release: `task_update` with new `scheduled_end`

**Tray chip → timeline drag:**
- Chips in the Due Today tray are draggable
- When dragged over the time grid, a ghost block appears (height based on `estimated_minutes`, default 30min if no estimate)
- On drop: `task_update` with `scheduled_start` = drop position, `scheduled_end` = start + duration

**Visual feedback:**
- Scheduled blocks: `--timeline-todo` color with border-left accent, visible resize handle (subtle bar at bottom)
- Due-today chips: compact pills with task title (truncated), `--timeline-todo` color
- During drag: original block `opacity-50`, ghost gets `ring-2 ring-brand`

**Optimistic updates:**
- `useTimelineDrag` stores pre-drag state in a ref before starting the drag
- UI updates position immediately on drop
- If `task_update` fails, revert to the stored pre-drag position

### Overlap Layout

Scheduled task blocks use the existing `computeOverlapLayout()` utility from `timeline-utils.ts`. The layout map is computed in `DayColumnsView` (same as for other columns) and passed to each `DraggableTaskBlock` as a `layout` prop. Multiple tasks at the same time are laid out side-by-side (Google Calendar style).

### What This Does NOT Include

- Recurring task scheduling
- Multi-day task blocks spanning across days
- Drag between columns (e.g., tasks → calendar)
- Week/month view scheduling
- Undo/redo for drag operations

These can be added incrementally later.

## Files to Modify

### Rust (backend)
- `crates/feature-tasks/migrations/001_create_tasks.sql` — add `scheduled_start` and `scheduled_end` columns to CREATE TABLE
- `crates/feature-tasks/src/lib.rs` — bump `FeatureMigration` version
- `crates/feature-tasks/src/types/entity.rs` — add fields to `Task`, update `From<TaskRow>`
- `crates/storage/src/rows/task.rs` — add fields to `TaskRow`
- `crates/storage/src/repos/task_repo/mod.rs` — add fields to `TaskPatch`
- `crates/storage/src/repos/task_repo/core.rs` — add `CASE WHEN` clauses to UPDATE SQL, add columns to INSERT
- `crates/storage/src/repos/task_repo/time_entries.rs` — update `tasks_for_timeline` SQL WHERE clause
- `crates/app-core/src/handlers/timeline.rs` — update `normalize_task()` to use scheduled times and add metadata flags
- `crates/app-core/src/handlers/tasks/crud.rs` — map new fields in `task_update`, set `None` in `task_create`
- `crates/app-core/src/handlers/tasks/converters.rs` — map new fields in `row_to_task_response`
- `crates/desktop-shared/src/commands/tasks.rs` — add fields to `TaskUpdateParams`, `TaskResponse`

### TypeScript (frontend)
- `desktop-ui/src/shared/types/tasks.ts` — add fields to `Task`, `TaskUpdateParams`
- `desktop-ui/src/features/dashboard/components/DayColumnsView.tsx` — split tasks entries, integrate `DueTodayTray` and `DraggableTaskBlock`, adjust column layout for tasks
- `desktop-ui/src/features/dashboard/components/DueTodayTray.tsx` — new component
- `desktop-ui/src/features/dashboard/components/DraggableTaskBlock.tsx` — new component
- `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts` — new hook
