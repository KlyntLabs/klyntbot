# Task2 Remaining Work (Option B) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete three Task2 features: dynamic status workflow, focus session controls, and activity tab enrichment.

**Architecture:** Feature-by-feature full-stack approach. Feature 1 (status workflow) is frontend-only since the backend exists. Feature 2 (focus sessions) is mostly frontend wiring. Feature 3 (activity tab) requires backend changes (new DomainEvent variants + timeline normalization) plus frontend mapper updates.

**Tech Stack:** Rust (backend: app-core, bus, desktop-shared, desktop), TypeScript/React (frontend: tasks2 feature), Tauri IPC, Tanstack Query, SQLite.

**Spec:** `docs/superpowers/specs/2026-03-15-task2-remaining-b-design.md`

---

## Chunk 1: Feature 1 — Status Workflow Integration

### File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Create | `desktop-ui/src/features/tasks2/contexts/StatusWorkflowContext.tsx` | Context provider + `useStatusWorkflow()` hook |
| Modify | `desktop-ui/src/features/tasks2/lib/status-icons.tsx` | Refactor to icon registry + `matchIcon()` export, remove `allStatus` array |
| Modify | `desktop-ui/src/features/tasks2/lib/status-utils.tsx` | Update `renderStatusIcon` to use name-based matching |
| Modify | `desktop-ui/src/features/tasks2/lib/mappers.ts` | `resolveStatus()` accepts `StatusLabel[]` param; ripple to `taskToIssue`, `taskToDetailTask`, `taskToSubIssue`; replace `statusToBackend()` |
| Modify | `desktop-ui/src/features/tasks2/components/StatusSelector.tsx` | Use `useStatusWorkflow()` |
| Modify | `desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx` | Use `useStatusWorkflow()` |
| Modify | `desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx` | Use `useStatusWorkflow()` |
| Modify | `desktop-ui/src/features/tasks2/components/IssueBoard.tsx` | Use `useStatusWorkflow()` for columns |
| Modify | `desktop-ui/src/features/tasks2/components/AllIssues.tsx` | Use `useStatusWorkflow()` |
| Modify | `desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx` | Use `useStatusWorkflow()` |
| Modify | `desktop-ui/src/features/tasks2/components/Filter.tsx` | Use `useStatusWorkflow()` |
| Modify | Tasks2 layout/root | Wrap with `StatusWorkflowProvider` |

---

### Task 1: Refactor `status-icons.tsx` to icon registry + `matchIcon()`

**Files:**
- Modify: `desktop-ui/src/features/tasks2/lib/status-icons.tsx`

- [ ] **Step 1: Read the current file to confirm exact structure**

Read `desktop-ui/src/features/tasks2/lib/status-icons.tsx` and note:
- The `Status` interface (lines 3-10) — has `id`, `name`, `color`, `icon`, `backendStatus`
- 6 icon components (lines 12-118): `BacklogIcon`, `PausedIcon`, `ToDoIcon`, `InProgressIcon`, `TechnicalReviewIcon`, `CompletedIcon`
- The `status` array (lines 120-127): 6 hardcoded entries
- `statusById` record (line 129)
- `StatusIcon` component (lines 131-136)

- [ ] **Step 2: Extend `Status` interface with optional `statusGroup`**

Add `statusGroup?: StatusGroup` to the `Status` interface. Import `StatusGroup` from `@shared/types/common`.

```typescript
import type { StatusGroup } from "@shared/types/common";

export interface Status {
  id: string;
  name: string;
  color: string;
  icon: React.FC<{ className?: string }>;
  backendStatus: string;
  statusGroup?: StatusGroup;
}
```

- [ ] **Step 3: Add `matchIcon()` function**

Add after the icon component definitions, before the `status` array:

```typescript
const ICON_NAME_MAP: Record<string, React.FC<{ className?: string }>> = {
  backlog: BacklogIcon,
  todo: ToDoIcon,
  "to do": ToDoIcon,
  "in progress": InProgressIcon,
  inprogress: InProgressIcon,
  "in review": TechnicalReviewIcon,
  inreview: TechnicalReviewIcon,
  "technical review": TechnicalReviewIcon,
  review: TechnicalReviewIcon,
  done: CompletedIcon,
  completed: CompletedIcon,
  complete: CompletedIcon,
  blocked: PausedIcon,
  paused: PausedIcon,
  "on hold": PausedIcon,
};

function normalizeName(name: string): string {
  return name.toLowerCase().trim().replace(/[-_]/g, " ").replace(/\s+/g, " ");
}

export function matchIcon(
  labelName: string,
): React.FC<{ className?: string }> | null {
  return ICON_NAME_MAP[normalizeName(labelName)] ?? null;
}
```

- [ ] **Step 4: Update `StatusIcon` component to use name-based matching**

Replace the current `StatusIcon` that uses `statusById[statusId]` with one that accepts a `Status` object:

```typescript
export function StatusIcon({
  status,
  className,
}: { status: Status; className?: string }) {
  const Icon = status.icon;
  return <Icon className={className} />;
}
```

- [ ] **Step 5: Remove `allStatus` array and `statusById` exports**

Delete the `status` array export (lines 120-127) and `statusById` record (line 129). Keep the individual icon components exported for use in `matchIcon`.

Also add and export `makeColoredCircle` (shared utility for fallback icons):

```typescript
export function makeColoredCircle(color: string): React.FC<{ className?: string }> {
  return ({ className }) => (
    <svg className={className} width="14" height="14" viewBox="0 0 14 14" fill="none">
      <circle cx="7" cy="7" r="5" stroke={color} strokeWidth="1.5" fill="none" />
      <circle cx="7" cy="7" r="2" fill={color} />
    </svg>
  );
}
```

