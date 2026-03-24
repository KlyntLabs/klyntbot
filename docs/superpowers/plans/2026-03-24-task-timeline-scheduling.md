# Task Timeline Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `scheduled_start`/`scheduled_end` fields to tasks and enable drag-and-drop scheduling on the dashboard timeline.

**Architecture:** Two new nullable DateTime columns on the tasks table, threaded through the full Rust stack (storage → feature-tasks → app-core → desktop-shared) and the TypeScript frontend. The dashboard tasks column gains a "Due Today" tray for unscheduled tasks and draggable blocks for scheduled ones.

**Tech Stack:** Rust (SQLite/sqlx, chrono, serde), TypeScript/React (Tailwind v4, Tauri IPC)

**Spec:** `docs/superpowers/specs/2026-03-24-task-timeline-scheduling-design.md`

---

### Task 1: Add schema columns and storage layer fields

**Files:**
- Modify: `crates/feature-tasks/migrations/001_create_tasks.sql:10-51`
- Modify: `crates/feature-tasks/src/lib.rs:76`
- Modify: `crates/storage/src/rows/task.rs:10-54`
- Modify: `crates/storage/src/repos/task_repo/mod.rs:47-78`

- [ ] **Step 1: Add columns to migration SQL**

In `crates/feature-tasks/migrations/001_create_tasks.sql`, add after `complexity_score INTEGER` (line 50):

```sql
    scheduled_start      TEXT,
    scheduled_end        TEXT
```

- [ ] **Step 2: Bump FeatureMigration version**

In `crates/feature-tasks/src/lib.rs:76`, change `version: 1` to `version: 2`.

- [ ] **Step 3: Add fields to TaskRow**

In `crates/storage/src/rows/task.rs`, add after `objective_id` (line 54):

```rust
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
```

- [ ] **Step 4: Add fields to TaskPatch**

In `crates/storage/src/repos/task_repo/mod.rs`, add after `objective_id` (line 78):

```rust
    pub scheduled_start: Option<Option<DateTime<Utc>>>,
    pub scheduled_end: Option<Option<DateTime<Utc>>>,
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p storage 2>&1 | head -20`
Expected: Compilation errors in `core.rs` (SQL bindings mismatch) — that's Task 2.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/migrations/ crates/feature-tasks/src/lib.rs crates/storage/src/rows/task.rs crates/storage/src/repos/task_repo/mod.rs
git commit -m "feat(tasks): add scheduled_start/scheduled_end to schema and storage types"
```

---

### Task 2: Update SQL INSERT and UPDATE queries

**Files:**
- Modify: `crates/storage/src/repos/task_repo/core.rs:11-86` (INSERT)
- Modify: `crates/storage/src/repos/task_repo/core.rs:106-208` (UPDATE)

- [ ] **Step 1: Add columns to INSERT statement**

In `core.rs`, the INSERT column list (around line 25) — add after `objective_id`:

```sql
    scheduled_start, scheduled_end
```

And in the VALUES clause, add two more positional params after the current last one (`?40`):

```sql
    ?41, ?42
```

Add corresponding `.bind()` calls after the `objective_id` bind:

```rust
.bind(row.scheduled_start.map(|dt| dt.to_rfc3339()))
.bind(row.scheduled_end.map(|dt| dt.to_rfc3339()))
```

- [ ] **Step 2: Add CASE WHEN clauses to UPDATE statement**

In the UPDATE SQL (around line 140), add before `updated_at = datetime('now')`:

```sql
    scheduled_start    = CASE WHEN ?48 THEN ?49 ELSE scheduled_start END,
    scheduled_end      = CASE WHEN ?50 THEN ?51 ELSE scheduled_end END,
