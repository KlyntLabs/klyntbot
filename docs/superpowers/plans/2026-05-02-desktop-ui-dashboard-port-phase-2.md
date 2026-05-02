# Desktop UI Dashboard Port — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Phase 1 "coming in next phase" placeholder with three working views (Week, Month, Year) and turn the Phase 1 `CalendarTrack` stub into a real Day-view calendar-event overlay.

**Architecture:** Three new view components under `desktop-ui/src/features/dashboard/components/views/`, plus rewriting the `CalendarTrack` stub. Each view reads `mode` + `date` from `useDashboardState`, fetches timeline data via `useTauriQuery`, and navigates by mutating dashboard state (no `react-router`). One new endpoint wrapper (`productivityCalendarEvents`), one new `qk.productivity` query-key block, one new CSS section per view. SummaryPanel stays a no-op stub until Phase 3.

**Tech Stack:** React 18 + Vitest + React Query (`useTauriQuery`) + plain CSS with BEM-ish naming + design tokens from `ds-tokens.css`.

**Spec:** [`docs/superpowers/specs/2026-05-02-desktop-ui-dashboard-port-phase-2-design.md`](../specs/2026-05-02-desktop-ui-dashboard-port-phase-2-design.md)
**Parent spec:** [`docs/superpowers/specs/2026-04-30-desktop-ui-dashboard-port-design.md`](../specs/2026-04-30-desktop-ui-dashboard-port-design.md)
**Phase 1 plan (shipped):** [`docs/superpowers/plans/2026-04-30-desktop-ui-dashboard-port-phase-1.md`](2026-04-30-desktop-ui-dashboard-port-phase-1.md)

---

## Reference paths (memorize these)

**Current files (read-only context):**
- `desktop-ui/src/features/dashboard/components/views/DayView.tsx` — model for the new view files
- `desktop-ui/src/features/dashboard/components/views/DayView.test.tsx` — model for the new test files
- `desktop-ui/src/features/dashboard/components/views/DayColumns.tsx` — gets a small modification (Task 5)
- `desktop-ui/src/features/dashboard/components/Dashboard.tsx` — gets a small modification (Task 12)
- `desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx` — current stub, replaced in Task 3
- `desktop-ui/src/features/dashboard/hooks/useDashboardState.ts` — `DashboardStateContext` + `useDashboardStateImpl` for tests
- `desktop-ui/src/features/dashboard/lib/layers.ts` — `LayerContext` + `useEnabledLayers` + `useSidebarOpen`
- `desktop-ui/src/features/dashboard/lib/timeline-utils.ts` — `computeOverlapLayout`, `IDLE_APPS`, `DAY_LABELS`, `computeDayStats`, `isActiveAppEntry`
- `desktop-ui/src/utils/dashboardDates.ts` — `todayISO`, `toLocalISO`, `shiftDate`, `formatHumanDuration`, `minutesSinceMidnight`, `SHORT_MONTHS`, `TZ_OFFSET_MINS`
- `desktop-ui/src/api/endpoints/dashboard.ts` — Phase 1 endpoint wrappers (`timelineQuery`, `taskUpdate`, `calendarSyncEvents`, `EMPTY_TIMELINE_RESPONSE`)
- `desktop-ui/src/lib/query/queryKeys.ts` — `qk.dashboard.timeline` already lives here
- `desktop-ui/src/lib/query/useTauriQuery.ts` — query hook signature
- `desktop-ui/src/bindings.ts` — types: `CalendarEvent` (line ~4234), `TimelineEntry`, `TimelineResponse`, `TimelineSource`
- `desktop-ui/src/styles/dashboard.css` — feature-scoped CSS (538 lines after Phase 1)

**Backup files (port sources — read-only):**
- `desktop-ui.bak/src/features/dashboard/components/CalendarTrack.tsx` (~105L)
- `desktop-ui.bak/src/features/dashboard/components/WeekCalendarView.tsx` (~390L)
- `desktop-ui.bak/src/features/dashboard/components/MonthCalendarView.tsx` (~243L)
- `desktop-ui.bak/src/features/dashboard/components/YearHeatmapView.tsx` (~150L)

---

## Backup ⇄ current API differences (translation cheat sheet)

| Backup pattern | Current pattern |
|---|---|
| `useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE)` | `useTauriQuery({ queryKey: qk.dashboard.timeline(start, end, sources), queryFn: () => timelineQuery(start, end, sources, true, TZ_OFFSET_MINS), fallback: EMPTY_TIMELINE_RESPONSE })` |
| `useQuery<CalendarEvent[]>("productivity_calendar_events", { date }, [])` | `useTauriQuery<CalendarEvent[]>({ queryKey: qk.productivity.calendarEvents(date), queryFn: () => productivityCalendarEvents(date), fallback: [] })` |
| `useNavigate(); navigate(\`/day/${date}\`)` | `const { setMode, setDate } = useDashboardState(); setMode("day"); setDate(date);` |
| `useParams<{ date: string }>(); const { date } = useParams()` | `const { date } = useDashboardState();` |
| `useParams<{ year: string }>(); Number(useParams().year)` | `const { date } = useDashboardState(); Number(date)` (mode `"year"` stores `YYYY` directly) |
| Tailwind `bg-card`, `text-muted-foreground`, `text-xs`, `glass-card`, `bg-success`, `bg-timeline-focus/40` | Plain CSS classes from this plan, against tokens in `ds-tokens.css` |
| `cn("a", cond && "b")` | template literals or array `.filter(Boolean).join(" ")` |
| `<SummaryPanel ...>` rendered inline | **omit entirely** (Phase 3) |
| `import { ... } from "@shared/lib/dates"` / `"@shared/types"` | `from "@/utils/dashboardDates"` / `from "@/bindings"` |

---

## Task 1: Add `productivityCalendarEvents` endpoint wrapper

**Files:**
- Modify: `desktop-ui/src/api/endpoints/dashboard.ts`

**Why:** Phase 1 deferred this wrapper to Phase 3, but the real `CalendarTrack` (Task 3) needs it. Mirrors the pattern of `timelineQuery` and `taskUpdate` already in this file.

- [ ] **Step 1: Read the current file**

Run: `cat desktop-ui/src/api/endpoints/dashboard.ts`
Confirm it ends with the `calendarSyncEvents` function and imports `CalendarEventInput`, `TaskResponse`, `TaskUpdateParams`, `TimelineResponse`, `TimelineSource`, `commands`.

- [ ] **Step 2: Edit the imports to add `CalendarEvent`**

Change the import block at the top of `desktop-ui/src/api/endpoints/dashboard.ts` from:

```ts
import type {
  CalendarEventInput,
  TaskResponse,
  TaskUpdateParams,
  TimelineResponse,
  TimelineSource,
} from "@/bindings";
```

to:

```ts
import type {
  CalendarEvent,
  CalendarEventInput,
  TaskResponse,
  TaskUpdateParams,
  TimelineResponse,
  TimelineSource,
} from "@/bindings";
```

- [ ] **Step 3: Append the new wrapper to the end of the file**

Append (after the `calendarSyncEvents` function):

```ts

/**
 * Fetch calendar events overlapping the given local-date day.
 * Returns an empty array if the backend has no events for that date.
 */
export async function productivityCalendarEvents(date: string): Promise<CalendarEvent[]> {
  const r = await commands.productivityCalendarEvents(date);
  if (r.status !== "ok") throw new Error(r.error.message ?? "calendar events fetch failed");
  return r.data;
}
```

- [ ] **Step 4: Verify typecheck is clean**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean (no errors).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/api/endpoints/dashboard.ts
git commit -m "feat(dashboard): add productivityCalendarEvents endpoint wrapper"
```

---

## Task 2: Add `qk.productivity.calendarEvents` query key (TDD)

**Files:**
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`

**Why:** A new query-key namespace for productivity data. Phase 2 only needs `calendarEvents`; the remaining productivity keys land in Phase 3.

- [ ] **Step 1: Write the failing test**

The existing `desktop-ui/src/lib/query/tests/queryKeys.test.ts` is structured as multiple top-level `describe(...)` blocks each closed with `});`. Append a new top-level block at the very end of the file:

```ts

describe("queryKeys — phase 2 productivity", () => {
  it("productivity.all is stable", () => {
    expect(qk.productivity.all()).toEqual(["productivity"]);
  });

  it("productivity.calendarEvents encodes date", () => {
    expect(qk.productivity.calendarEvents("2026-05-02")).toEqual([
      "productivity",
      "calendarEvents",
      "2026-05-02",
    ]);
  });
});
```

The existing file already imports `describe`, `expect`, `it`, and `qk`; no new imports needed.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd desktop-ui && bun run test -- queryKeys`
Expected: FAIL with a TypeScript error like `Property 'productivity' does not exist on type ...`.

- [ ] **Step 3: Add the `productivity` key block**

In `desktop-ui/src/lib/query/queryKeys.ts`, find the existing `dashboard:` block:

```ts
  dashboard: {
    all: () => ["dashboard"] as const,
    timeline: (startDate: string, endDate: string, sources: readonly string[]) =>
      ["dashboard", "timeline", startDate, endDate, [...sources].sort().join(",")] as const,
  },
