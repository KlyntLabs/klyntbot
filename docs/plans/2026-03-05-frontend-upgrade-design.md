# Desktop UI Comprehensive Frontend Upgrade

**Date:** 2026-03-05
**Status:** Approved
**Scope:** `desktop-ui/src/` - ~97 TSX files, ~12,000 LOC

## Context

Single-user, offline-first AI agent desktop app (Tauri + React 19 + Tailwind v4). Not yet released to production, so breaking changes are acceptable. Upgrade priorities (user-ranked):

1. Web Interface Guidelines compliance
2. UX polish (animations, keyboard, micro-interactions)
3. Performance (re-renders, data fetching, bundle)
4. Data layer (enhance custom hooks with cache + dedup)
5. Accessibility (ARIA, screen readers, keyboard nav)
6. DX (component patterns, composition)

## Current State

- React 19.0.0 + React Compiler (Babel plugin) already active
- Tailwind v4 CSS-driven (no config file), dark-mode-only theme tokens in `theme.css`
- Lazy routes via React Router v7 (hash-based)
- No barrel files, no forwardRef (React 19 ref prop)
- Custom `useQuery`/`useMutation` hooks wrapping Tauri IPC
- 184 instances of `useMemo`/`useCallback` (many redundant under React Compiler)

## Audits Performed

Three parallel code-reviewer agents scanned all 97 component files against:
- Web Interface Guidelines (accessibility, forms, focus, animation, typography, content, performance, navigation)
- Vercel React Best Practices (waterfalls, bundle, re-renders, rendering, JS perf)
- Vercel Composition Patterns (boolean props, compound components, state management)

Total findings: 180+ across all files.

---

## Section 1: Web Interface Guidelines Compliance

### 1a. `color-scheme: dark` on `<html>` [CRITICAL]

- **File:** `index.html:2`
- Add `style="color-scheme: dark"` to `<html>` tag
- Add `<meta name="theme-color" content="#0a0a0f">` matching `--surface-lowest`
- Fixes native `<select>`, `<input type="date">`, scrollbars rendering white in dark UI

### 1b. `prefers-reduced-motion` support [CRITICAL]

- **File:** `theme.css`
- Zero motion support currently. Add global rule:
```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```
- Covers: `cursor-blink`, `animate-bounce` (thinking indicators), `animate-spin` (spinners), `animate-pulse` (skeletons)

### 1c. Replace all `transition-all` [12 instances]

| File | Line | Replace with |
|------|------|-------------|
| `ui/Progress.tsx` | 16 | `transition-[width]` |
| `productivity/TopApps.tsx` | 70 | `transition-[width]` |
| `productivity/WorkHoursCard.tsx` | 38 | `transition-[width]` |
| `productivity/PomodoroTimer.tsx` | 96 | `transition-[width]` |
| `productivity/GoalsProgress.tsx` | 102 | `transition-[width]` |
| `productivity/ProductivityScoreRing.tsx` | 24 | `transition-[width]` |
| `productivity/ProductivityScoreRing.tsx` | 83 | `transition-[stroke-dashoffset]` |
| `views/ProjectDetail.tsx` | 182 | `transition-shadow` |
| `views/ProjectDetail.tsx` | 195 | `transition-shadow` |
| `views/Launcher.tsx` | 212,243 | `transition-colors` |
| `finance/FinanceLiabilities.tsx` | 130 | `transition-[width]` |
| `finance/Donut.tsx` | 39 | Wrap `<circle>` in `<g>` for SVG transform |

### 1d. Typography fixes

- Replace ASCII `...` with `...` in: `TaskTable.tsx:201`, `DistractionOverlay.tsx:47`, `McpServerCard.tsx:134`, `DistractionOverlay.tsx:40`, and any other loading/truncation strings
- Add `text-wrap: balance` to `h1`-`h4` in `theme.css`
- Add `.tabular-nums { font-variant-numeric: tabular-nums; }` utility, apply to: finance amounts, transaction dates, timer displays, progress percentages

### 1e. Native `<select>` explicit styling

- `TaskDetail.tsx:153` - add `bg-surface-base text-primary border-border`
- `FinanceTransactions.tsx:143` - same treatment

### 1f. URL-reflected state

- `MainApp.tsx` - persist tab + view mode via `useSearchParams` (`?tab=work&view=board`)
- `Chat.tsx` - persist selected thread via `useSearchParams` (`?thread=<id>`)
- Use React Router's `useSearchParams` hook

### 1g. Destructive action confirmation

- `Chat.tsx:184` (thread deletion) - add confirmation dialog or undo toast
- `McpServersSettings.tsx:80` (server removal) - add confirmation dialog

### 1h. Flex text truncation

- `TaskRow.tsx:82` - add `min-w-0` to flex container
- `TaskRow.tsx:239` (SubtaskRow) - add `min-w-0` to flex container

---

## Section 2: UX Polish

