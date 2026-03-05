# Frontend Comprehensive Upgrade Design

**Date:** 2026-03-05
**Scope:** `desktop-ui/` only — no backend/IPC changes
**Approach:** Option A — Phased by impact tier (3 tiers, each independently shippable)

## Constraints

- React 19 upgrade included (unlocks `useOptimistic`, ref-as-prop, `use(context)`, `useActionState`)
- React Compiler included (auto-memoization via Vite/Babel plugin — replaces most manual `useMemo`/`useCallback`)
- No server-side rules apply (Tauri desktop app, no RSC/server actions)
- All existing functionality preserved exactly
- Breaking changes acceptable — not yet in production
- Tailwind v4 CSS token system stays intact

---

## Tier 1: Critical — Performance

### 1. React 19 + React Compiler upgrade

**Package changes:**
- `react` → `^19.0.0`
- `react-dom` → `^19.0.0`
- `@types/react` → `^19.0.0`
- `@types/react-dom` → `^19.0.0`
- Add `babel-plugin-react-compiler` + `@vitejs/plugin-react` config for compiler

**Vite config update:** enable React Compiler in `@vitejs/plugin-react` options.

**Verify peer deps:** `@radix-ui/react-checkbox`, `@radix-ui/react-progress`, `react-router v7`, `recharts v3.7`, `react-markdown v10.1` all support React 19.

**New APIs to leverage:**
- `useOptimistic` for task toggle (replaces manual completedTasks desync fix)
- `ref` as prop (remove `forwardRef` from any components using it)
- `use(Context)` instead of `useContext` in compound components (Tier 2)

### 2. Route-based lazy loading (`App.tsx`)

All 30+ route components currently eagerly imported — entire bundle parsed before first frame.

Replace every route component with `React.lazy(() => import(...))`. Wrap `RouterProvider` in a `<Suspense fallback={null}>`. This splits the bundle per route, deferring Finance, Productivity (recharts-heavy), and Settings pages until first navigation.

### 3. Fix `completedTasks` optimistic state (`MainApp.tsx`)

Current: `completedTasks` initialized from `tasks` in `useState`, then managed independently — drifts after IPC refetch.

Fix: use `useOptimistic(tasks, ...)` to derive optimistic completed state directly from server state with an overlay for in-flight toggles. Eliminates the desync entirely.

### 4. Extract `renderThread` and `renderGroupHeader` as memo'd components (`Chat.tsx`)

Both are inline functions recreated on every render — any state change (typing, streaming, context menu) recreates all thread buttons, defeating React's reconciliation. With React Compiler, extracting as named components is sufficient for auto-memoization.

Extract: `ThreadButton` component, `GroupHeader` component. Each receives only the props it needs.

### 5. Hoist static JSX in `TaskTable`

Loading row and empty-state row defined as JSX literals inside render loops. Extract as module-level constants. React Compiler handles the rest.

---

## Tier 2: Medium — DX & Composition

### 1. Extract `Chat.tsx` into focused sub-components

Current: 580-line file covering thread list, thread groups, context menu, rename flow, message area, transparency panel, input area.

Split:
- `ThreadList.tsx` — grouped thread tree (areas → projects → threads)
- `ThreadContextMenu.tsx` — right-click menu (proper `role="menu"` + keyboard nav)
- `ChatInput.tsx` — textarea, send button, toolbar row
- `Chat.tsx` — orchestrator: owns session state, composes the above

`useChatSession` stays unchanged (already clean separation).

### 2. TaskTable prop drilling → compound components + context

Current: `TaskTable → TaskGroup → TaskWithSubtasks → TaskRow` passes 13 props through 3 levels (callbacks, sets, maps all threaded through).

Fix: introduce `TaskTableContext` with `createContext`. Provider at `TaskTable` level holds: `completedTasks`, `expandedTasks`, `childrenCache`, `showArea`, `onToggleTask`, `onToggleExpandTask`, `onUpdate`, `onCreateSubtask`. Child components read via `use(TaskTableContext)` (React 19).

