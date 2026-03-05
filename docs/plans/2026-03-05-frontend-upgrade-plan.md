# Desktop UI Comprehensive Frontend Upgrade — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the desktop-ui frontend to comply with Web Interface Guidelines, improve UX polish, optimize performance, enhance the data layer, fix accessibility gaps, and improve component patterns.

**Architecture:** Incremental per-file changes organized in dependency order — foundation CSS/HTML first, then data hooks, then component fixes, then new features. No new dependencies added. Breaking changes acceptable (pre-release).

**Tech Stack:** React 19 + React Compiler, Tailwind v4 (CSS-driven), React Router v7, Tauri IPC, Biome

**Verification:** After each task, run `cd desktop-ui && bun run lint` to check for errors. After each group, run `cd desktop-ui && bun run build` to verify compilation.

---

## Task 1: Foundation — `index.html` dark mode + theme-color

**Files:**
- Modify: `desktop-ui/index.html:2,4-6`

**Step 1: Add color-scheme and theme-color**

Replace the `<html>` tag and add meta tag:

```html
<html lang="en" style="color-scheme: dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="theme-color" content="#0e0e0d" />
    <title>Klynt</title>
  </head>
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`

**Step 3: Commit**

```
feat(desktop-ui): add color-scheme dark and theme-color meta
```

---

## Task 2: Foundation — `theme.css` prefers-reduced-motion + focus ring + typography

**Files:**
- Modify: `desktop-ui/src/styles/theme.css:102-156` (base layer), append after L208

**Step 1: Add prefers-reduced-motion at end of file (after L208)**

```css
/* ── Reduced motion ──────────────────────────── */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

**Step 2: Add text-wrap to headings in base layer (L115-137)**

Update each heading rule to include `text-wrap: balance`:

```css
  h1 {
    font-size: var(--text-2xl);
    font-weight: var(--font-weight-medium);
    line-height: 1.5;
    text-wrap: balance;
  }

  h2 {
    font-size: var(--text-xl);
    font-weight: var(--font-weight-medium);
    line-height: 1.5;
    text-wrap: balance;
  }

  h3 {
    font-size: var(--text-lg);
    font-weight: var(--font-weight-medium);
    line-height: 1.5;
    text-wrap: balance;
  }

  h4 {
    font-size: var(--text-base);
    font-weight: var(--font-weight-medium);
    line-height: 1.5;
    text-wrap: balance;
  }
```

**Step 3: Add global focus ring styles in base layer (after L155, before closing `}`)**

Add base focus styles for interactive elements:

```css
  /* Focus ring — visible on keyboard nav only */
  button:focus-visible,
  input:focus-visible,
  textarea:focus-visible,
  select:focus-visible,
  [role="button"]:focus-visible,
  [tabindex="0"]:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--brand) 50%, transparent);
    outline-offset: 2px;
  }
```

**Step 4: Add tabular-nums utility in utilities layer (after glass-panel, before closing)**

```css
  .tabular-nums {
    font-variant-numeric: tabular-nums;
  }
```

**Step 5: Add fade-in keyframe (after distraction-pulse)**

```css
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
```

**Step 6: Lint and verify**

Run: `cd desktop-ui && bun run lint && bun run build`

**Step 7: Commit**

```
feat(desktop-ui): add reduced-motion, focus ring, text-wrap, tabular-nums, fade-in
```

---

## Task 3: Data layer — enhance `useQuery` with cache + dedup + SWR

**Files:**
- Modify: `desktop-ui/src/hooks/useQuery.ts` (full rewrite)

**Step 1: Rewrite useQuery with cache and dedup**

```typescript
import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiError } from "../lib/types";
import { parseApiError } from "../lib/utils";
import { ipc } from "./useIpc";

interface QueryResult<T> {
  data: T;
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
}

interface CacheEntry<T> {
  data: T;
  timestamp: number;
  promise?: Promise<T>;
}

const cache = new Map<string, CacheEntry<unknown>>();
const DEFAULT_STALE_TIME = 30_000; // 30s

function cacheKey(cmd: string, args?: Record<string, unknown> | null): string | null {
  if (args === null) return null;
  return args === undefined ? cmd : `${cmd}:${JSON.stringify(args)}`;
}

/**
 * Fetches data from a Tauri command with SWR caching and request dedup.
 *
 * Pass `null` for `args` to skip fetching.
 * Pass `undefined` for commands that take no arguments.
 */
