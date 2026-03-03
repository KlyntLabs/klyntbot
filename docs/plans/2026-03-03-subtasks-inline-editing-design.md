# Sub-Tasks Display + Notion-Style Inline Editing

**Date:** 2026-03-03
**Approach:** Flat Table with Virtual Nesting (Approach 1)

## Summary

Upgrade the tasks table to display sub-tasks as expandable indented rows with progress indicators, and make all table cells inline-editable (Notion-style). The backend already supports sub-tasks via `parent_id` on the `actions` table — this design bridges the gap to the frontend.

## Backend Contract Changes

### `TaskResponse` — add 3 fields

```rust
pub parent_id: Option<String>,          // null for root tasks, Some(id) for sub-tasks
pub subtask_count: u32,                 // total immediate children
pub subtask_completed_count: u32,       // completed immediate children
```

### `TaskCreateParams` — add 1 field

```rust
pub parent_id: Option<String>,          // set when creating a sub-task
```

### New command: `task_list_children`

Fetches immediate children for a parent task ID. Called lazily when a row is expanded.

### `task_list` changes

Default query adds `WHERE parent_id IS NULL` to return root tasks only. Keeps initial load fast.

### New repo method: `count_completed_children()`

In `ActionRepo`, counts children where `status = 'done'` for a given parent ID.

### Unchanged commands

`task_update`, `task_delete`, `task_toggle_complete` — already work on any action row regardless of `parent_id`.

## Data Flow & State Management

### Lazy child loading

When user expands a row, frontend calls `task_list_children(parentId)`. Children cached in `Map<string, Task[]>` — re-collapsing/re-expanding doesn't re-fetch.

### Cache invalidation

On `entity:updated` event for `entityKind: 'task'`:
1. Refetch root task list (as today)
2. Clear children cache for expanded parents — re-fetch on next render

### New state in MainApp

```typescript
expandedTasks: Set<string>         // useSetToggle — which rows are expanded
childrenCache: Map<string, Task[]> // taskId -> children array
loadingChildren: Set<string>       // taskIds currently fetching
```

### TypeScript `Task` interface additions

```typescript
parentId: string | null;
subtaskCount: number;
subtaskCompletedCount: number;
```

### Flat display array construction

```
for each root task:
  push task (depth=0)
  if expanded and children loaded:
    for each child:
      push child (depth=1)
```

Render: `displayList.map(item => <TaskRow ...depth={item.depth} />)`

## Notion-Style Inline Editing

All cells become click-to-edit:

| Cell | Editor | Behavior |
|------|--------|----------|
| Title | Click -> inline `<input>` | Blur/Enter saves, Escape cancels |
| Priority | Click -> dropdown (P1/P2/P3/P4/None) | Color-coded, instant save |
| Status | Click -> dropdown (To Do/In Progress/Done) | Instant save |
| Due Date | Click -> date picker popover | Select saves, clear button to remove |
| Tags | Click -> combobox multi-select popover | Existing tags as suggestions, type to add new |
| Project | Click -> dropdown listing all projects | Instant save |
| Area | Click -> dropdown listing all areas | Instant save |

### Editing contract

1. Click cell -> show editor
2. Selection or blur -> `task_update` mutation
3. Optimistic UI update (show new value immediately)
4. On error -> revert + show toast

### Focus management

One cell in edit mode at a time. Clicking another cell closes current editor, opens new one. Tab moves to next editable cell in same row.

### Sub-task rows

Get identical inline editors — same `TaskRow` component with `depth` prop.

### "+ Add subtask" ghost row

Below last child when expanded. Type title, press Enter to create. Inherits parent's area, project, key result.

## Visual Design

### Parent row with sub-tasks

- Chevron (▸/▼) before checkbox — toggles expand
- After title: mini progress bar (~40px, `brand` color) + fraction `2/5` in `text-muted`
- Tasks with 0 sub-tasks: no chevron, no progress

### Sub-task rows (depth=1)

- 32px indent from parent
- Tree connector lines (`├─` middle, `└─` last) in `border-border` color
- Background: `bg-surface-low`
- Title font: `text-[13px]` (vs parent `text-[14px]`)
- All columns render at normal size, fully editable

### "+ Add subtask" ghost row

- Same indent as sub-task rows
- `+` icon in `text-muted` + placeholder "Add subtask..."
- Click -> input focus, Enter creates, Escape dismisses
- Only visible when expanded

### Editable cell hover

- `bg-surface-higher` background + `border-border` outline on hover
- Cursor: pointer/text depending on cell type

### Row click behavior

- Click non-editable space -> navigate to `/task/:id`
- Click editable cell -> enter edit mode (no navigation)
- Checkbox toggles completion (parent cascade-completes children)

## Component Architecture

### Modified components

| Component | Changes |
|-----------|---------|
| `TaskRow.tsx` | Add `depth` prop, indent, tree connectors, progress indicator. Convert cells to click-to-edit. |
| `TaskTable.tsx` | Build flat display array from roots + expanded children. Manage expand/children state. Render `AddSubtaskRow`. |
| `MainApp.tsx` | Add `expandedTasks`, `childrenCache`, `loadingChildren` state. |

### New components

| Component | Purpose |
|-----------|---------|
| `InlineEditor.tsx` | Generic click-to-edit wrapper. Display mode / edit mode. Blur/escape/enter handling. |
| `PrioritySelect.tsx` | Dropdown for P1-P4 + None, color-coded. |
| `StatusSelect.tsx` | Dropdown for To Do / In Progress / Done. |
| `TagsEditor.tsx` | Combobox multi-select popover with tag suggestions. |
| `DatePicker.tsx` | Date picker popover for due dates. |
| `ProjectSelect.tsx` | Dropdown listing all projects. |
| `AreaSelect.tsx` | Dropdown listing all areas. |
| `SubtaskProgress.tsx` | Mini progress bar + fraction (e.g., `██░░░ 2/5`). |
| `AddSubtaskRow.tsx` | Ghost row with inline input for sub-task creation. |
| `TreeConnector.tsx` | `├─` / `└─` tree line rendering for indented rows. |

### Rust changes

| File | Changes |
|------|---------|
| `desktop-shared/src/commands.rs` | Add `parent_id`, `subtask_count`, `subtask_completed_count` to `TaskResponse`. Add `parent_id` to `TaskCreateParams`. |
| `desktop/src/commands/tasks.rs` | Update `action_to_task()`. Add `task_list_children`. Filter root tasks in `task_list`. |
| `storage/src/repos/action_repo.rs` | Add `count_completed_children()`. |

## Out of Scope

- Multi-level nesting (depth > 1) — backend supports it, UI stays at 1 level
- Drag-and-drop reordering of sub-tasks
- Sub-tasks in Kanban board view (follow-up)
- Bulk operations on sub-tasks
