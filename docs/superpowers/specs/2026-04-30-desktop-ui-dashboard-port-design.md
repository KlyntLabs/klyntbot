# Desktop UI — Dashboard / Calendar port

**Date:** 2026-04-30
**Status:** Approved for implementation
**Source:** `desktop-ui.bak/src/features/dashboard/` (~3,900 lines across 14 components + hooks + lib)
**Target:** `desktop-ui/src/features/dashboard/`

## Goal

Port the full Day/Week/Month/Year calendar dashboard from the previous desktop UI (`desktop-ui.bak/`) into the current `desktop-ui/`, restyled to match the current UI's plain-CSS, BEM-ish, design-token-driven conventions. Backend Rust code does not change — all required Tauri commands already exist.

## Non-goals

- No backend changes. All ten Tauri commands the dashboard depends on (`timeline_query`, `productivity_calendar_events`, `productivity_weekly`, `productivity_goals`, `productivity_goal_delete`, `productivity_auto_focus_confirm`, `calendar_sync_events`, `task_update`, `flashcard_total_due`, `app_info`) are already present in `crates/desktop/src/commands/`.
- No `react-router` introduction. The current UI is a single-shell window app; routing is intentionally not adopted.
- No persisted dashboard view-mode (`day`/`week`/...) in Phase 1. May be added later via `localStorage` if desired.
- No CLAUDE.md edit as part of this work. CLAUDE.md says "no `useQuery`/`useMutation` wrapper" but the current codebase has `useTauriQuery`/`useTauriMutation` in `src/lib/query/`. Flagged for a separate doc-update task.

## Hosting decision

The dashboard is **not** a separate route or window. It is a new view-mode rendered inside `MainAppShell`, mirroring how `PluginsView` is wired today.

`MainApp.tsx` already holds an `appView` state union (`"home" | "chat" | "plugins"`). We extend it to include `"calendar"`, add an `onSelectCalendar` callback, surface a `dashboardNode` through the layout-surfaces hook, and wire a "Calendar" sidebar nav item with a lucide `Calendar` icon. Exactly the pattern Plugins follows.

## Phasing

Three independent merges. Each is fully working and shippable.

### Phase 1 — Foundation + Day view (~1,400 lines ported + shell wiring)
Validates IPC translation, style translation, drag mechanics, state-replacement-for-router. Day view is the most complex view, so doing it first means the other three views are easy variations.

### Phase 2 — Week / Month / Year views + CalendarTrack (~900 lines)
Reuses Phase 1's `DraggableTaskBlock` and `useTauriQuery` calls with different date ranges. Adds `CalendarTrack.tsx` (~105L — calendar-event overlay used by Week/Month) and the three view components. Day view stays unchanged.

### Phase 3 — SummaryPanel + productivity overlays + activity tracks (~1,300 lines)
Right-side `SummaryPanel` (583 lines) with all 9 productivity sub-components, the `FocusStateIndicator` / `AutoFocusToast` / `FocusTrayIndicator` overlays (event-driven), and `ActivityTrack` + `activity-sessions` rendering. The `useDashboardState` `sidebar` toggle wired in Phase 1 starts actually surfacing the panel here.

## File structure (Phase 1)

```
desktop-ui/src/features/dashboard/
  index.ts                              # Public exports (Dashboard root)
  components/
    Dashboard.tsx                       # Root: provides contexts, renders topbar + active view
    DashboardTopbar.tsx                 # Date label, view-pill switcher, layers, sync, nav, sidebar toggle
    MiniCalendar.tsx                    # Date picker popover (port from @shared/components/MiniCalendar)
    CalendarSync.tsx                    # Sync status indicator
    views/
      DayView.tsx                       # Was DayCalendarView — wrapper for DayColumns
      DayColumns.tsx                    # Was DayColumnsView (~742L) — the timeline grid
      DraggableTaskBlock.tsx            # Was DraggableTaskBlock (~119L)
      ContextRibbon.tsx                 # Was ContextRibbon (~57L)
      DueTodayTray.tsx                  # Was DueTodayTray (~49L)
  hooks/
    useDashboardState.ts                # viewMode + dateParam + nav helpers (replaces react-router)
    useTimelineDrag.ts                  # Port as-is (~188L, pure mouse logic)
  lib/
    layers.ts                           # Layer defs + DataMode/Layer/Sidebar contexts (~166L)
    timeline-utils.ts                   # Time math (~93L)
    buildContainers.ts                  # Layout calculations (~159L)
```