```

Note: adjust the parameter numbers based on the current last param number (currently `?47` for `objective_id`). The exact numbers depend on final ordering — follow the existing sequential pattern.

Add corresponding `.bind()` calls after the `objective_id` binds:

```rust
.bind(patch.scheduled_start.is_some())
.bind(patch.scheduled_start.unwrap_or_default())
.bind(patch.scheduled_end.is_some())
.bind(patch.scheduled_end.unwrap_or_default())
```

- [ ] **Step 3: Verify storage crate compiles**

Run: `cargo build -p storage 2>&1 | head -20`
Expected: Success (or errors in dependent crates, which is expected at this stage).

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/repos/task_repo/core.rs
git commit -m "feat(tasks): add scheduled_start/scheduled_end to INSERT and UPDATE SQL"
```

---

### Task 3: Update domain entity and From<TaskRow>

**Files:**
- Modify: `crates/feature-tasks/src/types/entity.rs:15-72` (Task struct)
- Modify: `crates/feature-tasks/src/types/entity.rs:143-206` (From<TaskRow>)

- [ ] **Step 1: Add fields to Task struct**

In `entity.rs`, add after `completed: bool` (line 59):

```rust
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
```

- [ ] **Step 2: Update From<TaskRow> impl**

In the `From<TaskRow>` impl, add after `completed: row.completed` (around line 193):

```rust
            scheduled_start: row.scheduled_start,
            scheduled_end: row.scheduled_end,
```

- [ ] **Step 3: Verify feature-tasks compiles**

Run: `cargo build -p feature-tasks 2>&1 | head -20`
Expected: Success.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/types/entity.rs
git commit -m "feat(tasks): add scheduled_start/scheduled_end to domain entity"
```

---

### Task 4: Update DTOs and IPC layer

**Files:**
- Modify: `crates/desktop-shared/src/commands/tasks.rs:5-36` (TaskResponse)
- Modify: `crates/desktop-shared/src/commands/tasks.rs:135-156` (TaskUpdateParams)

- [ ] **Step 1: Add fields to TaskResponse**

In `TaskResponse`, add after `updated_at` (line 35):

```rust
    pub scheduled_start: Option<String>,
    pub scheduled_end: Option<String>,
```

- [ ] **Step 2: Add fields to TaskUpdateParams**

In `TaskUpdateParams`, add after `estimated_minutes` (line 155):

```rust
    pub scheduled_start: Option<Option<String>>,
    pub scheduled_end: Option<Option<String>>,
```

- [ ] **Step 3: Verify desktop-shared compiles**

Run: `cargo build -p desktop-shared 2>&1 | head -20`
Expected: Success.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/tasks.rs
git commit -m "feat(tasks): add scheduled_start/scheduled_end to IPC DTOs"
```

---

### Task 5: Wire through app-core handlers

**Files:**
- Modify: `crates/app-core/src/handlers/tasks/converters.rs:16-51`
- Modify: `crates/app-core/src/handlers/tasks/crud.rs:101-142` (task_create)
- Modify: `crates/app-core/src/handlers/tasks/crud.rs:186-205` (task_update)

- [ ] **Step 1: Update row_to_task_response**

In `converters.rs`, add after `updated_at` mapping (line 50):

```rust
        scheduled_start: row.scheduled_start.map(|dt| dt.to_rfc3339()),
        scheduled_end: row.scheduled_end.map(|dt| dt.to_rfc3339()),
```

- [ ] **Step 2: Update task_create TaskRow initializer**

In `crud.rs`, in the `TaskRow` initializer (around line 141), add after `objective_id: None`:

```rust
        scheduled_start: None,
        scheduled_end: None,
```

- [ ] **Step 3: Update task_update TaskPatch builder**

In `crud.rs`, in the `TaskPatch` builder (around line 205), replace `..Default::default()` or add before it:

```rust
    scheduled_start: params.scheduled_start.map(|opt| opt.and_then(|d| parse_datetime(&d))),
    scheduled_end: params.scheduled_end.map(|opt| opt.and_then(|d| parse_datetime(&d))),
```

Note: `scheduled_start`/`scheduled_end` are full datetimes (not date-only like `due_date`), so use `parse_datetime` (or `DateTime::parse_from_rfc3339`). Check if `parse_datetime` exists in the crud module — if only `parse_date` exists (date-only), add a `parse_datetime` helper:

```rust
fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}
```

- [ ] **Step 4: Build and test the full workspace**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Success.

