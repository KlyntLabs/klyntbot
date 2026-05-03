# Desktop UI — Dashboard port, Phase 2 (Week / Month / Year + CalendarTrack)

**Date:** 2026-05-02
**Status:** Approved for implementation
**Parent spec:** [`2026-04-30-desktop-ui-dashboard-port-design.md`](2026-04-30-desktop-ui-dashboard-port-design.md)
**Phase 1 plan (shipped):** [`../plans/2026-04-30-desktop-ui-dashboard-port-phase-1.md`](../plans/2026-04-30-desktop-ui-dashboard-port-phase-1.md)
**Source:** `desktop-ui.bak/src/features/dashboard/components/{WeekCalendarView,MonthCalendarView,YearHeatmapView,CalendarTrack}.tsx`
**Target:** `desktop-ui/src/features/dashboard/components/views/`

## Goal

Replace the "coming in next phase" placeholder in `Dashboard.tsx` with three working views (Week, Month, Year) and turn the Phase 1 `CalendarTrack` stub into a functional Day-view overlay backed by `productivity_calendar_events`. Restyled to plain CSS + design tokens, freed from `react-router`.

Roughly 890 lines from the backup port to ~`desktop-ui/src/features/dashboard/components/views/`, plus one new endpoint wrapper, one query-key block, and a CSS section.

## Non-goals