export function useQuery<T>(
  cmd: string,
  args?: Record<string, unknown> | null,
  fallback?: T,
  staleTime = DEFAULT_STALE_TIME,
): QueryResult<T> {
  const key = cacheKey(cmd, args);
  const cached = key ? (cache.get(key) as CacheEntry<T> | undefined) : undefined;
  const isStale = !cached || Date.now() - cached.timestamp > staleTime;

  const [data, setData] = useState<T>(cached?.data ?? (fallback as T));
  const [loading, setLoading] = useState(args !== null && !cached);
  const [error, setError] = useState<ApiError | null>(null);

  const argsRef = useRef(args);
  argsRef.current = args;
  const fallbackRef = useRef(fallback);
  fallbackRef.current = fallback;
  const keyRef = useRef(key);
  keyRef.current = key;

  const doFetch = useCallback(() => {
    const k = keyRef.current;
    if (k === null) return;

    // Dedup: if there's already an in-flight request for this key, reuse it
    const existing = cache.get(k) as CacheEntry<T> | undefined;
    if (existing?.promise) {
      existing.promise.then(setData).catch((e) => setError(parseApiError(e)));
      return;
    }

    setError(null);
    // Only show loading if we have no cached data to display
    if (!existing?.data) setLoading(true);

    const promise = ipc<T>(cmd, argsRef.current ?? undefined);

    // Store in-flight promise for dedup
    cache.set(k, { ...existing, promise } as CacheEntry<unknown>);

    promise
      .then((result) => {
        cache.set(k, { data: result as unknown, timestamp: Date.now() });
        setData(result);
      })
      .catch((e) => {
        // Clear failed promise but keep stale data
        if (existing) cache.set(k, { data: existing.data, timestamp: existing.timestamp });
        else cache.delete(k);
        setError(parseApiError(e));
      })
      .finally(() => setLoading(false));
  }, [cmd]);

  // Re-fetch when cmd or args change
  const argsKey = args === null ? null : args === undefined ? "" : JSON.stringify(args);

  useEffect(() => {
    if (argsKey === null) {
      setData(fallbackRef.current as T);
      setLoading(false);
      return;
    }
    // Return cached data immediately, fetch in background if stale
    const k = keyRef.current;
    if (k) {
      const entry = cache.get(k) as CacheEntry<T> | undefined;
      if (entry?.data !== undefined) setData(entry.data);
    }
    if (isStale) doFetch();
  }, [doFetch, argsKey, isStale]);

  return { data, loading, error, refetch: doFetch };
}

/** Invalidate all cache entries matching a command prefix. */
export function invalidateQueries(cmdPrefix: string) {
  for (const k of cache.keys()) {
    if (k.startsWith(cmdPrefix)) cache.delete(k);
  }
}
```

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
feat(desktop-ui): add SWR cache + request dedup to useQuery
```

---

## Task 4: Data layer — stabilize `useMutation` return

**Files:**
- Modify: `desktop-ui/src/hooks/useMutation.ts:48`

**Step 1: Wrap return in useMemo**

Add `useMemo` import, then replace line 48:

```typescript
import { useCallback, useMemo, useRef, useState } from "react";
```

And replace the return statement (L48):

```typescript
  return useMemo(() => ({ mutate, loading, error }), [mutate, loading, error]);
```

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
fix(desktop-ui): stabilize useMutation return with useMemo
```

---

## Task 5: Fix TaskDetail data fetching waterfall

**Files:**
- Modify: `desktop-ui/src/components/views/TaskDetail.tsx:17,32`

**Step 1: Replace task_list with task_get**

Change line 17 from:
```typescript
  const { data: tasks, refetch } = useQuery<Task[]>("task_list", undefined, []);
```
To:
```typescript
  const { data: task, refetch } = useQuery<Task | null>("task_get", id ? { id } : null, null);
```

**Step 2: Remove the useMemo derivation**

Delete line 32:
```typescript
  const task = useMemo(() => tasks.find((t) => t.id === id), [tasks, id]);
```

**Step 3: Lint and verify**

Run: `cd desktop-ui && bun run lint`

**Step 4: Commit**

```
perf(desktop-ui): fetch single task by ID instead of full list in TaskDetail
```

---

## Task 6: Stabilize TaskTable context value

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskTable.tsx:1,76-87`

**Step 1: Add useMemo import**

Line 2 already imports `useMemo`. Good.