Run: `cargo nextest run -p feature-tasks 2>&1 | tail -10`
Expected: All existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/tasks/
git commit -m "feat(tasks): wire scheduled_start/scheduled_end through app-core handlers"
```

---

### Task 6: Update tasks_for_timeline SQL query

**Files:**
- Modify: `crates/storage/src/repos/task_repo/time_entries.rs:140-163`

- [ ] **Step 1: Add scheduled time overlap to WHERE clause**

In `time_entries.rs`, update the SQL WHERE clause (around line 152) to:

```sql
SELECT * FROM tasks
WHERE is_template = 0 AND (
    (due_date >= ?1 AND due_date < ?2)
    OR (created_at >= ?1 AND created_at < ?2)
    OR (completed_at >= ?1 AND completed_at < ?2)
    OR (scheduled_start < ?2 AND scheduled_end > ?1)
)
ORDER BY COALESCE(scheduled_start, due_date, created_at) ASC
```

The new `OR` clause uses half-open interval overlap: a scheduled task overlaps the day if it starts before the day ends AND ends after the day starts. Also update `ORDER BY` to prefer `scheduled_start`.

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p storage 2>&1 | head -10`
Expected: Success.

- [ ] **Step 3: Commit**

```bash
git add crates/storage/src/repos/task_repo/time_entries.rs
git commit -m "feat(tasks): include scheduled tasks in timeline query"
```

---

### Task 7: Update normalize_task for scheduled entries

**Files:**
- Modify: `crates/app-core/src/handlers/timeline.rs:348-419`

- [ ] **Step 1: Update normalize_task to handle scheduled tasks**

Replace the `TaskDue` generation block (the `if let Some(ref due) = t.due_date` block at ~line 355) with logic that handles both scheduled and due-date-only tasks:

```rust
// Scheduled task — full time block
if let (Some(ref sched_start), Some(ref sched_end)) = (&t.scheduled_start, &t.scheduled_end) {
    let start_str = sched_start.to_rfc3339();
    let end_str = sched_end.to_rfc3339();
    let duration = (*sched_end - *sched_start).num_seconds();
    out.push(TimelineEntry {
        id: format!("{}-scheduled", t.id),
        source: TimelineSource::Todo,
        entry_type: TimelineEntryType::TaskDue,
        title: t.title.clone(),
        description: t.description.clone(),
        started_at: start_str,
        ended_at: Some(end_str),
        duration_secs: Some(duration),
        entity_id: Some(t.id.clone()),
        entity_route: Some(task_route.clone()),
        color: "var(--timeline-todo)".into(),
        metadata: Some(serde_json::json!({
            "scheduled": true,
            "taskId": t.id,
            "status": t.status,
            "priority": t.priority,
        })),
    });
} else if let Some(ref due) = t.due_date {
    // Due-date-only — tray chip (no scheduled time block)
    let due_str = due.to_rfc3339();
    if due_str >= start_bound && due_str <= end_bound {
        out.push(TimelineEntry {
            id: format!("{}-due", t.id),
            source: TimelineSource::Todo,
            entry_type: TimelineEntryType::TaskDue,
            title: t.title.clone(),
            description: t.description.clone(),
            started_at: due_str,
            ended_at: None,
            duration_secs: t.estimated_minutes.map(|m| m as i64 * 60),
            entity_id: Some(t.id.clone()),
            entity_route: Some(task_route.clone()),
            color: "var(--timeline-todo)".into(),
            metadata: Some(serde_json::json!({
                "scheduled": false,
                "taskId": t.id,
                "status": t.status,
                "priority": t.priority,
            })),
        });
    }
}
```

Keep the `TaskCreated` and `TaskCompleted` entry generation unchanged.

- [ ] **Step 2: Build and run tests**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Success.

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/timeline.rs
git commit -m "feat(tasks): generate scheduled vs due-only timeline entries with metadata"
```

---

### Task 8: Update frontend TypeScript types

**Files:**
- Modify: `desktop-ui/src/shared/types/tasks.ts:5-34` (Task)
- Modify: `desktop-ui/src/shared/types/tasks.ts:132-151` (TaskUpdateParams)

- [ ] **Step 1: Add fields to Task interface**

In `tasks.ts`, add after `updatedAt?: string` (line 33):

```typescript
  scheduledStart: string | null;
  scheduledEnd: string | null;