The file now exports: `Status` interface, all 6 icon components, `matchIcon()`, `makeColoredCircle()`, `StatusIcon` component.

- [ ] **Step 6: Verify the file compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -50`

Expected: Compilation errors in consumer files (they still import `status as allStatus`). This is expected — we'll fix them in later tasks.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/status-icons.tsx
git commit -m "refactor(tasks2): convert status-icons to icon registry with matchIcon"
```

---

### Task 2: Update `mappers.ts` — pure `resolveStatus()` with `StatusLabel[]` param

**Files:**
- Modify: `desktop-ui/src/features/tasks2/lib/mappers.ts`

- [ ] **Step 1: Read current `mappers.ts` to confirm exact code**

Read `desktop-ui/src/features/tasks2/lib/mappers.ts`. Note:
- Line 6: imports `status as allStatusDefs, BacklogIcon, type Status` from `./status-icons`
- Lines 117-120: `statusByBackend` map built from `allStatusDefs`
- Lines 122-145: `resolveStatus(task: Task): Status`
- Lines 147-150: `statusToBackend(status: Status): string`
- Lines 235-252: `taskToIssue(task, projectMap)`
- Lines 256-285: `taskToDetailTask(task, projectMap, areaMap)`
- Lines 289-298: `taskToSubIssue(task)`

- [ ] **Step 2: Update imports**

Replace the import of `status as allStatusDefs` with `matchIcon`:

```typescript
import { matchIcon, makeColoredCircle, BacklogIcon, type Status } from "./status-icons";
```

Add import for `StatusLabel`:

```typescript
import type { StatusLabel } from "@shared/types/common";
```

- [ ] **Step 3: Rewrite `resolveStatus()` to accept `labels` parameter**

Replace the existing `resolveStatus` and `statusByBackend` map with:

```typescript
export function resolveStatus(task: Task, labels: StatusLabel[]): Status {
  // 1. Match by statusLabel if present
  if (task.statusLabel) {
    const icon = matchIcon(task.statusLabel.name);
    return {
      id: task.statusLabel.id,
      name: task.statusLabel.name,
      color: task.statusLabel.color,
      icon: icon ?? makeColoredCircle(task.statusLabel.color),
      backendStatus: task.statusLabel.statusGroup,
      statusGroup: task.statusLabel.statusGroup,
    };
  }

  // 2. Match task.status against labels by statusGroup
  const matchedLabel = labels.find(
    (l) => l.statusGroup === task.status || l.name.toLowerCase() === task.status,
  );
  if (matchedLabel) {
    const icon = matchIcon(matchedLabel.name);
    return {
      id: matchedLabel.id,
      name: matchedLabel.name,
      color: matchedLabel.color,
      icon: icon ?? makeColoredCircle(matchedLabel.color),
      backendStatus: matchedLabel.statusGroup,
      statusGroup: matchedLabel.statusGroup,
    };
  }

  // 3. Fallback — backlog icon
  return {
    id: "fallback",
    name: task.status || "Backlog",
    color: "#94a3b8",
    icon: BacklogIcon,
    backendStatus: task.status || "not_started",
    statusGroup: "not_started",
  };
}
```

Note: `makeColoredCircle` is imported from `./status-icons` (added in Task 1 Step 5).

- [ ] **Step 4: Replace `statusToBackend()` with `statusToMutationParams()`**

```typescript
export function statusToMutationParams(status: Status): {
  status: string;
  statusLabelId: string | null;
} {
  return {
    status: status.statusGroup ?? status.backendStatus,
    statusLabelId: status.id === "fallback" ? null : status.id,
  };
}
```

- [ ] **Step 5: Update `taskToIssue()` signature to accept `labels`**

Change from `taskToIssue(task, projectMap)` to `taskToIssue(task, projectMap, labels: StatusLabel[])`.

Update the internal call: `resolveStatus(task)` → `resolveStatus(task, labels)`.

- [ ] **Step 6: Update `taskToDetailTask()` signature to accept `labels`**

Change from `taskToDetailTask(task, projectMap, areaMap)` to `taskToDetailTask(task, projectMap, areaMap, labels: StatusLabel[])`.

Update the internal call: `resolveStatus(task)` → `resolveStatus(task, labels)`.

- [ ] **Step 7: Update `taskToSubIssue()` signature to accept `labels`**

Change from `taskToSubIssue(task)` to `taskToSubIssue(task, labels: StatusLabel[])`.

Update the internal call: `resolveStatus(task)` → `resolveStatus(task, labels)`.

- [ ] **Step 8: Verify the file compiles (ignoring downstream errors)**

Run: `cd desktop-ui && bun run build 2>&1 | head -50`

Expected: Errors in callers of `taskToIssue`, `taskToDetailTask`, `taskToSubIssue` (missing `labels` arg). This is expected.

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/mappers.ts
git commit -m "refactor(tasks2): resolveStatus accepts StatusLabel[] param, add statusToMutationParams"
```

---

### Task 3: Update `status-utils.tsx`

**Files:**
- Modify: `desktop-ui/src/features/tasks2/lib/status-utils.tsx`

- [ ] **Step 1: Read the current file**

Read `desktop-ui/src/features/tasks2/lib/status-utils.tsx`. It's a tiny file (7 lines) that wraps `StatusIcon` component.

- [ ] **Step 2: Update to pass `Status` object instead of ID**

Update the import and function to use the new `StatusIcon` signature:

```typescript
import { StatusIcon, type Status } from "./status-icons";