**Step 2: Wrap ctx in useMemo**

Replace lines 76-87:

```typescript
  const ctx = useMemo<import("./TaskTableContext").TaskTableCtx>(
    () => ({
      completedTasks,
      expandedTasks,
      childrenCache,
      projects,
      areas,
      showArea,
      onToggleTask,
      onToggleExpandTask,
      onUpdate,
      onCreateSubtask,
    }),
    [completedTasks, expandedTasks, childrenCache, projects, areas, showArea, onToggleTask, onToggleExpandTask, onUpdate, onCreateSubtask],
  );
```

**Step 3: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 4: Commit**

```
perf(desktop-ui): memoize TaskTable context value to prevent cascading re-renders
```

---

## Task 7: Fix ProjectDetail stale completion state

**Files:**
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx:66-68,137,444`

**Step 1: Replace useSetToggle with server-derived completion**

Remove line 66-68:
```typescript
  const [completedTasks, toggleTask] = useSetToggle(
    tasks.filter((t) => t.completed).map((t) => t.id),
  );
```

Replace with a derived set that stays in sync:
```typescript
  const completedTasks = useMemo(
    () => new Set(tasks.filter((t) => t.completed).map((t) => t.id)),
    [tasks],
  );
```

**Step 2: Update handleToggleTask**

Replace the `handleToggleTask` callback (L80-86). Since we no longer have `toggleTask`, just call the mutation and refetch:

```typescript
  const handleToggleTask = useCallback(
    async (taskId: string) => {
      await toggleComplete.mutate({ id: taskId });
      refetchTasks();
    },
    [toggleComplete.mutate, refetchTasks],
  );
```

**Step 3: Remove useSetToggle import if no longer used**

Check if `useSetToggle` is still used for `expandedOkrs`. It is (L36), so keep the import but remove the `toggleTask` reference.

**Step 4: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 5: Commit**

```
fix(desktop-ui): derive ProjectDetail completion from server state instead of stale snapshot
```

---

## Task 8: Fix module-level date bug in App.tsx

**Files:**
- Modify: `desktop-ui/src/App.tsx:109-112`

**Step 1: Replace static Navigate with a redirect component**

Replace lines 109-112:

```typescript
  {
    path: "/productivity",
    element: <ProductivityRedirect />,
  },
```

Add above the router definition (before `const router = ...`):

```typescript
function ProductivityRedirect() {
  const today = new Date().toISOString().slice(0, 10);
  return <Navigate to={`/productivity/day/${today}`} replace />;
}
```

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
fix(desktop-ui): compute productivity redirect date at render time, not module load
```

---

## Task 9: Fix MessageList auto-scroll

**Files:**
- Modify: `desktop-ui/src/components/chat/MessageList.tsx:43-45`

**Step 1: Add scroll deps and scroll-to-bottom logic**

Replace the useEffect (L43-45):

```typescript
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages/streaming, unless user scrolled up
  useEffect(() => {
    if (!userScrolledUp) {
      endRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages.length, segments.length, isStreaming, userScrolledUp]);

  // Detect user scroll position
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
    setUserScrolledUp(!isNearBottom);
  }, []);
```

**Step 2: Update the container div (L48)**

Change:
```tsx
    <div className="space-y-6">
```
To:
```tsx
    <div ref={containerRef} onScroll={handleScroll} className="space-y-6" aria-live="polite">
```

**Step 3: Add scroll-to-bottom button before the end ref (before L133)**

```tsx
      {userScrolledUp && (
        <div className="sticky bottom-2 flex justify-center">
          <button
            type="button"
            onClick={() => {
              endRef.current?.scrollIntoView({ behavior: "smooth" });
              setUserScrolledUp(false);
            }}
            className="px-3 py-1.5 rounded-full bg-surface-raised text-[11px] text-muted font-light hover:bg-surface-highest transition-colors"
            aria-label="Scroll to bottom"
          >
            Scroll to bottom
          </button>
        </div>
      )}
```

**Step 4: Add useState and useCallback to imports**

Ensure `useState` and `useCallback` are in the import (L1).

**Step 5: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 6: Commit**

```
feat(desktop-ui): auto-scroll chat on new messages with scroll-to-bottom button
```

---

## Task 10: Replace all `transition-all` with specific properties