`TaskTable` public API unchanged — all 13 props stay on the outer component, just not threaded through.

### 3. Explicit variants for `TaskRow`

`TaskRow` uses `depth={0|1}` as an implicit variant flag controlling indentation and expand button visibility. Replace with explicit `RootTaskRow` and `SubtaskRow` components. `TaskWithSubtasks` renders `RootTaskRow`; subtask loop renders `SubtaskRow`.

### 4. Stabilize `useMutation` internals

`useMutation` currently returns a new `mutate` function reference on every call. Add `useCallback` internally so the returned `mutate` is stable — consumers no longer need to manually wrap in `useCallback`.

---

## Tier 3: Polish — UX & Accessibility

### 1. Accessibility sweep

Affected components: all icon-only buttons throughout the app.

Changes:
- Add `aria-label` to every icon-only `<button>` (MessageSquare, Mic, Send, Plus, Close, etc.)
- Add `aria-expanded` to all collapsible group headers (`GroupHeader`, project collapse buttons)
- `ThreadContextMenu`: change outer `div[role=menu]` items from `<button>` to `role="menuitem"`, add keyboard navigation (ArrowUp/Down to move focus, Escape to close, Enter to activate)
- Focus trap on context menu open, restore focus to trigger on close

### 2. Auto-resize textarea (Chat input)

Chat `textarea` currently fixed with `rows={1}` and `style={{ maxHeight: "200px" }}` — doesn't grow with content.

Use CSS `field-sizing: content` (Chrome 123+, supported in Electron/WebView2). Add a JS fallback via `onInput` that sets `style.height = 'auto'` then `style.height = scrollHeight + 'px'` for Safari. Reset to `rows={1}` on send.

### 3. Loading + error UI

`useQuery` tracks `loading` and `error` but both are silently dropped in all consuming components.

Add:
- `TaskTableSkeleton`: shimmer rows (5 rows, matching table column structure) shown while `loading` is true in MainApp
- Inline error state in `MainApp`, `Chat` thread list, and productivity pages: small error message + "Retry" button that calls `refetch()`
- `useTransition` for tab switches in `MainApp` (All / Work / Personal) — marks filter as non-urgent, keeps UI responsive during large task lists

### 4. Replace `&&` conditionals with ternaries

Several components use `{count && <Badge>{count}</Badge>}` patterns — renders `"0"` as text when count is 0. Audit and replace with explicit ternaries throughout.

---

## File Impact Summary

| File | Tier | Change Type |
|------|------|-------------|
| `package.json` | 1 | React 19 + compiler deps |
| `vite.config.ts` | 1 | React Compiler plugin |
| `App.tsx` | 1 | All imports → lazy |
| `hooks/useMutation.ts` | 2 | Stabilize mutate ref |
| `components/views/MainApp.tsx` | 1+3 | useOptimistic, useTransition, loading UI |
| `components/views/Chat.tsx` | 1+2+3 | Split into sub-components, aria |
| `components/chat/ThreadButton.tsx` | 1+3 | New component (extracted from Chat) |
| `components/chat/GroupHeader.tsx` | 1+3 | New component (extracted from Chat) |
| `components/chat/ThreadContextMenu.tsx` | 2+3 | New component (extracted from Chat) |
| `components/chat/ChatInput.tsx` | 2+3 | New component, auto-resize, aria |
| `components/tasks/TaskTable.tsx` | 2+3 | Context provider, skeleton |
| `components/tasks/TaskRow.tsx` | 2 | Split into RootTaskRow + SubtaskRow |
| `components/tasks/TaskGroup.tsx` | 2 | Reads from context |
| `components/tasks/TaskWithSubtasks.tsx` | 2 | Reads from context |
| All icon-button components | 3 | aria-label sweep |

## Success Criteria

- Tier 1: Initial bundle parse time reduced; no visible re-renders on Chat typing; no completedTasks desync
- Tier 2: No prop chain deeper than 2 levels for callbacks; Chat.tsx < 150 lines
- Tier 3: Zero missing aria-labels on interactive elements; textarea grows with input; all fetch errors visible to user
