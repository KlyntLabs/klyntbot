# Frontend Comprehensive Upgrade Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade `desktop-ui/` to React 19 + React Compiler, add route lazy loading, fix re-render hotspots, decompose oversized components, introduce compound component pattern for TaskTable, and sweep accessibility/UX gaps.

**Architecture:** Three independent tiers — Tier 1 targets runtime performance (React 19, lazy routes, optimistic state), Tier 2 targets DX and composition (Chat decomposition, TaskTable compound context, explicit variants), Tier 3 targets UX polish (aria, auto-resize textarea, loading/error UI). Each tier commits independently and leaves the app in a working state.

**Tech Stack:** React 19, React Compiler (`babel-plugin-react-compiler`), Vite 6 + `@vitejs/plugin-react` v4, TypeScript 5.7, Tailwind v4, react-router v7, Tauri IPC, `useOptimistic` (React 19).

**Verification:** No test framework — verify each task with `cd desktop-ui && bun run build` (TypeScript + Vite compile) and `bun run lint` (Biome). Manual smoke test in dev mode where noted.

---

## TIER 1: Critical — Performance

---

### Task 1: Upgrade to React 19

**Files:**
- Modify: `desktop-ui/package.json`
- Modify: `desktop-ui/src/main.tsx`

**Step 1: Update package.json deps**

```json
{
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0"
  }
}
```

**Step 2: Install**

```bash
cd desktop-ui && bun install
```

Expected: bun resolves React 19 and peer deps without errors. If `@radix-ui` or `recharts` report peer dep warnings, they are warnings only — the app still works.

**Step 3: Verify build**

```bash
cd desktop-ui && bun run build
```

Expected: Vite compiles with 0 TypeScript errors. Common React 19 breakage: `ReactDOM.render` (not used here), `string refs` (not used here). If you see type errors on `ref` props, that's expected in Task 5.

**Step 4: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "chore(desktop-ui): upgrade React 18 → 19"
```

---

### Task 2: Add React Compiler

**Files:**
- Modify: `desktop-ui/package.json`
- Modify: `desktop-ui/vite.config.ts`

**Step 1: Install compiler plugin**

```bash
cd desktop-ui && bun add -d babel-plugin-react-compiler
```

**Step 2: Update vite.config.ts**

Full new content:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [
    react({
      babel: {
        plugins: ["babel-plugin-react-compiler"],
      },
    }),
    tailwindcss(),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:3456",
        changeOrigin: true,
      },
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "esnext",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
```

**Step 3: Verify build**

```bash
cd desktop-ui && bun run build
```

Expected: build succeeds. React Compiler emits no errors for well-formed components. If you see "React Compiler: hooks must be called at the top level" errors, there is a rule violation in that component — fix it before proceeding.

**Step 4: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock desktop-ui/vite.config.ts
git commit -m "chore(desktop-ui): add React Compiler via babel-plugin-react-compiler"
```

---

### Task 3: Route-based lazy loading

**Files:**
- Modify: `desktop-ui/src/App.tsx`

**Step 1: Replace all static imports with React.lazy**

Replace the entire file content with:

```tsx
import { lazy, Suspense } from "react";
import { createHashRouter, Navigate, RouterProvider } from "react-router";