export function renderStatusIcon(status: Status, className?: string) {
  return <StatusIcon status={status} className={className} />;
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/status-utils.tsx
git commit -m "refactor(tasks2): renderStatusIcon takes Status object"
```

---

### Task 4: Create `StatusWorkflowContext`

**Files:**
- Create: `desktop-ui/src/features/tasks2/contexts/StatusWorkflowContext.tsx`

- [ ] **Step 1: Check the `useEffectiveLabels` hook signature**

Read `desktop-ui/src/shared/hooks/useWorkflows.ts` and the actual implementation at `desktop-ui/src/features/tasks/hooks/useWorkflows.ts` to confirm the hook returns `StatusLabel[]` and accepts `projectId: string | null`.

- [ ] **Step 2: Create the context file**

```typescript
import {
  createContext,
  useContext,
  useMemo,
  type ReactNode,
} from "react";
import { useEffectiveLabels } from "@shared/hooks/useWorkflows";
import type { StatusLabel } from "@shared/types/common";
import type { Task } from "@shared/types/tasks";
import { matchIcon, makeColoredCircle, type Status } from "../lib/status-icons";
import { resolveStatus } from "../lib/mappers";

interface StatusWorkflowContextValue {
  statuses: Status[];
  labels: StatusLabel[];
  resolveStatusById: (id: string) => Status | undefined;
  resolveStatusByTask: (task: Task) => Status;
}

const StatusWorkflowContext = createContext<StatusWorkflowContextValue | null>(
  null,
);

function labelsToStatuses(labels: StatusLabel[]): Status[] {
  return labels.map((label) => {
    const icon = matchIcon(label.name);
    return {
      id: label.id,
      name: label.name,
      color: label.color,
      icon: icon ?? makeColoredCircle(label.color),
      backendStatus: label.statusGroup,
      statusGroup: label.statusGroup,
    };
  });
}

export function StatusWorkflowProvider({
  projectId,
  children,
}: {
  projectId: string | null;
  children: ReactNode;
}) {
  const { data: labels = [] } = useEffectiveLabels(projectId);

  const value = useMemo<StatusWorkflowContextValue>(() => {
    const statuses = labelsToStatuses(labels);
    const statusMap = new Map(statuses.map((s) => [s.id, s]));

    return {
      statuses,
      labels,
      resolveStatusById: (id: string) => statusMap.get(id),
      resolveStatusByTask: (task: Task) => resolveStatus(task, labels),
    };
  }, [labels]);

  return (
    <StatusWorkflowContext.Provider value={value}>
      {children}
    </StatusWorkflowContext.Provider>
  );
}

export function useStatusWorkflow(): StatusWorkflowContextValue {
  const ctx = useContext(StatusWorkflowContext);
  if (!ctx) {
    throw new Error(
      "useStatusWorkflow must be used within StatusWorkflowProvider",
    );
  }
  return ctx;
}
```

- [ ] **Step 3: Verify the file compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -50`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/contexts/StatusWorkflowContext.tsx
git commit -m "feat(tasks2): add StatusWorkflowProvider context"
```

---

### Task 5: Wire `StatusWorkflowProvider` into Tasks2 layout

**Files:**
- Modify: Tasks2 root layout/provider component

- [ ] **Step 1: Find the tasks2 root layout**

Search for the component that renders the tasks2 feature root. Check:
- `desktop-ui/src/features/tasks2/index.tsx` or `Tasks2.tsx` or similar
- Look for where `TasksContext` provider is used (from the earlier integration work)

Run: `grep -rn "TasksContext\|Tasks2Provider\|tasks2.*Provider" desktop-ui/src/features/tasks2/ --include="*.tsx" | head -20`

- [ ] **Step 2: Wrap the existing layout with `StatusWorkflowProvider`**

Import `StatusWorkflowProvider` and wrap the children. The `projectId` should come from the current navigation state (selected project filter).

```typescript
import { StatusWorkflowProvider } from "./contexts/StatusWorkflowContext";

// Inside the component, get the current selected project:
// const selectedProjectId = ... (from navigation/filter state)

<StatusWorkflowProvider projectId={selectedProjectId ?? null}>
  {/* existing children */}
</StatusWorkflowProvider>
```

The exact integration depends on how the tasks2 layout is structured. Read the file first to determine where the provider wraps.

- [ ] **Step 3: Verify the app compiles and loads**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 4: Commit**

```bash
git add -u desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): wire StatusWorkflowProvider into layout"
```

---

### Task 6: Update hooks to pass `labels` through mappers

**Files:**
- Modify: `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`
- Modify: `desktop-ui/src/features/tasks2/hooks/useTasks.ts` (if it calls `taskToIssue`)
- Modify: Any other hooks that call `taskToIssue`, `taskToDetailTask`, or `taskToSubIssue`

- [ ] **Step 1: Read `useIssueDetail.ts` to find all mapper calls**

Read `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`. Note:
- Line 67-70: calls `taskToDetailTask(task, projectMap, areaMap)`
- Line 113: calls `statusToBackend(value)` in updateTask
- Line 154: calls `deriveFocusSession(task)` (will be addressed in Feature 2)

- [ ] **Step 2: Add `useStatusWorkflow()` to `useIssueDetail`**

Import and use the context:

```typescript
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
```

Inside the hook:

```typescript
const { labels } = useStatusWorkflow();
```

Update the mapper call:

```typescript
const detailTask = task ? taskToDetailTask(task, projectMap, areaMap, labels) : null;
```

- [ ] **Step 3: Update `statusToBackend` call to `statusToMutationParams`**

In the `updateTask` callback, replace the `statusToBackend(value)` call:

```typescript
// Old:
status: statusToBackend(value),

