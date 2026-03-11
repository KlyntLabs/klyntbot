# Column Visibility & Agentic Fields Design

## Goal

Add a customizable column visibility system to the Tasks table and display all missing agentic fields in the Task detail panel.

## Context

The Tasks table currently shows: checkbox, Task, Project, Area (conditional on "All" tab), Priority, Status, Due Date, Tags. The Task type has 8 additional agentic fields (taskType, energyLevel, estimatedMinutes, actualMinutes, executionState, complexityScore, totalTrackedSecs, focusedAt) that aren't rendered anywhere. The detail panel also lacks these fields.

---

## 1. Column Visibility System

### Architecture

A `useColumnVisibility` custom hook backed by `localStorage` manages which columns are shown. A `ColumnPicker` dropdown in the Toolbar lets users toggle columns on/off.

### Column Registry

```ts
type ColumnId =
  | "project" | "area" | "priority" | "status" | "dueDate" | "tags"
  | "taskType" | "energyLevel" | "estimatedMinutes" | "actualMinutes"
  | "executionState" | "complexityScore" | "totalTrackedSecs" | "focusedAt";
```

### Default Visibility

- **Default visible:** project, priority, status, dueDate, tags
- **Area:** always visible when "All" tab is active (not user-toggleable — driven by tab)
- **Default hidden:** taskType, energyLevel, estimatedMinutes, actualMinutes, executionState, complexityScore, totalTrackedSecs, focusedAt

### Hook: `useColumnVisibility`

- **Storage key:** `klyntbot:tasks:visibleColumns`
- **API:** `{ visibleColumns: Set<ColumnId>, toggleColumn(id: ColumnId): void, resetToDefaults(): void, isVisible(id: ColumnId): boolean }`
- **Persistence:** Reads/writes `ColumnId[]` JSON to localStorage. Falls back to defaults if missing/corrupt.

### ColumnPicker Component

- **Trigger:** Icon button using `Columns3` icon (already imported in Toolbar), placed next to the view mode toggle group on the right side.
- **Dropdown:** `glass-panel` positioned below the button. Contains grouped checkboxes:
  - **Core:** Project, Priority, Status, Due Date, Tags
  - **Agentic:** Task Type, Energy Level, Est. Minutes, Actual Minutes, Execution State, Complexity, Time Tracked, Focused At
- **Dismiss:** Click outside closes dropdown.
- **Footer:** "Reset to defaults" link at bottom.

### Data Flow

`TasksPage` owns `useColumnVisibility` → passes `visibleColumns` + `toggleColumn` + `resetToDefaults` to `Toolbar` (for ColumnPicker) and passes `visibleColumns` to `TaskTable` (via existing `TaskTableContext`).

`TaskTable` uses `visibleColumns` to conditionally render `<th>` headers. `TaskRow` uses it to conditionally render `<td>` cells.

---

## 2. New Table Column Renderers

| Column | Display | Editable | Editor |
|--------|---------|----------|--------|
| Task Type | Pill badge (Manual/Agentic/Hybrid) | Yes | InlineSelect |
| Energy Level | Color-coded badge (Low=green, Medium=yellow, High=orange, Deep=red) | Yes | InlineSelect |
| Estimated Min | `30m` or `—` | Yes | InlineNumber (new) |
| Actual Min | `45m` or `—` | No | styled span |
| Execution State | Color badge (idle=dim, running=brand, failed=destructive) | No | styled badge |
| Complexity | Score `0–100` | No | styled span |
| Time Tracked | Formatted `1h 23m` from totalTrackedSecs | No | styled span |
| Focused At | Relative time `2h ago` or `—` | No | styled span |

### InlineNumber Component

New `editors/InlineNumber.tsx`: click to edit, shows a small `<input type="number">` with min/max/step. Enter saves, Escape cancels, blur saves. Same pattern as InlineTextEditor.

### Formatting Utilities

- `formatDuration(secs: number): string` — e.g. `1h 23m`, `45m`, `< 1m`
- `formatRelativeTime(iso: string): string` — e.g. `2h ago`, `3d ago`

These go in `desktop-ui/src/lib/dates.ts` (existing file).

---

## 3. Detail Panel Agentic Section

### Placement

New collapsible section between Description and Custom Fields.

### Layout

Uses the same `grid-cols-[90px_1fr]` metadata grid as the existing fields above.

### Fields

| Field | Widget | Editable |
|-------|--------|----------|
| Type | Pill selector (Manual / Agentic / Hybrid) | Yes |
| Execution | Badge with color | No |
| Energy | Pill selector (Low / Medium / High / Deep) | Yes |
| Est / Actual | Number input + read-only display, side-by-side | Est: yes, Act: no |
| Tracked | Formatted duration | No |
| Complexity | Score / 100 | No |
| Focused | Relative timestamp | No |
| Acceptance Criteria | Full-width multi-line text editor (same pattern as Description) | Yes |

### Collapse Behavior

- Collapsible via a section header toggle ("Agentic ▸")
- Defaults to expanded
- Collapse state persisted in `localStorage` key `klyntbot:tasks:agenticExpanded`

---

## Files

| Action | Path |
|--------|------|
| Create | `desktop-ui/src/features/tasks/hooks/useColumnVisibility.ts` |
| Create | `desktop-ui/src/features/tasks/components/ColumnPicker.tsx` |
| Create | `desktop-ui/src/features/tasks/components/editors/InlineNumber.tsx` |
| Modify | `desktop-ui/src/features/tasks/components/Toolbar.tsx` |
| Modify | `desktop-ui/src/features/tasks/components/TaskTable.tsx` |
| Modify | `desktop-ui/src/features/tasks/components/TaskTableContext.tsx` |
| Modify | `desktop-ui/src/features/tasks/components/TaskRow.tsx` |
| Modify | `desktop-ui/src/features/tasks/components/TaskDetailPanel.tsx` |
| Modify | `desktop-ui/src/lib/dates.ts` |