const MainApp = lazy(() => import("./components/views/MainApp").then(m => ({ default: m.MainApp })));
const Chat = lazy(() => import("./components/views/Chat").then(m => ({ default: m.Chat })));
const ProjectDetail = lazy(() => import("./components/views/ProjectDetail").then(m => ({ default: m.ProjectDetail })));
const TaskDetail = lazy(() => import("./components/views/TaskDetail").then(m => ({ default: m.TaskDetail })));
const ObjectiveDetail = lazy(() => import("./components/views/ObjectiveDetail").then(m => ({ default: m.ObjectiveDetail })));
const ProductivityDayPage = lazy(() => import("./components/productivity/pages/DayPage").then(m => ({ default: m.ProductivityDayPage })));
const ProductivityWeekPage = lazy(() => import("./components/productivity/pages/WeekPage").then(m => ({ default: m.ProductivityWeekPage })));
const ProductivityMonthPage = lazy(() => import("./components/productivity/pages/MonthPage").then(m => ({ default: m.ProductivityMonthPage })));
const Finance = lazy(() => import("./components/views/Finance").then(m => ({ default: m.Finance })));
const FinanceAccounts = lazy(() => import("./components/views/FinanceAccounts").then(m => ({ default: m.FinanceAccounts })));
const FinanceTransactions = lazy(() => import("./components/views/FinanceTransactions").then(m => ({ default: m.FinanceTransactions })));
const FinanceBudgets = lazy(() => import("./components/views/FinanceBudgets").then(m => ({ default: m.FinanceBudgets })));
const FinanceInvestments = lazy(() => import("./components/views/FinanceInvestments").then(m => ({ default: m.FinanceInvestments })));
const FinanceGoals = lazy(() => import("./components/views/FinanceGoals").then(m => ({ default: m.FinanceGoals })));
const FinanceLiabilities = lazy(() => import("./components/views/FinanceLiabilities").then(m => ({ default: m.FinanceLiabilities })));
const SettingsLayout = lazy(() => import("./components/settings/SettingsLayout").then(m => ({ default: m.SettingsLayout })));
const GeneralSettings = lazy(() => import("./components/settings/pages/GeneralSettings").then(m => ({ default: m.GeneralSettings })));
const ConfigurationSettings = lazy(() => import("./components/settings/pages/ConfigurationSettings").then(m => ({ default: m.ConfigurationSettings })));
const PersonalizationSettings = lazy(() => import("./components/settings/pages/PersonalizationSettings").then(m => ({ default: m.PersonalizationSettings })));
const McpServersSettings = lazy(() => import("./components/settings/pages/McpServersSettings").then(m => ({ default: m.McpServersSettings })));
const GitSettings = lazy(() => import("./components/settings/pages/GitSettings").then(m => ({ default: m.GitSettings })));
const EnvironmentsSettings = lazy(() => import("./components/settings/pages/EnvironmentsSettings").then(m => ({ default: m.EnvironmentsSettings })));
const ArchivedSettings = lazy(() => import("./components/settings/pages/ArchivedSettings").then(m => ({ default: m.ArchivedSettings })));
const Launcher = lazy(() => import("./components/views/Launcher").then(m => ({ default: m.Launcher })));
const SystemTray = lazy(() => import("./components/views/SystemTray").then(m => ({ default: m.SystemTray })));
const DistractionOverlay = lazy(() => import("./components/distraction/DistractionOverlay").then(m => ({ default: m.DistractionOverlay })));

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Chat /> },
  { path: "/project/:id", element: <ProjectDetail /> },
  { path: "/task/:id", element: <TaskDetail /> },
  { path: "/objective/:id", element: <ObjectiveDetail /> },
  {
    path: "/productivity",
    element: <Navigate to={`/productivity/day/${new Date().toISOString().slice(0, 10)}`} replace />,
  },
  { path: "/productivity/day/:date", element: <ProductivityDayPage /> },
  { path: "/productivity/week/:weekStart", element: <ProductivityWeekPage /> },
  { path: "/productivity/month/:yearMonth", element: <ProductivityMonthPage /> },
  { path: "/finance", element: <Finance /> },
  { path: "/finance/accounts", element: <FinanceAccounts /> },
  { path: "/finance/transactions", element: <FinanceTransactions /> },
  { path: "/finance/budgets", element: <FinanceBudgets /> },
  { path: "/finance/investments", element: <FinanceInvestments /> },
  { path: "/finance/goals", element: <FinanceGoals /> },
  { path: "/finance/liabilities", element: <FinanceLiabilities /> },
  { path: "/settings", element: <Navigate to="/settings/general" replace /> },
  {
    path: "/settings/general",
    element: <SettingsLayout><GeneralSettings /></SettingsLayout>,
  },
  {
    path: "/settings/configuration",
    element: <SettingsLayout><ConfigurationSettings /></SettingsLayout>,
  },
  {
    path: "/settings/personalization",
    element: <SettingsLayout><PersonalizationSettings /></SettingsLayout>,
  },
  {
    path: "/settings/mcp",
    element: <SettingsLayout><McpServersSettings /></SettingsLayout>,
  },
  {
    path: "/settings/git",
    element: <SettingsLayout><GitSettings /></SettingsLayout>,
  },
  {
    path: "/settings/environments",
    element: <SettingsLayout><EnvironmentsSettings /></SettingsLayout>,
  },
  {
    path: "/settings/archived",
    element: <SettingsLayout><ArchivedSettings /></SettingsLayout>,
  },
  { path: "/launcher", element: <Launcher /> },
  { path: "/tray", element: <SystemTray /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <Navigate to="/" replace /> },
]);