**Files:**
- Modify: `desktop-ui/src/components/ui/Progress.tsx` — `transition-all` → `transition-[width]`
- Modify: `desktop-ui/src/components/productivity/TopApps.tsx` — `transition-all` → `transition-[width]`
- Modify: `desktop-ui/src/components/productivity/WorkHoursCard.tsx` — `transition-all` → `transition-[width]`
- Modify: `desktop-ui/src/components/productivity/PomodoroTimer.tsx` — `transition-all` → `transition-[width]`
- Modify: `desktop-ui/src/components/productivity/GoalsProgress.tsx` — `transition-all` → `transition-[width]`
- Modify: `desktop-ui/src/components/productivity/ProductivityScoreRing.tsx` — L24: `transition-all` → `transition-[width]`, L83: `transition-all` → `transition-[stroke-dashoffset]`
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx` — L182,195: `transition-all` → `transition-shadow`
- Modify: `desktop-ui/src/components/views/Launcher.tsx` — `transition-all` → `transition-colors`
- Modify: `desktop-ui/src/components/views/FinanceLiabilities.tsx` — `transition-all` → `transition-[width]`

**Step 1: Search and replace each file**

In each file, find `transition-all` and replace with the specific property listed above.

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
fix(desktop-ui): replace transition-all with specific transition properties
```

---

## Task 11: Typography — replace ASCII `...` with `\u2026`

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskTable.tsx:201` — `Loading...` → `Loading\u2026`
- Modify: `desktop-ui/src/components/distraction/DistractionOverlay.tsx` — truncation `...` → `\u2026`
- Modify: `desktop-ui/src/components/settings/mcp/McpServerCard.tsx` — `Waiting...` → `Waiting\u2026`
- Search all files for `...` in user-visible strings (not spread operators)

**Step 1: Fix each instance**

Search for `"..."` and `'...'` patterns in JSX text and template literals. Replace with `\u2026` or the literal `…` character. Do NOT touch spread operators (`...obj`), rest parameters, or JSX props.

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
fix(desktop-ui): use Unicode ellipsis character in loading states and truncation
```

---

## Task 12: Keyboard navigation for clickable rows

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx:48-51,219-222`
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx:82,239` (add `min-w-0`)

**Step 1: Add keyboard nav to RootTaskRow `<tr>` (L48-51)**

Replace:
```tsx
    <tr
      onClick={() => navigate(`/task/${task.id}`)}
      className="hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0 cursor-pointer whitespace-nowrap"
    >
```
With:
```tsx
    <tr
      onClick={() => navigate(`/task/${task.id}`)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          navigate(`/task/${task.id}`);
        }
      }}
      tabIndex={0}
      role="link"
      className="hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0 cursor-pointer whitespace-nowrap"
    >
```

**Step 2: Same for SubtaskRow `<tr>` (L219-222)**

Add `onKeyDown`, `tabIndex={0}`, `role="link"` — same pattern.

**Step 3: Add `min-w-0` to flex text containers**

Line 82: change `<div className="flex items-center gap-1.5">` to `<div className="flex items-center gap-1.5 min-w-0">`

Line 239: change `<div className="flex items-center gap-1.5 pl-6">` to `<div className="flex items-center gap-1.5 pl-6 min-w-0">`

**Step 4: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 5: Commit**

```
a11y(desktop-ui): add keyboard navigation to task rows and fix text truncation
```

---

## Task 13: Accessibility — aria-labels on icon buttons

**Files:**
- Modify: `desktop-ui/src/components/productivity/DateNavigator.tsx` — add `aria-label` to prev/today/next buttons
- Modify: `desktop-ui/src/components/tasks/editors/MiniCalendar.tsx` — add `aria-label` to prev/next month + day buttons
- Modify: `desktop-ui/src/components/settings/mcp/AddServerDialog.tsx` — change `title` to `aria-label` on icon buttons
- Modify: `desktop-ui/src/components/settings/mcp/McpServerCard.tsx` — change `title` to `aria-label`
- Modify: `desktop-ui/src/components/tasks/editors/InlineDatePicker.tsx` — add `aria-label="Select date"`, `aria-haspopup="dialog"`, `aria-expanded`
- Modify: `desktop-ui/src/components/tasks/editors/InlineSelect.tsx` — add `aria-label`, `aria-haspopup="listbox"`, `aria-expanded`
- Modify: `desktop-ui/src/components/views/Launcher.tsx` — add `aria-label="Search commands"` to input
- Modify: `desktop-ui/src/components/views/LauncherChat.tsx` — add `aria-label="Message Klynt"` to textarea
- Modify: `desktop-ui/src/components/views/SystemTray.tsx` — add `aria-label="Ask Klynt"` to input