```

Insert immediately after it (before `calendarSync:`):

```ts
  productivity: {
    all: () => ["productivity"] as const,
    calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,
  },
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd desktop-ui && bun run test -- queryKeys`
Expected: PASS for both new cases (and all pre-existing tests continue to pass).

- [ ] **Step 5: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts desktop-ui/src/lib/query/tests/queryKeys.test.ts
git commit -m "feat(dashboard): add qk.productivity.calendarEvents query key"
```

---

## Task 3: Port `CalendarTrack.tsx` (TDD)

**Files:**
- Modify (overwrite): `desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx`

**Why:** Replace the Phase 1 stub with the real implementation. Renders calendar events as absolutely-positioned, click-to-select blocks on the day-view hour grid. Uses overlap layout from existing `computeOverlapLayout`.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx`:

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CalendarEvent } from "@/bindings";

const mockEvents: CalendarEvent[] = [
  {
    id: "evt-1",
    calendarId: "cal-a",
    title: "Standup",
    description: null,
    startedAt: "2026-05-02T09:00:00Z",
    endedAt: "2026-05-02T09:30:00Z",
    location: null,
    attendeesCount: 0,
    isRecurring: false,
    recurrenceId: null,
    source: "google",
    externalUid: "ext-1",
    sessionId: null,
    color: null,
    syncedAt: "2026-05-02T08:00:00Z",
    createdAt: "2026-05-02T08:00:00Z",
    updatedAt: "2026-05-02T08:00:00Z",
  },
  {
    id: "evt-2",
    calendarId: "cal-a",
    title: "Design review",
    description: null,
    startedAt: "2026-05-02T09:15:00Z",
    endedAt: "2026-05-02T10:00:00Z",
    location: "Room 4",
    attendeesCount: 3,
    isRecurring: false,
    recurrenceId: null,
    source: "google",
    externalUid: "ext-2",
    sessionId: null,
    color: "#ff8800",
    syncedAt: "2026-05-02T08:00:00Z",
    createdAt: "2026-05-02T08:00:00Z",
    updatedAt: "2026-05-02T08:00:00Z",
  },
];

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    productivityCalendarEvents: vi.fn(),
  };
});

import { productivityCalendarEvents } from "@/api/endpoints/dashboard";
import { CalendarTrack } from "./CalendarTrack";

const mockedProductivityCalendarEvents = vi.mocked(productivityCalendarEvents);

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("CalendarTrack", () => {
  it("renders one block per event from productivityCalendarEvents", async () => {
    mockedProductivityCalendarEvents.mockResolvedValue(mockEvents);

    render(
      wrap(
        <CalendarTrack
          date="2026-05-02"
          hourHeight={48}
          selectedEventId={null}
          onSelectEvent={() => {}}
        />,
      ),
    );

    await waitFor(() => {
      expect(screen.getAllByRole("button")).toHaveLength(2);
    });
    expect(screen.getByText("Standup")).toBeTruthy();
    expect(screen.getByText("Design review")).toBeTruthy();
  });

  it("calls onSelectEvent with the clicked event", async () => {
    mockedProductivityCalendarEvents.mockResolvedValue(mockEvents);
    const onSelectEvent = vi.fn();

    render(
      wrap(
        <CalendarTrack
          date="2026-05-02"
          hourHeight={48}
          selectedEventId={null}
          onSelectEvent={onSelectEvent}
        />,
      ),
    );

    const standup = await screen.findByText("Standup");
    fireEvent.click(standup);

    expect(onSelectEvent).toHaveBeenCalledTimes(1);
    expect(onSelectEvent.mock.calls[0][0].id).toBe("evt-1");
  });

  it("applies the --selected modifier to the selected event", async () => {
    mockedProductivityCalendarEvents.mockResolvedValue(mockEvents);

    render(
      wrap(
        <CalendarTrack
          date="2026-05-02"
          hourHeight={48}
          selectedEventId="evt-2"
          onSelectEvent={() => {}}
        />,
      ),
    );

    const designReview = await screen.findByText("Design review");
    const button = designReview.closest("button");
    expect(button?.className).toContain("dashboard__calendar-event--selected");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd desktop-ui && bun run test -- CalendarTrack`
Expected: FAIL — current `CalendarTrack` returns `null`, so all three tests fail (no buttons found, etc.).

- [ ] **Step 3: Replace the stub with the real implementation**

Overwrite `desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx`:

```tsx
import { useMemo } from "react";
import type { CalendarEvent } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { productivityCalendarEvents } from "@/api/endpoints/dashboard";
import { formatHumanDuration, minutesSinceMidnight } from "@/utils/dashboardDates";
import { computeOverlapLayout } from "../../lib/timeline-utils";

interface CalendarTrackProps {
  date: string; // YYYY-MM-DD
  hourHeight: number; // px per hour
  selectedEventId: string | null;
  onSelectEvent: (event: CalendarEvent) => void;
}

const EMPTY_EVENTS: CalendarEvent[] = [];

export function CalendarTrack({
  date,
  hourHeight,
  selectedEventId,
  onSelectEvent,
}: CalendarTrackProps) {
  const pxPerMin = hourHeight / 60;

  const { data: events } = useTauriQuery<CalendarEvent[]>({
    queryKey: qk.productivity.calendarEvents(date),
    queryFn: () => productivityCalendarEvents(date),
    fallback: EMPTY_EVENTS,
  });

  const layouts = useMemo(() => {
    const items = events.map((e) => ({
      id: e.id,
      startedAt: e.startedAt,
      durationSecs: Math.round(
        (new Date(e.endedAt).getTime() - new Date(e.startedAt).getTime()) / 1000,
      ),
    }));
    return computeOverlapLayout(items);
  }, [events]);

  return (
    <>
      {events.map((event) => {
        const startMin = minutesSinceMidnight(event.startedAt);
        const endMin = minutesSinceMidnight(event.endedAt);
        const top = startMin * pxPerMin;
        const height = Math.max((endMin - startMin) * pxPerMin, 14);
        const isSelected = selectedEventId === event.id;
        const color = event.color ?? "var(--border-accent)";
        const layout = layouts.get(event.id);
        const hasOverlap = layout && layout.totalCols > 1;

        const posStyle: React.CSSProperties = hasOverlap
          ? {
              top,
              height,
              left: `${(layout.colIndex / layout.totalCols) * 100}%`,
              width: `${(1 / layout.totalCols) * 100}%`,
              paddingLeft: 4,
              paddingRight: 2,
            }
          : { top, height, left: 4, right: 2 };

        const durationSecs = Math.round((endMin - startMin) * 60);
        const className = [
          "dashboard__calendar-event",
          isSelected ? "dashboard__calendar-event--selected" : "",
        ]
          .filter(Boolean)
          .join(" ");

        return (
          <button
            type="button"
            key={event.id}
            className={className}
            style={{
              ...posStyle,
              borderLeftColor: color,
              backgroundColor: `color-mix(in oklch, ${color} 12%, transparent)`,
            }}
            onClick={() => onSelectEvent(event)}
            aria-label={`${event.title}, ${formatHumanDuration(durationSecs)}`}
          >
            {height > 16 && (
              <span className="dashboard__calendar-event-title">{event.title}</span>
            )}
            {height > 30 && (
              <span className="dashboard__calendar-event-meta">
                {formatHumanDuration(durationSecs)}
                {event.location ? ` · ${event.location}` : ""}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd desktop-ui && bun run test -- CalendarTrack`
Expected: PASS (all three test cases).

- [ ] **Step 5: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx
git commit -m "feat(dashboard): port real CalendarTrack with selection + overlap layout"
```

---

## Task 4: Add CSS for `.dashboard__calendar-event*`

**Files:**
- Modify: `desktop-ui/src/styles/dashboard.css` (append)

**Why:** The component's class names need styles. `border-left` accent, subtle tinted background, selection ring.

- [ ] **Step 1: Append the new CSS section**

Append to the end of `desktop-ui/src/styles/dashboard.css`:

```css