export default function App() {
  return (
    <Suspense fallback={null}>
      <RouterProvider router={router} />
    </Suspense>
  );
}
```

**Step 2: Verify build**

```bash
cd desktop-ui && bun run build
```

Expected: Vite emits multiple chunks — one per lazy route. You should see output like `dist/assets/Chat-[hash].js`, `dist/assets/Finance-[hash].js`, etc. in the build output.

**Step 3: Commit**

```bash
git add desktop-ui/src/App.tsx
git commit -m "perf(desktop-ui): route-based lazy loading with React.lazy + Suspense"
```

---

### Task 4: Fix completedTasks with useOptimistic

**Files:**
- Modify: `desktop-ui/src/components/views/MainApp.tsx`

**Step 1: Read current MainApp.tsx** (already read — see lines 29–83)

**Step 2: Replace completedTasks state management**

Find these lines (roughly 56–84):

```tsx
const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");
const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
const createTask = useMutation<Task, TaskCreateParams>("task_create", "params");

const [completedTasks, toggleTask] = useSetToggle(
  tasks.filter((t) => t.completed).map((t) => t.id),
);
// ...
const handleToggleTask = useCallback(
  async (taskId: string) => {
    toggleTask(taskId);
    await toggleComplete.mutate({ id: taskId });
  },
  [toggleTask, toggleComplete],
);
```

Replace with:

```tsx
const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");
const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
const createTask = useMutation<Task, TaskCreateParams>("task_create", "params");

// useOptimistic derives from server state; overlay flips completion for in-flight toggles.
const [optimisticTasks, addOptimistic] = useOptimistic(
  tasks,
  (current, toggledId: string) =>
    current.map((t) => (t.id === toggledId ? { ...t, completed: !t.completed } : t)),
);

const completedTasks = new Set(optimisticTasks.filter((t) => t.completed).map((t) => t.id));

const handleToggleTask = useCallback(
  async (taskId: string) => {
    addOptimistic(taskId);
    await toggleComplete.mutate({ id: taskId });
    refetchTasks();
  },
  [addOptimistic, toggleComplete, refetchTasks],
);
```

Also add `useOptimistic` to the React import at the top of the file:

```tsx
import { useCallback, useEffect, useMemo, useRef, useState, useOptimistic } from "react";
```

Remove the now-unused `useSetToggle` import for `toggleTask` (keep `useSetToggle` if it's still used for `collapsedProjects`).

**Step 3: Verify build**

```bash
cd desktop-ui && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/src/components/views/MainApp.tsx
git commit -m "perf(desktop-ui): replace completedTasks useState with useOptimistic"
```

---

### Task 5: Extract ThreadButton and GroupHeader from Chat

**Files:**
- Create: `desktop-ui/src/components/chat/ThreadButton.tsx`
- Create: `desktop-ui/src/components/chat/GroupHeader.tsx`
- Modify: `desktop-ui/src/components/views/Chat.tsx`

**Step 1: Create ThreadButton.tsx**

```tsx
import { Check, MessageSquare, X } from "lucide-react";
import type { ChatThread } from "../../lib/types";