```
desktop-ui/src/styles/dashboard.css     # All dashboard CSS, registered in src/styles/index.css
desktop-ui/src/api/endpoints/dashboard.ts  # Typed wrappers for the 10 backend commands
```

## Wiring (changes outside the dashboard feature)

### `src/features/app/components/MainApp.tsx`
- Line 327: extend `appView` union to `"home" | "chat" | "plugins" | "calendar"`.
- Add `onSelectCalendar = useCallback(() => setAppView("calendar"), [])` near `onSelectPlugins`.
- Pass `onSelectCalendar` and (computed) `activeNavId` through to surfaces hook.

### `src/features/app/components/SidebarChatLayout.tsx`
- Import `Calendar` from `lucide-react`.
- Add `{ id: "calendar", label: "Calendar", icon: <Calendar />, onClick: onSelectCalendar }` to `navItems`.
- Accept new prop `activeNavId: string | null`. Render `sidebar-chat__nav-item--active` modifier class on the matching item.

### `src/features/app/hooks/useMainAppLayoutSurfaces.ts`
- Add `dashboardNode` to layout params; treat `appView === "calendar"` like `"plugins"` in the surface-suppression logic so `home`/`chat` UI is hidden when the dashboard is active.

### `src/features/app/hooks/useMainAppShellProps.ts` (or wherever the layout consumes nodes)
- Render `dashboardNode` in the center surface when present.

### `src/styles/index.css`
- Add `@import "./dashboard.css";`

### `src/styles/sidebar-chat.css`
- Add `.sidebar-chat__nav-item--active { background: var(--surface-active); color: var(--ds-text-strong); }`.

## State replacement for `react-router`

The backup uses `useNavigate` / `useLocation` / `useParams<{ date?, year? }>()` to drive view mode and the active date. The port replaces all three with a single hook held in `Dashboard.tsx` and exposed via React context.

```ts
// src/features/dashboard/hooks/useDashboardState.ts
type ViewMode = "day" | "week" | "month" | "year";

interface DashboardState {
  mode: ViewMode;
  date: string;          // YYYY-MM-DD (or YYYY for year mode)
  setMode(m: ViewMode): void;
  setDate(d: string): void;
  navigatePrev(): void;
  navigateNext(): void;
  navigateToday(): void;
}
```

`mode` defaults to `"day"`, `date` defaults to today's local-ISO. Sub-views read via `useContext(DashboardStateContext)`.

State is **not** persisted in Phase 1 — opening the dashboard always lands on today's day view.

## IPC translation

### Endpoint wrappers — `src/api/endpoints/dashboard.ts`

One typed wrapper per backend command, mirroring `endpoints/github.ts`. Args camelCase at the wrapper boundary (matches `bindings.ts`); types come from `@/types` (specta-emitted).

**Phase 1 wrappers** (added now):

```ts
export async function timelineQuery(
  startDate: string,
  endDate: string,
  sources: TimelineSource[] | null,
  includePointEvents: boolean | null,
  tzOffsetMins: number | null,
): Promise<TimelineResponse>;

export async function taskUpdate(params: TaskUpdateParams): Promise<TaskResponse>;
```

`appInfo` already exists in another endpoint file and is reused. The remaining seven wrappers (`productivityWeekly`, `productivityGoals`, `productivityGoalDelete`, `productivityCalendarEvents`, `productivityAutoFocusConfirm`, `calendarSyncEvents`, `flashcardTotalDue`) are deferred to Phase 3 — exact arg shapes will be derived at implementation time from the Rust signatures and added to this same `dashboard.ts` file.

If a Rust type isn't in `bindings.ts` (some backup types may have been internal-only), either add `specta::Type` to the Rust struct or define the TS shape locally — decide per case during implementation.

### Query keys — `src/lib/query/queryKeys.ts` additions