// New:
import { statusToMutationParams } from "../lib/mappers";
// ...
if (key === "status") {
  const { status, statusLabelId } = statusToMutationParams(value as Status);
  Object.assign(updateParams, { status, statusLabelId });
}
```

- [ ] **Step 4: Update subIssues mapping**

Find where `taskToSubIssue` is called (likely with children query results). Add `labels` parameter.

- [ ] **Step 5: Read and update `useTasks.ts`**

Read `desktop-ui/src/features/tasks2/hooks/useTasks.ts`. Find calls to `taskToIssue()` and add `labels` parameter from `useStatusWorkflow()`.

- [ ] **Step 6: Search for other callers**

Run: `grep -rn "taskToIssue\|taskToDetailTask\|taskToSubIssue\|statusToBackend" desktop-ui/src/features/tasks2/ --include="*.ts" --include="*.tsx"`

Update any remaining callers with the `labels` parameter.

- [ ] **Step 7: Verify compilation**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 8: Commit**

```bash
git add -u desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): pass StatusLabel[] through mapper functions"
```

---

### Task 7: Update 7 consumer components to use `useStatusWorkflow()`

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/StatusSelector.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/IssueBoard.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/AllIssues.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/Filter.tsx`

- [ ] **Step 1: Update `StatusSelector.tsx`**

Read the file. Replace:
```typescript
import { status as allStatus } from "../lib/status-icons";
```
With:
```typescript
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
```

Inside the component:
```typescript
const { statuses } = useStatusWorkflow();
```

Replace all references to `allStatus` with `statuses`.
Also update any `renderStatusIcon(s.id)` calls to `renderStatusIcon(s)` (status-utils now takes a `Status` object, not an ID).

- [ ] **Step 2: Update `SidebarProperties.tsx`**

Same pattern: replace `allStatus` import with `useStatusWorkflow()`, use `statuses` from context.

- [ ] **Step 3: Update `IssueContextMenu.tsx`**

Same pattern.

- [ ] **Step 4: Update `IssueBoard.tsx`**

Same import pattern. Additionally:

1. Board columns derive from `statuses` via context instead of hardcoded array.
2. **Drag-and-drop fix:** `handleDragEnd` currently does `allStatus.find(s => s.id === over.id)`. With dynamic statuses, column IDs are now `StatusLabel.id` (UUIDs). Update to use `statuses.find(s => s.id === over.id)`. The `onUpdateStatus` callback must use `statusToMutationParams(targetStatus)` to send proper `status` + `statusLabelId` in the mutation.
3. **Graceful degradation (spec 1.4):** When grouping tasks into board columns, tasks whose status doesn't match any column should be bucketed by `statusGroup`:
   - `"not_started"` → first column
   - `"active"` → middle column (e.g., "In Progress")
   - `"done"` → done column
   - `"stuck"` → last column

   Add a fallback in the grouping logic: if `task.status` doesn't match any column's `id`, find the column whose `statusGroup` matches `task.statusLabel?.statusGroup`.

- [ ] **Step 5: Update `AllIssues.tsx`**

Same pattern. Also update the `handleUpdateStatus` callback to use `statusToMutationParams()` if it calls `statusToBackend()`.

- [ ] **Step 6: Update `CreateIssueModal.tsx`**

Same pattern. Status options in the creation form come from `statuses`.

- [ ] **Step 7: Update `Filter.tsx`**

Same pattern. Filter options come from `statuses`.

- [ ] **Step 8: Verify full compilation and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 9: Manual smoke test**

Run: `cargo tauri dev`

Verify:
- Board view shows columns from workflow labels
- Status picker dropdown shows dynamic statuses
- Changing status works and persists
- "All Issues" view works with status grouping
- Creating a new issue shows status options

- [ ] **Step 10: Commit**

```bash
git add -u desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): replace hardcoded statuses with dynamic workflow labels"
```

---

## Chunk 2: Feature 2 — Focus Session Controls

### File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `desktop-ui/src/features/tasks2/lib/mappers.ts` | Replace `FocusSession` interface + `deriveFocusSession()` → `buildFocusSession()` |
| Modify | `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts` | Wire stop mutation, use `buildFocusSession()` |
| Modify | `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx` | Enable Stop button, null-guard quality sections, Pause tooltip |

---

### Task 8: Replace `FocusSession` interface and `deriveFocusSession()`

**Files:**
- Modify: `desktop-ui/src/features/tasks2/lib/mappers.ts`

- [ ] **Step 1: Read the current `FocusSession` and `deriveFocusSession`**

In `mappers.ts`:
- Lines 85-91: `FocusSession` interface (`focusMode`, `qualityScore`, `distractionCount`, `flowState`, `qualityHistory` — all required)
- Lines 333-342: `deriveFocusSession()` returns hardcoded values

- [ ] **Step 2: Replace `FocusSession` interface**

```typescript
export interface FocusSession {
  startedAt: string;
  elapsed: number;
  totalTracked: number;
  qualityScore: number | null;
  distractionCount: number | null;
  flowState: string | null;
  qualityHistory: number[] | null;
}
```

- [ ] **Step 3: Replace `deriveFocusSession()` with `buildFocusSession()`**

