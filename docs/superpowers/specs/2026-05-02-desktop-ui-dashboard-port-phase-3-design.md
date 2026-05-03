# Desktop UI — Dashboard port, Phase 3 (SummaryPanel + productivity overlays + activity tracks)

**Date:** 2026-05-02
**Status:** Approved for implementation
**Parent spec:** [`2026-04-30-desktop-ui-dashboard-port-design.md`](2026-04-30-desktop-ui-dashboard-port-design.md)
**Phase 1 plan (shipped):** [`../plans/2026-04-30-desktop-ui-dashboard-port-phase-1.md`](../plans/2026-04-30-desktop-ui-dashboard-port-phase-1.md)
**Phase 2 plan (shipped):** [`../plans/2026-05-02-desktop-ui-dashboard-port-phase-2.md`](../plans/2026-05-02-desktop-ui-dashboard-port-phase-2.md) *(in main repo)*
**Phase 2 design:** [`2026-05-02-desktop-ui-dashboard-port-phase-2-design.md`](2026-05-02-desktop-ui-dashboard-port-phase-2-design.md) *(in main repo)*
**Source:** `desktop-ui.bak/src/features/dashboard/components/{SummaryPanel,ActivityTrack,ProductivityStrip}.tsx` + `desktop-ui.bak/src/features/dashboard/components/productivity/*.tsx`
**Target:** `desktop-ui/src/features/dashboard/`

## Goal

Bring the remaining backup dashboard surface forward: the full `SummaryPanel` (with `ProductivityScoreRing`, `WeeklySparkline`, `PatternsCard`, `HourlyHeatmap`, `GoalsProgress`, `AddGoalDialog`, `EntryDetail`, `SessionDetail`), `ActivityTrack` with intelligence-enriched session blocks, the live `ActivityFeed`, the `FocusStateIndicator` + `AutoFocusToast` + `FocusTrayIndicator` real-time overlays, and `ProductivityStrip` — all restyled to the current UI's plain-CSS, BEM, design-token conventions. Every backup file under `desktop-ui.bak/src/features/dashboard/components/` lands in `desktop-ui/src/features/dashboard/components/`. Phase 3 is the final phase; after this, the dashboard is fully ported.

Roughly 1,300 lines of TSX + ~150 lines of new lib helpers + ~250 lines of CSS, plus 9 new endpoint wrappers, 3 new mutation wrappers, 2 new event subscription hubs, and a handful of folded Phase-2 follow-ups.

## Non-goals

- No backend changes. All Tauri commands and events used here already exist (`productivity_today`, `productivity_summary_range`, `productivity_weekly`, `productivity_patterns`, `productivity_hourly_breakdown`, `productivity_timeline`, `productivity_categories`, `productivity_intelligence_sessions`, `productivity_activity_feed`, `productivity_goals`, `productivity_goal_create`, `productivity_goal_delete`, `productivity_auto_focus_confirm`, `get_dashboard_intelligence` (already wrapped), plus the `focus:state_changed` and `focus:auto_detected` events). Implementation may need to add `#[derive(specta::Type)]` to a small number of internal Rust structs only if their TS shapes are missing from `bindings.ts`; track per case during implementation.
- No architectural refactor of selection state. SummaryPanel stays per-view (backup parity, decided in brainstorm).
- No `react-router` introduction. EntryDetail's `entityRoute` field — used by the backup to navigate to other features — is rendered as a non-functional link (`preventDefault` on click), matching the current partial port. Wiring entity-route navigation to the existing `appView` switch in `MainApp.tsx` is deferred (it's a separate concern, not a port).
- No persistence of SummaryPanel sub-state (e.g. `ActivityFeed`'s `feedExpanded` toggle is per-mount, not localStorage'd). Backup behavior.
- No theme work, no keyboard-shortcut additions, no new tests for code already covered.
- No new dashboard sources beyond what Phase 1/2 already plumbed.

## Corrections to the parent spec

The Phase 1 design spec line "Phase 3 — SummaryPanel + productivity overlays + activity tracks (~1,300 lines)" listed three deliverables. After reading the backup in full, the surface expands to four:

1. **SummaryPanel + 9 productivity sub-components.** As written.
2. **ActivityTrack + a new `lib/activity-sessions.ts` module.** The activity-session merging logic isn't in `timeline-utils.ts` — it's a separate lib (~120 lines) at `@shared/lib/activity-sessions` in the backup. It needs porting alongside ActivityTrack.
3. **`ProductivityStrip` is exported but unmounted in the backup.** It appears in `dashboard/index.ts` exports but no view renders it. Per the user's directive to bring everything over, the file ports but stays unmounted (matching backup behavior). Re-exported from `desktop-ui/src/features/dashboard/index.ts` for parity.
4. **Real-time overlays mount in the dashboard topbar/banner area.** The backup's `DashboardLayout.tsx` mounts `<FocusTrayIndicator />` inline in the topbar (between the date label and the view-switcher pill) and renders `<FocusStateIndicator />` + `<AutoFocusToast />` as full-width banners between the topbar and `dashboard__content`. The current `DashboardTopbar.tsx` and `Dashboard.tsx` need small additions to expose those mount points.

The parent spec's Phase 3 acceptance criteria already cover all 9 productivity components + `FocusStateIndicator` + `AutoFocusToast` + `FocusTrayIndicator` as deliverables — this section just acknowledges the additional `mergeActivitySessions` lib and the `ProductivityStrip` orphan-component port, neither of which the parent spec called out by name.

## Decisions taken in brainstorm