```

- [ ] **Step 2: Add fields to TaskUpdateParams interface**

In `tasks.ts`, add after `estimatedMinutes?: number | null` (line 150):

```typescript
  scheduledStart?: string | null;
  scheduledEnd?: string | null;
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Success (or type errors in components that destructure Task — fix those in later tasks).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/types/tasks.ts
git commit -m "feat(tasks): add scheduledStart/scheduledEnd to frontend types"
```

---

### Task 9: Create useTimelineDrag hook

**Files:**
- Create: `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import type { TaskUpdateParams } from "@shared/types";
import { useCallback, useRef, useState } from "react";

const SNAP_MINUTES = 15;

interface DragState {
  mode: "move" | "resize" | "tray";
  taskId: string;
  /** Original scheduled start (minutes since midnight) */
  origStartMin: number;
  /** Original scheduled end (minutes since midnight) */
  origEndMin: number;
  /** Mouse Y at drag start */
  startY: number;
  /** Estimated minutes for tray-to-timeline drops */
  estimatedMinutes?: number;
}

interface GhostPosition {
  topMin: number;
  endMin: number;
}

function snapTo(minutes: number, grid: number): number {
  return Math.round(minutes / grid) * grid;
}

function minsToIso(date: string, minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = Math.floor(minutes % 60);
  return `${date}T${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:00Z`;
}

export function useTimelineDrag(date: string, pxPerMin: number) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const [ghost, setGhost] = useState<GhostPosition | null>(null);
  const preDragRef = useRef<{ scheduledStart: string; scheduledEnd: string } | null>(null);
  const { mutate } = useMutation<unknown, TaskUpdateParams>("task_update");

  const startMove = useCallback(
    (e: React.MouseEvent, taskId: string, startMin: number, endMin: number) => {
      e.preventDefault();
      preDragRef.current = {
        scheduledStart: minsToIso(date, startMin),
        scheduledEnd: minsToIso(date, endMin),
      };
      setDrag({ mode: "move", taskId, origStartMin: startMin, origEndMin: endMin, startY: e.clientY });
      setGhost({ topMin: startMin, endMin });
    },
    [date],
  );

  const startResize = useCallback(
    (e: React.MouseEvent, taskId: string, startMin: number, endMin: number) => {
      e.preventDefault();
      e.stopPropagation();
      preDragRef.current = {
        scheduledStart: minsToIso(date, startMin),
        scheduledEnd: minsToIso(date, endMin),
      };
      setDrag({ mode: "resize", taskId, origStartMin: startMin, origEndMin: endMin, startY: e.clientY });
      setGhost({ topMin: startMin, endMin });
    },
    [date],
  );

  const startTrayDrag = useCallback(
    (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => {
      e.preventDefault();
      setDrag({
        mode: "tray",
        taskId,
        origStartMin: 0,
        origEndMin: 0,
        startY: e.clientY,
        estimatedMinutes,
      });
      setGhost(null);
    },
    [],
  );

  const onMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!drag) return;
      const deltaMin = (e.clientY - drag.startY) / pxPerMin;

      if (drag.mode === "move") {
        const duration = drag.origEndMin - drag.origStartMin;
        const newStart = snapTo(drag.origStartMin + deltaMin, SNAP_MINUTES);
        const clampedStart = Math.max(0, Math.min(newStart, 24 * 60 - duration));
        setGhost({ topMin: clampedStart, endMin: clampedStart + duration });
      } else if (drag.mode === "resize") {
        const newEnd = snapTo(drag.origEndMin + deltaMin, SNAP_MINUTES);
        const clampedEnd = Math.max(drag.origStartMin + SNAP_MINUTES, Math.min(newEnd, 24 * 60));
        setGhost({ topMin: drag.origStartMin, endMin: clampedEnd });
      } else if (drag.mode === "tray") {
        const containerTop = drag.startY; // approximate
        const cursorMin = snapTo((e.clientY - containerTop + window.scrollY) / pxPerMin, SNAP_MINUTES);
        const duration = drag.estimatedMinutes ?? 30;
        setGhost({ topMin: Math.max(0, cursorMin), endMin: Math.max(0, cursorMin) + duration });
      }
    },
    [drag, pxPerMin],
  );

  const onMouseUp = useCallback(async () => {
    if (!drag || !ghost) {
      setDrag(null);
      setGhost(null);
      return;
    }

    const newStart = minsToIso(date, ghost.topMin);
    const newEnd = minsToIso(date, ghost.endMin);

    setDrag(null);
    setGhost(null);

    const result = await mutate({
      id: drag.taskId,
      scheduledStart: newStart,
      scheduledEnd: newEnd,
    });

    if (result != null) {
      invalidateQueries("timeline_");
      invalidateQueries("task");
    }
    // On failure, the next timeline refetch restores original position
  }, [drag, ghost, date, mutate]);

  return {
    drag,
    ghost,
    isDragging: drag != null,
    startMove,
    startResize,
    startTrayDrag,
    onMouseMove,
    onMouseUp,
  };
}
```

- [ ] **Step 2: Verify frontend builds**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Success.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts
git commit -m "feat(dashboard): add useTimelineDrag hook for drag-move and resize"
```