```typescript
export function buildFocusSession(task: DetailTask): FocusSession | null {
  if (!task.focusedAt) return null;

  const startedAt = task.focusedAt;
  const elapsed = Math.floor(
    (Date.now() - new Date(startedAt).getTime()) / 1000,
  );

  // Note: elapsed is a snapshot — the FocusTimer component in SidebarWorkState
  // independently computes live elapsed time from startedAt via setInterval.
  return {
    startedAt,
    elapsed,
    totalTracked: task.totalTrackedSecs ?? 0,
    qualityScore: null,
    distractionCount: null,
    flowState: null,
    qualityHistory: null,
  };
}
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/mappers.ts
git commit -m "refactor(tasks2): replace FocusSession interface, buildFocusSession returns real data"
```

---

### Task 9: Wire Stop mutation in `useIssueDetail`

**Files:**
- Modify: `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`

- [ ] **Step 1: Read `useIssueDetail.ts` current state**

Note line 154: `const focusSession = task ? deriveFocusSession(task) : null;`

- [ ] **Step 2: Replace `deriveFocusSession` with `buildFocusSession`**

Update the import and call:

```typescript
import { buildFocusSession } from "../lib/mappers";

// Replace deriveFocusSession call:
const focusSession = task ? buildFocusSession(task) : null;
```

- [ ] **Step 3: Add stop focus mutation**

Note: `useMutation` in this codebase exposes `mutate` (which returns `Promise<T | undefined>`), NOT `mutateAsync`. Use `mutate` throughout.

```typescript
const endFocusMutation = useMutation<Task | null, { id: string }>(
  "task_end_focus",
  "params",
);

const stopFocus = useCallback(async () => {
  if (!issueId) return;
  await endFocusMutation.mutate({ id: issueId });
  // The entity:updated event will fire and refetch the task automatically.
  // Edge case: if end_focus returns null (app restarted, in-memory state lost),
  // the task still shows focusedAt. This is a known limitation — the user can
  // work around it by updating the task status, which triggers a refetch.
}, [issueId, endFocusMutation]);
```

- [ ] **Step 4: Add `stopFocus` to return object**

Add `stopFocus` to the returned object from the hook so `SidebarWorkState` can call it.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts
git commit -m "feat(tasks2): wire task_end_focus mutation in useIssueDetail"
```

---

### Task 10: Update `SidebarWorkState.tsx` — enable Stop, null-guard quality sections

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx`

- [ ] **Step 1: Read the current file**

Note:
- Lines 8-12: Props interface
- Lines 32-62: Quality/distraction/flow sections that reference `focusSession.qualityScore`, `focusSession.distractionCount`, `focusSession.flowState`
- Lines 64-81: Pause/Stop buttons (both `disabled`)

- [ ] **Step 2: Update props to accept `onStopFocus`**

```typescript
interface SidebarWorkStateProps {
  task: DetailTask;
  taskState: TaskState;
  focusSession: FocusSession | null;
  onStopFocus?: () => void;
}
```

- [ ] **Step 3: Null-guard quality sections**

Wrap each quality section in a null check:

```typescript
{focusSession.qualityScore != null && (
  <div className="...">
    <span>Quality</span>
    <span>{(focusSession.qualityScore * 100).toFixed(0)}%</span>
  </div>
)}

{focusSession.distractionCount != null && (
  <div className="...">
    <span>Distractions</span>
    <span>{focusSession.distractionCount}</span>
  </div>
)}

{focusSession.flowState != null && (
  <FlowBadge state={focusSession.flowState} />
)}

{focusSession.qualityHistory != null && (
  <QualitySparkline values={focusSession.qualityHistory} />
)}
```

- [ ] **Step 4: Enable Stop button, add Pause tooltip**

```typescript
<button
  disabled
  title="Coming soon"
  className="..."
>
  <Pause className="..." />
  Pause
</button>
<button
  onClick={onStopFocus}
  className="..."
>
  <Square className="..." />
  Stop
</button>
```

Remove `disabled` from Stop button, add `onClick={onStopFocus}`.
Add `title="Coming soon"` to Pause button.

- [ ] **Step 5: Remove `focusMode` reference**

The component references `focusSession.focusMode` in the Mode row (~line 35). Remove the entire Mode row — `focusMode` no longer exists in the new `FocusSession` interface.

- [ ] **Step 6: Update the parent component that renders `SidebarWorkState`**

Find where `SidebarWorkState` is rendered (likely in the detail panel) and pass `onStopFocus={stopFocus}` from `useIssueDetail`.

- [ ] **Step 7: Verify compilation and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 8: Manual smoke test**

Run: `cargo tauri dev`

Verify:
- Focus session shows timer when a task is focused
- Quality/distraction/flow sections are hidden (no fake data)
- Stop button is enabled and clickable
- Clicking Stop ends the focus session and the panel disappears
- Pause button is disabled with "Coming soon" tooltip
- Total tracked time displays correctly after stopping

- [ ] **Step 9: Commit**

```bash
git add -u desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): enable Stop button, null-guard quality metrics in focus session"
```

---

## Chunk 3: Feature 3 — Activity Tab Enrichment (Backend)

### File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `crates/bus/src/domain_events.rs` | Add `TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated` variants |
| Modify | `crates/desktop-shared/src/commands/timeline.rs` | Add new `TimelineEntryType` variants |
| Modify | `crates/app-core/src/handlers/tasks/crud.rs` | Fetch old task, diff fields, emit domain events, accept `actor` param |
| Modify | `crates/app-core/src/handlers/timeline.rs` | Handle new events in `normalize_domain_event`, update `compute_summary` |
| Modify | `crates/desktop/src/commands/tasks.rs` | Pass `actor` to `task_update` handler |