```ts
dashboard: {
  all: () => ["dashboard"] as const,
  timeline: (startDate: string, endDate: string, sources: readonly string[]) =>
    ["dashboard", "timeline", startDate, endDate, [...sources].sort().join(",")] as const,
},
productivity: {
  all: () => ["productivity"] as const,
  weekly: () => ["productivity", "weekly"] as const,
  goals: () => ["productivity", "goals"] as const,
  calendarEvents: (startDate: string, endDate: string) =>
    ["productivity", "calendarEvents", startDate, endDate] as const,
},
calendarSync: {
  all: () => ["calendarSync"] as const,
  status: () => ["calendarSync", "status"] as const,
},
```

Sources are normalized in the timeline key (sorted, joined) so callers passing the same set in different orders share the cache entry. The existing `qk.calendar.eventsForDate` namespace serves a different surface and is left untouched.

### Query hook pattern

Backup:
```ts
const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);
```

Translated:
```ts
const { data, isLoading } = useTauriQuery<TimelineResponse>({
  queryKey: qk.dashboard.timeline(startDate, endDate, sources),
  queryFn: () => timelineQuery(startDate, endDate, sources, true, tzOffset),
  fallback: EMPTY_TIMELINE_RESPONSE,
});
```

`fallback` makes `data` defined as `EMPTY_TIMELINE_RESPONSE` until the first successful fetch — downstream `data.tasks?.length` patterns continue to work without optional-chaining changes.

### Mutation hook pattern

Backup:
```ts
const updateTask = useMutation("task_update");
await updateTask.mutate({ params: { id, schedulingDate, ... } });
```

Translated:
```ts
const updateTask = useTauriMutation<TaskResponse, TaskUpdateParams>({
  mutationFn: taskUpdate,
  invalidates: [qk.dashboard.all()],
  optimistic: {
    queryKey: qk.dashboard.timeline(startDate, endDate, sources),
    update: (vars, prev) => applyTaskUpdate(prev, vars),
  },
});
```

`invalidates` replaces the backup's manual `dayInvalidate` arrays. `optimistic` makes drag-to-reschedule feel instant — the visible block moves immediately, only rolls back on backend error.

### Tauri events (Phase 3 only)

Add to `src/services/events.ts`, returning unlisten functions:

```ts
export function subscribeFocusStateChanged(handler: (p: FocusStatePayload) => void): Promise<() => void>;
export function subscribeFocusAutoDetected(handler: (p: AutoFocusPayload) => void): Promise<() => void>;
export function subscribeFocusAutoStarted(handler: (p: { sessionId: string; appName: string }) => void): Promise<() => void>;
```

Components subscribe in `useEffect` and call the unlisten on cleanup. Replaces backup's `useEvent("focus:state_changed", handler)`.

## Styling translation

### Token map (Tailwind/shadcn → existing CSS vars)

| Backup class | Replacement |
|---|---|
| `text-foreground` | `var(--text)` / `var(--ds-text-strong)` |
| `text-muted-foreground` | `var(--ds-text-subtle)` |
| `bg-accent` | `var(--surface-control)` |
| `bg-muted` | `var(--surface-control-hover)` |
| `bg-card` / `glass-card` | `var(--surface-card-strong)` + 1px subtle border |
| `glass-dropdown` | `var(--ds-popover-bg)` + `var(--ds-popover-border)` + `var(--ds-popover-shadow)` |
| `text-xs` | `var(--fs-xs)` |
| `text-sm` | `var(--fs-base)` |
| `text-[11px]` | `var(--fs-2xs)` |
| `rounded-full` | `border-radius: 999px` |
| `transition-colors` | `transition: background-color var(--ds-dur-fast) var(--ds-ease-out)` |
| `accent-brand` | `accent-color: var(--border-accent)` |
| `size-N` (icons) | explicit `width`/`height` in px |

### Class naming (BEM-ish under one root)

```
.dashboard
.dashboard__topbar
.dashboard__topbar-date
.dashboard__view-switcher
.dashboard__view-pill
.dashboard__view-pill--active
.dashboard__icon-button
.dashboard__icon-button--active
.dashboard__nav-pills
.dashboard__popover
.dashboard__popover-item
.dashboard__layer-swatch
.dashboard__content
.dashboard__day-grid
.dashboard__day-column
.dashboard__hour-row
.dashboard__task-block
.dashboard__task-block--dragging
```