1. **Scope = full backup parity.** Right-rail core + ActivityTrack + Goals + focus event overlays + Phase-2 follow-up cleanup. Phase 3 is the final phase — no Phase 4.
2. **EntryDetail layout = mode swap inside SummaryPanel.** Backup pattern preserved: `selectedSession > selectedEntry > summary` precedence; `onClose` clears selection.
3. **Focus event mount points match backup.** `<FocusTrayIndicator/>` inside `DashboardTopbar` between date label and view-switcher; `<FocusStateIndicator/>` and `<AutoFocusToast/>` as banners between topbar and `dashboard__content`.
4. **SummaryPanel rendered per-view (not via shell-level lift).** Backup parity. Each view passes its own slice; only one view mounts at a time.
5. **`focus:auto-started` subscription dropped** in `FocusTrayIndicator`. The event has no current backend publisher (verified against `bindings.ts` event map). `focus:state_changed` covers all transitions.
6. **`ActivityFeed` invalidation via 30 s polling, not event subscription.** The backup subscribed to `entity:updated` + `activity:switch`, but neither has a current event hub in `desktop-ui/src/services/events.ts`. Adding them requires backend coordination outside Phase 3's scope.
7. **`ProductivityStrip` ports as a file but stays unmounted** — backup parity (it's exported but never rendered).
8. **`entityRoute` link in `EntryDetail` is non-functional** in this phase (`preventDefault`). Wiring entity-route navigation to the `appView` switch is a separate concern.
9. **YearView heatmap honors `enabledSources`.** Replaces the hardcoded `entry.source === "focus"` filter with `enabledSources.includes(entry.source as TimelineSource)`. Closes the parent spec's open follow-up.
10. **Phase-2 cleanups fold in.** Inline `style={{fontSize:...}}` extractions to BEM, the WeekView dead-code guard at line 93, MonthView click-test improvement, CalendarTrack overlap/empty-state test coverage all land in Phase 3 since they touch the same files.

## Architecture

### Mount points (top-down)

```
Dashboard.tsx
├── DashboardTopbar
│     ├── … (existing: date, view pills, layers, sync, nav, sidebar toggle)
│     └── <FocusTrayIndicator />          ← NEW: small pill, between date and view switcher
├── <FocusStateIndicator />               ← NEW: full-width banner, between topbar and content
├── <AutoFocusToast />                    ← NEW: full-width banner, below FocusStateIndicator
└── dashboard__content
      ├── DayView   → DayColumns → SummaryPanel (Day variant)        ← already mounted; expand props + selection
      ├── WeekView  → SummaryPanel (sidebar slice)                    ← NEW per-view mount
      ├── MonthView → SummaryPanel (sidebar slice)                    ← NEW per-view mount
      └── YearView  → SummaryPanel (sidebar slice)                    ← NEW per-view mount
```

### Data flow

- **Day view (`DayColumns.tsx`):** owns `selectedEntry: TimelineEntry | null`, `selectedSession: SessionBlock | null`, `selectedCalendarEvent: CalendarEvent | null`. Phase 3 adds the `selectedSession` state (driven by `ActivityTrack.onSelectSession`) and converts `selectedCalendarEvent` into a `TimelineEntry`-shaped value before passing to `SummaryPanel.selectedEntry` (when no `selectedSession`). Day fetches `productivity_today` (today) or `productivity_summary_range` (other days) and passes the result to SummaryPanel as `productivitySummary`.
- **Week / Month / Year views:** each owns its own `selectedEntry: TimelineEntry | null` (Week populates it via session-block click; Month and Year always pass `null` per backup). SummaryPanel renders DaySummary with `productivitySummary={undefined}`, which short-circuits the productivity sections and shows just `summary.totalTrackedSecs` + Top Apps from `summary.topApps`.
- **SummaryPanel mode switch (DaySummary ↔ EntryDetail ↔ SessionDetail):** precedence `selectedSession > selectedEntry > summary`. `onClose` clears all selection state in the parent view.
- **Real-time event subscriptions:** `FocusStateIndicator`, `FocusTrayIndicator`, `AutoFocusToast` each call subscription helpers from `src/services/events.ts` inside `useEffect`, store payload in component-local `useState`.

## File structure

```
desktop-ui/src/features/dashboard/components/
  Dashboard.tsx                          # MODIFY — render FocusStateIndicator + AutoFocusToast as banners
  DashboardTopbar.tsx                    # MODIFY — render <FocusTrayIndicator/> in topbar
  ProductivityStrip.tsx                  # NEW — port (file only; not mounted, parity-only)
  SummaryPanel.tsx                       # REWRITE — replace 248L partial with full backup port
  productivity/
    ActivityFeed.tsx                     # REWRITE — replace stub with real impl
    AddGoalDialog.tsx                    # NEW
    AutoFocusToast.tsx                   # NEW
    FocusStateIndicator.tsx              # NEW
    FocusTrayIndicator.tsx               # NEW
    GoalsProgress.tsx                    # NEW
    HourlyHeatmap.tsx                    # NEW
    PatternsCard.tsx                     # NEW
    ProductivityScoreRing.tsx            # NEW (also exports `ScoreBar`)
    ActivityFeed.test.tsx                # NEW
    GoalsProgress.test.tsx               # NEW
    ProductivityScoreRing.test.tsx       # NEW
  views/
    ActivityTrack.tsx                    # REWRITE — replace null-stub with real impl
    ActivityTrack.test.tsx               # NEW
    DayColumns.tsx                       # MODIFY — add productivity-summary fetch, ActivityFeed mount, selectedSession state, calendar→detail conversion, ActivityTrack wiring
    WeekView.tsx                         # MODIFY — render SummaryPanel; click session block to select; replace inline styles with BEM; remove dead-code guard
    MonthView.tsx                        # MODIFY — render SummaryPanel; replace inline styles with BEM
    YearView.tsx                         # MODIFY — render SummaryPanel; wire `enabledSources` into heatmap aggregation; replace inline styles with BEM
    SummaryPanel.test.tsx                # NEW (covers Day variant, EntryDetail, SessionDetail)

desktop-ui/src/features/dashboard/lib/
  activity-sessions.ts                   # NEW — port `mergeActivitySessions` + types
  activity-sessions.test.ts              # NEW (optional; smoke-tested via ActivityTrack.test.tsx)
  productivity.ts                        # NEW — port helpers from backup `@shared/lib/productivity`

desktop-ui/src/features/dashboard/index.ts
                                         # MODIFY — re-export ProductivityStrip + new productivity components

desktop-ui/src/api/endpoints/dashboard.ts
                                         # MODIFY — add 9 read wrappers + 3 mutation wrappers

desktop-ui/src/lib/query/queryKeys.ts
                                         # MODIFY — extend qk.productivity with new key factories

desktop-ui/src/services/events.ts
                                         # MODIFY — add subscribeFocusStateChanged + subscribeFocusAutoDetected

desktop-ui/src/styles/dashboard.css
                                         # MODIFY — add summary-panel sub-blocks, productivity-* sub-blocks, focus-indicator/toast banner blocks, activity-track block, activity-feed block, productivity-strip block (orphan but parity)

desktop-ui/src/features/dashboard/__tests__/
  dashboardCommandMocks.ts               # NEW — shared default-empty `vi.mock("@tauri-apps/api/core", ...)` helpers (avoid cross-test mock duplication)
```

## IPC

### Endpoint wrappers — additions to `src/api/endpoints/dashboard.ts`

All wrappers follow the existing `r.status !== "ok" → throw` pattern. Types come from `@/bindings`. Specta has already emitted everything needed; no Rust changes required.

```ts
import type {
  ActivityCategoryResponse,
  ActivityTimelineResponse,
  AutoFocusPayload,
  FocusSessionResponse,
  GoalProgressResponse,
  HourlyBreakdownResponse,
  IntelligenceSessionResponse,
  ProductivityPatternsResponse,
  ProductivitySummaryResponse,
} from "@/bindings";

// Read endpoints
export async function productivitySummaryRangeQuery(
  startDate: string,
  endDate: string,
): Promise<ProductivitySummaryResponse[]>;

export async function productivityWeeklyQuery(): Promise<ProductivitySummaryResponse[]>;

export async function productivityPatternsQuery(
  days: number | null,
): Promise<ProductivityPatternsResponse>;

export async function productivityHourlyBreakdownQuery(
  startDate: string,
  endDate: string,
  tzOffsetMins: number | null,
): Promise<HourlyBreakdownResponse[]>;

export async function productivityTimelineQuery(
  date: string,
  limit: number | null,
  offset: number | null,
  tzOffsetMins: number | null,
): Promise<ActivityTimelineResponse[]>;

export async function productivityCategoriesQuery(): Promise<ActivityCategoryResponse[]>;

export async function productivityIntelligenceSessionsQuery(
  date: string,
  tzOffsetMins: number | null,
): Promise<IntelligenceSessionResponse[]>;

export async function productivityActivityFeedQuery(
  limit: number | null,
): Promise<ActivityTimelineResponse[]>;

export async function productivityGoalsQuery(): Promise<GoalProgressResponse[]>;

// Mutations
export interface GoalCreateParams {
  goalType: string;
  metric: string;
  targetValue: number;
}
export async function productivityGoalCreate(
  params: GoalCreateParams,
): Promise<GoalProgressResponse>;

export async function productivityGoalDelete(id: number): Promise<void>;

export async function productivityAutoFocusConfirm(
  payload: AutoFocusPayload,
): Promise<FocusSessionResponse>;
```

### Query keys — additions to `src/lib/query/queryKeys.ts`

Existing `qk.dashboard` keeps `timeline`, `productivityToday`, `intelligence`. Existing `qk.productivity` keeps `calendarEvents`. Phase 3 extends `qk.productivity`:

```ts
productivity: {
  all: () => ["productivity"] as const,
  calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,

  // NEW
  summaryRange: (startDate: string, endDate: string) =>
    ["productivity", "summaryRange", startDate, endDate] as const,
  weekly: () => ["productivity", "weekly"] as const,
  patterns: (days: number | null) => ["productivity", "patterns", days ?? "default"] as const,
  hourlyBreakdown: (startDate: string, endDate: string) =>
    ["productivity", "hourlyBreakdown", startDate, endDate] as const,
  timeline: (date: string) => ["productivity", "timeline", date] as const,
  categories: () => ["productivity", "categories"] as const,
  intelligenceSessions: (date: string) =>
    ["productivity", "intelligenceSessions", date] as const,
  activityFeed: (limit: number) => ["productivity", "activityFeed", limit] as const,
  goals: () => ["productivity", "goals"] as const,
},
```

Goal mutations invalidate `qk.productivity.goals()`. Auto-focus confirmation invalidates `qk.dashboard.all()` (timeline picks up the new focus session) + any active `qk.productivity.summaryRange(...)`.

### Event subscriptions — additions to `src/services/events.ts`

Two new hubs follow the existing `createEventHub<T>(eventName)` factory pattern.

```ts
import type { AutoFocusPayload, FocusStatePayload } from "@/bindings";

const focusStateChangedHub = createEventHub<FocusStatePayload>("focus:state_changed");
const focusAutoDetectedHub = createEventHub<AutoFocusPayload>("focus:auto_detected");

export function subscribeFocusStateChanged(
  onEvent: (payload: FocusStatePayload) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return focusStateChangedHub.subscribe(onEvent, options);
}

export function subscribeFocusAutoDetected(
  onEvent: (payload: AutoFocusPayload) => void,
  options?: SubscriptionOptions,
): Unsubscribe {
  return focusAutoDetectedHub.subscribe(onEvent, options);
}
```

Components call them in `useEffect`:

```tsx
useEffect(() => subscribeFocusStateChanged(handler), []);
```

The backup's `focus:auto-started` subscription is dropped — no current backend publisher, and `focus:state_changed` covers transitions.

### Hook patterns

| Component | Hook |
|---|---|
| `SummaryPanel` (Day) | For today: `productivityTodayQuery` (existing). For other days: `productivitySummaryRangeQuery(date, date)`, then `result[0] ?? null`. |
| `WeeklySparkline` | `productivityWeeklyQuery` keyed by `qk.productivity.weekly()`. |
| `PatternsCard` | `productivityPatternsQuery(null)` keyed by `qk.productivity.patterns(null)`, `staleTime: 5 * 60_000`. |
| `HourlyHeatmap` | `productivityHourlyBreakdownQuery(start, end, TZ_OFFSET_MINS)` keyed by `qk.productivity.hourlyBreakdown(start, end)`, `staleTime: 60_000`. |
| `ActivityFeed` | `productivityActivityFeedQuery(30)` keyed by `qk.productivity.activityFeed(30)`. Polled every 30 s via `setInterval` calling `queryClient.invalidateQueries(...)`. |
| `ActivityTrack` | Three queries: `productivityTimelineQuery`, `productivityCategoriesQuery`, `productivityIntelligenceSessionsQuery`. Categories cached `staleTime: Infinity`; the other two ride on Day-view's existing 30 s poll. |
| `GoalsProgress` | `productivityGoalsQuery` keyed by `qk.productivity.goals()`. Create/delete via `useTauriMutation`; both invalidate `qk.productivity.goals()`. |
| `AutoFocusToast` | `productivityAutoFocusConfirm` via `useTauriMutation`; on success, invalidate `qk.dashboard.all()` + `qk.productivity.all()`. |

## Component contracts

### `SummaryPanel` (rewrite)

```ts
interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  selectedSession?: SessionBlock | null;     // NEW — Day view only
  onClose: () => void;
  productivitySummary?: ProductivitySummaryResponse | null;
  date?: string;                              // YYYY-MM-DD; defaults to todayISO()
}
```

Render dispatch: `selectedSession ? <SessionDetail/> : selectedEntry ? <EntryDetail/> : summary ? <DaySummary/> : null`.

`<DaySummary/>` sections (top→bottom, conditional):
1. Score ring + active-time bar + 4 ScoreBars + deep-work / recovery rows — only when `productivitySummary?.totalActiveSecs > 0`. Falls back to a simple "{totalTrackedSecs} tracked" header when not.
2. AI focus recommendation (italic) — from `intel.focusRecommendation`.
3. WeeklySparkline — when `productivityWeekly` returns ≥ 2 entries. SVG polyline + arrow indicator showing % change between halves.
4. PatternsCard — only when productivity data present.
5. HourlyHeatmap — `startDate=endDate=date`, only when productivity data present.
6. Top Apps chart — bars width-proportional to top app's `durationSecs`.
7. Insights (Brain icon for patterns, Lightbulb icon for nudges).
8. AI summary box — from `productivitySummary.aiSummary`.
9. GoalsProgress — always rendered (own loading state).

`<EntryDetail/>`: swatch + title; description; Started/Ended/Duration/Source rows; non-functional `entityRoute` link (`preventDefault` + lucide `ExternalLink`).

`<SessionDetail/>`: swatch + label; intelligence description; quality badge + category badge row; intelligence stats grid (Focus purity / Context switches / Distractions); time range + duration; per-app breakdown list with category dots.

### `ActivityTrack` (rewrite from null-stub)

```ts
interface ActivityTrackProps {
  date: string;
  hourHeight: number;
  isToday: boolean;
  onSelectSession: (session: SessionBlock) => void;
  onSelectEntry: (entry: TimelineEntry) => void;
  selectedSession: SessionBlock | null;
  selectedEntryId: string | null;
  /** When provided, skip independent fetch — parent owns the data. */
  timelineEntries?: ActivityTimelineResponse[];
}
```

Internals:
- Three queries: `productivityTimeline`, `productivityCategories`, `productivityIntelligenceSessions` (all skipped when `timelineEntries` provided — parent owns).
- Calls `mergeActivitySessions` (new lib) to consolidate adjacent same-category entries.
- `matchIntelligence(startMin, endMin, intellSessions)` (inline helper) picks the intelligence session with the largest overlap, requires ≥ 30% match.
- Color: `qualityToColor(matched.qualityScore)` if matched; else `resolveActivityColor(catType, isIdle)`.
- Opacity: `purityToOpacity(matched.categoryPurity)`; hover-dim other blocks to 0.3.
- Renders `<button>` blocks: `top = startMin × pxPerMin`, `height = max((endMin-startMin) × pxPerMin, 8)`.
- Quality badge (top-right pill) when `height > 24`; session label when `height > 18`; description/duration when `height > 32`; secondary duration when `height > 48 && matched?.description`.

### `ActivityFeed` (rewrite)

No props. Always-live, last-30 entries (not date-scoped).

Internals:
- `productivityActivityFeedQuery(30)` polled every 30 s.
- Local refs for new-entry detection: diff `currentKeys` vs `prevKeysRef`, animate fresh entries with `fade-in 0.4s`, scroll container to top.
- Per-entry display: `resolveDisplayName(e)` strips browser-suffix from window titles; falls back to `siteName`/`appName`/projectId.
- Relative-time tag: `now` < 10 s, `Ns` < 60 s, `Nm` < 5 min, then null.
- Per-30 s `setInterval` for re-rendering relative-time labels.
- `AppIcon` from `lib/productivity.ts`.

### `GoalsProgress`

No props. Renders a card with header + plus-button; lists each goal with progress bar; `MET` / `IN PROGRESS` label + delete button per row. Plus-button opens `<AddGoalDialog/>`. Mutations: `productivityGoalCreate`, `productivityGoalDelete`. Both invalidate `qk.productivity.goals()`.

### `AddGoalDialog`

```ts
interface AddGoalDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: { goalType: string; metric: string; targetValue: number }) => void;
}
```

Modal overlay (`fixed inset-0 z-50`). Period (daily/weekly) toggle, Metric radio-list (`productive_hours`, `focus_sessions`, `productivity_score`, `max_distracting_mins`), target number input. Note: the backup uses snake_case `goal_type` when calling the IPC; the new endpoint wrapper accepts camelCase `goalType` (matches the `bindings.ts` signature `productivityGoalCreate(goalType, metric, targetValue)`).

### `ProductivityScoreRing`

```ts
interface ProductivityScoreRingProps {
  score: number;
  size?: number;          // default 110
  summary?: {
    productiveSecs: number;
    neutralSecs: number;
    distractingSecs: number;
    totalActiveSecs: number;
    avgSessionQuality: number | null;
    focusSessionsCount: number;
    contextSwitches: number;
  } | null;
}
export function ScoreBar({ label, value }: { label: string; value: number }): JSX.Element;
```

SVG ring (background track + progress arc), -90° rotation, transitioning `stroke-dashoffset`. Color via `scoreColor(score)`. Glow gradient behind. Center: rounded score / "/100". Hover tooltip showing focus%, context switches, session quality, distraction% — only when `summary && totalActiveSecs > 0`.

### `HourlyHeatmap`

```ts
interface Props { startDate: string; endDate: string; }
```

Filters hours to 6–22. Computes `peakHour` and `maxRatio`. Per-hour row: hour label + horizontal bar. Color scale: red → orange → yellow → green via HSL interpolation in `heatColor(ratio)`.

### `PatternsCard`

No props. Single `productivityPatternsQuery(null)` call, 5-min stale time. Renders peak hours, best day-of-week, average session minutes, days-analyzed footer. Returns `null` until `daysAnalyzed >= 3`.

### `FocusStateIndicator`

No props. Subscribes to `focus:state_changed`. Three configured states: `building` / `focused` / `cooldown`. Renders a small pill with state-colored dot (pulsing for `building` and `cooldown`). Hidden for `unfocused` and `ended`.

### `FocusTrayIndicator`

No props. Subscribes to `focus:state_changed` only (drops backup's `focus:auto-started` — no current publisher). Visible whenever state ≠ `unfocused` and ≠ `ended`. Rendered inside topbar between date label and view-switcher pill.

### `AutoFocusToast`

No props. Subscribes to `focus:auto_detected`. Banner with `AppIcon`, "Focus session detected" header, `{ratio}% productive` chip, `{durationMins}min in {dominantApp}` body, **Confirm** + **Dismiss** buttons. Confirm calls `productivityAutoFocusConfirm` mutation; clears local state on success or dismiss.

### `ProductivityStrip`

```ts
interface ProductivityStripProps { summary: ProductivitySummary | null; }
```

Ported file-only, not mounted by any view. Renders a horizontal expandable bar with `<MiniScore/>`, `<CategoryBar/>`, top-3 apps, breakdown chips. Backup parity. Re-exported from `dashboard/index.ts`.

### Modified existing components

**`Dashboard.tsx`** — adds two siblings between topbar and `dashboard__content`:

```tsx
<DashboardTopbar />
<FocusStateIndicator />
<AutoFocusToast />
<div className="dashboard__content">{view}</div>
```

**`DashboardTopbar.tsx`** — adds `<FocusTrayIndicator />` after the date label, before the view-switcher pill.

**`DayColumns.tsx`** — three changes:
1. Add productivity-summary fetch: `productivityTodayQuery` for today, `productivitySummaryRangeQuery` for other dates. Pass result to SummaryPanel as `productivitySummary`.
2. Add `selectedSession: SessionBlock | null` state. Wire ActivityTrack with the real props (currently it's a no-prop call against the null-stub). Convert `selectedCalendarEvent` into a TimelineEntry-shaped object before handing to SummaryPanel as `selectedEntry` (when no `selectedSession`).
3. Mount `<ActivityFeed/>` in the bottom area inside an expand/collapse container with `feedExpanded: useState(false)`, matching backup `DayColumnsView` lines 519–541.

**`WeekView.tsx`** — adds `selectedEntry` state + click handler on session blocks; renders `<SummaryPanel summary={data.summary} selectedEntry={selectedEntry} onClose={…} />` when `sidebarOpen`. Replaces inline `style={{ fontSize: ... }}` with BEM classes. Removes `WeekView.tsx:93` dead-code guard.

**`MonthView.tsx`** — renders `<SummaryPanel summary={data.summary} selectedEntry={null} onClose={() => {}} />` when `sidebarOpen`. Replaces inline styles with BEM.

**`YearView.tsx`** — renders `<SummaryPanel summary={data.summary} selectedEntry={null} onClose={() => {}} />` when `sidebarOpen`. Wires `enabledSources` into the per-day aggregation: filter `data.entries` by `enabledSources.includes(entry.source as TimelineSource)` instead of hardcoding `entry.source === "focus"`. Replaces inline styles with BEM.

### New libs

**`lib/activity-sessions.ts`** — port from `desktop-ui.bak/src/shared/lib/activity-sessions.ts`. Exports `MergeableEvent` interface, `mergeActivitySessions(events): MergedSession[]`. Algorithm: sort by `startSecs`, merge adjacent same-category events within `MERGE_GAP_SECS` (60 s in backup), produce `MergedSession` with dominant category by total duration + per-app breakdown.

**`lib/productivity.ts`** — port helper functions from `desktop-ui.bak/src/shared/lib/productivity.ts`:
- `getAppColor(name, categoryId): string`
- `scoreColor(score): string`
- `qualityToColor(quality): string`
- `purityToOpacity(purity): number`
- `resolveActivityColor(categoryType, isIdle): string`
- `resolveCategoryLabel(categoryType): string`
- `AppIcon({ appName, color }): JSX.Element` — small colored pill with first letter

Implementation reads the backup helper file verbatim; CSS-variable references (e.g. `var(--brand)`) stay as-is — only Tailwind class strings get translated per the parent spec's token table. `SessionBlock` is exported from `views/ActivityTrack.tsx` (current stub already exports a placeholder type with the same name; the rewrite replaces both the component and the type's fields).

## Styling

All new BEM blocks under the `.dashboard` namespace, added to `src/styles/dashboard.css`. Tokens follow the parent spec's translation table — no new design tokens.

```
/* SummaryPanel — Day variant */
.dashboard__summary-panel
.dashboard__summary-section
.dashboard__summary-active
.dashboard__summary-active-time
.dashboard__summary-dim
.dashboard__summary-bar
.dashboard__summary-bar-seg
.dashboard__summary-bar-seg--productive
.dashboard__summary-bar-seg--neutral
.dashboard__summary-bar-seg--distracting
.dashboard__summary-productive-pct
.dashboard__summary-metrics
.dashboard__summary-score-bar
.dashboard__summary-score-label
.dashboard__summary-score-track
.dashboard__summary-score-fill
.dashboard__summary-score-value
.dashboard__summary-stat-row
.dashboard__summary-recommendation
.dashboard__summary-heading
.dashboard__summary-apps
.dashboard__summary-app-row
.dashboard__summary-app-name
.dashboard__summary-app-track
.dashboard__summary-app-fill
.dashboard__summary-app-dur
.dashboard__summary-insights
.dashboard__summary-insight-item
.dashboard__summary-aibox

/* Detail variants */
.dashboard__summary-detail-header
.dashboard__summary-close
.dashboard__summary-entry-title
.dashboard__summary-entry-swatch
.dashboard__summary-entry-desc
.dashboard__summary-entry-meta
.dashboard__summary-entry-source
.dashboard__summary-entry-link
.dashboard__summary-session-header
.dashboard__summary-session-stats
.dashboard__summary-session-quality-badge
.dashboard__summary-session-category-badge
.dashboard__summary-session-app-row

/* WeeklySparkline */
.dashboard__sparkline
.dashboard__sparkline-svg
.dashboard__sparkline-trend
.dashboard__sparkline-trend--up
.dashboard__sparkline-trend--down

/* ProductivityScoreRing */
.dashboard__score-ring
.dashboard__score-ring-track
.dashboard__score-ring-glow
.dashboard__score-ring-svg
.dashboard__score-ring-value
.dashboard__score-ring-label
.dashboard__score-ring-tooltip
.dashboard__score-ring-tooltip-row

/* PatternsCard */
.dashboard__patterns
.dashboard__patterns-title
.dashboard__patterns-row
.dashboard__patterns-footer

/* HourlyHeatmap */
.dashboard__hourly
.dashboard__hourly-title
.dashboard__hourly-peak
.dashboard__hourly-row
.dashboard__hourly-hour-label
.dashboard__hourly-bar-track
.dashboard__hourly-bar-fill

/* GoalsProgress + AddGoalDialog */
.dashboard__goals
.dashboard__goals-header
.dashboard__goals-add-btn
.dashboard__goals-empty
.dashboard__goal-row
.dashboard__goal-meta
.dashboard__goal-status
.dashboard__goal-status--met
.dashboard__goal-status--in-progress
.dashboard__goal-project-tag
.dashboard__goal-delete-btn
.dashboard__goal-bar-track
.dashboard__goal-bar-fill
.dashboard__goal-bar-fill--met
.dashboard__goal-dialog-backdrop
.dashboard__goal-dialog
.dashboard__goal-dialog-header
.dashboard__goal-dialog-body
.dashboard__goal-dialog-section
.dashboard__goal-dialog-period-toggle
.dashboard__goal-dialog-period-btn
.dashboard__goal-dialog-period-btn--active
.dashboard__goal-dialog-metric-list
.dashboard__goal-dialog-metric-btn
.dashboard__goal-dialog-metric-btn--active
.dashboard__goal-dialog-input
.dashboard__goal-dialog-footer

/* ActivityFeed */
.dashboard__activity-feed
.dashboard__activity-feed-header
.dashboard__activity-feed-live-dot
.dashboard__activity-feed-list
.dashboard__activity-feed-row
.dashboard__activity-feed-row--first
.dashboard__activity-feed-row--new
.dashboard__activity-feed-icon
.dashboard__activity-feed-time
.dashboard__activity-feed-tag
.dashboard__activity-feed-tag--recent
.dashboard__activity-feed-name
.dashboard__activity-feed-name--idle
.dashboard__activity-feed-name--first
.dashboard__activity-feed-subtitle

/* AppIcon (used by ActivityFeed + AutoFocusToast) */
.dashboard__app-icon

/* ActivityTrack — replaces null-stub */
.dashboard__activity-block
.dashboard__activity-block--selected
.dashboard__activity-block--shadow
.dashboard__activity-block-quality-badge
.dashboard__activity-block-title
.dashboard__activity-block-desc
.dashboard__activity-block-duration

/* Focus overlays */
.dashboard__focus-tray-pill
.dashboard__focus-state-pill
.dashboard__focus-state-banner
.dashboard__focus-state-pill-dot
.dashboard__focus-state-pill-dot--pulsing
.dashboard__auto-focus-toast
.dashboard__auto-focus-toast-icon
.dashboard__auto-focus-toast-body
.dashboard__auto-focus-toast-ratio
.dashboard__auto-focus-toast-actions
.dashboard__auto-focus-toast-confirm
.dashboard__auto-focus-toast-dismiss

/* ProductivityStrip — orphan, parity-only */
.dashboard__strip
.dashboard__strip-toggle
.dashboard__strip-mini-score
.dashboard__strip-category-bar
.dashboard__strip-category-seg
.dashboard__strip-quick-stats
.dashboard__strip-chevron
.dashboard__strip-detail
.dashboard__strip-top-apps
.dashboard__strip-breakdown

/* Phase-2 follow-up: replace inline styles */
.dashboard__week-loading
.dashboard__week-day-active
.dashboard__month-loading
.dashboard__year-loading
```

The existing `@keyframes fade-in` used by ActivityFeed is added to `dashboard.css` (not promoted to `ds-tokens.css` since it's local).

**Typography:** every `font-size` references a `--fs-*` token from `ds-tokens.css`. Backup uses `text-[9px]`, `text-2xs`, `text-[7px]`, `text-[8px]` extensively in ActivityTrack and ActivityFeed. Translation:
- `text-2xs`, `text-[11px]` → `var(--fs-2xs)` (10.5 px)
- `text-[9px]`, `text-[8px]`, `text-[7px]` → `var(--fs-2xs)` (rounded up to the smallest existing token)

Per `CLAUDE.md`, no new sub-2xs tokens. If a stakeholder later objects to ActivityTrack micro-text being slightly larger, add a `--fs-3xs` token in a follow-up.

## Tests

All use the existing `vi.mock("@tauri-apps/api/core", ...)` pattern. Files colocated with components.

1. **`SummaryPanel.test.tsx`** (covers all three render modes):
   - `null` summary + no selection → renders nothing.
   - `summary` only → DaySummary renders; "tracked" header visible.
   - `summary` + `productivitySummary.totalActiveSecs > 0` → ProductivityScoreRing + ScoreBars render; productive% computed correctly.
   - `selectedEntry` set → EntryDetail mode; clicking close calls `onClose`.
   - `selectedSession` set → SessionDetail mode; quality badge renders when `qualityScore != null`.

2. **`ActivityFeed.test.tsx`**:
   - Mock `productivityActivityFeedQuery` returning two entries → both rows render with app names.
   - Empty result → empty-state message visible.
   - Polling: advance fake timers by 30 s → query refetches.

3. **`ActivityTrack.test.tsx`**:
   - Stub all three queries; provide one timeline entry overlapping one intelligence session at ≥ 30% → `qualityToColor`-derived background applied.
   - Click block → `onSelectSession` called with the matched SessionBlock.
   - `selectedSession` matching by `startMin + label` → block gets `--selected` modifier.

4. **`GoalsProgress.test.tsx`**:
   - Mock `productivityGoalsQuery` returning two goals (one met, one in-progress) → both render with correct status pill.
   - Click delete → `productivityGoalDelete` mutation called with the goal's id; query invalidates.
   - Click plus-button → AddGoalDialog opens.

5. **`ProductivityScoreRing.test.tsx`**:
   - Score 75 → text "75" + "Good" label.
   - Hover with `summary` → tooltip rows visible.
   - Score 0 → glow opacity is 0.

6. **`Dashboard.test.tsx`** (extend existing):
   - FocusStateIndicator + AutoFocusToast siblings present in tree.
   - Mock `subscribeFocusStateChanged` → state-change payload renders the pill.

7. **`MonthView.test.tsx`** (extend):
   - Click on a day cell → assert `setDate` called with the cell's date AND `setMode("day")` called. Replaces the smoke "doesn't throw" test.

8. **`CalendarTrack.test.tsx`** (extend):
   - Two overlapping events → assert `computeOverlapLayout` produces `colIndex`/`totalCols` such that one block renders left-half, the other right-half.
   - Empty result → component returns `null` (assertion: container has no children).

9. **`WeekView.test.tsx`** (extend):
   - Click a session block → `setSelectedEntry` updates; SummaryPanel switches to EntryDetail mode.

10. **`YearView.test.tsx`** (extend):
   - Toggle layers (mock `useEnabledLayers` to exclude `focus`) → days previously tinted by focus go neutral.

No tests for `AutoFocusToast`, `FocusStateIndicator`, `FocusTrayIndicator`, `PatternsCard`, `HourlyHeatmap`, `AddGoalDialog`, `ProductivityStrip` — they're either trivial render-on-data components or fully covered indirectly via SummaryPanel / GoalsProgress tests.

## Risks and mitigations

- **Bindings drift on Goal mutation argument shape.** `bindings.ts` shows `productivityGoalCreate(goalType, metric, targetValue)` (camelCase positional), but the backup's `useMutation` call passed `{ goal_type, metric, target_value }` (snake_case object). The new endpoint wrapper accepts a typed `GoalCreateParams` object and unpacks to positional camelCase args at the call site. The bindings shape is authoritative.
- **`mergeActivitySessions` algorithm complexity.** ~120 lines, multiple thresholds (`MERGE_GAP_SECS`, `MIN_SESSION_SECS`, dominant-category logic). Port verbatim from backup, smoke-test via `ActivityTrack.test.tsx`. If algorithm-level bugs surface, add a unit test for `mergeActivitySessions` directly in `lib/activity-sessions.test.ts`.
- **`scoreColor` and friends use specific CSS-var names.** Backup references `--success`, `--brand`, `--destructive`, `--text-muted-foreground`, `--surface-raised`, `--popover`. Verify each token exists in current `ds-tokens.css` during implementation; map missing ones to nearest equivalent (e.g. `--text-muted-foreground` → `--ds-text-subtle`). One-time audit, not a per-component risk.
- **`AppIcon` location.** Lives in new `lib/productivity.ts` per the parent spec's "helpers" location. If implementation finds it cleaner to put it in `components/AppIcon.tsx`, that's fine — port location is non-load-bearing.
- **Specta type coverage.** All required types confirmed present in `bindings.ts` (`ActivityTimelineResponse`, `IntelligenceSessionResponse`, `ActivityCategoryResponse`, `GoalProgressResponse`, `ProductivityPatternsResponse`, `HourlyBreakdownResponse`, `FocusSessionResponse`, `AutoFocusPayload`, `FocusStatePayload`). If implementation finds a `null`-vs-non-null mismatch with backup behavior, fall back to optional-chaining at the call site; do NOT modify the Rust `specta::Type` derives unless you find a hard mismatch.
- **30 s ActivityFeed poll vs new-entry detection animation.** New-entry detection compares `prevKeysRef` against current `events` keys. Polling triggers a refetch every 30 s; if the cached query result is byte-equal, React Query won't re-render and `prevKeysRef` doesn't update. If the result changes, the effect runs with the diff and animates new entries. No additional refresh logic needed.
- **`AutoFocusToast` and `FocusStateIndicator` z-index.** Both render as siblings of `dashboard__content` inside `Dashboard.tsx`; they're in normal flow, not floating. No z-index management needed.
- **Test mocking surface grows.** Phase 3 adds 9 new endpoint wrappers + 2 event subscriptions. A shared test helper `__tests__/dashboardCommandMocks.ts` provides default-empty stubs to keep individual test files small.
- **`feedExpanded` state in DayColumns.** Backup keeps it as local `useState(false)` (collapsed by default). On dashboard remount the feed re-collapses. Match backup; persistence is out of scope.

## Acceptance criteria

```bash
cd desktop-ui && bun run typecheck   # clean
cd desktop-ui && bun run lint        # clean
cd desktop-ui && bun run test        # all pass
```

Manual smoke (`cargo tauri dev`):

1. Open Calendar → Day view. Right-rail SummaryPanel renders with productivity score ring + score bars + top apps + AI summary (if today has data).
2. Click an activity-session block (ActivityTrack) → SummaryPanel switches to SessionDetail with quality badge, category badge, app breakdown.
3. Click a calendar event → SummaryPanel switches to EntryDetail with title, time range, source.
4. Click EntryDetail's close button → returns to DaySummary.
5. Toggle the "Live Activity Feed" expand button at the bottom of Day view → feed slides open showing recent ~30 entries with relative-time tags; new entries fade-in when they arrive (poll cycle).
6. Switch to Week → SummaryPanel renders with `data.summary.totalTrackedSecs` + Top Apps from timeline summary (no productivity sections, since `productivitySummary` not passed in non-Day views).
7. Switch to Month / Year → same fallback summary; clicking cells still works.
8. In Year, toggle off the **Activity** layer in the Layers popover → days previously tinted go neutral; Focus-only tinting remains. Toggle Activity back on → tint returns.
9. Click the **Goals** card's plus button → AddGoalDialog opens; create a "Productive hours" daily goal with target 6 → goal appears in the list with an in-progress bar; click trash icon → goal removed.
10. Trigger a focus state-change from the backend (or mock) → FocusStateIndicator banner shows "Building focus" / "Deep focus" / "Cooldown" pill; FocusTrayIndicator pill in topbar appears in sync.
11. Trigger a `focus:auto_detected` event → AutoFocusToast banner appears with Confirm / Dismiss buttons; clicking Confirm fires `productivityAutoFocusConfirm` and clears the toast.
12. Sidebar toggle in topbar still hides/shows SummaryPanel across all four views.
13. CalendarTrack overlap layout still places overlapping events side-by-side in Day view (regression check).
14. Day-view task drag still works (regression check).