interface ThreadButtonProps {
  thread: ChatThread;
  isActive: boolean;
  isRenaming: boolean;
  renameValue: string;
  onSelect: (key: string) => void;
  onContextMenu: (e: React.MouseEvent, thread: ChatThread) => void;
  onRenameChange: (value: string) => void;
  onRenameConfirm: () => void;
  onRenameCancel: () => void;
  renameRef?: React.Ref<HTMLInputElement>;
}

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}mo`;
}

export function ThreadButton({
  thread,
  isActive,
  isRenaming,
  renameValue,
  onSelect,
  onContextMenu,
  onRenameChange,
  onRenameConfirm,
  onRenameCancel,
  renameRef,
}: ThreadButtonProps) {
  if (isRenaming) {
    return (
      <div className="flex items-center gap-1 px-2 py-1">
        <input
          ref={renameRef}
          value={renameValue}
          onChange={(e) => onRenameChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onRenameConfirm();
            if (e.key === "Escape") onRenameCancel();
          }}
          className="flex-1 min-w-0 bg-surface-highest text-primary text-[12px] font-light px-2 py-1 rounded border border-border focus:outline-none focus:border-brand"
        />
        <button
          type="button"
          onClick={onRenameConfirm}
          aria-label="Confirm rename"
          className="text-success hover:text-success/80 shrink-0"
        >
          <Check className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
        <button
          type="button"
          onClick={onRenameCancel}
          aria-label="Cancel rename"
          className="text-muted hover:text-secondary shrink-0"
        >
          <X className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={() => onSelect(thread.sessionKey)}
      onContextMenu={(e) => onContextMenu(e, thread)}
      className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
        isActive
          ? "bg-surface-highest text-primary"
          : "text-muted hover:bg-surface-base hover:text-secondary"
      }`}
    >
      <MessageSquare className="w-3 h-3 shrink-0" strokeWidth={1.5} />
      <span className="flex-1 text-left truncate">{thread.title}</span>
      <span className="text-[11px] shrink-0">{formatRelativeTime(thread.updatedAt)}</span>
    </button>
  );
}
```

**Step 2: Create GroupHeader.tsx**

```tsx
import { ChevronDown } from "lucide-react";

interface GroupHeaderProps {
  groupKey: string;
  label: string;
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  isExpanded: boolean;
  onToggle: (key: string) => void;
}

export function GroupHeader({ groupKey, label, icon: Icon, isExpanded, onToggle }: GroupHeaderProps) {
  return (
    <button
      type="button"
      onClick={() => onToggle(groupKey)}
      aria-expanded={isExpanded}
      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
    >
      <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
      <span className="flex-1 text-left">{label}</span>
      <ChevronDown
        className={`w-3.5 h-3.5 transition-transform ${isExpanded ? "rotate-0" : "-rotate-90"}`}
        strokeWidth={1.5}
      />
    </button>
  );
}
```

**Step 3: Update Chat.tsx — remove inline functions, use new components**

In `Chat.tsx`:

1. Remove `formatRelativeTime` function (moved to `ThreadButton.tsx`)
2. Remove the `renderThread` inline function entirely
3. Remove the `renderGroupHeader` inline function entirely
4. Add imports at top:
   ```tsx
   import { ThreadButton } from "../chat/ThreadButton";
   import { GroupHeader } from "../chat/GroupHeader";
   ```
5. Remove the `renameRef` local declaration (it moves to ThreadButton via ref prop)
   - Keep `renameRef` in Chat but pass it down to `ThreadButton` as `renameRef={renameRef}`
6. Replace every call to `renderThread(thread)` with:
   ```tsx
   <ThreadButton
     key={thread.sessionKey}
     thread={thread}
     isActive={selectedThread === thread.sessionKey}
     isRenaming={renaming?.sessionKey === thread.sessionKey}
     renameValue={renaming?.value ?? ""}
     onSelect={selectThread}
     onContextMenu={openContextMenu}
     onRenameChange={(value) => setRenaming(r => r ? { ...r, value } : null)}
     onRenameConfirm={confirmRename}
     onRenameCancel={cancelRename}
     renameRef={renaming?.sessionKey === thread.sessionKey ? renameRef : undefined}
   />
   ```
7. Replace every call to `renderGroupHeader(key, label, Icon)` with:
   ```tsx
   <GroupHeader
     groupKey={key}
     label={label}
     icon={Icon}
     isExpanded={expandedGroups.has(key)}
     onToggle={toggleGroup}
   />
   ```

**Step 4: Verify build**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 5: Commit**

```bash
git add desktop-ui/src/components/chat/ThreadButton.tsx \
        desktop-ui/src/components/chat/GroupHeader.tsx \
        desktop-ui/src/components/views/Chat.tsx
git commit -m "perf(desktop-ui): extract ThreadButton and GroupHeader from Chat"
```

---

## TIER 2: Medium — DX & Composition

---

### Task 6: Stabilize useMutation

**Files:**
- Modify: `desktop-ui/src/hooks/useMutation.ts`

**Step 1: Read current file**

Read `desktop-ui/src/hooks/useMutation.ts` in full before editing.

**Step 2: Wrap returned mutate in useCallback**

The `mutate` function should be stable across renders. Find where `mutate` is defined and wrap the returned function in `useCallback` with an empty dep array (the implementation refs handle reactivity internally). The exact change depends on the current implementation — read the file first.

Pattern to apply:
```ts
// Before
return { mutate, loading, error };