### File organization

Single feature CSS file `src/styles/dashboard.css`, registered with one new `@import` in `src/styles/index.css`. No CSS-in-JS, no scoped modules.

`.glass-card` and `.glass-dropdown` from the backup become two reusable rules **inside dashboard.css** (not promoted to `ds-tokens.css`). They build on the existing `--ds-popover-*` and `--surface-card-strong` tokens so they integrate with the rest of the app's surface treatment.

## Pure helpers ported as-is

These are pure logic with no React or routing dependencies — they port verbatim:

- `useTimelineDrag` (188L)
- `timeline-utils.ts` (93L)
- `buildContainers.ts` (159L)
- `layers.ts` (166L) — keep its three React contexts (`DataModeContext`, `LayerContext`, `SidebarContext`)

## Tests added in Phase 1

1. **`Dashboard.test.tsx`** — renders without crashing with mocked `timelineQuery` returning empty `TimelineResponse`; verifies date label, view-pill switcher, layers button, sidebar toggle present.
2. **`useDashboardState.test.ts`** — `navigatePrev`/`navigateNext` move date by the right amount per mode; `setMode("year")` formats date as `YYYY`.
3. **`DayView.test.tsx`** — given a stub `TimelineResponse` with two tasks at known times, asserts task blocks render at expected vertical positions; clicking a task triggers a known callback.
4. **Drag mutation test** — uses `useTauriMutation` mock; simulating a drag fires `taskUpdate` with the expected new `schedulingDate` and the optimistic update appears immediately.
5. **`SidebarChatLayout.test.tsx`** (existing) — extended to assert `--active` modifier class is applied when `activeNavId` matches.

All tests use the existing `vi.mock("@tauri-apps/api/core", ...)` per-test pattern.

## Acceptance criteria — Phase 1

```bash
cd desktop-ui && bun run typecheck     # clean
cd desktop-ui && bun run lint           # clean
cd desktop-ui && bun run test           # all pass
```

Manual smoke (`cargo tauri dev`):

1. Click **Calendar** in sidebar → dashboard renders today's day view.
2. Active-state highlight appears on Calendar nav item; switching to Plugins moves the highlight.
3. Date arrows move forward/back; date label updates.
4. Click date icon → MiniCalendar popover opens; pick a date → day view rerenders.
5. Click view pills (Day/Week/Month/Year) — Day shows data, Week/Month/Year show a "Coming in next phase" placeholder.
6. Click a task block, drag to a new time → block visually moves immediately (optimistic), stays after the network roundtrip.
7. Click Layers → toggle a layer → corresponding tracks hide/show.
8. Click sidebar toggle (PanelRight icon) → toggles sidebar context (no panel to show until Phase 3, but the button is wired and the state context updates).

## Acceptance criteria — Phases 2 + 3 (preview)

- **Phase 2:** Week/Month/Year views render real data; switching between modes preserves the active date where meaningful (a date in week view jumps to that week in month view).
- **Phase 3:** SummaryPanel renders on the right when sidebar context is open; all 9 productivity sub-components render (ActivityFeed, AddGoalDialog, AutoFocusToast, FocusStateIndicator, FocusTrayIndicator, GoalsProgress, HourlyHeatmap, PatternsCard, ProductivityScoreRing); focus events surface as toasts/indicators in real time.

## Risks and mitigations

- **`bindings.ts` drift.** `cargo tauri dev` regenerates bindings; the `bindings_are_current` test fails if stale. The dashboard endpoints intentionally do **not** call commands that don't already exist, so the regenerator should be a no-op for this work.
- **Type gaps in `bindings.ts`.** Some backup types (e.g. internal `EMPTY_TIMELINE_RESPONSE` shapes) may not have specta annotations. Implementation may need to add `#[derive(specta::Type)]` to a small number of Rust structs. Track and call out per case in the implementation plan.
- **Pre-existing `qk.calendar.eventsForDate`.** A different namespace than `qk.productivity.calendarEvents`. Intentional — the existing `calendar` keys serve another surface. Keep them separate to avoid cache collisions.
- **Active-nav-item visual is a small new convention.** A two-line CSS rule plus one prop change in `SidebarChatLayout` and its test. Low blast radius.