/* ── Calendar event blocks (CalendarTrack) ──────────────────── */
.dashboard__calendar-event {
  position: absolute;
  border: none;
  border-left: 2px solid var(--border-accent);
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
  padding: 0;
  background: transparent;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__calendar-event:hover {
  filter: brightness(1.1);
}

.dashboard__calendar-event--selected {
  outline: 1px solid var(--border-accent);
  outline-offset: -1px;
}

.dashboard__calendar-event-title {
  display: block;
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
  padding: 2px 6px 0 6px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dashboard__calendar-event-meta {
  display: block;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  padding: 0 6px;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
```

- [ ] **Step 2: Verify the CSS file imports are still clean**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 3: Run the CalendarTrack test again to confirm no regression**

Run: `cd desktop-ui && bun run test -- CalendarTrack`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css
git commit -m "style(dashboard): add .dashboard__calendar-event styles"
```

---

## Task 5: Wire `CalendarTrack` into `DayColumns` with selection state

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/DayColumns.tsx`

**Why:** The current call site is `<CalendarTrack />` (no props, against the stub). Pass the four real props and own `selectedEventId` state locally. The selection has no visible side panel until Phase 3 — this just sets up the wiring.

- [ ] **Step 1: Confirm the `useState` import + the dynamic `hourHeight` already exist**

Run: `grep -n "useState\|const \[hourHeight\|<CalendarTrack" desktop-ui/src/features/dashboard/components/views/DayColumns.tsx`
Expected output:
- A line importing `useState` from `react`.
- A line `const [hourHeight, setHourHeight] = useState(DEFAULT_HOUR_HEIGHT);` (around line 114).
- A line `<CalendarTrack />` (around line 405).

The dynamic `hourHeight` state is what the calendar grid already uses for zoom. We pass that same value to `CalendarTrack` so events scale with zoom.

- [ ] **Step 2: Add the selection state inside the `DayColumns` function body**

Find the line (around 114):

```tsx
  const [hourHeight, setHourHeight] = useState(DEFAULT_HOUR_HEIGHT);
```

Insert immediately above it:

```tsx
  const [selectedCalendarEventId, setSelectedCalendarEventId] = useState<string | null>(null);
```

- [ ] **Step 3: Replace the stub call site**

Find the block (around lines 402–408):

```tsx
                // Calendar column: fetches its own data
                if (col.key === "calendar") {
                  return (
                    <div key={col.key} className="dashboard__day-column">
                      <CalendarTrack />
                    </div>
                  );
                }
```

Replace with:

```tsx
                // Calendar column: fetches its own data
                if (col.key === "calendar") {
                  return (
                    <div key={col.key} className="dashboard__day-column">
                      <CalendarTrack
                        date={date}
                        hourHeight={hourHeight}
                        selectedEventId={selectedCalendarEventId}
                        onSelectEvent={(event) => setSelectedCalendarEventId(event.id)}
                      />
                    </div>
                  );
                }
```

- [ ] **Step 4: Verify typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 5: Run the existing DayView test to confirm no regression**

Run: `cd desktop-ui && bun run test -- DayView`
Expected: PASS (the test mocks `productivityCalendarEvents` indirectly only if CalendarTrack triggers a fetch; if the test fails because of an unmocked call, see Step 6).

- [ ] **Step 6: If DayView test fails because CalendarTrack now triggers `productivityCalendarEvents`**

Open `desktop-ui/src/features/dashboard/components/views/DayView.test.tsx` and add `productivityCalendarEvents: vi.fn().mockResolvedValue([])` to the existing `vi.mock("@/api/endpoints/dashboard", ...)` factory's returned object:

Find:

```ts
vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn(),
    taskUpdate: vi.fn(),
  };
});
```

Change to:

```ts
vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn(),
    taskUpdate: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
  };
});
```

Re-run `bun run test -- DayView` and expect PASS.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DayColumns.tsx desktop-ui/src/features/dashboard/components/views/DayView.test.tsx
git commit -m "feat(dashboard): wire CalendarTrack into DayColumns with selection state"
```

---

## Task 6: Port `WeekView.tsx` (TDD)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/WeekView.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/WeekView.test.tsx`

**Why:** Renders 7 day columns with merged activity sessions per day. Click a day header → drop to Day view. The largest of the three new views (~390 lines from backup). Algorithm (`buildWeekSessions`) ports verbatim — keep helpers inline.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/dashboard/components/views/WeekView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const emptyTimeline: TimelineResponse = {
  entries: [],
  summary: {
    totalTrackedSecs: 0,
    focusSecs: 0,
    tasksCompleted: 0,
    tasksCreated: 0,
    notesTouched: 0,
    transactionsCount: 0,
    topApps: [],
    sourceBreakdown: [],
  },
};

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(emptyTimeline),
    taskUpdate: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
  };
});

import {
  DashboardStateContext,
  type DashboardState,
} from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { WeekView } from "./WeekView";

afterEach(() => cleanup());

function makeState(over: Partial<DashboardState> = {}): DashboardState {
  return {
    mode: "week",
    date: "2026-04-27", // Monday
    setMode: vi.fn(),
    setDate: vi.fn(),
    navigatePrev: vi.fn(),
    navigateNext: vi.fn(),
    navigateToday: vi.fn(),
    ...over,
  };
}

function wrap(node: ReactNode, state: DashboardState) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <DashboardStateContext.Provider value={state}>
        <LayerContext.Provider
          value={{
            enabled: new Set(["activity"]),
            enabledSources: ["productivity", "focus"],
            toggle: () => {},
            reset: () => {},
          }}
        >
          {node}
        </LayerContext.Provider>
      </DashboardStateContext.Provider>
    </QueryClientProvider>
  );
}