---

### Task 11: Add new `DomainEvent` variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Read `domain_events.rs` to find the Task events section**

Note lines 87-102 for existing task-related events (`TaskCreated`, `TaskCompleted`, `TaskDeferred`).

- [ ] **Step 2: Add 3 new variants after the existing task events**

```rust
TaskStatusChanged {
    task_id: String,
    from: String,
    to: String,
    actor: Option<String>,
},
TaskPriorityChanged {
    task_id: String,
    from: String,
    to: String,
    actor: Option<String>,
},
TaskFieldUpdated {
    task_id: String,
    field: String,
    from: String,
    to: String,
    actor: Option<String>,
},
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --workspace 2>&1 | head -30`

Expected: Should compile (new enum variants don't require match exhaustiveness updates unless there are non-exhaustive matches).

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add TaskStatusChanged, TaskPriorityChanged, TaskFieldUpdated domain events"
```

---

### Task 12: Add new `TimelineEntryType` variants

**Files:**
- Modify: `crates/desktop-shared/src/commands/timeline.rs`

- [ ] **Step 1: Read the `TimelineEntryType` enum**

Note lines 54-71: existing variants ending with `CalendarEvent`.

- [ ] **Step 2: Add 3 new variants**

After `CalendarEvent`, add:

```rust
TaskStatusChanged,
TaskPriorityChanged,
TaskFieldUpdated,
```

- [ ] **Step 3: Also add these to the frontend TypeScript type**

Read `desktop-ui/src/shared/types/common.ts` lines 107-121 for the `TimelineEntryType` type. Add:

```typescript
| "taskStatusChanged"
| "taskPriorityChanged"
| "taskFieldUpdated"
```

(The Rust enum uses `#[serde(rename_all = "camelCase")]`, so `TaskStatusChanged` serializes to `"taskStatusChanged"`)

- [ ] **Step 4: Verify compilation**

Run: `cargo build --workspace 2>&1 | head -30`

Check for any `match` exhaustiveness warnings on `TimelineEntryType`.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands/timeline.rs desktop-ui/src/shared/types/common.ts
git commit -m "feat(timeline): add TaskStatusChanged/PriorityChanged/FieldUpdated entry types"
```

---

### Task 13: Modify `task_update` handler to diff and emit events

**Files:**
- Modify: `crates/app-core/src/handlers/tasks/crud.rs`

- [ ] **Step 1: Read the `task_update` method (lines 166-202)**

Note the current flow:
1. Build `TaskPatch` from params
2. Call `repos.tasks.update(&patch)`
3. Create `EntityUpdate` for UI refresh
4. Return result

- [ ] **Step 2: Add `actor` parameter to `task_update` signature**

Change from:
```rust
pub async fn task_update(&self, params: TaskUpdateParams) -> Result<(TaskResponse, Vec<EntityUpdate>)>
```
To:
```rust
pub async fn task_update(&self, params: TaskUpdateParams, actor: Option<String>) -> Result<(TaskResponse, Vec<EntityUpdate>)>
```

- [ ] **Step 3: Fetch old task before update**

Add before the patch construction:

```rust
let old_task = self.repos.tasks.get(&params.id).await?;
```

- [ ] **Step 4: After the update, diff and emit domain events**

**IMPORTANT:** `DomainEventBus::publish()` is synchronous (returns `()`), NOT async. Access via `self.domain_event_bus()` (returns `Result<&Arc<DomainEventBus>>`), NOT `self.bus`. Follow the existing pattern from `task_create`:

```rust
if let Ok(bus) = self.domain_event_bus() {
    bus.publish(bus::DomainEvent::TaskCreated { ... });
}
```

After `repos.tasks.update(&patch)` and before returning:

```rust
// Diff and emit change events
if let Some(ref old) = old_task {
    if let Ok(bus) = self.domain_event_bus() {
        if let Some(ref new_status) = params.status {
            if old.status != *new_status {
                bus.publish(bus::DomainEvent::TaskStatusChanged {
                    task_id: params.id.clone(),
                    from: old.status.clone(),
                    to: new_status.clone(),
                    actor: actor.clone(),
                });
            }
        }

        if let Some(ref new_priority_opt) = params.priority {
            let old_str = old.priority.map(|p| p.to_string()).unwrap_or_default();
            let new_str = new_priority_opt.map(|p| p.to_string()).unwrap_or_default();
            if old_str != new_str {
                bus.publish(bus::DomainEvent::TaskPriorityChanged {
                    task_id: params.id.clone(),
                    from: old_str,
                    to: new_str,
                    actor: actor.clone(),
                });
            }
        }

        // Track title changes
        if let Some(ref new_title) = params.title {
            if old.title != *new_title {
                bus.publish(bus::DomainEvent::TaskFieldUpdated {
                    task_id: params.id.clone(),
                    field: "title".to_string(),
                    from: old.title.clone(),
                    to: new_title.clone(),
                    actor: actor.clone(),
                });
            }
        }
    }
}
```

**Note on `params.priority`:** The type is `Option<Option<i16>>`. The outer `Some` means "this field is being updated", the inner `Option<i16>` is the new value (which can be `None` to clear priority). Destructure carefully.

Note: The exact field access patterns (`old.status`, `old.priority`, `old.title`) depend on the `TaskRow` type returned by `repos.tasks.get()`. Read the type to confirm field names. The `get()` call returns `Result<Option<TaskRow>>`, so use `if let Some(ref old) = old_task`.

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p app-core 2>&1 | head -30`

Expected: Compilation errors in callers of `task_update` that don't pass `actor` yet. This is expected — we'll fix the Tauri command next.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/tasks/crud.rs
git commit -m "feat(tasks2): emit status/priority/field change domain events from task_update"
```

---

### Task 14: Update Tauri command to pass `actor`

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`

- [ ] **Step 1: Read the `task_update` Tauri command (lines 67-75)**

Note the current call: `state.task_update(params).await?`

- [ ] **Step 2: Pass `actor: Some("user".into())`**

```rust
let (result, updates) = state.task_update(params, Some("user".into())).await?;
```

- [ ] **Step 3: Update dev server dispatch (line ~169)**

Find the "task_update" case in `dispatch_dev()` and update similarly:

```rust
core.task_update(params, Some("user".into())).await
```

- [ ] **Step 4: Search for other callers of `task_update`**

Run: `grep -rn "task_update\b" crates/ --include="*.rs" | grep -v "test" | grep -v "mod.rs"`

Update any other call sites (e.g., agent pipeline calls should pass `Some("agent".into())`).

Also check `task_toggle_complete` — it changes status too but is a separate method. For now, pass `Some("user".into())` from its Tauri command as well. If `task_toggle_complete` has its own domain event emission pattern, add actor there too.

- [ ] **Step 5: Verify full compilation**

Run: `cargo build --workspace 2>&1 | head -30`

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs
git commit -m "feat(tasks2): pass actor param through Tauri command to task_update"
```

---

### Task 15: Handle new events in `normalize_domain_event` and `compute_summary`

**Files:**
- Modify: `crates/app-core/src/handlers/timeline.rs`

- [ ] **Step 1: Read `normalize_domain_event` (lines 206-301)**

Note the pattern match structure on `e.event_type` string.

- [ ] **Step 2: Add handlers for new event types**

**IMPORTANT CAVEATS** (from code review):
1. **Payload nesting:** `DomainEvent` uses serde's default externally-tagged representation. When serialized, the payload is nested: `{"TaskStatusChanged": {"task_id": "x", "from": "a", ...}}`. You must extract the inner object first: `let inner = payload.get(e.event_type.as_str()).unwrap_or(&payload);`
2. **Existing function structure:** `normalize_domain_event` returns values via a 6-tuple `(entry_type, source, title, entity_id, entity_route, color)` from the match, then constructs `TimelineEntry` once at the bottom. New variants must return tuples in the same pattern, NOT direct `Some(TimelineEntry {...})`.
3. **Field names:** Use `e.timestamp` (not `e.created_at`), `e.id` (already `String`, no `.to_string()`), `color: "var(--timeline-task)".into()` (not `None` — field is `String`, not `Option`).

Add an inner-payload extraction helper near the top of the function:

```rust
let inner = payload.get(e.event_type.as_str()).unwrap_or(&payload);
```

Then add match arms that return the same 6-tuple shape. The metadata is handled separately — read how existing arms handle it and follow the pattern. If the function builds metadata after the match, add the metadata construction there.

For the new match arms:

```rust
"TaskStatusChanged" => {
    let task_id = inner.get("task_id").and_then(|v| v.as_str()).unwrap_or_default();
    let from = inner.get("from").and_then(|v| v.as_str()).unwrap_or_default();
    let to = inner.get("to").and_then(|v| v.as_str()).unwrap_or_default();
    let actor = inner.get("actor").and_then(|v| v.as_str());
    // Build metadata with actor
    let mut meta = serde_json::json!({"field": "status", "from": from, "to": to});
    if let Some(a) = actor { meta["actor"] = serde_json::json!(a); }
    // Return tuple matching existing pattern — adapt to actual tuple fields
    (TimelineEntryType::TaskStatusChanged, TimelineSource::Task,
     format!("Status changed from {} to {}", from, to),
     Some(task_id.to_string()), Some(format!("/tasks/{}", task_id)),
     "var(--timeline-task)")
}
"TaskPriorityChanged" => {
    let task_id = inner.get("task_id").and_then(|v| v.as_str()).unwrap_or_default();
    let from = inner.get("from").and_then(|v| v.as_str()).unwrap_or_default();
    let to = inner.get("to").and_then(|v| v.as_str()).unwrap_or_default();
    let actor = inner.get("actor").and_then(|v| v.as_str());
    let mut meta = serde_json::json!({"field": "priority", "from": from, "to": to});
    if let Some(a) = actor { meta["actor"] = serde_json::json!(a); }
    (TimelineEntryType::TaskPriorityChanged, TimelineSource::Task,
     format!("Priority changed from {} to {}", from, to),
     Some(task_id.to_string()), Some(format!("/tasks/{}", task_id)),
     "var(--timeline-task)")
}
"TaskFieldUpdated" => {
    let task_id = inner.get("task_id").and_then(|v| v.as_str()).unwrap_or_default();
    let field = inner.get("field").and_then(|v| v.as_str()).unwrap_or_default();
    let actor = inner.get("actor").and_then(|v| v.as_str());
    let mut meta = serde_json::json!({"field": field});
    if let Some(a) = actor { meta["actor"] = serde_json::json!(a); }
    (TimelineEntryType::TaskFieldUpdated, TimelineSource::Task,
     format!("Updated {}", field),
     Some(task_id.to_string()), Some(format!("/tasks/{}", task_id)),
     "var(--timeline-task)")
}
```

**Note:** The metadata construction above is pseudocode — adapt to how the existing function passes metadata to the final `TimelineEntry` construction. If metadata is built inside the match arm but set on the entry after the match, store `meta` in a variable that outlives the match. Read the full function structure carefully before implementing.

- [ ] **Step 3: Update `compute_summary()` to handle new variants**

In the entry_type match inside `compute_summary()` (lines ~511-524), add cases or verify the new variants don't need special handling. If they fall through a catch-all `_ => {}`, they'll be silently skipped — which is fine for summary computation (status changes don't have duration to accumulate).

- [ ] **Step 4: Verify deduplication filter**

Check that the `domain_entries.retain(|e| { !matches!(...) })` block (lines ~137-146) does NOT include `TaskStatusChanged`, `TaskPriorityChanged`, or `TaskFieldUpdated`. These should pass through since they are NOT generated by direct pipelines.

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p app-core`

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/timeline.rs
git commit -m "feat(timeline): normalize TaskStatusChanged/PriorityChanged/FieldUpdated events"
```

---

## Chunk 4: Feature 3 — Activity Tab Enrichment (Frontend)

### File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `desktop-ui/src/features/tasks2/lib/mappers.ts` | Enrich `timelineToActivity()` with new entry types + actor mapping |

---

### Task 16: Enrich `timelineToActivity()` mapper

**Files:**
- Modify: `desktop-ui/src/features/tasks2/lib/mappers.ts`

- [ ] **Step 1: Read `timelineToActivity` (lines 320-329)**

Note the current implementation: creates `ActivityEntry` with `actorType: "system"`, `actorName: "System"`, maps `entry.entryType` to a simple action string.

- [ ] **Step 2: Add entry type → action mapping for new types**

Update the action mapping to include new entry types:

```typescript
const ENTRY_TYPE_ACTIONS: Record<string, string> = {
  taskCreated: "created task",
  taskCompleted: "completed task",
  taskUpdated: "updated task",
  taskStatusChanged: "changed status",
  taskPriorityChanged: "changed priority",
  taskFieldUpdated: "updated field",
  // ... existing mappings
};
```

- [ ] **Step 3: Parse metadata for detail text**

```typescript
function buildActivityDetail(entry: TimelineEntry): string | undefined {
  if (!entry.metadata) return entry.description ?? undefined;

  const meta = entry.metadata as Record<string, unknown>;
  const from = meta.from as string | undefined;
  const to = meta.to as string | undefined;
  const field = meta.field as string | undefined;

  if (entry.entryType === "taskStatusChanged" && from && to) {
    return `${from} → ${to}`;
  }
  if (entry.entryType === "taskPriorityChanged" && from && to) {
    // Priority values are numeric strings from backend ("1", "2", etc.)
    // Map to human-readable names if possible
    const priorityNames: Record<string, string> = {
      "1": "Urgent", "2": "High", "3": "Medium", "4": "Low",
    };
    const fromName = priorityNames[from] ?? from;
    const toName = priorityNames[to] ?? to;
    return `${fromName} → ${toName}`;
  }
  if (entry.entryType === "taskFieldUpdated" && field) {
    return `Updated ${field}`;
  }

  return entry.description ?? undefined;
}
```

- [ ] **Step 4: Map actor from metadata**

```typescript
function resolveActor(entry: TimelineEntry): {
  actorType: "user" | "agent" | "system";
  actorName: string;
} {
  const meta = entry.metadata as Record<string, unknown> | undefined;
  const actor = meta?.actor as string | undefined;

  switch (actor) {
    case "user":
      return { actorType: "user", actorName: "You" };
    case "agent":
      return { actorType: "agent", actorName: "Klyntbot" };
    default:
      return { actorType: "system", actorName: "System" };
  }
}
```

- [ ] **Step 5: Update `timelineToActivity()` to use new helpers**

```typescript
export function timelineToActivity(entry: TimelineEntry): ActivityEntry {
  const { actorType, actorName } = resolveActor(entry);
  const action = ENTRY_TYPE_ACTIONS[entry.entryType] ?? entry.entryType;
  const detail = buildActivityDetail(entry);

  return {
    id: entry.id,
    actorType,
    actorName,
    action,
    detail,
    createdAt: entry.startedAt,
  };
}
```

- [ ] **Step 6: Verify compilation and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/mappers.ts
git commit -m "feat(tasks2): enrich timelineToActivity with status/priority changes and actor types"
```

---

### Task 17: Full integration test

- [ ] **Step 1: Build everything**

Run: `cargo build --workspace && cd desktop-ui && bun run build`

- [ ] **Step 2: Run Rust tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`

Fix any test failures.

- [ ] **Step 3: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Run Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`

Fix any warnings (zero clippy warnings policy).

- [ ] **Step 5: Manual smoke test**

Run: `cargo tauri dev`

Full verification checklist:
- [ ] Board view shows columns from real workflow labels
- [ ] Status picker shows dynamic statuses from project workflow
- [ ] Changing status persists and updates board correctly
- [ ] Custom/unknown status labels show colored circle fallback icon
- [ ] Focus timer shows when a task is focused
- [ ] Stop button ends focus session, panel disappears
- [ ] Pause button is disabled with "Coming soon" tooltip
- [ ] Quality/distraction/flow sections are hidden (no fake data)
- [ ] Activity tab shows "You changed status: Todo → In Progress" entries
- [ ] Activity tab shows correct actor (You / Klyntbot / System)
- [ ] Creating an issue with a status works
- [ ] Filtering by status works
- [ ] All Issues view works with status operations

- [ ] **Step 6: Final commit (if any fixes needed)**

```bash
git add -u
git commit -m "fix(tasks2): integration fixes for status workflow, focus, activity"
```
