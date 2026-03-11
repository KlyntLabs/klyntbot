# Column Visibility & Agentic Fields Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add customizable column visibility to the Tasks table and display all agentic fields in the detail panel.

**Architecture:** localStorage-backed column visibility hook + ColumnPicker dropdown in Toolbar + conditional column rendering in TaskTable/TaskRow + new Agentic section in TaskDetailPanel.

**Tech Stack:** React 19, TypeScript, Tailwind v4, CSS tokens, lucide-react icons

---

## Chunk 1: Column Visibility Hook & Picker

### Task 1: useColumnVisibility hook

**Files:**
- Create: `desktop-ui/src/features/tasks/hooks/useColumnVisibility.ts`

- [ ] **Step 1: Create the hook**

```ts
import { useCallback, useMemo, useSyncExternalStore } from "react";

export type ColumnId =
  | "project" | "area" | "priority" | "status" | "dueDate" | "tags"
  | "taskType" | "energyLevel" | "estimatedMinutes" | "actualMinutes"
  | "executionState" | "complexityScore" | "totalTrackedSecs" | "focusedAt";

export interface ColumnDef {
  id: ColumnId;
  label: string;
  group: "core" | "agentic";
}

export const ALL_COLUMNS: ColumnDef[] = [
  { id: "project", label: "Project", group: "core" },
  { id: "priority", label: "Priority", group: "core" },
  { id: "status", label: "Status", group: "core" },
  { id: "dueDate", label: "Due Date", group: "core" },
  { id: "tags", label: "Tags", group: "core" },
  { id: "taskType", label: "Task Type", group: "agentic" },
  { id: "energyLevel", label: "Energy Level", group: "agentic" },
  { id: "estimatedMinutes", label: "Est. Minutes", group: "agentic" },
  { id: "actualMinutes", label: "Actual Minutes", group: "agentic" },
  { id: "executionState", label: "Execution State", group: "agentic" },
  { id: "complexityScore", label: "Complexity", group: "agentic" },
  { id: "totalTrackedSecs", label: "Time Tracked", group: "agentic" },
  { id: "focusedAt", label: "Focused At", group: "agentic" },
];

const STORAGE_KEY = "klyntbot:tasks:visibleColumns";
const DEFAULT_VISIBLE: ColumnId[] = ["project", "priority", "status", "dueDate", "tags"];

function getStored(): ColumnId[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_VISIBLE;
    return JSON.parse(raw) as ColumnId[];
  } catch {
    return DEFAULT_VISIBLE;
  }
}

function setStored(cols: ColumnId[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(cols));
  window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
}

function subscribe(cb: () => void) {
  const handler = (e: StorageEvent) => { if (e.key === STORAGE_KEY) cb(); };
  window.addEventListener("storage", handler);
  return () => window.removeEventListener("storage", handler);
}

export function useColumnVisibility() {
  const stored = useSyncExternalStore(subscribe, getStored, () => DEFAULT_VISIBLE);
  const visibleSet = useMemo(() => new Set(stored), [stored]);

  const toggleColumn = useCallback((id: ColumnId) => {
    const current = getStored();
    const next = current.includes(id) ? current.filter((c) => c !== id) : [...current, id];
    setStored(next);
  }, []);

  const resetToDefaults = useCallback(() => setStored(DEFAULT_VISIBLE), []);
  const isVisible = useCallback((id: ColumnId) => visibleSet.has(id), [visibleSet]);

  return { visibleColumns: visibleSet, toggleColumn, resetToDefaults, isVisible };
}
```

- [ ] **Step 2: Verify dev server compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -20`

### Task 2: ColumnPicker dropdown

**Files:**
- Create: `desktop-ui/src/features/tasks/components/ColumnPicker.tsx`

- [ ] **Step 1: Create the picker component**

```tsx
import { useEffect, useRef, useState } from "react";
import { Columns3 } from "lucide-react";
import { ALL_COLUMNS, type ColumnId } from "../hooks/useColumnVisibility";

interface ColumnPickerProps {
  visibleColumns: Set<ColumnId>;
  onToggle: (id: ColumnId) => void;
  onReset: () => void;
}