### 2a. Global focus ring system

- **File:** `theme.css`
- Add base focus ring utility:
```css
.focus-ring {
  @apply outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-surface-base;
}
```
- Apply via base element styles for `button`, `input`, `textarea`, `select`, `[role="button"]`, `[tabindex="0"]`
- Remove all per-component `focus:outline-none` / `focus:border-brand` patterns (~20+ files)

### 2b. Keyboard navigation for clickable elements

- `TaskRow.tsx:49` (`<tr onClick>`) - add `tabIndex={0}`, `role="link"`, `onKeyDown` (Enter navigates)
- `TaskRow.tsx:219` (SubtaskRow) - same
- `Finance.tsx:199` (`<Card onClick>`) - add `tabIndex={0}`, `role="link"`, `onKeyDown`

### 2c. Micro-interactions

- Add `@keyframes fade-in` (opacity 0->1, 150ms) in `theme.css`
- Skeleton -> content: wrap loaded content in `animate-in fade-in`
- Primary buttons: add `active:scale-[0.98]` for tactile feedback
- Consistent `hover:bg-surface-hover` on all interactive rows

### 2d. Auto-scroll chat on new messages

- `MessageList.tsx:43` - change `useEffect([])` to depend on `messages.length` + streaming flag
- Smooth scroll behavior with `scrollIntoView({ behavior: 'smooth' })`
- Add floating "scroll to bottom" button when user has scrolled up

### 2e. Global keyboard shortcuts

- Add `useGlobalShortcuts` hook at App level
- `Cmd+K` - open Launcher overlay
- `Escape` - close overlays, go back
- Wire into existing Launcher route

---

## Section 3: Performance

### 3a. Fix data fetching waterfall [CRITICAL]

- `TaskDetail.tsx:17` - replace `useQuery("task_list")` + `useMemo(find)` with `useQuery("task_get", { id })`
- Requires corresponding Tauri IPC command (verify exists or add)

### 3b. Stabilize TaskTable context value

- `TaskTable.tsx:76-87` - wrap `ctx` object in `useMemo` with explicit deps
- Prevents cascading re-renders to every `TaskRow` on parent updates

### 3c. Fix stale completion state

- `ProjectDetail.tsx:66-68` - `useSetToggle` seeded from mount-time snapshot
- Fix: derive `isCompleted` directly from task data on each render, or re-seed on refetch

### 3d. Stabilize mutation references

- `useMutation.ts` - wrap return object in `useMemo` so `{ mutate, loading, error }` is referentially stable
- Fixes cascading instability in `MainApp.tsx:99-122` callbacks

### 3e. Remove redundant memoization (~30 instances)

React Compiler auto-memoizes these. Remove explicit wrappers from:
- `InlineTextEditor.tsx:14,23` - trivial `useCallback`
- `InlineSelect.tsx:29` - `useCallback(() => setOpen(false), [])`
- `InlineDatePicker.tsx:15` - same
- `useAutoResizeTextarea.ts:7` - `useCallback` with no deps
- `ThreadList.tsx:61` - `useCallback` render function
- `Chat.tsx:118` - passthrough `useCallback`
- `Timeline.tsx:88` - simple Map derivation `useMemo`

### 3f. Combine array iterations

- `FinanceTransactions.tsx:54-80` - unify 3 separate `useMemo` calls into single pass

### 3g. Module-level date bug

- `App.tsx:111` - `new Date().toISOString().slice(0, 10)` captured at module load
- Fix: compute inside a redirect component or use `loader` function

---

## Section 4: Data Layer Enhancement

### 4a. Enhance `useQuery` with cache + dedup

- **File:** `hooks/useQuery.ts`
- Add module-level `Map<string, { data, timestamp, promise }>` cache
- Cache key: `cmd + JSON.stringify(args)`
- Return stale data immediately while refetching (SWR pattern)
- Dedup concurrent calls with same key (share Promise)
- Add `refetchOnFocus` via Tauri window focus event
- Add configurable `staleTime` (default: 30s for most queries)

### 4b. Stabilize `useMutation` return

- **File:** `hooks/useMutation.ts`
- Wrap return `{ mutate, loading, error }` in `useMemo`
- Ensures stable references for consumers

---

## Section 5: Accessibility

### 5a. ARIA live regions

- `MessageList.tsx:48` - add `aria-live="polite"` to message list container
- `TaskTableSkeleton.tsx:9` - add `aria-busy="true"` + `aria-label="Loading tasks"`

### 5b. Missing `aria-label` on icon buttons (~15 instances)