---

### Task 10: Create DueTodayTray component

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/DueTodayTray.tsx`

- [ ] **Step 1: Create the component**

```typescript
import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";

interface DueTodayTrayProps {
  entries: TimelineEntry[];
  onStartDrag: (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => void;
  onSelect: (entry: TimelineEntry) => void;
  selectedEntryId: string | null;
}

export function DueTodayTray({ entries, onStartDrag, onSelect, selectedEntryId }: DueTodayTrayProps) {
  if (entries.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1 px-1 py-1 border-b border-border min-h-[24px]">
      {entries.map((entry) => {
        const taskId = (entry.metadata as Record<string, unknown>)?.taskId as string;
        const estimatedMins = entry.durationSecs ? Math.round(entry.durationSecs / 60) : 30;
        return (
          <button
            key={entry.id}
            type="button"
            onMouseDown={(e) => {
              if (taskId) onStartDrag(e, taskId, estimatedMins);
            }}
            onClick={() => onSelect(entry)}
            className={cn(
              "px-1.5 py-0.5 rounded text-2xs truncate max-w-[120px] cursor-grab",
              "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/10 hover:bg-[var(--timeline-todo)]/20",
              "text-muted-foreground transition-colors",
              selectedEntryId === entry.id && "ring-1 ring-brand",
            )}
            title={entry.title}
          >
            {entry.title}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DueTodayTray.tsx
git commit -m "feat(dashboard): add DueTodayTray component for unscheduled task chips"
```

---

### Task 11: Create DraggableTaskBlock component

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/DraggableTaskBlock.tsx`

- [ ] **Step 1: Create the component**

```typescript
import { minutesSinceMidnight } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";
import type { OverlapLayout } from "../lib/timeline-utils";

interface DraggableTaskBlockProps {
  entry: TimelineEntry;
  pxPerMin: number;
  selected: boolean;
  layout?: OverlapLayout;
  isDragging: boolean;
  ghostTopMin?: number;
  ghostEndMin?: number;
  onMouseDownMove: (e: React.MouseEvent) => void;
  onMouseDownResize: (e: React.MouseEvent) => void;
  onClick: () => void;
}

export function DraggableTaskBlock({
  entry,
  pxPerMin,
  selected,
  layout,
  isDragging,
  ghostTopMin,
  ghostEndMin,
  onMouseDownMove,
  onMouseDownResize,
  onClick,
}: DraggableTaskBlockProps) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const dur = entry.durationSecs ?? 1800;
  const endMin = startMin + dur / 60;

  const top = startMin * pxPerMin;
  const height = Math.max((endMin - startMin) * pxPerMin, 20);

  // Overlap layout
  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const hasOverlap = totalCols > 1;

  const posStyle: React.CSSProperties = hasOverlap
    ? { top, left: `${(colIndex / totalCols) * 100}%`, width: `${(1 / totalCols) * 100}%`, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

  const status = (entry.metadata as Record<string, unknown>)?.status as string | undefined;

  return (
    <>
      {/* Actual block */}
      <button
        type="button"
        onMouseDown={onMouseDownMove}
        onClick={onClick}
        className={cn(
          "absolute rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden transition-colors",
          "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/15 hover:bg-[var(--timeline-todo)]/25",
          "cursor-grab active:cursor-grabbing",
          selected && "ring-1 ring-brand",
          isDragging && "opacity-40",
        )}
        style={{ ...posStyle, height }}
        title={entry.title}
      >
        <span className="text-muted-foreground truncate block">{entry.title}</span>
        {status && height > 28 && (
          <span className="text-muted-foreground text-2xs truncate block capitalize">{status}</span>
        )}
        {/* Resize handle */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: resize handle */}
        <div
          className="absolute bottom-0 left-0 right-0 h-1.5 cursor-ns-resize hover:bg-[var(--timeline-todo)]/30"
          onMouseDown={onMouseDownResize}
        />
      </button>

      {/* Ghost preview during drag */}
      {isDragging && ghostTopMin != null && ghostEndMin != null && (
        <div
          className="absolute rounded-md border-2 border-brand/50 bg-brand/10 pointer-events-none z-10"
          style={{
            ...(hasOverlap
              ? { top: ghostTopMin * pxPerMin, left: `${(colIndex / totalCols) * 100}%`, width: `${(1 / totalCols) * 100}%` }
              : { top: ghostTopMin * pxPerMin, left: 4, right: 4 }),
            height: Math.max((ghostEndMin - ghostTopMin) * pxPerMin, 20),
          }}
        />
      )}
    </>
  );
}
```

- [ ] **Step 2: Run lint**

Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -5`
Expected: No errors (only the pre-existing warning).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DraggableTaskBlock.tsx
git commit -m "feat(dashboard): add DraggableTaskBlock with drag-move, resize, and ghost preview"
```

---

### Task 12: Integrate into DayColumnsView

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/DayColumnsView.tsx`

This is the integration task — wiring `DueTodayTray`, `DraggableTaskBlock`, and `useTimelineDrag` into the existing column grid.

- [ ] **Step 1: Add imports**

At the top of `DayColumnsView.tsx`, add:

```typescript
import { DraggableTaskBlock } from "./DraggableTaskBlock";
import { DueTodayTray } from "./DueTodayTray";
import { useTimelineDrag } from "../hooks/useTimelineDrag";
```

- [ ] **Step 2: Initialize the drag hook**

Inside `DayColumnsView` component, after the existing state declarations, add:

```typescript
const {
  drag: timelineDrag,
  ghost,
  isDragging,
  startMove,
  startResize,
  startTrayDrag,
  onMouseMove: onDragMouseMove,
  onMouseUp: onDragMouseUp,
} = useTimelineDrag(date, pxPerMin);
```

- [ ] **Step 3: Add global mouse listeners for drag**

Add a `useEffect` that attaches `mousemove`/`mouseup` listeners when dragging:

```typescript
useEffect(() => {
  if (!isDragging) return;
  document.addEventListener("mousemove", onDragMouseMove);
  document.addEventListener("mouseup", onDragMouseUp);
  return () => {
    document.removeEventListener("mousemove", onDragMouseMove);
    document.removeEventListener("mouseup", onDragMouseUp);
  };
}, [isDragging, onDragMouseMove, onDragMouseUp]);
```

- [ ] **Step 4: Split task entries into scheduled and tray groups**

Inside the `columnEntries`/`columnLayouts` useMemo, or as a separate memo, split task entries:

```typescript
const { scheduledTaskEntries, trayTaskEntries } = useMemo(() => {
  const taskEntries = columnEntries.get("tasks") ?? [];
  const scheduled: TimelineEntry[] = [];
  const tray: TimelineEntry[] = [];
  for (const entry of taskEntries) {
    if (entry.entryType === "taskDue") {
      const meta = entry.metadata as Record<string, unknown> | undefined;
      if (meta?.scheduled === true) {
        scheduled.push(entry);
      } else {
        tray.push(entry);
      }
    } else {
      // taskCreated, taskCompleted — keep in timeline as-is
      scheduled.push(entry);
    }
  }
  return { scheduledTaskEntries: scheduled, trayTaskEntries: tray };
}, [columnEntries]);
```

- [ ] **Step 5: Render the tasks column with tray + draggable blocks**

In the column rendering loop (where `col.key` is checked), replace the generic tasks column rendering with:

```typescript
if (col.key === "tasks") {
  const layouts = columnLayouts.get(col.key);
  return (
    <div key={col.key} className="relative border-r border-border last:border-r-0 min-w-0 flex flex-col">
      <DueTodayTray
        entries={trayTaskEntries}
        onStartDrag={startTrayDrag}
        onSelect={handleSelectEntry}
        selectedEntryId={selectedEntry?.id ?? null}
      />
      <div className="relative flex-1">
        {scheduledTaskEntries.map((entry) => {
          const meta = entry.metadata as Record<string, unknown> | undefined;
          const taskId = (meta?.taskId as string) ?? entry.entityId;
          const isThisDragging = isDragging && timelineDrag?.taskId === taskId;

          if (meta?.scheduled === true && taskId) {
            return (
              <DraggableTaskBlock
                key={entry.id}
                entry={entry}
                pxPerMin={pxPerMin}
                selected={selectedEntry?.id === entry.id}
                layout={layouts?.get(entry.id)}
                isDragging={isThisDragging}
                ghostTopMin={isThisDragging ? ghost?.topMin : undefined}
                ghostEndMin={isThisDragging ? ghost?.endMin : undefined}
                onMouseDownMove={(e) => {
                  const startMin = minutesSinceMidnight(entry.startedAt);
                  const endMin = startMin + (entry.durationSecs ?? 1800) / 60;
                  startMove(e, taskId, startMin, endMin);
                }}
                onMouseDownResize={(e) => {
                  const startMin = minutesSinceMidnight(entry.startedAt);
                  const endMin = startMin + (entry.durationSecs ?? 1800) / 60;
                  startResize(e, taskId, startMin, endMin);
                }}
                onClick={() => handleSelectEntry(entry)}
              />
            );
          }
          // taskCreated/taskCompleted — use existing ColumnEntry
          return (
            <ColumnEntry
              key={entry.id}
              entry={entry}
              column={col}
              pxPerMin={pxPerMin}
              selected={selectedEntry?.id === entry.id}
              onClick={() => handleSelectEntry(entry)}
              layout={layouts?.get(entry.id)}
            />
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 6: Run lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build 2>&1 | tail -10`
Expected: Success.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DayColumnsView.tsx
git commit -m "feat(dashboard): integrate DueTodayTray and DraggableTaskBlock into tasks column"
```

---

### Task 13: End-to-end manual testing

**Files:** None (testing only)

- [ ] **Step 1: Rebuild the Rust backend**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Success with zero warnings (clippy clean).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10`
Expected: Zero warnings.

- [ ] **Step 3: Run all Rust tests**

Run: `cargo nextest run --workspace 2>&1 | tail -15`
Expected: All tests pass.

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 5: Run the full app**

Start the dev server and Tauri app:
```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Verify:
1. Dashboard tasks column shows a "Due Today" tray at the top with due-date-only tasks as chips
2. Tasks with no due date and no scheduled times do NOT appear on the timeline
3. Scheduled tasks (if any) appear as colored blocks at the correct time
4. Drag a tray chip onto the timeline — it should create a scheduled block
5. Drag a scheduled block up/down — it moves, snapped to 15-min grid
6. Drag the bottom edge of a scheduled block — it resizes
7. After drop, the task's `scheduledStart`/`scheduledEnd` should be updated (check via the detail sidebar or task list)