- No backend changes. `timeline_query`, `task_update`, `productivity_calendar_events` already exist.
- No new dashboard sources beyond `productivityCalendarEvents` — `productivityWeekly`, `productivityGoals`, etc. remain Phase 3.
- No SummaryPanel, ActivityFeed, or any Phase-3 productivity components. Their stubs stay no-ops.
- No drag-to-reschedule in the new views. Backup Week/Month/Year are read-only — match that. Day view's existing drag stays unchanged.
- No persisted view-mode (still per-session, defaults to today's day view).

## Corrections to the parent spec

The parent spec line "CalendarTrack.tsx (~105L — calendar-event overlay used by Week/Month)" is wrong about the consumer:

- **CalendarTrack is used by Day view only.** It absolutely-positions events on an hour grid using `pxPerMin = HOUR_HEIGHT/60`. Backup Week renders merged session bars (no calendar-event overlay); backup Month has no hour grid at all. The current `DayColumns.tsx` already calls `<CalendarTrack />` (with no props, against the stub) — Phase 2 fills in the props.
- **`productivityCalendarEvents` wrapper moves from Phase 3 into Phase 2.** CalendarTrack depends on it; deferring the wrapper would block this work.

The parent spec's other Phase 2 statements (Week/Month/Year scope, ~900 line budget, reuse of `useTauriQuery` with different date ranges) hold.

## Decisions taken in brainstorm

1. **CalendarTrack scope = full wiring on Day view.** Fetch events via `productivityCalendarEvents`, render blocks, click selects. Selected-event state lives in `DayColumns` local `useState`. The selection has no visible side panel until Phase 3 SummaryPanel lands; Phase 2 surfaces selection only as the `.dashboard__calendar-event--selected` ring on the clicked block.
2. **Date preservation across mode switches = preserve underlying day, reformat as needed.** Already implemented correctly in `useDashboardState.setMode` from Phase 1: Day↔Week↔Month all keep the same `YYYY-MM-DD`; entering Year collapses to `YYYY`; leaving Year expands to `YYYY-01-01`. No hook changes required.
3. **Helpers stay inline** in each view file (`getWeekRange`, `buildWeekSessions`, `getMonthRange`, `buildCalendarGrid`, `buildMonthGrid`, `intensityClass`, `focusIntensityBg`, etc.). Backup pattern; each helper has a single consumer; extracting them costs file boundaries without payoff.
4. **SummaryPanel is dropped from view files.** Backup Week/Month/Year render `<SummaryPanel>` inline — Phase 2 views render full-width content. Phase 3 will wrap the dashboard content area with the right-rail SummaryPanel via `SidebarContext`.
5. **Navigation uses `useDashboardState`, not router.** Backup `navigate("/day/{date}")` becomes `setMode("day"); setDate(date)`. Backup `navigate(\`/day/${day}\`)` from a week column header same.

## File structure

```
desktop-ui/src/features/dashboard/components/views/
  WeekView.tsx               # NEW — port WeekCalendarView (~390L)
  MonthView.tsx              # NEW — port MonthCalendarView (~243L)
  YearView.tsx               # NEW — port YearHeatmapView (~150L)
  CalendarTrack.tsx          # REWRITE — replace null-stub with ~105L real impl
  CalendarTrack.test.tsx     # NEW
  WeekView.test.tsx          # NEW
  MonthView.test.tsx         # NEW
  YearView.test.tsx          # NEW

desktop-ui/src/features/dashboard/components/
  Dashboard.tsx              # MODIFY — switch arms render real views

desktop-ui/src/features/dashboard/components/views/
  DayColumns.tsx             # MODIFY — pass real props to <CalendarTrack/>

desktop-ui/src/api/endpoints/dashboard.ts
                             # MODIFY — add productivityCalendarEvents wrapper

desktop-ui/src/lib/query/queryKeys.ts
                             # MODIFY — add qk.productivity.calendarEvents key

desktop-ui/src/styles/dashboard.css
                             # MODIFY — add week/month/year/calendar-track BEM blocks
```

## IPC

### Endpoint wrapper — add to `src/api/endpoints/dashboard.ts`

```ts
export async function productivityCalendarEvents(date: string): Promise<CalendarEvent[]> {
  const r = await commands.productivityCalendarEvents(date);
  if (r.status !== "ok") throw new Error(r.error.message ?? "calendar events fetch failed");
  return r.data;
}
```

`CalendarEvent` is already in `bindings.ts` (line 4234). No `specta` work required.

### Query keys — add to `src/lib/query/queryKeys.ts`

Add a new top-level domain:

```ts
productivity: {
  all: () => ["productivity"] as const,
  calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,
},
```

Other `productivity.*` entries listed in the parent spec stay deferred to Phase 3.

## Component contracts

### `CalendarTrack` (rewrite)

```ts
interface CalendarTrackProps {
  date: string;                              // YYYY-MM-DD
  hourHeight: number;                        // px per hour
  selectedEventId: string | null;
  onSelectEvent: (event: CalendarEvent) => void;
}
```

Internals: `useTauriQuery({ queryKey: qk.productivity.calendarEvents(date), queryFn: () => productivityCalendarEvents(date), fallback: [] })`. Computes overlap layout via the existing `computeOverlapLayout` in `lib/timeline-utils.ts`. Renders `<button>` blocks with absolute positioning; selection adds `--selected` modifier. No internal state.

### `DayColumns.tsx` — change

Replace `<CalendarTrack />` (no-prop call against stub) with:

```tsx
const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
// ...
<CalendarTrack
  date={date}
  hourHeight={HOUR_HEIGHT}
  selectedEventId={selectedEventId}
  onSelectEvent={(e) => setSelectedEventId(e.id)}
/>
```

`HOUR_HEIGHT` already declared in `DayColumns.tsx`. No other DayColumns changes.

### `WeekView`

Reads `mode === "week"` date from `useDashboardState` (Monday-start). Computes 7-day range, fetches via `useTauriQuery` keyed by `qk.dashboard.timeline(start, end, sources)`. Per-day session merging via inline `buildWeekSessions` (port verbatim from backup). Click on a day header or session block → `setMode("day"); setDate(day)`.

### `MonthView`

Computes month range via inline `getMonthRange`. Fetches timeline. Builds 6×7 grid via inline `buildCalendarGrid` (Monday-start, with prev/next month padding). Each cell shows day number, focus duration, and a proportional active-time bar tinted by `focusIntensityBg`. Keyboard nav: arrow keys move `focusedDate`, Enter opens day. Click cell → `setMode("day"); setDate(cell.date)`.

### `YearView`

Reads `mode === "year"` date (`YYYY`) from state. Fetches `${year}-01-01` → `${year}-12-31` timeline. Aggregates focus seconds per day. Renders 12 mini-month grids in a `grid-cols-3`-style layout. Cell intensity from `intensityClass` (5 tiers). Click cell → `setMode("day"); setDate(day)`.

### `Dashboard.tsx` — change

```tsx
switch (state.mode) {
  case "day":   view = <DayView />; break;
  case "week":  view = <WeekView />; break;
  case "month": view = <MonthView />; break;
  case "year":  view = <YearView />; break;
}
```

Drop `dashboard__placeholder`. Keep all four context providers untouched.

## Styling

New BEM blocks in `dashboard.css`:

```
.dashboard__week-grid                  /* outer week-view container */
.dashboard__week-day-header            /* per-day column header (label + date + active time) */
.dashboard__week-day-header--today
.dashboard__week-hours                 /* scrolling hour grid wrapper */
.dashboard__week-hour-line             /* horizontal hour rule + label */
.dashboard__week-day-column            /* one of 7 columns within the hour grid */
.dashboard__week-session               /* merged activity session block */
.dashboard__week-session--focus        /* focus-overlay accent stripe */
.dashboard__week-now-line              /* today's "now" horizontal line */

.dashboard__month-grid
.dashboard__month-dow-header           /* Mon/Tue/... row */
.dashboard__month-cell
.dashboard__month-cell--other-month
.dashboard__month-cell--today
.dashboard__month-cell--focused        /* keyboard-focused cell */
.dashboard__month-activity-bar         /* proportional active-time bar */

.dashboard__year-grid                  /* 3-column month layout */
.dashboard__year-month
.dashboard__year-month-name
.dashboard__year-week                  /* one week-row within a mini-month */
.dashboard__year-cell                  /* a square */
.dashboard__year-cell--today
.dashboard__year-cell--tier-1          /* heatmap tiers 1..4; tier-0 = empty */
.dashboard__year-cell--tier-2
.dashboard__year-cell--tier-3
.dashboard__year-cell--tier-4
.dashboard__year-legend

.dashboard__calendar-event             /* a CalendarTrack block */
.dashboard__calendar-event--selected   /* selection ring */
```

Token mapping follows the parent spec's table. Focus tints use `color-mix(in oklch, var(--timeline-focus) Npct, transparent)`. The `--success` and `--timeline-focus` variables already live in `ds-tokens.css` from Phase 1.

## Tests

All use the existing `vi.mock("@tauri-apps/api/core", ...)` pattern.

1. **`CalendarTrack.test.tsx`** — mock `productivityCalendarEvents` to return two overlapping `CalendarEvent`s. Assert two `.dashboard__calendar-event` blocks render. Click one → `onSelectEvent` called with that event; passing the resulting `id` as `selectedEventId` prop adds the `--selected` modifier.
2. **`WeekView.test.tsx`** — mock `timelineQuery` to return entries on Mon and Wed. Render with `<DashboardStateContext.Provider value={{ mode: "week", date: <a known Monday>, ... }}>`. Assert 7 day-header buttons render with the correct day numbers; clicking Wed's header calls `setMode("day")` then `setDate("<Wed iso>")`.
3. **`MonthView.test.tsx`** — render with a fixed date in April 2026. Assert 42 cells render; today (`2026-05-02`) gets the `--today` modifier when applicable; pressing ArrowRight after focus moves the focused-cell modifier to the next day.
4. **`YearView.test.tsx`** — render with `mode: "year", date: "2026"`. Mock `timelineQuery` returning two focus entries on different days. Assert 12 month sub-grids render; the two days with focus entries get a `--tier-N` modifier (any N ≥ 1); clicking one calls `setMode("day")` then `setDate(<iso>)`.

No new test for `useDashboardState` — Phase 1 already covers `setMode` formatting and `navigatePrev/Next` per-mode arithmetic.

## Acceptance criteria

```bash
cd desktop-ui && bun run typecheck   # clean
cd desktop-ui && bun run lint        # clean
cd desktop-ui && bun run test        # all pass
```

Manual smoke (`cargo tauri dev`):

1. From Day view, click view-pill **Week** → 7-column hour grid renders; hour rules + Mon..Sun headers visible; today's column highlighted.
2. Switch to **Month** → 6×7 grid renders; today has the today-ring; cells show day numbers and (for days with data) focus duration + activity bar.
3. Switch to **Year** → 12 mini-month heatmaps render in a 3-column layout; days with focus minutes are tinted; legend visible.
4. From Year view, click any day cell → drops to Day view on that date.
5. From Month view, focus the grid (Tab) and arrow-key around → focused-cell ring follows; Enter opens that day.
6. From Day view, calendar events appear as colored blocks overlaid on the hour grid (after `productivity_calendar_events` returns). Clicking a block adds the selection ring.
7. Day view's existing task drag still works (no Phase 2 regression).
8. Switch Day → Year while sitting on `2026-04-30` → Year shows `2026`. Switch Year → Day → land on `2026-01-01`. (Matches the existing `setMode` behavior.)

## Risks and mitigations

- **`computeOverlapLayout` API drift.** Phase 1 ported this with a generic signature `<T extends { id; startedAt; durationSecs }>`. CalendarTrack adapts `CalendarEvent` (which has `endedAt`, not `durationSecs`) by computing `durationSecs` inline before calling. Same pattern as the backup. Verify shape during implementation.
- **`CalendarEvent.color` may be null.** Backup falls back to `var(--timeline-focus)`. Match.
- **Week-view session merging is non-trivial** (`buildWeekSessions` ~50 lines: filter idle, merge cross-app within `SESSION_GAP_MIN`, drop sub-`MIN_SESSION_SECS`, attach focus overlay flag). Port verbatim — algorithm works in the backup. Snapshot in a unit test if it drifts.
- **Monday-start week** (backup convention) differs from Sunday-start in some locales. Stay Monday-start to preserve backup behavior; revisit if a user complaint surfaces.
- **`buildCalendarGrid` always emits 42 cells** (6 rows). When the month fits in 5 rows, the 6th row is next-month padding — visible as muted text. Match backup.