// After — stable reference, consumers don't need useCallback wrappers
const stableMutate = useCallback(mutate, []);
return { mutate: stableMutate, loading, error };
```

If `mutate` already closes over mutable state, use a ref pattern:
```ts
const mutateRef = useRef(mutate);
mutateRef.current = mutate;
const stableMutate = useCallback((...args) => mutateRef.current(...args), []);
```

**Step 3: Verify build**

```bash
cd desktop-ui && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/src/hooks/useMutation.ts
git commit -m "fix(desktop-ui): stabilize useMutation mutate reference"
```

---

### Task 7: TaskTable compound components with context

**Files:**
- Create: `desktop-ui/src/components/tasks/TaskTableContext.tsx`
- Modify: `desktop-ui/src/components/tasks/TaskTable.tsx`
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx` (read first)
- Modify: `desktop-ui/src/components/tasks/AddSubtaskRow.tsx` (read first)

**Step 1: Create TaskTableContext.tsx**

```tsx
import { createContext, use } from "react";
import type { Area, Project, Task, TaskUpdateParams } from "../../lib/types";

export interface TaskTableCtx {
  completedTasks: Set<string>;
  expandedTasks: Set<string>;
  childrenCache: Map<string, Task[]>;
  projects: Project[];
  areas: Area[];
  showArea: boolean;
  onToggleTask: (id: string) => void;
  onToggleExpandTask: (id: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
}

export const TaskTableContext = createContext<TaskTableCtx | null>(null);

export function useTaskTable(): TaskTableCtx {
  const ctx = use(TaskTableContext);
  if (!ctx) throw new Error("useTaskTable must be used inside TaskTable");
  return ctx;
}
```

**Step 2: Update TaskTable.tsx**

1. Add import: `import { TaskTableContext } from "./TaskTableContext";`
2. Keep all 13 props on `TaskTable` (public API unchanged)
3. Wrap the returned `<table>` in a `<TaskTableContext.Provider value={{ completedTasks, expandedTasks, childrenCache, projects, areas, showArea, onToggleTask, onToggleExpandTask, onUpdate, onCreateSubtask }}>`
4. Update `TaskGroup` and `TaskWithSubtasks` props: remove all callback/set props from their interfaces and destructuring — they will read from context instead
5. Update `TaskGroup` call sites to only pass `header`, `tasks`, `isCollapsed`
6. Update `TaskWithSubtasks` call sites to only pass `task`

**Step 3: Update TaskGroup to read from context**

In `TaskTable.tsx`, the `TaskGroup` internal component:
- Remove from its props: `childrenCache`, `projects`, `areas`, `completedTasks`, `expandedTasks`, `showArea`, `onToggleTask`, `onToggleExpandTask`, `onUpdate`, `onCreateSubtask`
- Add at top of function body: `const { showArea, ... } = useTaskTable();`
- Keep: `header`, `tasks`, `isCollapsed`

**Step 4: Update TaskWithSubtasks to read from context**

Same pattern — remove all props except `task`, pull everything from `useTaskTable()`.

**Step 5: Verify build**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 6: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskTableContext.tsx \
        desktop-ui/src/components/tasks/TaskTable.tsx
git commit -m "refactor(desktop-ui): TaskTable compound components with context"
```

---

### Task 8: Explicit TaskRow variants

**Files:**
- Read first: `desktop-ui/src/components/tasks/TaskRow.tsx`
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx`
- Modify: `desktop-ui/src/components/tasks/TaskTable.tsx`

**Step 1: Read TaskRow.tsx in full**

**Step 2: Extract variants**

In `TaskRow.tsx`:
- Identify all logic gated on `depth === 0` vs `depth === 1` (indent, expand button, etc.)
- Create `RootTaskRow` component: same as current TaskRow with `depth=0` logic hardcoded, no `depth` prop
- Create `SubtaskRow` component: same as current TaskRow with `depth=1` logic hardcoded, no `depth` prop, no expand button
- Remove `depth` prop from both
- Keep the existing `TaskRow` export as a thin dispatcher if it's used outside `TaskTable` (check with grep), otherwise remove it