| File | Element | Label |
|------|---------|-------|
| `DateNavigator.tsx:13` | ChevronLeft button | "Previous period" |
| `DateNavigator.tsx:23` | Calendar button | "Go to today" |
| `DateNavigator.tsx:33` | ChevronRight button | "Next period" |
| `MiniCalendar.tsx:131` | ChevronLeft button | "Previous month" |
| `MiniCalendar.tsx:144` | ChevronRight button | "Next month" |
| `MiniCalendar.tsx:171` | Day buttons | `aria-label={fullDateString}` |
| `AddServerDialog.tsx:68` | X button | "Close dialog" |
| `AddServerDialog.tsx:157,221` | Plus buttons | "Add environment variable" / "Add header" |
| `AddServerDialog.tsx:187,261` | Minus buttons | "Remove environment variable" / "Remove header" |
| `McpServerCard.tsx:37-59` | Power/Settings/Trash | Change `title` to `aria-label` |
| `InlineDatePicker.tsx:23` | Trigger button | "Select date" + `aria-haspopup` + `aria-expanded` |
| `InlineSelect.tsx:36` | Trigger button | Dynamic label + `aria-haspopup="listbox"` + `aria-expanded` |
| `Launcher.tsx:182` | Search input | "Search commands" |
| `LauncherChat.tsx:205` | Textarea | "Message Klynt" |
| `SystemTray.tsx:234` | Input | "Ask Klynt" |

### 5c. Decorative icons

- Add `aria-hidden="true"` to Lucide icons inside labeled buttons:
  - `Launcher.tsx:209` (result item icons)
  - `Sidebar.tsx:52` (KlyntLogo), `Sidebar.tsx:71` (nav icons)

### 5d. Dialog semantics

- `AddServerDialog.tsx:60` - add `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, focus trap
- `AddGoalDialog.tsx:39` - same treatment

### 5e. Tab semantics

- `FinanceLayout.tsx:43` - add `role="tablist"` to container, `role="tab"` + `aria-selected` to buttons

### 5f. Form enhancements

- `AddServerDialog.tsx:208` - change `type="text"` to `type="url"` for URL field
- `TaskDetail.tsx:144,153` - associate labels with `htmlFor`/`id`
- `ObjectiveDetail.tsx:223` - add `aria-label` to key result input
- `InteractionCard.tsx:282` - add `aria-labelledby` linking to question text

---

## Section 6: DX - Component Patterns

### 6a. InlineSelect ARIA + keyboard nav

- Add `aria-haspopup="listbox"`, `aria-expanded`, `role="listbox"` on dropdown
- `role="option"` + `aria-selected` on items
- Arrow key navigation (up/down to move, Enter to select, Escape to close)

### 6b. Shared `useInlineEdit` hook

- Extract common edit pattern from `InlineTextEditor`, `InlineSelect`, `InlineDatePicker`, `InlineTagsEditor`
- Shared behavior: click-to-edit, save on blur/Enter, cancel on Escape, `stopPropagation`
- Reduces duplication across 4 editor components

---

## Implementation Order

Recommended execution in dependency order:

1. **Foundation** (theme.css, index.html) - Sections 1a, 1b, 2a
2. **Data layer** (hooks) - Sections 4a, 4b, 3d
3. **Component fixes** (per-file) - Sections 1c-1h, 2b-2d, 3a-3g, 5a-5f
4. **UX enhancements** (new features) - Sections 2c, 2e, 1f
5. **DX patterns** (refactors) - Sections 6a, 6b, 3e

## Files Changed (estimated ~40 files)

Core: `index.html`, `theme.css`, `useQuery.ts`, `useMutation.ts`, `App.tsx`
Views: `MainApp.tsx`, `Chat.tsx`, `TaskDetail.tsx`, `ProjectDetail.tsx`, `Finance.tsx`, `FinanceTransactions.tsx`, `Launcher.tsx`, `LauncherChat.tsx`, `SystemTray.tsx`, `OkrView.tsx`, `ObjectiveDetail.tsx`
Tasks: `TaskTable.tsx`, `TaskTableSkeleton.tsx`, `TaskRow.tsx`, `TaskTableContext.tsx`
Editors: `InlineSelect.tsx`, `InlineDatePicker.tsx`, `InlineTextEditor.tsx`, `InlineTagsEditor.tsx`, `MiniCalendar.tsx`
Chat: `MessageList.tsx`, `ChatInput.tsx`, `SidebarChat.tsx`, `ThreadList.tsx`, `SegmentedMessage.tsx`, `MarkdownContent.tsx`, `InteractionCard.tsx`
Layout: `Sidebar.tsx`
Productivity: `DateNavigator.tsx`, `PomodoroTimer.tsx`, `Timeline.tsx`, `TopApps.tsx`, `WorkHoursCard.tsx`, `GoalsProgress.tsx`, `ProductivityScoreRing.tsx`
Finance: `FinanceLayout.tsx`, `FinanceLiabilities.tsx`, `Donut.tsx`
Settings: `AddServerDialog.tsx`, `McpServerCard.tsx`, `McpServersSettings.tsx`, `AddGoalDialog.tsx`
Distraction: `DistractionOverlay.tsx`
UI: `Progress.tsx`