**Step 1: Apply each fix**

For each file, add the appropriate `aria-label` attribute. For buttons that use `title=`, replace with `aria-label=`. For InlineSelect/InlineDatePicker, add `aria-haspopup` and `aria-expanded={open}`.

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
a11y(desktop-ui): add aria-labels to icon buttons, inline editors, and inputs
```

---

## Task 14: Accessibility — decorative icons + live regions + dialog semantics

**Files:**
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` — add `aria-hidden="true"` to Lucide icons inside labeled buttons
- Modify: `desktop-ui/src/components/views/Launcher.tsx` — add `aria-hidden="true"` to result item icons
- Modify: `desktop-ui/src/components/tasks/TaskTableSkeleton.tsx` — add `aria-busy="true"` and `aria-label="Loading tasks"` to table
- Modify: `desktop-ui/src/components/settings/mcp/AddServerDialog.tsx` — add `role="dialog"`, `aria-modal="true"`, `aria-labelledby`
- Modify: `desktop-ui/src/components/finance/FinanceLayout.tsx` — add `role="tablist"` / `role="tab"` / `aria-selected`
- Modify: `desktop-ui/src/components/views/TaskDetail.tsx` — associate labels with `htmlFor`/`id` on due date input and project select
- Modify: `desktop-ui/src/components/settings/mcp/AddServerDialog.tsx:208` — change URL field `type="text"` to `type="url"`

**Step 1: Apply each fix per file**

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
a11y(desktop-ui): add aria-hidden, live regions, dialog semantics, and form associations
```

---

## Task 15: Native select styling + tabular-nums

**Files:**
- Modify: `desktop-ui/src/components/views/TaskDetail.tsx:156` — add `bg-surface-low text-secondary` to select (already has it, verify)
- Modify: `desktop-ui/src/components/views/FinanceTransactions.tsx` — add explicit bg/color to `<select>`
- Modify: `desktop-ui/src/components/views/Finance.tsx` — add `tabular-nums` class to financial values
- Modify: `desktop-ui/src/components/views/FinanceTransactions.tsx` — add `tabular-nums` to amount/date columns
- Modify: `desktop-ui/src/components/productivity/PomodoroTimer.tsx` — add `tabular-nums` to timer display

**Step 1: Apply each fix**

**Step 2: Lint and build**

Run: `cd desktop-ui && bun run lint && bun run build`

**Step 3: Commit**

```
fix(desktop-ui): style native selects for dark mode and add tabular-nums to numeric displays
```

---

## Task 16: URL-reflected state in MainApp and Chat

**Files:**
- Modify: `desktop-ui/src/components/views/MainApp.tsx:2,38,43,191-204`
- Modify: `desktop-ui/src/components/views/Chat.tsx` (selected thread → URL)

**Step 1: MainApp — persist tab and viewMode in URL**

Add `useSearchParams` import:
```typescript
import { useNavigate, useSearchParams } from "react-router";
```

Replace state with URL-derived values:
```typescript
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = (searchParams.get("tab") as Tab) || "All";
  const viewMode = (searchParams.get("view") as ViewMode) || "table";

  const setActiveTab = (tab: Tab) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      if (tab === "All") next.delete("tab");
      else next.set("tab", tab);
      return next;
    });
  };

  const setViewMode = (mode: ViewMode) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev);
      if (mode === "table") next.delete("view");
      else next.set("view", mode);
      return next;
    });
  };