```bash
grep -r "TaskRow" desktop-ui/src --include="*.tsx" -l
```

**Step 3: Update TaskWithSubtasks to use variants**

In `TaskTable.tsx`, `TaskWithSubtasks`:
- Root task: `<RootTaskRow task={task} />`
- Subtasks map: `<SubtaskRow key={sub.id} task={sub} />`

**Step 4: Verify build**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 5: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskRow.tsx \
        desktop-ui/src/components/tasks/TaskTable.tsx
git commit -m "refactor(desktop-ui): explicit RootTaskRow and SubtaskRow variants"
```

---

### Task 9: Decompose Chat.tsx

**Files:**
- Create: `desktop-ui/src/components/chat/ChatInput.tsx`
- Create: `desktop-ui/src/components/chat/ThreadContextMenu.tsx`
- Create: `desktop-ui/src/components/chat/ThreadList.tsx`
- Modify: `desktop-ui/src/components/views/Chat.tsx`

**Step 1: Create ChatInput.tsx**

Extract the entire input area from Chat.tsx (the `<div className="p-6">` block at the bottom of the message panel). Props:

```tsx
interface ChatInputProps {
  input: string;
  isStreaming: boolean;
  onInputChange: (value: string) => void;
  onSend: () => void;
}
```

Move the `<textarea>` with auto-resize behavior (implemented in Task 11) here. Add `aria-label="Message input"` to textarea.

**Step 2: Create ThreadContextMenu.tsx**

Extract the context menu block (the `{contextMenu && (...)}` div). Props:

```tsx
interface ThreadContextMenuProps {
  x: number;
  y: number;
  thread: ChatThread;
  onRename: (thread: ChatThread) => void;
  onDelete: (sessionKey: string) => void;
  onClose: () => void;
}
```

Fix accessibility: change outer container to `role="menu"`, change inner `<button>` elements to use `role="menuitem"`, add keyboard nav (ArrowUp/Down, Escape):

```tsx
export function ThreadContextMenu({ x, y, thread, onRename, onDelete, onClose }: ThreadContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = menuRef.current?.querySelector<HTMLElement>("[role=menuitem]");
    el?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    const items = menuRef.current?.querySelectorAll<HTMLElement>("[role=menuitem]");
    if (!items) return;
    const arr = Array.from(items);
    const idx = arr.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") { e.preventDefault(); arr[(idx + 1) % arr.length]?.focus(); }
    if (e.key === "ArrowUp") { e.preventDefault(); arr[(idx - 1 + arr.length) % arr.length]?.focus(); }
    if (e.key === "Escape") onClose();
  };

  return (
    <div
      ref={menuRef}
      role="menu"
      onMouseDown={(e) => e.stopPropagation()}
      onKeyDown={handleKeyDown}
      className="fixed z-50 bg-surface-raised border border-border rounded-lg shadow-lg py-1 min-w-[140px]"
      style={{ left: x, top: y }}
    >
      <button type="button" role="menuitem" onClick={() => onRename(thread)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-secondary hover:bg-surface-base transition-colors">
        <Pencil className="w-3 h-3" strokeWidth={1.5} /> Rename
      </button>
      <button type="button" role="menuitem" onClick={() => onDelete(thread.sessionKey)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-destructive hover:bg-surface-base transition-colors">
        <Trash2 className="w-3 h-3" strokeWidth={1.5} /> Delete
      </button>
    </div>
  );
}
```

**Step 3: Create ThreadList.tsx**

Extract the entire left sidebar thread list (`<div className="w-[250px]...">`) into `ThreadList`. Props: `threads`, `selectedThread`, `expandedGroups`, `renaming`, `renameRef`, `onSelectThread`, `onNewThread`, `onToggleGroup`, `onContextMenu`, `onRenameChange`, `onRenameConfirm`, `onRenameCancel`.

Uses `ThreadButton` and `GroupHeader` already extracted in Task 5.

**Step 4: Update Chat.tsx to compose sub-components**

After extraction, `Chat.tsx` should only contain:
- State declarations
- Event handlers
- Top-level layout: `<Sidebar>` + `<ThreadList>` + message panel + `<TransparencyPanel>`
- Target: under 200 lines

**Step 5: Verify build**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 6: Commit**

```bash
git add desktop-ui/src/components/chat/ChatInput.tsx \
        desktop-ui/src/components/chat/ThreadContextMenu.tsx \
        desktop-ui/src/components/chat/ThreadList.tsx \
        desktop-ui/src/components/views/Chat.tsx
git commit -m "refactor(desktop-ui): decompose Chat.tsx into focused sub-components"
```

---

## TIER 3: Polish — UX & Accessibility

---

### Task 10: Accessibility sweep — aria-labels and aria-expanded

**Files:**
- Modify: `desktop-ui/src/components/views/Chat.tsx` and sub-components
- Modify: `desktop-ui/src/components/views/MainApp.tsx`
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` (read first)
- Modify: `desktop-ui/src/components/chat/SidebarChat.tsx` (read first)
- And any other files found in step 1

**Step 1: Find all icon-only buttons missing aria-label**

```bash
grep -rn "type=\"button\"" desktop-ui/src --include="*.tsx" -l
```

For each file, look for buttons that contain only an icon component (Lucide icons) with no visible text. These all need `aria-label`.

Common patterns to find and fix:
```tsx
// Before
<button type="button" onClick={...}>
  <MessageSquare className="w-4 h-4" />
</button>

// After
<button type="button" onClick={...} aria-label="Open chat">
  <MessageSquare className="w-4 h-4" />
</button>
```

**Step 2: Add aria-expanded to all collapsible controls**

Search for `collapsedProjects`, `expandedGroups`, toggle patterns. Wherever a button controls show/hide of content:
```tsx
<button aria-expanded={!isCollapsed} ...>
```

**Step 3: Verify build and lint**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 4: Commit**

```bash
git add -u desktop-ui/src
git commit -m "a11y(desktop-ui): add aria-labels and aria-expanded throughout"
```

---

### Task 11: Auto-resize textarea

**Files:**
- Modify: `desktop-ui/src/components/chat/ChatInput.tsx`
- Modify: `desktop-ui/src/components/chat/SidebarChat.tsx` (read first — has its own textarea)

**Step 1: Add auto-resize to ChatInput textarea**

Replace the static `textarea` with an auto-resizing version:

```tsx
const textareaRef = useRef<HTMLTextAreaElement>(null);

// Resize on input
const handleInput = useCallback(() => {
  const el = textareaRef.current;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight}px`;
}, []);

