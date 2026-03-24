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
- Add `scheduled_start TEXT` and `scheduled_end TEXT` nullable columns to `tasks` table
- Add corresponding fields to `TaskRow` and `TaskPatch`

**Feature-tasks** (`crates/feature-tasks`):
- Add `scheduled_start: Option<DateTime<Utc>>` and `scheduled_end: Option<DateTime<Utc>>` to `Task` domain entity
- Update `From<TaskRow>` conversion

**Desktop-shared DTOs** (`crates/desktop-shared`):
- Add `scheduled_start: Option<Option<String>>` and `scheduled_end: Option<Option<String>>` to `TaskUpdateParams` (double-`Option` pattern: outer `None` = don't change, inner `None` = set to null)
- Add `scheduled_start: Option<String>` and `scheduled_end: Option<String>` to `TaskResponse`

**Frontend types** (`desktop-ui/src/shared/types/tasks.ts`):
- Add `scheduledStart?: string` and `scheduledEnd?: string` to `Task`
- Add `scheduledStart?: string | null` and `scheduledEnd?: string | null` to `TaskUpdateParams`

### Timeline Entry Generation

Update `normalize_task()` in `crates/app-core/src/handlers/timeline.rs`:

- If a task has `scheduled_start` + `scheduled_end`:
  - Generate a `TaskDue` entry with `started_at = scheduled_start`, `ended_at = scheduled_end`, `duration_secs` computed from the range
  - Add `"scheduled": true` to the entry's metadata
- If a task has only `due_date` (no scheduled times):
  - Generate a `TaskDue` entry as today (same as current behavior)
  - Add `"scheduled": false` to the entry's metadata
- Filter: include tasks where `due_date` falls on the queried day OR `scheduled_start`/`scheduled_end` overlaps the queried day

No new Tauri commands needed — `task_update` already supports arbitrary field updates.

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

**New components:**

1. **`DueTodayTray`** — renders inside the tasks column header area, above the time grid. Shows compact pills for due-date-only tasks using `--timeline-todo` color. Chips are draggable onto the timeline grid.

2. **`DraggableTaskBlock`** — replaces the current static `ColumnEntry` rendering for scheduled tasks. Handles drag-move (body) and resize (bottom edge).

3. **`useTimelineDrag` hook** — shared drag state machine: converts pixel offsets to minute deltas, snaps to 15-minute grid, renders ghost preview during drag, calls `task_update` mutation on drop.

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
- Scheduled blocks: `--timeline-todo` color with border-left accent, visible resize handle (3 horizontal lines or subtle bar at bottom)
- Due-today chips: compact pills with task title (truncated), `--timeline-todo` color
- During drag: original block `opacity-50`, ghost gets `ring-2 ring-brand`

**Optimistic updates:**
- UI updates position immediately on drop
- If `task_update` fails, revert to original position

### Overlap Layout

Scheduled task blocks use the existing `computeOverlapLayout()` utility from `timeline-utils.ts`. Multiple tasks at the same time are laid out side-by-side (Google Calendar style), already implemented.

### What This Does NOT Include

- Recurring task scheduling
- Multi-day task blocks spanning across days
- Drag between columns (e.g., tasks → calendar)
- Week/month view scheduling
- Undo/redo for drag operations

These can be added incrementally later.

## Files to Modify

### Rust (backend)
- `crates/storage/src/rows/task.rs` — add fields to `TaskRow`, `TaskPatch`
- `crates/storage/src/repos/task.rs` — update queries for new columns
- `crates/feature-tasks/src/types/entity.rs` — add fields to `Task`
- `crates/feature-tasks/src/migrations.rs` — add migration for new columns
- `crates/app-core/src/handlers/timeline.rs` — update `normalize_task()`
- `crates/desktop-shared/src/commands/tasks.rs` — update `TaskUpdateParams`, `TaskResponse`

### TypeScript (frontend)
- `desktop-ui/src/shared/types/tasks.ts` — add fields to `Task`, `TaskUpdateParams`
- `desktop-ui/src/features/dashboard/components/DayColumnsView.tsx` — integrate `DueTodayTray` and `DraggableTaskBlock`
- `desktop-ui/src/features/dashboard/components/DueTodayTray.tsx` — new component
- `desktop-ui/src/features/dashboard/components/DraggableTaskBlock.tsx` — new component
- `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts` — new hook