```

Remove the `useState` lines for `activeTab` and `viewMode` (L38, L43).

**Step 2: Chat — persist selected thread in URL**

Similarly use `useSearchParams` for `selectedThread`. Read thread ID from `?thread=`, write on selection.

**Step 3: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 4: Commit**

```
feat(desktop-ui): persist tab, view mode, and chat thread in URL search params
```

---

## Task 17: Destructive action confirmation

**Files:**
- Modify: `desktop-ui/src/components/views/Chat.tsx` — thread deletion: add confirmation step
- Modify: `desktop-ui/src/components/settings/pages/McpServersSettings.tsx` — server removal: add confirmation

**Step 1: Thread deletion**

Before calling `ipc("chat_delete_thread")`, add a `window.confirm()` gate or use a two-click pattern (like `TaskDetail` already does for delete).

**Step 2: Server removal**

Same pattern — add confirmation before `handleRemove` executes.

**Step 3: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 4: Commit**

```
fix(desktop-ui): add confirmation step for destructive actions (thread delete, server remove)
```

---

## Task 18: Remove redundant memoization (React Compiler cleanup)

**Files:**
- Modify: `desktop-ui/src/components/tasks/editors/InlineTextEditor.tsx` — remove `useCallback` wrappers (L14, L23)
- Modify: `desktop-ui/src/components/tasks/editors/InlineSelect.tsx` — remove `useCallback(() => setOpen(false), [])` (L29)
- Modify: `desktop-ui/src/components/tasks/editors/InlineDatePicker.tsx` — remove `useCallback(() => setOpen(false), [])` (L15)
- Modify: `desktop-ui/src/hooks/useAutoResizeTextarea.ts` — remove `useCallback` wrapper (L7)
- Modify: `desktop-ui/src/components/chat/ThreadList.tsx` — remove `useCallback` from `renderThread` (L61)
- Modify: `desktop-ui/src/components/chat/Chat.tsx` — remove passthrough `useCallback` for `handleSend` (L118)

**Step 1: In each file, remove the `useCallback`/`useMemo` wrapper and use plain functions/expressions**

For example, in `InlineTextEditor.tsx`:
```typescript
// Before:
const startEdit = useCallback(() => { ... }, [value]);
// After:
const startEdit = () => { ... };
```

Remove unused `useCallback` imports where applicable.

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
refactor(desktop-ui): remove redundant memoization handled by React Compiler
```

---

## Task 19: Combine FinanceTransactions array iterations

**Files:**
- Modify: `desktop-ui/src/components/views/FinanceTransactions.tsx:54-80`

**Step 1: Unify three useMemo calls into one**

Replace the three separate `useMemo` for `totalIncome`, `totalExpense`, `catSegs` with:

```typescript
  const { totalIncome, totalExpense, catSegs } = useMemo(() => {
    let income = 0;
    let expense = 0;
    const catMap = new Map<string, number>();

    for (const tx of filtered) {
      if (tx.amount >= 0) income += tx.amount;
      else expense += Math.abs(tx.amount);
      const cat = tx.category ?? "Uncategorized";
      catMap.set(cat, (catMap.get(cat) ?? 0) + Math.abs(tx.amount));
    }

    const catSegs = Array.from(catMap.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([name, value]) => ({ name, value }));

    return { totalIncome: income, totalExpense: expense, catSegs };
  }, [filtered]);
```

**Step 2: Lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Commit**

```
perf(desktop-ui): unify FinanceTransactions derived data into single pass
```

---

## Task 20: Remove per-component focus:outline-none

**Files:** Multiple — search for `focus:outline-none` across all components and remove them, since the global base focus ring from Task 2 now handles this.

**Step 1: Search and clean up**

Run grep to find all instances. For each:
- Remove `focus:outline-none` from class strings
- Remove `focus:border-brand` (replaced by global ring)
- Keep any `focus-visible:ring-*` that was already correct

Files likely affected:
- `ChatInput.tsx`
- `ThreadButton.tsx`
- `InlineTextEditor.tsx`
- `InlineTagsEditor.tsx`
- `AddSubtaskRow.tsx`
- `ProjectDetail.tsx`
- `ObjectiveDetail.tsx`
- `TaskDetail.tsx`
- `InteractionCard.tsx`

**Step 2: Lint and build**

Run: `cd desktop-ui && bun run lint && bun run build`

**Step 3: Commit**

```
a11y(desktop-ui): remove per-component focus:outline-none, use global focus ring
```

---

## Task 21: Final verification

**Step 1: Full build**

Run: `cd desktop-ui && bun run build`

**Step 2: Full lint**

Run: `cd desktop-ui && bun run lint`

**Step 3: Verify no regressions**

Run: `cd desktop-ui && bun run dev` — manually verify in browser/Tauri that:
- Dark mode scrollbars and native inputs look correct
- Focus rings appear on keyboard Tab navigation
- Task rows are keyboard navigable
- Chat auto-scrolls on new messages
- Animations respect prefers-reduced-motion
- URL reflects tab/view state changes

**Step 4: Final commit if any fixups needed**

```
chore(desktop-ui): verify frontend upgrade — all lint and build passing
```