// Reset height after send
useEffect(() => {
  if (!input && textareaRef.current) {
    textareaRef.current.style.height = "auto";
  }
}, [input]);
```

Update textarea JSX:
```tsx
<textarea
  ref={textareaRef}
  value={input}
  onChange={(e) => onInputChange(e.target.value)}
  onInput={handleInput}
  onKeyDown={...}
  aria-label="Message input"
  placeholder="Ask Klynt anything, @ to add files, / for commands"
  rows={1}
  className="flex-1 bg-transparent py-3.5 text-[13px] text-primary placeholder:text-muted focus:outline-none font-light resize-none overflow-hidden"
  style={{ maxHeight: "200px" }}
/>
```

Note: `overflow-hidden` prevents scrollbar flash during resize. `resize-none` prevents manual resize handle.

**Step 2: Apply same pattern to SidebarChat textarea**

Read `SidebarChat.tsx` first, then apply the same auto-resize pattern to its input.

**Step 3: Verify build**

```bash
cd desktop-ui && bun run build
```

**Step 4: Commit**

```bash
git add desktop-ui/src/components/chat/ChatInput.tsx \
        desktop-ui/src/components/chat/SidebarChat.tsx
git commit -m "ux(desktop-ui): auto-resize textarea in chat inputs"
```

---

### Task 12: Loading skeleton + error UI + useTransition

**Files:**
- Create: `desktop-ui/src/components/tasks/TaskTableSkeleton.tsx`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`
- Modify: `desktop-ui/src/components/views/Chat.tsx`

**Step 1: Create TaskTableSkeleton.tsx**