describe("WeekView", () => {
  it("renders 7 day-header buttons for the week", async () => {
    const state = makeState();
    render(wrap(<WeekView />, state));
    await waitFor(() => {
      // 7 day headers, identifiable by role=button. Some other buttons may exist
      // but a week always has at least 7 day buttons.
      const buttons = screen.getAllByRole("button");
      expect(buttons.length).toBeGreaterThanOrEqual(7);
    });
    expect(screen.getByText("Mon")).toBeTruthy();
    expect(screen.getByText("Sun")).toBeTruthy();
  });

  it("clicking a day header switches to day mode and sets that date", async () => {
    const setMode = vi.fn();
    const setDate = vi.fn();
    const state = makeState({ setMode, setDate });
    render(wrap(<WeekView />, state));

    // Wed is the third day of a Monday-start week → 2026-04-29
    const wedHeader = await screen.findByText("Wed");
    fireEvent.click(wedHeader.closest("button")!);

    expect(setMode).toHaveBeenCalledWith("day");
    expect(setDate).toHaveBeenCalledWith("2026-04-29");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd desktop-ui && bun run test -- WeekView`
Expected: FAIL with `Cannot find module './WeekView'`.

- [ ] **Step 3: Create the WeekView component**

Create `desktop-ui/src/features/dashboard/components/views/WeekView.tsx`:

```tsx
import { useEffect, useMemo, useRef, useState } from "react";
import type { TimelineEntry } from "@/bindings";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  minutesSinceMidnight,
  toLocalISO,
  todayISO,
  TZ_OFFSET_MINS,
} from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";
import { computeDayStats, DAY_LABELS, IDLE_APPS } from "../../lib/timeline-utils";

const HOUR_HEIGHT = 48;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 4;
const HOUR_GUTTER = 40;
const PX_PER_MIN = HOUR_HEIGHT / 60;
const SESSION_GAP_MIN = 10;
const MIN_ENTRY_SECS = 30;
const MIN_SESSION_SECS = 120;
const HOURS = Array.from({ length: 24 }, (_, i) => i);

function getWeekRange(dateStr: string): { start: string; end: string; days: string[] } {
  const d = new Date(`${dateStr}T00:00:00`);
  const dayOfWeek = d.getDay();
  const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
  const monday = new Date(d);
  monday.setDate(d.getDate() + mondayOffset);

  const days: string[] = [];
  for (let i = 0; i < 7; i++) {
    const day = new Date(monday);
    day.setDate(monday.getDate() + i);
    days.push(toLocalISO(day));
  }
  return { start: days[0], end: days[6], days };
}

function formatHour(h: number): string {
  if (h === 0) return "";
  if (h < 12) return `${h}a`;
  if (h === 12) return "12p";
  return `${h - 12}p`;
}

interface WeekSession {
  startMin: number;
  endMin: number;
  totalSecs: number;
  label: string;
  appCount: number;
  type: "activity" | "focus";
  hasFocus?: boolean;
}

interface BuildingSession {
  startMin: number;
  endMin: number;
  totalSecs: number;
  appDurations: Map<string, number>;
}

function finishSession(cur: BuildingSession): WeekSession {
  let dominantApp = "Activity";
  let maxDur = 0;
  for (const [app, dur] of cur.appDurations) {
    if (dur > maxDur) {
      maxDur = dur;
      dominantApp = app;
    }
  }
  return {
    startMin: cur.startMin,
    endMin: cur.endMin,
    totalSecs: cur.totalSecs,
    label: dominantApp,
    appCount: cur.appDurations.size,
    type: "activity",
  };
}

function buildWeekSessions(entries: TimelineEntry[]): WeekSession[] {
  const activityEntries: TimelineEntry[] = [];
  const focusEntries: TimelineEntry[] = [];

  for (const e of entries) {
    if (e.entryType === "focusSession") {
      focusEntries.push(e);
    } else if (
      e.entryType === "appUsage" &&
      (e.durationSecs ?? 0) >= MIN_ENTRY_SECS &&
      !IDLE_APPS.has(e.title.toLowerCase())
    ) {
      activityEntries.push(e);
    }
  }
  activityEntries.sort(
    (a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime(),
  );

  const sessions: WeekSession[] = [];
  let cur: BuildingSession | null = null;

  for (const entry of activityEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const dur = (entry.durationSecs ?? 0) / 60;
    const endMin = startMin + dur;

    if (cur && startMin - cur.endMin <= SESSION_GAP_MIN) {
      cur.endMin = Math.max(cur.endMin, endMin);
      cur.totalSecs += entry.durationSecs ?? 0;
      cur.appDurations.set(
        entry.title,
        (cur.appDurations.get(entry.title) || 0) + (entry.durationSecs ?? 0),
      );
    } else {
      if (cur) sessions.push(finishSession(cur));
      const appDurations = new Map<string, number>();
      appDurations.set(entry.title, entry.durationSecs ?? 0);
      cur = { startMin, endMin, totalSecs: entry.durationSecs ?? 0, appDurations };
    }
  }
  if (cur) sessions.push(finishSession(cur));

  const filtered = sessions.filter((s) => s.totalSecs >= MIN_SESSION_SECS);

  for (const entry of focusEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const endMin = startMin + (entry.durationSecs ?? 0) / 60;
    filtered.push({
      startMin,
      endMin: Math.max(endMin, startMin + 1),
      totalSecs: entry.durationSecs ?? 0,
      label: entry.title || "Focus",
      appCount: 1,
      type: "focus",
    });
  }

  return filtered;
}

function sessionOpacity(totalSecs: number): number {
  const mins = totalSecs / 60;
  if (mins >= 30) return 0.85;
  if (mins >= 15) return 0.75;
  if (mins >= 5) return 0.65;
  return 0.5;
}

export function WeekView() {
  const { date, setMode, setDate } = useDashboardState();
  const dateStr = date || todayISO();
  const today = todayISO();
  const { start, end, days } = useMemo(() => getWeekRange(dateStr), [dateStr]);

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(start, end, sourcesKey),
    queryFn: () => timelineQuery(start, end, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  const scrollRef = useRef<HTMLDivElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-scroll when week changes
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 8 * HOUR_HEIGHT;
    }
  }, [dateStr]);

  const { dayData, activeByDay } = useMemo(() => {
    const entryMap = new Map<string, TimelineEntry[]>();
    for (const day of days) entryMap.set(day, []);
    for (const entry of data.entries) {
      const day = toLocalISO(new Date(entry.startedAt));
      entryMap.get(day)?.push(entry);
    }
    const dMap = new Map<string, { activity: WeekSession[]; focus: WeekSession[] }>();
    const actMap = new Map<string, number>();
    for (const [day, dayEntries] of entryMap) {
      const sessions = buildWeekSessions(dayEntries);
      const activity = sessions.filter((s) => s.type === "activity");
      const focus = sessions.filter((s) => s.type === "focus");
      for (const a of activity) {
        a.hasFocus = focus.some((f) => f.startMin <= a.endMin && f.endMin >= a.startMin);
      }
      dMap.set(day, { activity, focus });
      actMap.set(day, computeDayStats(dayEntries).activeSecs);
    }
    return { dayData: dMap, activeByDay: actMap };
  }, [data.entries, days]);

  const goToDay = (day: string) => {
    setMode("day");
    setDate(day);
  };

  return (
    <div className="dashboard__week">
      <div className="dashboard__week-header" style={{ paddingLeft: HOUR_GUTTER }}>
        {days.map((day, i) => {
          const activeSecs = activeByDay.get(day) || 0;
          const isToday = day === today;
          const headerClass = [
            "dashboard__week-day-header",
            isToday ? "dashboard__week-day-header--today" : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <button type="button" key={day} onClick={() => goToDay(day)} className={headerClass}>
              <div className="dashboard__week-day-label">{DAY_LABELS[i]}</div>
              <div className="dashboard__week-day-num">
                {new Date(`${day}T00:00:00`).getDate()}
              </div>
              {activeSecs > 0 && (
                <div className="dashboard__week-day-active">{formatHumanDuration(activeSecs)}</div>
              )}
            </button>
          );
        })}
      </div>

      {isLoading && <div className="dashboard__week-loading">Loading...</div>}

      <div ref={scrollRef} className="dashboard__week-scroll">
        <div className="dashboard__week-grid" style={{ height: TOTAL_HEIGHT }}>
          {HOURS.map((h) => (
            <div
              key={h}
              className="dashboard__week-hour-row"
              style={{ top: h * HOUR_HEIGHT }}
            >
              <div
                className="dashboard__week-hour-label"
                style={{ width: HOUR_GUTTER }}
              >
                {formatHour(h)}
              </div>
              <div className="dashboard__week-hour-line" />
            </div>
          ))}

          <div className="dashboard__week-columns" style={{ left: HOUR_GUTTER }}>
            {days.map((day) => {
              const cell = dayData.get(day) || { activity: [], focus: [] };

              return (
                <div key={day} className="dashboard__week-day-column">
                  {cell.activity.map((session) => {
                    const top = session.startMin * PX_PER_MIN;
                    const height = Math.max(
                      (session.endMin - session.startMin) * PX_PER_MIN,
                      MIN_BLOCK_HEIGHT,
                    );
                    const leftOffset = session.hasFocus ? 5 : 2;
                    const durationLabel = formatHumanDuration(session.totalSecs);
                    const appSuffix = session.appCount > 1 ? ` +${session.appCount - 1}` : "";

                    return (
                      <button
                        type="button"
                        key={`s-${session.startMin}`}
                        onClick={() => goToDay(day)}
                        className="dashboard__week-session"
                        style={{
                          top,
                          height,
                          left: leftOffset,
                          right: 2,
                          opacity: sessionOpacity(session.totalSecs),
                        }}
                        title={`${session.label}${appSuffix} · ${durationLabel}`}
                      >
                        {height > 20 && (
                          <span className="dashboard__week-session-title">{session.label}</span>
                        )}
                        {height > 32 && (
                          <span className="dashboard__week-session-meta">{durationLabel}</span>
                        )}
                      </button>
                    );
                  })}

                  {cell.focus.map((session) => {
                    const top = session.startMin * PX_PER_MIN;
                    const height = Math.max(
                      (session.endMin - session.startMin) * PX_PER_MIN,
                      MIN_BLOCK_HEIGHT,
                    );
                    return (
                      <div
                        key={`f-${session.startMin}`}
                        className="dashboard__week-session--focus"
                        style={{ top, height }}
                      />
                    );
                  })}

                  {day === today && <WeekNowLine />}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

function WeekNowLine() {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const mins = now.getHours() * 60 + now.getMinutes();
  const top = mins * PX_PER_MIN;
  return <div className="dashboard__week-now-line" style={{ top }} />;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd desktop-ui && bun run test -- WeekView`
Expected: PASS (both test cases).

- [ ] **Step 5: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/WeekView.tsx desktop-ui/src/features/dashboard/components/views/WeekView.test.tsx
git commit -m "feat(dashboard): port WeekView with merged session bars"
```

---

## Task 7: Add CSS for `.dashboard__week-*`

**Files:**
- Modify: `desktop-ui/src/styles/dashboard.css` (append)

- [ ] **Step 1: Append the new CSS section**

Append to the end of `desktop-ui/src/styles/dashboard.css`:

```css

/* ── Week view ──────────────────────────────────────────────── */
.dashboard__week {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
  overflow: hidden;
}

.dashboard__week-header {
  display: flex;
  border-bottom: 1px solid var(--ds-border-subtle);
}

.dashboard__week-day-header {
  flex: 1;
  text-align: center;
  padding: 6px 4px;
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ds-text-subtle);
  font-size: var(--fs-xs);
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__week-day-header:hover {
  background: var(--surface-control);
}

.dashboard__week-day-header--today {
  color: var(--border-accent);
  font-weight: 600;
}

.dashboard__week-day-label {
  font-size: var(--fs-xs);
}

.dashboard__week-day-num {
  font-size: var(--fs-2xs);
  margin-top: 2px;
}

.dashboard__week-day-active {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 60%, transparent);
  margin-top: 2px;
}

.dashboard__week-loading {
  padding: 4px 12px;
  font-size: var(--fs-xs);
  color: var(--ds-text-subtle);
}

.dashboard__week-scroll {
  flex: 1;
  overflow-y: auto;
}

.dashboard__week-grid {
  position: relative;
}

.dashboard__week-hour-row {
  position: absolute;
  width: 100%;
  display: flex;
  align-items: flex-start;
}

.dashboard__week-hour-label {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  text-align: right;
  padding-right: 4px;
  user-select: none;
}

.dashboard__week-hour-line {
  flex: 1;
  border-top: 1px solid var(--ds-border-subtle);
}

.dashboard__week-columns {
  position: absolute;
  inset: 0;
  display: flex;
}

.dashboard__week-day-column {
  flex: 1;
  position: relative;
  border-right: 1px solid var(--ds-border-subtle);
}

.dashboard__week-day-column:last-child {
  border-right: none;
}

.dashboard__week-session {
  position: absolute;
  border: none;
  border-radius: 3px;
  padding: 0;
  cursor: pointer;
  background-color: var(--success, #2fa84f);
  overflow: hidden;
  transition: filter var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__week-session:hover {
  filter: brightness(1.25);
}

.dashboard__week-session-title {
  display: block;
  font-size: var(--fs-2xs);
  color: var(--text);
  font-weight: 500;
  padding: 2px 4px 0 4px;
  line-height: 1.1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dashboard__week-session-meta {
  display: block;
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--text) 60%, transparent);
  padding: 0 4px;
  line-height: 1.1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dashboard__week-session--focus {
  position: absolute;
  left: 0;
  width: 3px;
  border-radius: 2px;
  background-color: var(--border-accent);
  opacity: 0.9;
  pointer-events: none;
}

.dashboard__week-now-line {
  position: absolute;
  width: 100%;
  border-top: 1px solid var(--danger, #d04848);
  pointer-events: none;
  z-index: 10;
}
```

- [ ] **Step 2: Verify lint and tests still pass**

Run: `cd desktop-ui && bun run lint && bun run test -- WeekView`
Expected: clean + PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css
git commit -m "style(dashboard): add .dashboard__week-* styles"
```

---

## Task 8: Port `MonthView.tsx` (TDD)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/MonthView.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/MonthView.test.tsx`

**Why:** A 6×7 grid for the month containing the active date. Each cell shows day number + focus duration + a proportional active-time bar. Click → drop to Day view. Arrow-key navigation moves the keyboard-focused cell.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/dashboard/components/views/MonthView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const emptyTimeline: TimelineResponse = {
  entries: [],
  summary: {
    totalTrackedSecs: 0,
    focusSecs: 0,
    tasksCompleted: 0,
    tasksCreated: 0,
    notesTouched: 0,
    transactionsCount: 0,
    topApps: [],
    sourceBreakdown: [],
  },
};

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(emptyTimeline),
    taskUpdate: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
  };
});

import {
  DashboardStateContext,
  type DashboardState,
} from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { MonthView } from "./MonthView";

afterEach(() => cleanup());

function makeState(over: Partial<DashboardState> = {}): DashboardState {
  return {
    mode: "month",
    date: "2026-04-15",
    setMode: vi.fn(),
    setDate: vi.fn(),
    navigatePrev: vi.fn(),
    navigateNext: vi.fn(),
    navigateToday: vi.fn(),
    ...over,
  };
}

function wrap(node: ReactNode, state: DashboardState) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <DashboardStateContext.Provider value={state}>
        <LayerContext.Provider
          value={{
            enabled: new Set(["activity"]),
            enabledSources: ["productivity", "focus"],
            toggle: () => {},
            reset: () => {},
          }}
        >
          {node}
        </LayerContext.Provider>
      </DashboardStateContext.Provider>
    </QueryClientProvider>
  );
}

describe("MonthView", () => {
  it("renders 42 day-cell buttons (6 weeks × 7 days)", async () => {
    render(wrap(<MonthView />, makeState()));
    await waitFor(() => {
      const cells = document.querySelectorAll(".dashboard__month-cell");
      expect(cells).toHaveLength(42);
    });
  });

  it("clicking a cell calls setMode('day') and setDate(cellDate)", async () => {
    const setMode = vi.fn();
    const setDate = vi.fn();
    render(wrap(<MonthView />, makeState({ setMode, setDate })));

    await waitFor(() => {
      expect(document.querySelectorAll(".dashboard__month-cell").length).toBe(42);
    });

    // April 15, 2026 is a Wednesday — find the cell with day "15" that is in-month.
    const cells = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".dashboard__month-cell"),
    );
    const fifteenth = cells.find(
      (c) => c.textContent?.includes("15") && !c.className.includes("--other-month"),
    );
    expect(fifteenth).toBeTruthy();
    fireEvent.click(fifteenth!);

    expect(setMode).toHaveBeenCalledWith("day");
    expect(setDate).toHaveBeenCalledWith("2026-04-15");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd desktop-ui && bun run test -- MonthView`
Expected: FAIL with `Cannot find module './MonthView'`.

- [ ] **Step 3: Create the MonthView component**

Create `desktop-ui/src/features/dashboard/components/views/MonthView.tsx`:

```tsx
import { useCallback, useMemo, useState } from "react";
import type { TimelineEntry } from "@/bindings";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  todayISO,
  toLocalISO,
  TZ_OFFSET_MINS,
} from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";
import { computeDayStats, DAY_LABELS } from "../../lib/timeline-utils";

interface MonthRange {
  start: string;
  end: string;
  year: number;
  month: number;
}

function getMonthRange(dateStr: string): MonthRange {
  const d = new Date(`${dateStr}T00:00:00`);
  const year = d.getFullYear();
  const month = d.getMonth();
  const first = new Date(year, month, 1);
  const last = new Date(year, month + 1, 0);
  return { start: toLocalISO(first), end: toLocalISO(last), year, month };
}

interface DayCell {
  date: string;
  day: number;
  isCurrentMonth: boolean;
  entries: TimelineEntry[];
}

function buildCalendarGrid(
  year: number,
  month: number,
  entries: TimelineEntry[],
): DayCell[][] {
  const first = new Date(year, month, 1);
  const startDay = first.getDay() === 0 ? 6 : first.getDay() - 1;
  const daysInMonth = new Date(year, month + 1, 0).getDate();

  const byDate = new Map<string, TimelineEntry[]>();
  for (const entry of entries) {
    const d = toLocalISO(new Date(entry.startedAt));
    if (!byDate.has(d)) byDate.set(d, []);
    byDate.get(d)?.push(entry);
  }

  const cells: DayCell[] = [];

  const prevMonth = new Date(year, month, 0);
  for (let i = startDay - 1; i >= 0; i--) {
    const day = prevMonth.getDate() - i;
    const d = new Date(year, month - 1, day);
    const iso = toLocalISO(d);
    cells.push({ date: iso, day, isCurrentMonth: false, entries: byDate.get(iso) || [] });
  }

  for (let day = 1; day <= daysInMonth; day++) {
    const iso = toLocalISO(new Date(year, month, day));
    cells.push({ date: iso, day, isCurrentMonth: true, entries: byDate.get(iso) || [] });
  }

  const remaining = 42 - cells.length;
  for (let day = 1; day <= remaining; day++) {
    const d = new Date(year, month + 1, day);
    const iso = toLocalISO(d);
    cells.push({ date: iso, day, isCurrentMonth: false, entries: byDate.get(iso) || [] });
  }

  const weeks: DayCell[][] = [];
  for (let i = 0; i < cells.length; i += 7) {
    weeks.push(cells.slice(i, i + 7));
  }
  return weeks;
}

function activeRatio(activeSecs: number, maxActiveSecs: number): number {
  if (maxActiveSecs === 0) return 0;
  return Math.min(1, activeSecs / maxActiveSecs);
}

function focusIntensityBg(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "transparent";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "color-mix(in oklch, var(--border-accent) 25%, transparent)";
  if (ratio > 0.5) return "color-mix(in oklch, var(--border-accent) 18%, transparent)";
  if (ratio > 0.25) return "color-mix(in oklch, var(--border-accent) 10%, transparent)";
  return "color-mix(in oklch, var(--border-accent) 5%, transparent)";
}

export function MonthView() {
  const { date, setMode, setDate } = useDashboardState();
  const dateStr = date || todayISO();
  const today = todayISO();

  const { start, end, year, month } = useMemo(() => getMonthRange(dateStr), [dateStr]);

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(start, end, sourcesKey),
    queryFn: () => timelineQuery(start, end, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  const weeks = useMemo(
    () => buildCalendarGrid(year, month, data.entries),
    [year, month, data.entries],
  );

  const [focusedDate, setFocusedDate] = useState<string>(today);

  const goToDay = (cellDate: string) => {
    setMode("day");
    setDate(cellDate);
  };

  const handleGridKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      let delta = 0;
      if (e.key === "ArrowLeft") delta = -1;
      else if (e.key === "ArrowRight") delta = 1;
      else if (e.key === "ArrowUp") delta = -7;
      else if (e.key === "ArrowDown") delta = 7;
      else if (e.key === "Enter") {
        goToDay(focusedDate);
        e.preventDefault();
        return;
      } else return;

      e.preventDefault();
      const next = new Date(`${focusedDate}T00:00:00`);
      next.setDate(next.getDate() + delta);
      setFocusedDate(toLocalISO(next));
    },
    // biome-ignore lint/correctness/useExhaustiveDependencies: goToDay is a stable closure
    [focusedDate],
  );

  const { dayStats, maxActiveSecs, maxFocusSecs } = useMemo(() => {
    const statsMap = new Map<string, { activeSecs: number; focusSecs: number }>();
    let maxA = 0;
    let maxF = 0;
    for (const week of weeks) {
      for (const cell of week) {
        const stats = computeDayStats(cell.entries);
        statsMap.set(cell.date, stats);
        if (stats.activeSecs > maxA) maxA = stats.activeSecs;
        if (stats.focusSecs > maxF) maxF = stats.focusSecs;
      }
    }
    return { dayStats: statsMap, maxActiveSecs: maxA, maxFocusSecs: maxF };
  }, [weeks]);

  return (
    <div className="dashboard__month">
      {isLoading && <div className="dashboard__month-loading">Loading...</div>}

      <div className="dashboard__month-dow-header">
        {DAY_LABELS.map((label) => (
          <div key={label} className="dashboard__month-dow-cell">
            {label}
          </div>
        ))}
      </div>

      {/* biome-ignore lint/a11y/useSemanticElements: CSS grid layout requires div */}
      <div
        className="dashboard__month-grid"
        role="grid"
        aria-label="Month calendar"
        tabIndex={0}
        onKeyDown={handleGridKeyDown}
      >
        {weeks.map((week) => (
          <div key={week[0].date} className="dashboard__month-week">
            {week.map((cell) => {
              const stats = dayStats.get(cell.date) || { activeSecs: 0, focusSecs: 0 };
              const aRatio = activeRatio(stats.activeSecs, maxActiveSecs);
              const isToday = cell.date === today;
              const isFocused = cell.date === focusedDate && cell.date !== today;

              const cellClass = [
                "dashboard__month-cell",
                cell.isCurrentMonth ? "" : "dashboard__month-cell--other-month",
                isToday ? "dashboard__month-cell--today" : "",
                isFocused ? "dashboard__month-cell--focused" : "",
              ]
                .filter(Boolean)
                .join(" ");

              return (
                <button
                  type="button"
                  key={cell.date}
                  onClick={() => goToDay(cell.date)}
                  className={cellClass}
                  style={{ backgroundColor: focusIntensityBg(stats.focusSecs, maxFocusSecs) }}
                >
                  <div className="dashboard__month-cell-row">
                    <span className="dashboard__month-cell-day">{cell.day}</span>
                    {stats.focusSecs > 0 && (
                      <span className="dashboard__month-cell-focus">
                        {formatHumanDuration(stats.focusSecs)}
                      </span>
                    )}
                  </div>

                  {stats.activeSecs > 0 && (
                    <div className="dashboard__month-cell-bar-wrap">
                      <div className="dashboard__month-activity-bar">
                        <div
                          className="dashboard__month-activity-bar-fill"
                          style={{
                            width: `${Math.max(aRatio * 100, 8)}%`,
                            opacity: 0.7 + aRatio * 0.3,
                          }}
                        />
                      </div>
                      <span className="dashboard__month-cell-active">
                        {formatHumanDuration(stats.activeSecs)}
                      </span>
                    </div>
                  )}
                </button>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd desktop-ui && bun run test -- MonthView`
Expected: PASS (both test cases).

- [ ] **Step 5: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/MonthView.tsx desktop-ui/src/features/dashboard/components/views/MonthView.test.tsx
git commit -m "feat(dashboard): port MonthView with focus-intensity calendar grid"
```

---

## Task 9: Add CSS for `.dashboard__month-*`

**Files:**
- Modify: `desktop-ui/src/styles/dashboard.css` (append)

- [ ] **Step 1: Append the new CSS section**

Append to the end of `desktop-ui/src/styles/dashboard.css`:

```css

/* ── Month view ─────────────────────────────────────────────── */
.dashboard__month {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
  padding: 12px;
}

.dashboard__month-loading {
  font-size: var(--fs-xs);
  color: var(--ds-text-subtle);
  margin-bottom: 4px;
}

.dashboard__month-dow-header {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  margin-bottom: 4px;
}

.dashboard__month-dow-cell {
  text-align: center;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  font-weight: 500;
  padding: 4px 0;
}

.dashboard__month-grid {
  flex: 1;
  display: grid;
  grid-template-rows: repeat(6, 1fr);
  gap: 1px;
  outline: none;
}

.dashboard__month-grid:focus-visible {
  outline: 1px solid var(--border-accent);
  outline-offset: 2px;
  border-radius: 6px;
}

.dashboard__month-week {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
}

.dashboard__month-cell {
  border-radius: 8px;
  padding: 6px;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  text-align: left;
  min-height: 64px;
  background: transparent;
  border: 1px solid transparent;
  cursor: pointer;
  color: var(--text);
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__month-cell:hover {
  background: var(--surface-control);
}

.dashboard__month-cell--other-month {
  color: color-mix(in srgb, var(--ds-text-subtle) 40%, transparent);
}

.dashboard__month-cell--today {
  outline: 1px solid color-mix(in srgb, var(--border-accent) 50%, transparent);
  outline-offset: -1px;
}

.dashboard__month-cell--today .dashboard__month-cell-day {
  color: var(--border-accent);
}

.dashboard__month-cell--focused {
  outline: 1px solid color-mix(in srgb, var(--text) 30%, transparent);
  outline-offset: -1px;
}

.dashboard__month-cell-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.dashboard__month-cell-day {
  font-size: var(--fs-xs);
  font-weight: 500;
}

.dashboard__month-cell-focus {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 60%, transparent);
}

.dashboard__month-cell-bar-wrap {
  width: 100%;
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.dashboard__month-activity-bar {
  width: 100%;
  height: 3px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}

.dashboard__month-activity-bar-fill {
  height: 100%;
  border-radius: 999px;
  background-color: var(--success, #2fa84f);
}

.dashboard__month-cell-active {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 50%, transparent);
}
```

- [ ] **Step 2: Verify lint and tests still pass**

Run: `cd desktop-ui && bun run lint && bun run test -- MonthView`
Expected: clean + PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css
git commit -m "style(dashboard): add .dashboard__month-* styles"
```

---

## Task 10: Port `YearView.tsx` (TDD)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/YearView.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/YearView.test.tsx`

**Why:** GitHub-style focus heatmap rendered as 12 mini-month grids in a 3-column layout. Click a cell → drop to Day view.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/dashboard/components/views/YearView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const focusTimeline: TimelineResponse = {
  entries: [
    {
      id: "f-1",
      source: "focus",
      entryType: "focusSession",
      title: "Deep work",
      description: null,
      startedAt: "2026-03-10T09:00:00Z",
      endedAt: "2026-03-10T11:00:00Z",
      durationSecs: 7200,
      entityId: null,
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
    {
      id: "f-2",
      source: "focus",
      entryType: "focusSession",
      title: "Deep work",
      description: null,
      startedAt: "2026-08-22T14:00:00Z",
      endedAt: "2026-08-22T15:00:00Z",
      durationSecs: 3600,
      entityId: null,
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
  ],
  summary: {
    totalTrackedSecs: 10800,
    focusSecs: 10800,
    tasksCompleted: 0,
    tasksCreated: 0,
    notesTouched: 0,
    transactionsCount: 0,
    topApps: [],
    sourceBreakdown: [],
  },
};

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(focusTimeline),
    taskUpdate: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
  };
});

import {
  DashboardStateContext,
  type DashboardState,
} from "../../hooks/useDashboardState";
import { LayerContext } from "../../lib/layers";
import { YearView } from "./YearView";

afterEach(() => cleanup());

function makeState(over: Partial<DashboardState> = {}): DashboardState {
  return {
    mode: "year",
    date: "2026",
    setMode: vi.fn(),
    setDate: vi.fn(),
    navigatePrev: vi.fn(),
    navigateNext: vi.fn(),
    navigateToday: vi.fn(),
    ...over,
  };
}

function wrap(node: ReactNode, state: DashboardState) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <DashboardStateContext.Provider value={state}>
        <LayerContext.Provider
          value={{
            enabled: new Set(["activity"]),
            enabledSources: ["focus"],
            toggle: () => {},
            reset: () => {},
          }}
        >
          {node}
        </LayerContext.Provider>
      </DashboardStateContext.Provider>
    </QueryClientProvider>
  );
}

describe("YearView", () => {
  it("renders 12 month sub-grids", async () => {
    render(wrap(<YearView />, makeState()));
    await waitFor(() => {
      const months = document.querySelectorAll(".dashboard__year-month");
      expect(months).toHaveLength(12);
    });
    expect(screen.getByText("Jan")).toBeTruthy();
    expect(screen.getByText("Dec")).toBeTruthy();
  });

  it("days with focus entries get a tier-N modifier (N >= 1)", async () => {
    render(wrap(<YearView />, makeState()));
    await waitFor(() => {
      // The two days with focus minutes should each get tier-4 (only entries
      // present, so each is the maximum). Look for any tier-N class.
      const tinted = document.querySelectorAll('[class*="dashboard__year-cell--tier-"]');
      expect(tinted.length).toBeGreaterThanOrEqual(2);
    });
  });

  it("clicking a focus-tinted cell drops to day view", async () => {
    const setMode = vi.fn();
    const setDate = vi.fn();
    render(wrap(<YearView />, makeState({ setMode, setDate })));

    await waitFor(() => {
      const tinted = document.querySelector('[class*="dashboard__year-cell--tier-"]');
      expect(tinted).toBeTruthy();
    });
    const tinted = document.querySelector<HTMLButtonElement>(
      '[class*="dashboard__year-cell--tier-"]',
    )!;
    fireEvent.click(tinted);

    expect(setMode).toHaveBeenCalledWith("day");
    expect(setDate).toHaveBeenCalled();
    const calledWith = setDate.mock.calls[0][0];
    expect(calledWith).toMatch(/^2026-\d{2}-\d{2}$/);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd desktop-ui && bun run test -- YearView`
Expected: FAIL with `Cannot find module './YearView'`.

- [ ] **Step 3: Create the YearView component**

Create `desktop-ui/src/features/dashboard/components/views/YearView.tsx`:

```tsx
import { useMemo } from "react";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  SHORT_MONTHS,
  todayISO,
  toLocalISO,
  TZ_OFFSET_MINS,
} from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";

const DOW_LABELS = ["M", "", "W", "", "F", "", ""];

function buildMonthGrid(year: number, month: number): (string | null)[][] {
  const first = new Date(year, month, 1);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const startDay = first.getDay() === 0 ? 6 : first.getDay() - 1;

  const weeks: (string | null)[][] = [];
  let week: (string | null)[] = Array.from({ length: startDay }, () => null);

  for (let d = 1; d <= daysInMonth; d++) {
    week.push(toLocalISO(new Date(year, month, d)));
    if (week.length === 7) {
      weeks.push(week);
      week = [];
    }
  }
  if (week.length > 0) {
    while (week.length < 7) week.push(null);
    weeks.push(week);
  }
  return weeks;
}

function intensityTier(secs: number, maxSecs: number): number {
  if (secs === 0 || maxSecs === 0) return 0;
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return 4;
  if (ratio > 0.5) return 3;
  if (ratio > 0.25) return 2;
  return 1;
}

export function YearView() {
  const { date, setMode, setDate } = useDashboardState();
  const year = Number(date.slice(0, 4)) || new Date().getFullYear();
  const today = todayISO();

  const start = `${year}-01-01`;
  const end = `${year}-12-31`;

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(start, end, sourcesKey),
    queryFn: () => timelineQuery(start, end, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  const { dayMap, maxSecs } = useMemo(() => {
    const map = new Map<string, number>();
    for (const entry of data.entries) {
      if (entry.source !== "focus") continue;
      const day = toLocalISO(new Date(entry.startedAt));
      map.set(day, (map.get(day) || 0) + (entry.durationSecs ?? 0));
    }
    let max = 0;
    for (const v of map.values()) {
      if (v > max) max = v;
    }
    return { dayMap: map, maxSecs: max };
  }, [data.entries]);

  const goToDay = (day: string) => {
    setMode("day");
    setDate(day);
  };

  return (
    <div className="dashboard__year">
      {isLoading && <div className="dashboard__year-loading">Loading...</div>}

      <div className="dashboard__year-grid">
        {Array.from({ length: 12 }, (_, monthIdx) => {
          const weeks = buildMonthGrid(year, monthIdx);
          const monthName = SHORT_MONTHS[monthIdx];
          return (
            <div key={monthName} className="dashboard__year-month">
              <div className="dashboard__year-month-name">{monthName}</div>

              <div className="dashboard__year-dow-row">
                {DOW_LABELS.map((label, i) => (
                  <div
                    // biome-ignore lint/suspicious/noArrayIndexKey: static labels with duplicates
                    key={`${monthName}-label-${i}`}
                    className="dashboard__year-dow-label"
                  >
                    {label}
                  </div>
                ))}
              </div>

              {weeks.map((week, wi) => (
                <div
                  // biome-ignore lint/suspicious/noArrayIndexKey: week rows have no unique id
                  key={`${monthName}-w${wi}`}
                  className="dashboard__year-week"
                >
                  {week.map((day, di) => {
                    if (!day) {
                      return (
                        <div
                          // biome-ignore lint/suspicious/noArrayIndexKey: empty padding cells
                          key={`empty-${monthName}-${wi}-${di}`}
                          className="dashboard__year-cell--empty"
                        />
                      );
                    }
                    const secs = dayMap.get(day) || 0;
                    const tier = intensityTier(secs, maxSecs);
                    const isToday = day === today;
                    const cellClass = [
                      "dashboard__year-cell",
                      `dashboard__year-cell--tier-${tier}`,
                      isToday ? "dashboard__year-cell--today" : "",
                    ]
                      .filter(Boolean)
                      .join(" ");
                    return (
                      <button
                        type="button"
                        key={day}
                        onClick={() => goToDay(day)}
                        className={cellClass}
                        title={`${day}: ${formatHumanDuration(secs)}`}
                      />
                    );
                  })}
                </div>
              ))}
            </div>
          );
        })}
      </div>

      <div className="dashboard__year-legend">
        <span>Less focus</span>
        <div className="dashboard__year-legend-swatch dashboard__year-cell--tier-0" />
        <div className="dashboard__year-legend-swatch dashboard__year-cell--tier-1" />
        <div className="dashboard__year-legend-swatch dashboard__year-cell--tier-2" />
        <div className="dashboard__year-legend-swatch dashboard__year-cell--tier-3" />
        <div className="dashboard__year-legend-swatch dashboard__year-cell--tier-4" />
        <span>More focus</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd desktop-ui && bun run test -- YearView`
Expected: PASS (all three test cases).

- [ ] **Step 5: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/YearView.tsx desktop-ui/src/features/dashboard/components/views/YearView.test.tsx
git commit -m "feat(dashboard): port YearView focus heatmap"
```

---

## Task 11: Add CSS for `.dashboard__year-*`

**Files:**
- Modify: `desktop-ui/src/styles/dashboard.css` (append)

- [ ] **Step 1: Append the new CSS section**

Append to the end of `desktop-ui/src/styles/dashboard.css`:

```css

/* ── Year view ──────────────────────────────────────────────── */
.dashboard__year {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
  padding: 16px;
  overflow-y: auto;
}

.dashboard__year-loading {
  font-size: var(--fs-xs);
  color: var(--ds-text-subtle);
  margin-bottom: 8px;
}

.dashboard__year-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.dashboard__year-month {
  display: flex;
  flex-direction: column;
}

.dashboard__year-month-name {
  font-size: var(--fs-xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
  margin-bottom: 6px;
}

.dashboard__year-dow-row {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
  margin-bottom: 2px;
}

.dashboard__year-dow-label {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 50%, transparent);
  text-align: center;
}

.dashboard__year-week {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
}

.dashboard__year-cell {
  aspect-ratio: 1;
  border: none;
  border-radius: 2px;
  cursor: pointer;
  transition: filter var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__year-cell:hover {
  filter: brightness(1.2);
}

.dashboard__year-cell--empty {
  aspect-ratio: 1;
}

.dashboard__year-cell--tier-0 {
  background: var(--surface-control);
}

.dashboard__year-cell--tier-1 {
  background: color-mix(in srgb, var(--border-accent) 10%, transparent);
}

.dashboard__year-cell--tier-2 {
  background: color-mix(in srgb, var(--border-accent) 25%, transparent);
}

.dashboard__year-cell--tier-3 {
  background: color-mix(in srgb, var(--border-accent) 40%, transparent);
}

.dashboard__year-cell--tier-4 {
  background: color-mix(in srgb, var(--border-accent) 60%, transparent);
}

.dashboard__year-cell--today {
  outline: 1px solid color-mix(in srgb, var(--border-accent) 60%, transparent);
  outline-offset: -1px;
}

.dashboard__year-legend {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: center;
  margin-top: 16px;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
}

.dashboard__year-legend-swatch {
  width: 12px;
  height: 12px;
  border-radius: 2px;
}
```

- [ ] **Step 2: Verify lint and tests still pass**

Run: `cd desktop-ui && bun run lint && bun run test -- YearView`
Expected: clean + PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css
git commit -m "style(dashboard): add .dashboard__year-* styles"
```

---

## Task 12: Wire all three views into `Dashboard.tsx` and drop the Phase 1 placeholder

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/Dashboard.tsx`
- Modify: `desktop-ui/src/styles/dashboard.css` (delete `.dashboard__placeholder` rule)
- Modify: `desktop-ui/src/features/dashboard/components/Dashboard.test.tsx` (extend Phase 1 smoke test)

**Why:** The placeholder must go and the new views must be reachable.

- [ ] **Step 1: Read the current Dashboard.tsx switch block**

Run: `cat desktop-ui/src/features/dashboard/components/Dashboard.tsx`
Confirm the `switch (state.mode)` block has placeholder arms for `week`/`month`/`year`.

- [ ] **Step 2: Replace the switch block**

In `desktop-ui/src/features/dashboard/components/Dashboard.tsx`:

Change the imports section to add the three new view imports. Find:

```tsx
import { DayView } from "./views/DayView";
```

Replace with:

```tsx
import { DayView } from "./views/DayView";
import { MonthView } from "./views/MonthView";
import { WeekView } from "./views/WeekView";
import { YearView } from "./views/YearView";
```

Change the switch block. Find:

```tsx
  let view: React.ReactNode;
  switch (state.mode) {
    case "day":
      view = <DayView />;
      break;
    case "week":
    case "month":
    case "year":
      view = (
        <div className="dashboard__placeholder">
          {state.mode.charAt(0).toUpperCase() + state.mode.slice(1)} view — coming in next phase
        </div>
      );
      break;
  }
```

Replace with:

```tsx
  let view: React.ReactNode;
  switch (state.mode) {
    case "day":
      view = <DayView />;
      break;
    case "week":
      view = <WeekView />;
      break;
    case "month":
      view = <MonthView />;
      break;
    case "year":
      view = <YearView />;
      break;
  }
```

- [ ] **Step 3: Delete the `.dashboard__placeholder` rule from CSS**

In `desktop-ui/src/styles/dashboard.css`, find and delete the entire block:

```css
/* ── Phase 1 placeholder for non-day views ──────────────────── */
.dashboard__placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-md);
  color: var(--ds-text-subtle);
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
}
```

- [ ] **Step 4: Update Dashboard.test.tsx — replace the placeholder assertion**

The existing Phase 1 test `"renders placeholder for non-day modes"` will fail now that the placeholder is gone. Open `desktop-ui/src/features/dashboard/components/Dashboard.test.tsx`.

Replace the test at the bottom of the file:

```tsx
  it("renders placeholder for non-day modes", () => {
    render(wrap(<Dashboard />));
    const weekPill = screen.getByText("Week").closest("button") as HTMLButtonElement;
    fireEvent.click(weekPill);
    expect(screen.getByText(/Week view — coming in next phase/)).toBeTruthy();
  });
```

with:

```tsx
  it("switches to WeekView when the Week pill is clicked", () => {
    render(wrap(<Dashboard />));
    const weekPill = screen.getByText("Week").closest("button") as HTMLButtonElement;
    fireEvent.click(weekPill);
    // The placeholder is gone — Mon..Sun headers should render instead.
    expect(screen.getByText("Mon")).toBeTruthy();
    expect(screen.getByText("Sun")).toBeTruthy();
  });

  it("switches to MonthView when the Month pill is clicked", () => {
    render(wrap(<Dashboard />));
    const monthPill = screen.getByText("Month").closest("button") as HTMLButtonElement;
    fireEvent.click(monthPill);
    // Month grid renders 42 day cells.
    const cells = document.querySelectorAll(".dashboard__month-cell");
    expect(cells.length).toBe(42);
  });

  it("switches to YearView when the Year pill is clicked", () => {
    render(wrap(<Dashboard />));
    const yearPill = screen.getByText("Year").closest("button") as HTMLButtonElement;
    fireEvent.click(yearPill);
    // 12 month sub-grids in the year heatmap.
    const months = document.querySelectorAll(".dashboard__year-month");
    expect(months.length).toBe(12);
  });
```

Then check the existing `vi.mock("@/api/endpoints/dashboard", ...)` factory at the top of the file. It mocks `timelineQuery`. Add `productivityCalendarEvents: vi.fn().mockResolvedValue([])` inside that returned object so CalendarTrack (now reachable via DayView) doesn't throw on the unmocked call.

For example, if the existing block is:

```tsx
vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(EMPTY_TIMELINE_RESPONSE),
  };
});
```

Change to:

```tsx
vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(EMPTY_TIMELINE_RESPONSE),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
  };
});
```

Verify by running: `cd desktop-ui && bun run test -- Dashboard.test`
Expected: PASS for all five tests in the file.

- [ ] **Step 5: Run the full dashboard test set**

Run: `cd desktop-ui && bun run test -- dashboard`
Expected: all dashboard tests PASS (Phase 1 + Phase 2).

- [ ] **Step 6: Run typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/Dashboard.tsx desktop-ui/src/styles/dashboard.css desktop-ui/src/features/dashboard/components/Dashboard.test.tsx
git commit -m "feat(dashboard): wire WeekView/MonthView/YearView into Dashboard"
```

---

## Task 13: End-to-end gate

**Files:** none (verification + smoke).

- [ ] **Step 1: Run the full desktop-ui suite**

Run: `cd desktop-ui && bun run typecheck && bun run lint && bun run test`
Expected: all clean / PASS.

- [ ] **Step 2: Build to ensure production output is valid**

Run: `cd desktop-ui && bun run build`
Expected: build succeeds.

- [ ] **Step 3: Manual smoke test**

Start: `cargo tauri dev` (with `cd desktop-ui && bun run dev` in another terminal if needed).

Verify each behavior:

  1. From the launcher / sidebar, open **Calendar** — Day view renders today's grid (Phase 1 baseline still works).
  2. Click view-pill **Week** → 7-column hour grid renders. Mon..Sun headers with day numbers visible. Today's column header highlighted in accent color. Hour rows render every 48px. Auto-scrolled to ~8am.
  3. If you have data, see merged session bars in green; days with focus sessions show a thin accent stripe on the left edge.
  4. Click any day header → drops to Day view on that date.
  5. Click view-pill **Month** → 6×7 grid renders. Today's cell has the today outline; cells with focus minutes show a tinted background; cells with active time show a proportional bar at the bottom.
  6. Click anywhere in the grid background → no nav. Click a cell → drops to Day view on that date.
  7. Tab into the grid → arrow keys move the focused-cell outline; Enter opens that day.
  8. Click view-pill **Year** → 12 mini-month heatmaps render in a 3-column layout. Today's cell has the today outline. Days with focus minutes show a tier-1..4 tint. Legend visible under the grid.
  9. Click any tinted cell → drops to Day view on that date.
  10. Date arrows in topbar work in each mode (Week ±7 days, Month ±1 month, Year ±1 year).
  11. Switch Day → Year while sitting on `2026-04-30` → Year shows `2026`. Switch Year → Day → land on `2026-01-01`.
  12. From Day view, click a calendar event block → block gets the `--selected` ring (no side panel until Phase 3, but the click registers).
  13. Day view's existing task drag still works — pick a task block, drag to a new time → moves visually, persists after the network roundtrip.

Note: items 2–12 fail gracefully when the user has no calendar / focus / activity data (empty grids, no sessions). Verify the empty case at minimum if no live data is available.

- [ ] **Step 4: Confirm git status is clean (no stragglers)**

Run: `git status`
Expected: clean working tree (everything committed in Tasks 1–12).

---

## Self-review — coverage map

Spec requirement → implementing task:

- `productivityCalendarEvents` wrapper added → Task 1
- `qk.productivity.calendarEvents` query key added → Task 2
- Real `CalendarTrack.tsx` (replaces stub) → Task 3 (component) + Task 4 (CSS)
- `DayColumns` passes real props to CalendarTrack with selection state → Task 5
- `WeekView.tsx` (port from `WeekCalendarView.tsx`) → Task 6 (component) + Task 7 (CSS)
- `MonthView.tsx` (port from `MonthCalendarView.tsx`) → Task 8 (component) + Task 9 (CSS)
- `YearView.tsx` (port from `YearHeatmapView.tsx`) → Task 10 (component) + Task 11 (CSS)
- `Dashboard.tsx` switch arms render real views (drop placeholder) → Task 12
- CalendarTrack tests → Task 3
- WeekView tests → Task 6
- MonthView tests → Task 8
- YearView tests → Task 10
- Acceptance — typecheck/lint/test/build clean → Task 13
- Acceptance — manual smoke for all 4 modes + selection + date preservation → Task 13
- SummaryPanel deliberately not wired in Phase 2 views → enforced by omission in Tasks 6/8/10
- Date-preservation across mode switches → already implemented in Phase 1's `useDashboardState.setMode`; Task 13 step 11 verifies behavior

## Notes for the implementer

- **Helpers stay inline** in each view file. Do not extract `getWeekRange`, `buildWeekSessions`, `getMonthRange`, `buildCalendarGrid`, `buildMonthGrid`, `intensityTier`, `focusIntensityBg`, `activeRatio`, `sessionOpacity`, or `formatHour` to `lib/timeline-utils.ts` — they each have a single consumer and the parent spec mandates inline placement.
- **Token fallbacks.** `var(--success, #2fa84f)` and `var(--danger, #d04848)` use fallback color literals because those tokens may not yet be in `ds-tokens.css`. This matches Phase 1's pattern of `var(--timeline-todo, #4d99ff)` in `dashboard.css`. If the tokens already exist, the fallback is harmless.
- **`computeOverlapLayout` accepts `durationSecs`.** `CalendarEvent` has `endedAt` instead — Task 3 derives `durationSecs` from `endedAt - startedAt` before calling. Don't change `computeOverlapLayout`.
- **`useDashboardState` is unchanged.** Phase 1 already implements the date-preservation behavior across mode switches. If a test fails because of mode-switch dates, fix the test — not the hook.
- **Don't render `SummaryPanel` in any new view.** Phase 3 wires it via `SidebarContext` at the layout level. Backup view code that imports/renders SummaryPanel is intentionally not ported.
- **Don't wire calendar events into the Week view.** Backup Week view does not render `CalendarTrack` and Phase 2 matches that.
- **Each task ends in a green build.** If a step takes longer than expected, stop, verify the in-progress files compile, and commit a WIP marker before continuing.