export function ColumnPicker({ visibleColumns, onToggle, onReset }: ColumnPickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const coreColumns = ALL_COLUMNS.filter((c) => c.group === "core");
  const agenticColumns = ALL_COLUMNS.filter((c) => c.group === "agentic");

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        aria-label="Toggle columns"
        className={`p-1.5 rounded-md transition-all ${
          open ? "glass-button-active text-brand" : "text-muted hover:text-secondary"
        }`}
      >
        <Columns3 className="w-[14px] h-[14px]" strokeWidth={1.5} />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1.5 w-52 glass-panel rounded-xl p-2 z-50 shadow-lg">
          <p className="text-[10px] text-dim font-light uppercase tracking-wider px-2 py-1">Core</p>
          {coreColumns.map((col) => (
            <label
              key={col.id}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.06] cursor-pointer"
            >
              <input
                type="checkbox"
                checked={visibleColumns.has(col.id)}
                onChange={() => onToggle(col.id)}
                className="accent-[var(--brand)] w-3 h-3"
              />
              <span className="text-[11px] font-light text-secondary">{col.label}</span>
            </label>
          ))}

          <div className="border-t border-white/[0.06] my-1.5" />

          <p className="text-[10px] text-dim font-light uppercase tracking-wider px-2 py-1">Agentic</p>
          {agenticColumns.map((col) => (
            <label
              key={col.id}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.06] cursor-pointer"
            >
              <input
                type="checkbox"
                checked={visibleColumns.has(col.id)}
                onChange={() => onToggle(col.id)}
                className="accent-[var(--brand)] w-3 h-3"
              />
              <span className="text-[11px] font-light text-secondary">{col.label}</span>
            </label>
          ))}

          <div className="border-t border-white/[0.06] my-1.5" />

          <button
            type="button"
            onClick={onReset}
            className="w-full text-[10px] font-light text-muted hover:text-brand px-2 py-1 text-left"
          >
            Reset to defaults
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Wire into Toolbar**

In `Toolbar.tsx`, add `ColumnPicker` next to the view mode toggle group. Add props for column visibility state.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks/hooks/useColumnVisibility.ts desktop-ui/src/features/tasks/components/ColumnPicker.tsx desktop-ui/src/features/tasks/components/Toolbar.tsx
git commit -m "feat(tasks-ui): add column visibility hook and picker component"
```

## Chunk 2: Table Column Rendering

### Task 3: Formatting utilities

**Files:**
- Modify: `desktop-ui/src/lib/dates.ts`

- [ ] **Step 1: Add formatDuration and formatRelativeTime**

```ts
export function formatDuration(totalSecs: number): string {
  if (totalSecs < 60) return "< 1m";
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  if (h > 0) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  return `${m}m`;
}

export function formatMinutes(mins: number): string {
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}
```

### Task 4: InlineNumber editor

**Files:**
- Create: `desktop-ui/src/features/tasks/components/editors/InlineNumber.tsx`

- [ ] **Step 1: Create InlineNumber component**

Small click-to-edit number input. Same interaction pattern as InlineTextEditor.

### Task 5: Wire columns into TaskTable + TaskRow

**Files:**
- Modify: `desktop-ui/src/features/tasks/components/TaskTableContext.tsx` — add `visibleColumns` to context
- Modify: `desktop-ui/src/features/tasks/components/TaskTable.tsx` — conditional `<th>` headers
- Modify: `desktop-ui/src/features/tasks/components/TaskRow.tsx` — conditional `<td>` cells with new renderers
- Modify: `desktop-ui/src/features/tasks/pages/TasksPage.tsx` — own useColumnVisibility, pass down

- [ ] **Step 1: Add visibleColumns to TaskTableContext**
- [ ] **Step 2: Update TaskTable headers to be conditional**
- [ ] **Step 3: Update RootTaskRow and SubtaskRow with conditional cells and new column renderers**
- [ ] **Step 4: Wire useColumnVisibility in TasksPage, pass to Toolbar and TaskTable**
- [ ] **Step 5: Verify build compiles**
- [ ] **Step 6: Commit**

```bash
git commit -m "feat(tasks-ui): conditional column rendering with agentic field columns"
```

## Chunk 3: Detail Panel Agentic Section

### Task 6: Add Agentic section to TaskDetailPanel

**Files:**
- Modify: `desktop-ui/src/features/tasks/components/TaskDetailPanel.tsx`

- [ ] **Step 1: Add collapsible Agentic section**

Between Description and Custom Fields. Contains:
- Task Type pill selector
- Execution State read-only badge
- Energy Level pill selector
- Estimated / Actual minutes (input + read-only)
- Time Tracked read-only
- Complexity Score read-only
- Focused At read-only
- Acceptance Criteria multi-line editor

- [ ] **Step 2: Verify build compiles**
- [ ] **Step 3: Commit**

```bash
git commit -m "feat(tasks-ui): add agentic fields section to task detail panel"
```