```tsx
export function TaskTableSkeleton() {
  return (
    <div className="mb-10 rounded-xl">
      <table className="w-full bg-surface-low border-collapse">
        <thead>
          <tr className="border-b border-border text-[11px] text-muted font-light text-left">
            <th className="px-5 py-3 w-9 font-light" />
            <th className="px-5 py-3 font-light">Task</th>
            <th className="px-5 py-3 font-light">Project</th>
            <th className="px-5 py-3 font-light">Priority</th>
            <th className="px-5 py-3 font-light">Status</th>
            <th className="px-5 py-3 font-light">Due Date</th>
            <th className="px-5 py-3 font-light">Tags</th>
          </tr>
        </thead>
        <tbody>
          {Array.from({ length: 5 }).map((_, i) => (
            <tr key={i} className="border-b border-border-subtle">
              <td className="px-5 py-3 w-9">
                <div className="w-4 h-4 rounded bg-surface-raised animate-pulse" />
              </td>
              <td className="px-5 py-3">
                <div className="h-3 rounded bg-surface-raised animate-pulse" style={{ width: `${60 + (i * 13) % 30}%` }} />
              </td>
              {[1,2,3,4,5].map(j => (
                <td key={j} className="px-5 py-3">
                  <div className="h-3 w-16 rounded bg-surface-raised animate-pulse" />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

**Step 2: Use skeleton + error in MainApp**

In `MainApp.tsx`:

1. Add `useTransition` import: `import { ..., useTransition, useOptimistic } from "react";`
2. Add `const [isPending, startTransition] = useTransition();`
3. Wrap tab change in transition:
   ```tsx
   onClick={() => startTransition(() => setActiveTab(tab))}
   ```
4. Show skeleton while loading:
   ```tsx
   const { data: tasks, loading: tasksLoading, error: tasksError, refetch: refetchTasks } = useQuery<Task[]>(...);

   // In render, replace the TaskTable with:
   {tasksLoading ? (
     <TaskTableSkeleton />
   ) : tasksError ? (
     <div className="flex flex-col items-center py-10 gap-2">
       <p className="text-muted text-sm font-light">Failed to load tasks</p>
       <button type="button" onClick={refetchTasks}
         className="text-brand text-xs font-light hover:underline">
         Retry
       </button>
     </div>
   ) : (
     <TaskTable ... />
   )}
   ```

**Step 3: Add error state to Chat thread list**

In `ThreadList.tsx`, the thread list receives `loading` and `error` props from Chat. Show inline error with retry when `error` is set.

**Step 4: Verify build and lint**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 5: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskTableSkeleton.tsx \
        desktop-ui/src/components/views/MainApp.tsx \
        desktop-ui/src/components/views/Chat.tsx \
        desktop-ui/src/components/chat/ThreadList.tsx
git commit -m "ux(desktop-ui): add loading skeleton, error retry UI, useTransition for tabs"
```

---

### Task 13: Fix && conditionals → ternaries

**Files:** Multiple — find with grep

**Step 1: Find all boolean && JSX patterns**

```bash
grep -rn "&&\s*<" desktop-ui/src --include="*.tsx"
```

**Step 2: For each match, determine if the left side could be a number**

Safe (always boolean): `isOpen && <Panel />`, `condition && <Badge />`
Unsafe (could be 0): `count && <Badge>{count}</Badge>`, `items.length && <List />`

Replace unsafe patterns with ternaries:
```tsx
// Before
{items.length && <List items={items} />}

// After
{items.length > 0 ? <List items={items} /> : null}
```

**Step 3: Verify build and lint**

```bash
cd desktop-ui && bun run build && bun run lint
```

**Step 4: Commit**

```bash
git add -u desktop-ui/src
git commit -m "fix(desktop-ui): replace && conditionals with ternaries to prevent 0 renders"
```

---

## Success Criteria

| Tier | Criterion |
|------|-----------|
| 1 | `bun run build` emits multiple lazy chunks; `useOptimistic` replaces completedTasks useState; ThreadButton/GroupHeader are named components |
| 2 | `Chat.tsx` < 200 lines; TaskTable children read from context not props; RootTaskRow and SubtaskRow exist |
| 3 | No icon-only button without `aria-label`; textarea grows with content; TaskTable shows skeleton on load; no `&&` rendering `0` |

## Execution Order

Tasks must be executed in order (1→13). Each task leaves the app in a buildable, working state. Commit after every task — do not batch commits.
