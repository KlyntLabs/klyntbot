# Desktop UI Dashboard Port — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Day-view calendar dashboard from `desktop-ui.bak/` into `desktop-ui/`, restyled to current UI conventions, hosted as a new `appView === "calendar"` mode inside `MainAppShell`.

**Architecture:** New feature `desktop-ui/src/features/dashboard/`. State held in a single `useDashboardState` hook (replaces `react-router`). Data fetched via `useTauriQuery` against existing Rust commands (no backend changes). Drag-to-reschedule wired through `useTauriMutation` with optimistic updates. Stub components for ActivityTrack/CalendarTrack/SummaryPanel/ActivityFeed (filled in later phases). Wired into the chat shell via the existing `appView` union.

**Tech Stack:** React 18, TanStack Query 5, Tauri 2, lucide-react, plain BEM-ish CSS with design tokens (`--fs-*`, `--surface-*`, `--ds-popover-*`), Vitest + @testing-library/react.

---

## Reference paths (memorize these)

- Spec: `docs/superpowers/specs/2026-04-30-desktop-ui-dashboard-port-design.md`
- Backup source root: `desktop-ui.bak/src/features/dashboard/`
- Target root: `desktop-ui/src/features/dashboard/`
- Endpoints: `desktop-ui/src/api/endpoints/`
- Query keys: `desktop-ui/src/lib/query/queryKeys.ts`
- Query hooks: `desktop-ui/src/lib/query/useTauriQuery.ts`, `useTauriMutation.ts`
- Generated bindings: `desktop-ui/src/bindings.ts` (do not hand-edit)
- Style entry: `desktop-ui/src/styles/index.css`
- Sidebar shell: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx`
- App shell wiring: `desktop-ui/src/features/app/components/MainApp.tsx` (line 327: `appView` union)
- Layout surfaces hook: `desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts`
- App layout type: `desktop-ui/src/features/app/components/AppLayout.tsx`
- Center-layout switch: `desktop-ui/src/features/layout/components/DesktopLayout.tsx` (`CenterMode` union, `pluginsNode` rendering)

## Backup ⇄ Current API differences (translation cheat sheet)

| Backup | Current |
|---|---|
| `import { useQuery } from "@shared/hooks/useQuery"` | `import { useTauriQuery } from "@/lib/query"` |
| `import { useMutation } from "@shared/hooks/useMutation"` | `import { useTauriMutation } from "@/lib/query"` |
| `useQuery("cmd", args, EMPTY_FALLBACK, invalidateConfig)` | `useTauriQuery({ queryKey, queryFn: () => fnWrapper(...), fallback })` |
| `useMutation("cmd", "params")` | `useTauriMutation({ mutationFn: fnWrapper, invalidates: [...], optimistic? })` |
| `invalidateQueries("timeline_")` | `queryClient.invalidateQueries({ queryKey: qk.dashboard.all() })` (or rely on `invalidates` config) |
| `useEvent("focus:state_changed", h)` | (Phase 3 only) `useEffect(() => { const u = subscribeFocusStateChanged(h); return () => u.then(fn => fn()); }, [])` |
| `cn(...)` from `@shared/lib/utils` | Inline conditional class-string composition (no `cn` in current UI). Use template strings: `` `dashboard__view-pill${active ? " dashboard__view-pill--active" : ""}` `` |
| `import { LONG_MONTHS, todayISO, ... } from "@shared/lib/dates"` | `import { ... } from "@/utils/dashboardDates"` (we create this file in Task 6) |
| `react-router` `useNavigate`/`useLocation`/`useParams` | `useDashboardState()` from `@/features/dashboard/hooks/useDashboardState` |
| Tailwind/shadcn classes | Plain BEM-ish CSS in `dashboard.css`. See spec §"Token map" |

## Type sources

All Phase 1 backend types come from `@/bindings`:
- `TimelineResponse`, `TimelineEntry`, `TimelineSource`, `TimelineSummary`
- `TaskUpdateParams`, `TaskResponse`

Define `EMPTY_TIMELINE_RESPONSE` locally in `dashboard.ts` (constant, not a type).

---

## Task 1: Create endpoint wrappers

**Files:**
- Create: `desktop-ui/src/api/endpoints/dashboard.ts`

**Why:** All dashboard data flows through typed wrappers. Mirrors the pattern in `endpoints/codingMemory.ts`.

- [ ] **Step 1: Create the file with the two Phase-1 wrappers and the `EMPTY_TIMELINE_RESPONSE` constant**

```ts
// desktop-ui/src/api/endpoints/dashboard.ts
import { commands } from "@/bindings";
import type {
  TaskResponse,
  TaskUpdateParams,
  TimelineResponse,
  TimelineSource,
} from "@/bindings";

export const EMPTY_TIMELINE_RESPONSE: TimelineResponse = {
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

export async function timelineQuery(
  startDate: string,
  endDate: string,
  sources: TimelineSource[] | null,
  includePointEvents: boolean | null,
  tzOffsetMins: number | null,
): Promise<TimelineResponse> {
  const r = await commands.timelineQuery(
    startDate,
    endDate,
    sources,
    includePointEvents,
    tzOffsetMins,
  );
  if (r.status !== "ok") throw new Error(r.error.message ?? "timeline query failed");
  return r.data;
}

export async function taskUpdate(params: TaskUpdateParams): Promise<TaskResponse> {
  const r = await commands.taskUpdate(params);
  if (r.status !== "ok") throw new Error(r.error.message ?? "task update failed");
  return r.data;
}

/**
 * Trigger a calendar sync. The backend command takes an `events` array; the
 * frontend sends an empty array to request a pull-mode sync.
 */
export async function calendarSyncEvents(): Promise<void> {
  // commands.calendarSyncEvents signature is auto-generated from bindings.ts;
  // pass an empty events array per the existing convention in CalendarSync.tsx.
  const r = await commands.calendarSyncEvents([] as never);
  if (r.status !== "ok") throw new Error(r.error.message ?? "calendar sync failed");
  return;
}
```

- [ ] **Step 2: Verify the file typechecks**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. If `commands.calendarSyncEvents` signature differs from the assumed `(events: Event[])`, open `desktop-ui/src/bindings.ts`, search for `calendarSyncEvents`, and adjust the call to match the generated signature. Common shape from the Rust side: `commands.calendarSyncEvents(events)` where `events` is an array — the empty-array sync trigger is established convention from the backup.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/api/endpoints/dashboard.ts
git commit -m "feat(dashboard): add typed endpoint wrappers for timeline + task update + calendar sync"
```

---

## Task 2: Add query keys

**Files:**
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`

- [ ] **Step 1: Read current queryKeys.test.ts to learn the test style**

Run: `cat desktop-ui/src/lib/query/tests/queryKeys.test.ts | head -40`

Note the assertion style — direct `expect(qk.tasks.today()).toEqual([...])` calls.

- [ ] **Step 2: Write failing tests for the new keys**

Append to `desktop-ui/src/lib/query/tests/queryKeys.test.ts`:

```ts
describe("dashboard keys", () => {
  it("timeline key normalizes source order", () => {
    const a = qk.dashboard.timeline("2026-04-30", "2026-04-30", ["task", "calendar"]);
    const b = qk.dashboard.timeline("2026-04-30", "2026-04-30", ["calendar", "task"]);
    expect(a).toEqual(b);
    expect(a).toEqual(["dashboard", "timeline", "2026-04-30", "2026-04-30", "calendar,task"]);
  });

  it("dashboard.all is the namespace root", () => {
    expect(qk.dashboard.all()).toEqual(["dashboard"]);
  });

  it("calendarSync.status is namespaced", () => {
    expect(qk.calendarSync.status()).toEqual(["calendarSync", "status"]);
  });
});
```

- [ ] **Step 3: Run the test, confirm it fails with "Cannot read properties of undefined (reading 'timeline')"**

Run: `cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts`
Expected: FAIL.

- [ ] **Step 4: Add the keys to `qk` in `queryKeys.ts`**

Add inside the `qk` object (after the `flashcards` block):

```ts
  dashboard: {
    all: () => ["dashboard"] as const,
    timeline: (startDate: string, endDate: string, sources: readonly string[]) =>
      ["dashboard", "timeline", startDate, endDate, [...sources].sort().join(",")] as const,
  },
  calendarSync: {
    all: () => ["calendarSync"] as const,
    status: () => ["calendarSync", "status"] as const,
  },
```

(Productivity keys are deferred to Phase 3 — don't add them now.)

- [ ] **Step 5: Run the test, confirm it passes**

Run: `cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts desktop-ui/src/lib/query/tests/queryKeys.test.ts
git commit -m "feat(dashboard): add dashboard + calendarSync query keys"
```

---

## Task 3: Port `timeline-utils.ts`

**Files:**
- Create: `desktop-ui/src/features/dashboard/lib/timeline-utils.ts`
- Create: `desktop-ui/src/features/dashboard/lib/timeline-utils.test.ts`

- [ ] **Step 1: Write failing tests**

```ts
// desktop-ui/src/features/dashboard/lib/timeline-utils.test.ts
import { describe, expect, it } from "vitest";
import { computeOverlapLayout, isActiveAppEntry, computeDayStats } from "./timeline-utils";
import type { TimelineEntry } from "@/bindings";

function entry(overrides: Partial<TimelineEntry>): TimelineEntry {
  return {
    id: "x",
    source: "productivity",
    entryType: "appUsage",
    title: "Code",
    description: null,
    startedAt: "2026-04-30T09:00:00Z",
    endedAt: null,
    durationSecs: 600,
    entityId: null,
    entityRoute: null,
    color: "#000",
    metadata: null,
    ...overrides,
  };
}

describe("computeOverlapLayout", () => {
  it("returns empty map for empty input", () => {
    expect(computeOverlapLayout([])).toEqual(new Map());
  });

  it("places non-overlapping items in column 0", () => {
    const items = [
      { id: "a", startedAt: "2026-04-30T09:00:00Z", durationSecs: 600 },
      { id: "b", startedAt: "2026-04-30T10:00:00Z", durationSecs: 600 },
    ];
    const layout = computeOverlapLayout(items);
    expect(layout.get("a")).toEqual({ colIndex: 0, totalCols: 1 });
    expect(layout.get("b")).toEqual({ colIndex: 0, totalCols: 1 });
  });

  it("places overlapping items in side-by-side columns", () => {
    const items = [
      { id: "a", startedAt: "2026-04-30T09:00:00Z", durationSecs: 1800 },
      { id: "b", startedAt: "2026-04-30T09:15:00Z", durationSecs: 1800 },
    ];
    const layout = computeOverlapLayout(items);
    expect(layout.get("a")?.totalCols).toBe(2);
    expect(layout.get("b")?.totalCols).toBe(2);
    expect(layout.get("a")?.colIndex).not.toBe(layout.get("b")?.colIndex);
  });
});

describe("isActiveAppEntry", () => {
  it("returns false for idle apps", () => {
    expect(isActiveAppEntry(entry({ title: "loginwindow" }))).toBe(false);
  });
  it("returns true for productive apps with duration", () => {
    expect(isActiveAppEntry(entry({ title: "VSCode" }))).toBe(true);
  });
});

describe("computeDayStats", () => {
  it("sums focus and active seconds", () => {
    const entries = [
      entry({ source: "focus", entryType: "focusSession", durationSecs: 1800 }),
      entry({ source: "productivity", title: "VSCode", durationSecs: 600 }),
      entry({ source: "productivity", title: "loginwindow", durationSecs: 9999 }),
    ];
    const stats = computeDayStats(entries);
    expect(stats.focusSecs).toBe(1800);
    expect(stats.activeSecs).toBe(600);
  });
});
```

- [ ] **Step 2: Run the tests, confirm they fail with "Cannot find module './timeline-utils'"**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/timeline-utils.test.ts`
Expected: FAIL.

- [ ] **Step 3: Create `timeline-utils.ts` (port from backup, swap import paths)**

Copy the content of `desktop-ui.bak/src/features/dashboard/lib/timeline-utils.ts` and apply only these import changes:

```ts
// desktop-ui/src/features/dashboard/lib/timeline-utils.ts
import { minutesSinceMidnight } from "@/utils/dashboardDates";
import type { TimelineEntry } from "@/bindings";

// ...rest is unchanged from backup...
```

**Important: do Task 6 before this task** so the `@/utils/dashboardDates` import resolves. The plan's "Task ordering" note (bottom) restates this. If you are reading top-to-bottom and haven't done Task 6 yet, jump there first.

- [ ] **Step 4: Run the tests, confirm all pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/timeline-utils.test.ts`
Expected: PASS (all four tests).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/lib/timeline-utils.ts desktop-ui/src/features/dashboard/lib/timeline-utils.test.ts
git commit -m "feat(dashboard): port timeline-utils with overlap layout"
```

---

## Task 4: Port `buildContainers.ts`

**Files:**
- Create: `desktop-ui/src/features/dashboard/lib/buildContainers.ts`
- Create: `desktop-ui/src/features/dashboard/lib/buildContainers.test.ts`

- [ ] **Step 1: Write failing test for the empty-input path**

```ts
// desktop-ui/src/features/dashboard/lib/buildContainers.test.ts
import { describe, expect, it } from "vitest";
import { buildContainers, focusColor } from "./buildContainers";

describe("buildContainers", () => {
  it("returns empty array for no entries", () => {
    expect(buildContainers([])).toEqual([]);
  });

  it("returns empty array when entries have no durations and no point events", () => {
    expect(
      buildContainers([
        {
          id: "a", source: "task", entryType: "taskTimeEntry", title: "x",
          description: null, startedAt: "2026-04-30T09:00:00Z", endedAt: null,
          durationSecs: null, entityId: null, entityRoute: null,
          color: "#000", metadata: null,
        },
      ]),
    ).toEqual([]);
  });
});

describe("focusColor", () => {
  it("returns high-quality color for score > 7", () => {
    expect(focusColor(8)).toContain("--timeline-focus-high");
  });
  it("returns low-quality color for score < 4", () => {
    expect(focusColor(2)).toContain("--timeline-focus-low");
  });
  it("returns neutral color for mid-range", () => {
    expect(focusColor(5)).toContain("--timeline-focus");
  });
});
```

- [ ] **Step 2: Run the tests, confirm failure**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/buildContainers.test.ts`
Expected: FAIL ("Cannot find module").

- [ ] **Step 3: Port `buildContainers.ts` from backup**

Copy `desktop-ui.bak/src/features/dashboard/lib/buildContainers.ts` to `desktop-ui/src/features/dashboard/lib/buildContainers.ts`. Apply only these import changes:

```ts
import { minutesSinceMidnight } from "@/utils/dashboardDates";
import type { TimelineEntry } from "@/bindings";
```

(Requires Task 6 to have been completed first — see the "Task ordering" note at the bottom of this plan.)

- [ ] **Step 4: Run the tests, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/buildContainers.test.ts`
Expected: PASS (all 5 tests).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/lib/buildContainers.ts desktop-ui/src/features/dashboard/lib/buildContainers.test.ts
git commit -m "feat(dashboard): port buildContainers algorithm"
```

---

## Task 5: Port `layers.ts` (with React contexts)

**Files:**
- Create: `desktop-ui/src/features/dashboard/lib/layers.ts`
- Create: `desktop-ui/src/features/dashboard/lib/layers.test.ts`

- [ ] **Step 1: Write failing test for the layer config and the source-flattening**

```ts
// desktop-ui/src/features/dashboard/lib/layers.test.ts
import { describe, expect, it } from "vitest";
import { LAYERS } from "./layers";

describe("LAYERS config", () => {
  it("has 6 layers in fixed order", () => {
    expect(LAYERS.map((l) => l.key)).toEqual([
      "activity",
      "calendar",
      "timeEntries",
      "tasks",
      "transactions",
      "notes",
    ]);
  });

  it("activity layer maps to productivity + focus sources", () => {
    const activity = LAYERS.find((l) => l.key === "activity");
    expect(activity?.sources).toEqual(["productivity", "focus"]);
  });

  it("timeEntries is off by default", () => {
    expect(LAYERS.find((l) => l.key === "timeEntries")?.defaultOn).toBe(false);
  });
});
```

- [ ] **Step 2: Run, confirm failure**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/layers.test.ts`
Expected: FAIL.

- [ ] **Step 3: Port `layers.ts` from backup**

Copy `desktop-ui.bak/src/features/dashboard/lib/layers.ts` verbatim to `desktop-ui/src/features/dashboard/lib/layers.ts` and apply only this import change at the top:

```ts
import type { TimelineSource } from "@/bindings";
```

Everything else (the 6 LAYERS entries, `STORAGE_KEY`, `useLayerToggle`, `LayerContext`, `useEnabledLayers`, `SidebarContext`, `useDataMode`, `DataModeContext`, `useDataModeValue`, `useSidebarOpen`, `useSidebarToggle`) is unchanged.

- [ ] **Step 4: Run tests, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/lib/layers.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/lib/layers.ts desktop-ui/src/features/dashboard/lib/layers.test.ts
git commit -m "feat(dashboard): port layers config + 3 React contexts"
```

---

## Task 6: Add date utilities

**Files:**
- Create: `desktop-ui/src/utils/dashboardDates.ts`
- Create: `desktop-ui/src/utils/dashboardDates.test.ts`

**Why:** The dashboard needs a dozen small date helpers. The existing `desktop-ui/src/features/tray/lib/dates.ts` has only two and lives in a feature folder — pulling it up isn't worth the cross-feature churn. Create a new dashboard-scoped utility file. Phase 2/3 may decide to consolidate.

- [ ] **Step 1: Write failing tests**

```ts
// desktop-ui/src/utils/dashboardDates.test.ts
import { describe, expect, it } from "vitest";
import {
  toLocalISO, todayISO, formatFullDate, formatMonthLabel, weekStartISO,
  shiftDate, shiftMonth, monthEndISO, minutesSinceMidnight, minutesToIso,
  formatHumanDuration, formatTime, LONG_MONTHS, SHORT_MONTHS,
} from "./dashboardDates";

describe("toLocalISO", () => {
  it("formats a Date as YYYY-MM-DD in local timezone", () => {
    expect(toLocalISO(new Date(2026, 3, 30))).toBe("2026-04-30");
  });
});

describe("todayISO", () => {
  it("returns a YYYY-MM-DD string for today", () => {
    expect(todayISO()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});

describe("formatFullDate", () => {
  it("formats as 'Weekday, Month D, YYYY'", () => {
    expect(formatFullDate("2026-04-30")).toBe("Thursday, April 30, 2026");
  });
});

describe("formatMonthLabel", () => {
  it("formats 'YYYY-MM' as 'Month YYYY'", () => {
    expect(formatMonthLabel("2026-04")).toBe("April 2026");
  });
});

describe("weekStartISO", () => {
  it("returns the Monday of the containing week", () => {
    // 2026-04-30 is a Thursday → Monday is 2026-04-27
    expect(weekStartISO("2026-04-30")).toBe("2026-04-27");
  });
  it("handles Sundays correctly (week starts Mon)", () => {
    expect(weekStartISO("2026-05-03")).toBe("2026-04-27");
  });
});

describe("shiftDate", () => {
  it("shifts forward by N days", () => {
    expect(shiftDate("2026-04-30", 3)).toBe("2026-05-03");
  });
  it("shifts backward by N days", () => {
    expect(shiftDate("2026-04-30", -2)).toBe("2026-04-28");
  });
});

describe("shiftMonth", () => {
  it("shifts forward across year boundary", () => {
    expect(shiftMonth("2026-12", 1)).toBe("2027-01");
  });
});

describe("monthEndISO", () => {
  it("returns the last day of a month", () => {
    expect(monthEndISO("2026-02")).toBe("2026-02-28");
  });
});

describe("minutesSinceMidnight", () => {
  it("returns minutes for an ISO timestamp", () => {
    expect(minutesSinceMidnight("2026-04-30T09:30:00")).toBe(570);
  });
});

describe("minutesToIso", () => {
  it("composes a UTC ISO from date+minutes", () => {
    expect(minutesToIso("2026-04-30", 570)).toBe("2026-04-30T09:30:00Z");
  });
  it("clamps out-of-range minutes", () => {
    expect(minutesToIso("2026-04-30", 9999)).toBe("2026-04-30T24:00:00Z");
  });
});

describe("formatHumanDuration", () => {
  it("formats hours and minutes", () => {
    expect(formatHumanDuration(3900)).toBe("1h 5m");
  });
  it("formats minutes only when under an hour", () => {
    expect(formatHumanDuration(600)).toBe("10m");
  });
});

describe("LONG_MONTHS / SHORT_MONTHS", () => {
  it("LONG_MONTHS has 12 entries starting with January", () => {
    expect(LONG_MONTHS.length).toBe(12);
    expect(LONG_MONTHS[0]).toBe("January");
  });
  it("SHORT_MONTHS has 12 entries starting with Jan", () => {
    expect(SHORT_MONTHS.length).toBe(12);
    expect(SHORT_MONTHS[0]).toBe("Jan");
  });
});

describe("formatTime", () => {
  it("returns HH:MM in 24h format", () => {
    expect(formatTime("2026-04-30T09:30:00Z")).toMatch(/^\d{2}:\d{2}$/);
  });
});
```

- [ ] **Step 2: Run, confirm failure**

Run: `cd desktop-ui && bun run test src/utils/dashboardDates.test.ts`
Expected: FAIL.

- [ ] **Step 3: Create `dashboardDates.ts`**

```ts
// desktop-ui/src/utils/dashboardDates.ts
export const SHORT_MONTHS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

export const LONG_MONTHS = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

const WEEKDAYS_LONG = [
  "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
];

export function toLocalISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function todayISO(): string {
  return toLocalISO(new Date());
}

export const TZ_OFFSET_MINS = new Date().getTimezoneOffset();

export function formatFullDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  return `${WEEKDAYS_LONG[d.getDay()]}, ${LONG_MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

export function formatMonthLabel(yearMonth: string): string {
  const [y, m] = yearMonth.split("-").map(Number);
  return `${LONG_MONTHS[m - 1]} ${y}`;
}

export function weekStartISO(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  const day = d.getDay();
  const diff = d.getDate() - day + (day === 0 ? -6 : 1);
  d.setDate(diff);
  return toLocalISO(d);
}

export function shiftDate(iso: string, days: number): string {
  const d = new Date(`${iso}T00:00:00`);
  d.setDate(d.getDate() + days);
  return toLocalISO(d);
}

export function shiftMonth(yearMonth: string, months: number): string {
  const [y, m] = yearMonth.split("-").map(Number);
  const d = new Date(y, m - 1 + months, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export function monthEndISO(yearMonth: string): string {
  const [y, m] = yearMonth.split("-").map(Number);
  const d = new Date(y, m, 0);
  return toLocalISO(d);
}

export function minutesSinceMidnight(isoStr: string): number {
  const d = new Date(isoStr);
  return d.getHours() * 60 + d.getMinutes();
}

export function minutesToIso(date: string, minutes: number): string {
  const clamped = Math.max(0, Math.min(1440, minutes));
  const h = Math.floor(clamped / 60);
  const m = Math.floor(clamped % 60);
  return `${date}T${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:00Z`;
}

export function formatHumanDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
```

- [ ] **Step 4: Run tests, confirm pass**

Run: `cd desktop-ui && bun run test src/utils/dashboardDates.test.ts`
Expected: PASS (all tests). The `formatFullDate` Thursday assertion depends on the calendar — `2026-04-30` is genuinely a Thursday; if your test env disagrees due to TZ weirdness, regenerate the assertion from `new Date("2026-04-30T00:00:00")`.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/utils/dashboardDates.ts desktop-ui/src/utils/dashboardDates.test.ts
git commit -m "feat(dashboard): add date utilities (toLocalISO, weekStartISO, minutesToIso, ...)"
```

---

## Task 7: Create `useDashboardState` hook + context

**Files:**
- Create: `desktop-ui/src/features/dashboard/hooks/useDashboardState.ts`
- Create: `desktop-ui/src/features/dashboard/hooks/useDashboardState.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
// desktop-ui/src/features/dashboard/hooks/useDashboardState.test.tsx
// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useDashboardStateImpl } from "./useDashboardState";

describe("useDashboardStateImpl", () => {
  it("defaults to day mode and today's date", () => {
    const { result } = renderHook(() => useDashboardStateImpl());
    expect(result.current.mode).toBe("day");
    expect(result.current.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("navigatePrev moves day back one day", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "day", date: "2026-04-30" }));
    act(() => result.current.navigatePrev());
    expect(result.current.date).toBe("2026-04-29");
  });

  it("navigateNext moves week forward seven days", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "week", date: "2026-04-27" }));
    act(() => result.current.navigateNext());
    expect(result.current.date).toBe("2026-05-04");
  });

  it("navigatePrev moves month back one month", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "month", date: "2026-04-30" }));
    act(() => result.current.navigatePrev());
    expect(result.current.date.slice(0, 7)).toBe("2026-03");
  });

  it("setMode('year') keeps the year portion of the date", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "day", date: "2026-04-30" }));
    act(() => result.current.setMode("year"));
    expect(result.current.mode).toBe("year");
    expect(result.current.date).toBe("2026");
  });

  it("navigateNext in year mode moves forward one year", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "year", date: "2026" }));
    act(() => result.current.navigateNext());
    expect(result.current.date).toBe("2027");
  });

  it("navigateToday returns to today's date and day mode", () => {
    const { result } = renderHook(() => useDashboardStateImpl({ mode: "year", date: "2020" }));
    act(() => result.current.navigateToday());
    expect(result.current.mode).toBe("day");
    expect(result.current.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
```

- [ ] **Step 2: Run, confirm failure**

Run: `cd desktop-ui && bun run test src/features/dashboard/hooks/useDashboardState.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Create the hook**

```ts
// desktop-ui/src/features/dashboard/hooks/useDashboardState.ts
import { createContext, useCallback, useContext, useState } from "react";
import { shiftDate, shiftMonth, todayISO, toLocalISO } from "@/utils/dashboardDates";

export type DashboardViewMode = "day" | "week" | "month" | "year";

export interface DashboardState {
  mode: DashboardViewMode;
  date: string; // YYYY-MM-DD for day/week, YYYY-MM-DD (week-Monday) for week, YYYY-MM-DD for month, YYYY for year
  setMode(m: DashboardViewMode): void;
  setDate(d: string): void;
  navigatePrev(): void;
  navigateNext(): void;
  navigateToday(): void;
}

interface InitArgs {
  mode?: DashboardViewMode;
  date?: string;
}

/**
 * Internal-only — the context-free hook. Used in tests and by `Dashboard.tsx`
 * (which then exposes the value via `DashboardStateContext`).
 */
export function useDashboardStateImpl(init?: InitArgs): DashboardState {
  const [mode, setModeRaw] = useState<DashboardViewMode>(init?.mode ?? "day");
  const [date, setDate] = useState<string>(init?.date ?? todayISO());

  const setMode = useCallback((next: DashboardViewMode) => {
    setModeRaw((prev) => {
      if (prev === next) return prev;
      // When entering year mode, collapse the date to the year; when leaving year mode, expand to Jan 1 of that year.
      setDate((d) => {
        if (next === "year" && prev !== "year") return d.slice(0, 4);
        if (next !== "year" && prev === "year") return `${d}-01-01`;
        return d;
      });
      return next;
    });
  }, []);

  const navigatePrev = useCallback(() => {
    setDate((d) => stepDate(mode, d, -1));
  }, [mode]);

  const navigateNext = useCallback(() => {
    setDate((d) => stepDate(mode, d, 1));
  }, [mode]);

  const navigateToday = useCallback(() => {
    setModeRaw("day");
    setDate(todayISO());
  }, []);

  return { mode, date, setMode, setDate, navigatePrev, navigateNext, navigateToday };
}

function stepDate(mode: DashboardViewMode, date: string, dir: 1 | -1): string {
  switch (mode) {
    case "day":
      return shiftDate(date, dir);
    case "week":
      return shiftDate(date, 7 * dir);
    case "month": {
      const [y, m] = date.split("-").map(Number);
      const d = new Date(y, m - 1 + dir, 1);
      return toLocalISO(d);
    }
    case "year": {
      const y = Number(date.slice(0, 4));
      return String(y + dir);
    }
  }
}

export const DashboardStateContext = createContext<DashboardState | null>(null);

export function useDashboardState(): DashboardState {
  const ctx = useContext(DashboardStateContext);
  if (!ctx) throw new Error("useDashboardState must be used inside <DashboardStateContext.Provider>");
  return ctx;
}
```

- [ ] **Step 4: Run tests, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/hooks/useDashboardState.test.tsx`
Expected: PASS (all 7 tests).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/hooks/useDashboardState.ts desktop-ui/src/features/dashboard/hooks/useDashboardState.test.tsx
git commit -m "feat(dashboard): add useDashboardState hook + context"
```

---

## Task 8: Port `useTimelineDrag` adapted to `useTauriMutation`

**Files:**
- Create: `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts`
- Create: `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.test.tsx`

- [ ] **Step 1: Write failing test (focused on the mutation call shape)**

```tsx
// desktop-ui/src/features/dashboard/hooks/useTimelineDrag.test.tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", () => ({
  taskUpdate: vi.fn(),
}));

import { taskUpdate } from "@/api/endpoints/dashboard";
import { useTimelineDrag } from "./useTimelineDrag";

const mockedTaskUpdate = vi.mocked(taskUpdate);

afterEach(() => {
  mockedTaskUpdate.mockReset();
});

function wrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useTimelineDrag", () => {
  it("calls taskUpdate with new scheduledStart/scheduledEnd after a move drag", async () => {
    mockedTaskUpdate.mockResolvedValue({} as never);
    const { result } = renderHook(() => useTimelineDrag("2026-04-30", 1), { wrapper: wrapper() });

    // Simulate startMove at 09:00 (540 min) ending at 09:30 (570)
    const fakeMouseEvent = {
      preventDefault: () => {},
      stopPropagation: () => {},
      clientY: 540,
      nativeEvent: { offsetY: 0 },
    } as unknown as React.MouseEvent;

    act(() => {
      result.current.startMove(fakeMouseEvent, "task-1", 540, 570);
    });

    // Drag down 60px → 60 minutes (pxPerMin = 1)
    act(() => {
      result.current.onMouseMove({ clientY: 600 } as MouseEvent);
    });

    await act(async () => {
      await result.current.onMouseUp();
    });

    expect(mockedTaskUpdate).toHaveBeenCalledTimes(1);
    const call = mockedTaskUpdate.mock.calls[0][0];
    expect(call.id).toBe("task-1");
    expect(call.scheduledStart).toBe("2026-04-30T10:00:00Z");
    expect(call.scheduledEnd).toBe("2026-04-30T10:30:00Z");
  });

  it("does not call taskUpdate when drag returns to origin", async () => {
    mockedTaskUpdate.mockResolvedValue({} as never);
    const { result } = renderHook(() => useTimelineDrag("2026-04-30", 1), { wrapper: wrapper() });

    act(() => {
      result.current.startMove(
        { preventDefault() {}, stopPropagation() {}, clientY: 540, nativeEvent: { offsetY: 0 } } as never,
        "t1", 540, 570,
      );
    });
    await act(async () => {
      await result.current.onMouseUp();
    });
    expect(mockedTaskUpdate).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run, confirm failure**

Run: `cd desktop-ui && bun run test src/features/dashboard/hooks/useTimelineDrag.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Port the hook with the new mutation API**

Create `desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts`. Start from the backup file, then apply these specific changes (everything else verbatim):

```ts
// desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts
import { useCallback, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { taskUpdate } from "@/api/endpoints/dashboard";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { minutesToIso } from "@/utils/dashboardDates";
import type { TaskUpdateParams } from "@/bindings";

const SNAP_MINUTES = 15;

interface DragState {
  taskId: string;
  mode: "move" | "resize" | "tray";
  originTopMin: number;
  originEndMin: number;
  startMouseY: number;
  offsetMin: number;
}

interface GhostPosition { topMin: number; endMin: number; }

function snapToGrid(minutes: number): number {
  return Math.round(minutes / SNAP_MINUTES) * SNAP_MINUTES;
}

function clampMinutes(min: number): number {
  return Math.max(0, Math.min(1440, min));
}

export function useTimelineDrag(date: string, pxPerMin: number) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const [ghost, setGhost] = useState<GhostPosition | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const ghostRef = useRef<GhostPosition | null>(null);
  const pxPerMinRef = useRef(pxPerMin);
  pxPerMinRef.current = pxPerMin;

  const queryClient = useQueryClient();
  const { mutate: updateTask } = useTauriMutation<unknown, TaskUpdateParams>({
    mutationFn: taskUpdate,
    invalidates: [qk.dashboard.all()],
  });

  const isDragging = drag !== null;

  const startMove = (e: React.MouseEvent, taskId: string, topMin: number, endMin: number) => {
    e.preventDefault();
    e.stopPropagation();
    const mouseMin = e.nativeEvent.offsetY / pxPerMinRef.current + topMin;
    const offsetMin = mouseMin - topMin;
    const state: DragState = {
      taskId, mode: "move",
      originTopMin: topMin, originEndMin: endMin,
      startMouseY: e.clientY, offsetMin,
    };
    dragRef.current = state;
    ghostRef.current = { topMin, endMin };
    setDrag(state);
    setGhost({ topMin, endMin });
  };

  const startResize = (e: React.MouseEvent, taskId: string, topMin: number, endMin: number) => {
    e.preventDefault();
    e.stopPropagation();
    const state: DragState = {
      taskId, mode: "resize",
      originTopMin: topMin, originEndMin: endMin,
      startMouseY: e.clientY, offsetMin: 0,
    };
    dragRef.current = state;
    ghostRef.current = { topMin, endMin };
    setDrag(state);
    setGhost({ topMin, endMin });
  };

  const startTrayDrag = (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => {
    e.preventDefault();
    e.stopPropagation();
    const duration = estimatedMinutes ?? 30;
    const state: DragState = {
      taskId, mode: "tray",
      originTopMin: -1, originEndMin: -1,
      startMouseY: e.clientY, offsetMin: 0,
    };
    dragRef.current = state;
    ghostRef.current = { topMin: 0, endMin: duration };
    setDrag(state);
    setGhost({ topMin: 0, endMin: duration });
  };

  const dateRef = useRef(date);
  dateRef.current = date;

  const onMouseMove = useCallback((e: MouseEvent) => {
    const d = dragRef.current;
    if (!d) return;

    const deltaY = e.clientY - d.startMouseY;
    const deltaMins = deltaY / pxPerMinRef.current;

    let newGhost: GhostPosition;
    if (d.mode === "move") {
      const newTop = snapToGrid(d.originTopMin + deltaMins);
      const duration = d.originEndMin - d.originTopMin;
      newGhost = { topMin: clampMinutes(newTop), endMin: clampMinutes(newTop + duration) };
    } else if (d.mode === "resize") {
      const newEnd = snapToGrid(d.originEndMin + deltaMins);
      newGhost = {
        topMin: d.originTopMin,
        endMin: clampMinutes(Math.max(d.originTopMin + SNAP_MINUTES, newEnd)),
      };
    } else {
      const duration = ghostRef.current ? ghostRef.current.endMin - ghostRef.current.topMin : 30;
      const rawTop = snapToGrid(d.originTopMin + deltaMins);
      newGhost = { topMin: clampMinutes(rawTop), endMin: clampMinutes(rawTop + duration) };
    }

    ghostRef.current = newGhost;
    setGhost(newGhost);
  }, []);

  const onMouseUp = useCallback(async () => {
    const d = dragRef.current;
    const g = ghostRef.current;
    if (!d || !g) {
      dragRef.current = null;
      ghostRef.current = null;
      setDrag(null);
      setGhost(null);
      return;
    }
    dragRef.current = null;
    ghostRef.current = null;
    setDrag(null);
    setGhost(null);

    if (d.mode !== "tray" && g.topMin === d.originTopMin && g.endMin === d.originEndMin) return;
    if (d.mode === "tray" && d.originTopMin === -1) return;

    await updateTask({
      id: d.taskId,
      title: null, description: null, priority: null, status: null,
      dueDate: null, projectId: null, areaId: null, tags: null,
      keyResultId: null, statusLabelId: null, position: null, groupId: null,
      taskType: null, energyLevel: null, estimatedMinutes: null,
      scheduledStart: minutesToIso(dateRef.current, g.topMin),
      scheduledEnd: minutesToIso(dateRef.current, g.endMin),
    });

    void queryClient.invalidateQueries({ queryKey: qk.dashboard.all() });
  }, [updateTask, queryClient]);

  return { drag, ghost, isDragging, startMove, startResize, startTrayDrag, onMouseMove, onMouseUp };
}
```

The diff vs. the backup is: (a) imports swapped, (b) `useMutation("task_update", "params")` → `useTauriMutation({ mutationFn, invalidates })`, (c) `updateTask({ id, scheduledStart, scheduledEnd })` payload expanded to a complete `TaskUpdateParams` (the type requires every field — the rest as `null`), (d) `invalidateQueries("timeline_")` → `queryClient.invalidateQueries({ queryKey: qk.dashboard.all() })`.

- [ ] **Step 4: Run tests, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/hooks/useTimelineDrag.test.tsx`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/hooks/useTimelineDrag.ts desktop-ui/src/features/dashboard/hooks/useTimelineDrag.test.tsx
git commit -m "feat(dashboard): port useTimelineDrag onto useTauriMutation"
```

---

## Task 9: Add empty `dashboard.css` and register it

**Files:**
- Create: `desktop-ui/src/styles/dashboard.css`
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Create the CSS file with the `.dashboard` root scaffold**

```css
/* desktop-ui/src/styles/dashboard.css */

/* ── Root + layout ───────────────────────────────────────────── */
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  flex: 1;
  height: 100%;
  overflow: hidden;
}

.dashboard__content {
  flex: 1;
  overflow: hidden;
  position: relative;
}

/* ── Topbar ──────────────────────────────────────────────────── */
.dashboard__topbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 8px 16px;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
}

.dashboard__topbar-date {
  font-size: var(--fs-base);
  font-weight: 500;
  color: var(--text);
  white-space: nowrap;
}

/* ── View switcher pill group ────────────────────────────────── */
.dashboard__view-switcher,
.dashboard__nav-pills {
  display: flex;
  align-items: center;
  background: var(--surface-control);
  border-radius: 999px;
  padding: 2px;
}

.dashboard__nav-pills {
  margin-left: auto;
}

.dashboard__view-pill {
  background: transparent;
  border: none;
  padding: 4px 14px;
  border-radius: 999px;
  font-size: var(--fs-xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
  cursor: pointer;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out),
    color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__view-pill:hover {
  color: var(--text);
}

.dashboard__view-pill--active {
  background: var(--surface-control-hover);
  color: var(--text);
}

/* ── Icon buttons (layers, sidebar toggle, nav arrows) ──────── */
.dashboard__icon-button {
  background: transparent;
  border: none;
  padding: 6px;
  border-radius: 999px;
  color: var(--ds-text-subtle);
  cursor: pointer;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out),
    color var(--ds-dur-fast) var(--ds-ease-out);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.dashboard__icon-button:hover {
  color: var(--text);
  background: var(--surface-control-hover);
}

.dashboard__icon-button--active {
  background: var(--surface-control-hover);
  color: var(--text);
}

.dashboard__icon-button svg {
  width: 16px;
  height: 16px;
}

/* ── Popovers (layers menu, mini calendar) ──────────────────── */
.dashboard__popover {
  position: fixed;
  z-index: var(--ds-layer-modal);
  background: var(--ds-popover-bg);
  border: 1px solid var(--ds-popover-border);
  box-shadow: var(--ds-popover-shadow);
  border-radius: 10px;
  padding: 6px;
  min-width: 180px;
}

.dashboard__popover-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  font-size: var(--fs-xs);
  color: var(--ds-text-subtle);
  cursor: pointer;
  border-radius: 8px;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__popover-item:hover {
  background: var(--surface-control);
}

.dashboard__popover-reset {
  width: 100%;
  text-align: left;
  margin-top: 4px;
  padding: 6px 10px;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  background: transparent;
  border: none;
  cursor: pointer;
  border-radius: 8px;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__popover-reset:hover {
  background: var(--surface-control);
  color: var(--text);
}

.dashboard__layer-swatch {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  display: inline-block;
}

/* ── Day grid ────────────────────────────────────────────────── */
.dashboard__day-grid {
  display: flex;
  height: 100%;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
  overflow: hidden;
}

.dashboard__day-gutter {
  width: 48px;
  flex-shrink: 0;
  border-right: 1px solid var(--ds-border-subtle);
}

.dashboard__day-hour-label {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  text-align: right;
  padding-right: 6px;
}

.dashboard__day-columns {
  flex: 1;
  display: flex;
  position: relative;
  overflow-y: auto;
}

.dashboard__day-column {
  flex: 1;
  position: relative;
  border-right: 1px solid var(--ds-border-subtle);
}

.dashboard__day-column:last-child {
  border-right: none;
}

.dashboard__hour-row {
  border-top: 1px solid var(--ds-border-subtle);
}

/* ── Task block (DraggableTaskBlock) ─────────────────────────── */
.dashboard__task-block {
  position: absolute;
  border-radius: 6px;
  padding: 2px 6px;
  font-size: var(--fs-2xs);
  line-height: 1.15;
  overflow: hidden;
  border-left: 2px solid var(--timeline-todo, #4d99ff);
  background: color-mix(in srgb, var(--timeline-todo, #4d99ff) 15%, transparent);
  cursor: grab;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__task-block:hover {
  background: color-mix(in srgb, var(--timeline-todo, #4d99ff) 25%, transparent);
}

.dashboard__task-block--dragging {
  opacity: 0.5;
  cursor: grabbing;
}

.dashboard__task-block--selected {
  outline: 1px solid var(--border-accent);
}

.dashboard__task-block-title {
  color: var(--ds-text-subtle);
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dashboard__task-block-status {
  color: var(--ds-text-subtle);
  font-size: var(--fs-2xs);
  text-transform: capitalize;
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dashboard__task-block-resize-handle {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 6px;
  cursor: ns-resize;
}

.dashboard__task-ghost {
  position: absolute;
  border: 2px solid var(--border-accent);
  background: color-mix(in srgb, var(--border-accent) 10%, transparent);
  border-radius: 6px;
  pointer-events: none;
  z-index: 10;
}

/* ── Due-today tray ──────────────────────────────────────────── */
.dashboard__due-today-tray {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 4px 6px;
  border-bottom: 1px solid var(--ds-border-subtle);
  background: color-mix(in srgb, var(--surface-card-muted) 50%, transparent);
}

.dashboard__due-today-chip {
  background: color-mix(in srgb, var(--timeline-todo, #4d99ff) 15%, transparent);
  color: var(--timeline-todo, #4d99ff);
  border: none;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: var(--fs-2xs);
  cursor: grab;
  max-width: 120px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__due-today-chip:hover {
  background: color-mix(in srgb, var(--timeline-todo, #4d99ff) 25%, transparent);
}

.dashboard__due-today-chip--selected {
  outline: 1px solid var(--border-accent);
}

/* ── Calendar sync button ────────────────────────────────────── */
.dashboard__calendar-sync {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: var(--fs-xs);
  color: var(--ds-text-subtle);
  background: transparent;
  border: none;
  cursor: pointer;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out),
    color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__calendar-sync:hover {
  color: var(--text);
  background: var(--surface-control);
}

.dashboard__calendar-sync:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dashboard__calendar-sync svg {
  width: 14px;
  height: 14px;
}

/* ── MiniCalendar ────────────────────────────────────────────── */
.dashboard__mini-calendar {
  width: 100%;
  min-width: 252px;
  border: none;
  padding: 0;
  margin: 0;
}

.dashboard__mini-shortcuts {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
  padding: 0 2px;
}

.dashboard__mini-shortcut {
  padding: 2px 8px;
  font-size: var(--fs-2xs);
  border-radius: 8px;
  background: var(--surface-control);
  color: var(--ds-text-subtle);
  border: none;
  cursor: pointer;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out),
    color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__mini-shortcut--active {
  background: var(--surface-control-hover);
  color: var(--border-accent);
}

.dashboard__mini-shortcut:hover {
  color: var(--text);
}

.dashboard__mini-month-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 2px;
  margin-bottom: 4px;
}

.dashboard__mini-month-label {
  font-size: var(--fs-xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
}

.dashboard__mini-weekdays,
.dashboard__mini-days {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
}

.dashboard__mini-weekdays {
  margin-bottom: 2px;
}

.dashboard__mini-days {
  gap: 2px;
}

.dashboard__mini-weekday {
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
}

.dashboard__mini-day {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--fs-2xs);
  font-weight: 500;
  border-radius: 8px;
  border: 1px solid transparent;
  background: var(--surface-card);
  color: var(--ds-text-subtle);
  cursor: pointer;
  transition: background-color var(--ds-dur-fast) var(--ds-ease-out),
    border-color var(--ds-dur-fast) var(--ds-ease-out);
}

.dashboard__mini-day:hover {
  background: var(--surface-control-hover);
}

.dashboard__mini-day--selected {
  background: var(--border-accent);
  color: var(--surface-card-strong);
  border-color: color-mix(in srgb, var(--border-accent) 40%, transparent);
}

.dashboard__mini-day--today {
  background: var(--surface-control-hover);
  color: var(--border-accent);
  border-color: color-mix(in srgb, var(--border-accent) 30%, transparent);
}

.dashboard__mini-day--other-month {
  color: color-mix(in srgb, var(--ds-text-subtle) 30%, transparent);
}

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

- [ ] **Step 2: Register the CSS file in `index.css`**

Open `desktop-ui/src/styles/index.css`. After the existing `@import "./sidebar-chat.css";` line, add:

```css
@import "./dashboard.css";
```

- [ ] **Step 3: Confirm imports apply**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css desktop-ui/src/styles/index.css
git commit -m "feat(dashboard): add dashboard.css with BEM-ish design tokens"
```

---

## Task 10: Port `MiniCalendar.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/MiniCalendar.tsx`

**Why:** Used by the topbar's date-picker popover. Port from `desktop-ui.bak/src/shared/components/MiniCalendar.tsx`. Class names switch from Tailwind/glass to `.dashboard__mini-*` (already in `dashboard.css`).

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/MiniCalendar.tsx
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { LONG_MONTHS, toLocalISO } from "@/utils/dashboardDates";

interface MiniCalendarProps {
  value: string | null;
  onSelect: (iso: string) => void;
  onClear?: () => void;
  showShortcuts?: boolean;
}

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

function nextWeekday(from: Date, weekday: number): Date {
  const current = from.getDay() || 7;
  const diff = weekday - current;
  return addDays(from, diff <= 0 ? diff + 7 : diff);
}

export function MiniCalendar({
  value,
  onSelect,
  onClear,
  showShortcuts = true,
}: MiniCalendarProps) {
  const [todayISO, setTodayISO] = useState(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return toLocalISO(d);
  });

  useEffect(() => {
    const now = new Date();
    const msUntilMidnight =
      new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime() - now.getTime();
    const id = setTimeout(() => {
      const d = new Date();
      d.setHours(0, 0, 0, 0);
      setTodayISO(toLocalISO(d));
    }, msUntilMidnight + 100);
    return () => clearTimeout(id);
  }, [todayISO]);

  const today = useMemo(() => new Date(`${todayISO}T00:00:00`), [todayISO]);

  const [viewYear, setViewYear] = useState(() => {
    const d = value ? new Date(`${value}T00:00:00`) : today;
    return d.getFullYear();
  });
  const [viewMonth, setViewMonth] = useState(() => {
    const d = value ? new Date(`${value}T00:00:00`) : today;
    return d.getMonth();
  });

  const prevMonth = () => {
    if (viewMonth === 0) {
      setViewYear((y) => y - 1);
      setViewMonth(11);
    } else setViewMonth((m) => m - 1);
  };

  const nextMonth = () => {
    if (viewMonth === 11) {
      setViewYear((y) => y + 1);
      setViewMonth(0);
    } else setViewMonth((m) => m + 1);
  };

  const cells = useMemo(() => {
    const first = new Date(viewYear, viewMonth, 1);
    const startOffset = (first.getDay() + 6) % 7;
    const gridStart = new Date(viewYear, viewMonth, 1 - startOffset);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(gridStart);
      d.setDate(gridStart.getDate() + i);
      return d;
    });
  }, [viewYear, viewMonth]);

  const shortcuts = useMemo(
    () => [
      { label: "Today", iso: todayISO },
      { label: "Tomorrow", iso: toLocalISO(addDays(today, 1)) },
      { label: "Next Mon", iso: toLocalISO(nextWeekday(today, 1)) },
    ],
    [today, todayISO],
  );

  return (
    <fieldset
      className="dashboard__mini-calendar"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      {showShortcuts && (
        <div className="dashboard__mini-shortcuts">
          {shortcuts.map((s) => (
            <button
              type="button"
              key={s.label}
              onClick={() => onSelect(s.iso)}
              className={`dashboard__mini-shortcut${value === s.iso ? " dashboard__mini-shortcut--active" : ""}`}
            >
              {s.label}
            </button>
          ))}
        </div>
      )}

      <div className="dashboard__mini-month-nav">
        <button
          type="button"
          onClick={prevMonth}
          aria-label="Previous month"
          className="dashboard__icon-button"
        >
          <ChevronLeft strokeWidth={1.5} />
        </button>
        <span className="dashboard__mini-month-label">
          {LONG_MONTHS[viewMonth]} {viewYear}
        </span>
        <button
          type="button"
          onClick={nextMonth}
          aria-label="Next month"
          className="dashboard__icon-button"
        >
          <ChevronRight strokeWidth={1.5} />
        </button>
      </div>

      <div className="dashboard__mini-weekdays">
        {WEEKDAYS.map((d) => (
          <div key={d} className="dashboard__mini-weekday">
            {d}
          </div>
        ))}
      </div>

      <div className="dashboard__mini-days">
        {cells.map((d) => {
          const iso = toLocalISO(d);
          const isCurrentMonth = d.getMonth() === viewMonth;
          const isToday = iso === todayISO;
          const isSelected = iso === value;
          const cls = ["dashboard__mini-day"];
          if (isSelected) cls.push("dashboard__mini-day--selected");
          else if (isToday) cls.push("dashboard__mini-day--today");
          else if (!isCurrentMonth) cls.push("dashboard__mini-day--other-month");

          return (
            <button
              type="button"
              key={iso}
              onClick={() => onSelect(iso)}
              aria-label={`${d.getDate()} ${LONG_MONTHS[d.getMonth()]} ${d.getFullYear()}`}
              className={cls.join(" ")}
            >
              {d.getDate()}
            </button>
          );
        })}
      </div>

      {onClear && (
        <button
          type="button"
          onClick={onClear}
          className="dashboard__popover-reset"
          style={{ color: "var(--text-error, #d97373)" }}
        >
          Clear date
        </button>
      )}
    </fieldset>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/MiniCalendar.tsx
git commit -m "feat(dashboard): port MiniCalendar to dashboard styles"
```

---

## Task 11: Port `CalendarSync.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/CalendarSync.tsx`

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/CalendarSync.tsx
import { Calendar, Loader2, RefreshCw } from "lucide-react";
import { useState } from "react";
import { calendarSyncEvents } from "@/api/endpoints/dashboard";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatTime } from "@/utils/dashboardDates";

export function CalendarSync() {
  const [lastSynced, setLastSynced] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { mutate, isLoading } = useTauriMutation<unknown, void>({
    mutationFn: () => calendarSyncEvents(),
    invalidates: [qk.dashboard.all(), qk.calendarSync.all()],
    onSuccess: () => {
      setError(null);
      setLastSynced(formatTime(new Date().toISOString()));
    },
    onError: (e) => {
      setError(e instanceof Error ? e.message : "Sync failed");
    },
  });

  const title = lastSynced
    ? `Last synced: ${lastSynced}`
    : error
      ? `Error: ${error}`
      : "Sync calendar events";

  return (
    <button
      type="button"
      onClick={() => void mutate()}
      disabled={isLoading}
      className="dashboard__calendar-sync"
      title={title}
    >
      {isLoading ? <Loader2 className="lc-spin" /> : <Calendar />}
      <span>Sync</span>
      {lastSynced && !isLoading && <RefreshCw />}
    </button>
  );
}
```

- [ ] **Step 2: Add a tiny CSS rule for the spinner**

Append to `desktop-ui/src/styles/dashboard.css`:

```css
.lc-spin {
  animation: dashboard-spin 1s linear infinite;
}

@keyframes dashboard-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/CalendarSync.tsx desktop-ui/src/styles/dashboard.css
git commit -m "feat(dashboard): port CalendarSync button"
```

---

## Task 12: Port `DraggableTaskBlock.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/DraggableTaskBlock.tsx`

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/views/DraggableTaskBlock.tsx
import type { TimelineEntry } from "@/bindings";
import { minutesSinceMidnight } from "@/utils/dashboardDates";
import type { OverlapLayout } from "../../lib/timeline-utils";

const MIN_BLOCK_HEIGHT = 14;

interface DraggableTaskBlockProps {
  entry: TimelineEntry;
  pxPerMin: number;
  selected: boolean;
  layout?: OverlapLayout;
  isDragging: boolean;
  ghostTopMin?: number;
  ghostEndMin?: number;
  onMouseDownMove: (e: React.MouseEvent) => void;
  onMouseDownResize: (e: React.MouseEvent) => void;
  onClick: () => void;
}

export function DraggableTaskBlock({
  entry,
  pxPerMin,
  selected,
  layout,
  isDragging,
  ghostTopMin,
  ghostEndMin,
  onMouseDownMove,
  onMouseDownResize,
  onClick,
}: DraggableTaskBlockProps) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const dur = entry.durationSecs ?? 0;
  const endMin = dur > 0 ? startMin + dur / 60 : startMin + 30;

  const displayTop = isDragging && ghostTopMin != null ? ghostTopMin : startMin;
  const displayEnd = isDragging && ghostEndMin != null ? ghostEndMin : endMin;

  const top = displayTop * pxPerMin;
  const height = Math.max((displayEnd - displayTop) * pxPerMin, MIN_BLOCK_HEIGHT);

  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const leftPct = totalCols > 1 ? `${(colIndex / totalCols) * 100}%` : undefined;
  const widthPct = totalCols > 1 ? `${(1 / totalCols) * 100}%` : undefined;

  const posStyle: React.CSSProperties = leftPct
    ? { top, left: leftPct, width: widthPct, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

  const status = (entry.metadata as { status?: string } | null)?.status;

  const blockClass = [
    "dashboard__task-block",
    isDragging ? "dashboard__task-block--dragging" : "",
    selected ? "dashboard__task-block--selected" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: drag handle — keyboard not applicable */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: drag handle for timeline scheduling */}
      <div
        className={blockClass}
        style={{ ...posStyle, height }}
        title={entry.title}
        onMouseDown={(e) => {
          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
          if (e.clientY > rect.bottom - 6) onMouseDownResize(e);
          else onMouseDownMove(e);
        }}
        onClick={(e) => {
          if (!isDragging) {
            e.stopPropagation();
            onClick();
          }
        }}
      >
        <span className="dashboard__task-block-title">{entry.title}</span>
        {status && height > 28 && (
          <span className="dashboard__task-block-status">{status}</span>
        )}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: resize handle */}
        <div
          className="dashboard__task-block-resize-handle"
          onMouseDown={(e) => {
            e.stopPropagation();
            onMouseDownResize(e);
          }}
        />
      </div>

      {isDragging && ghostTopMin != null && ghostEndMin != null && (
        <div
          className="dashboard__task-ghost"
          style={{
            top: ghostTopMin * pxPerMin,
            left: leftPct ?? 4,
            right: leftPct ? undefined : 4,
            width: widthPct,
            height: Math.max((ghostEndMin - ghostTopMin) * pxPerMin, MIN_BLOCK_HEIGHT),
          }}
        />
      )}
    </>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DraggableTaskBlock.tsx
git commit -m "feat(dashboard): port DraggableTaskBlock"
```

---

## Task 13: Port `DueTodayTray.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/DueTodayTray.tsx`

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/views/DueTodayTray.tsx
import type { TimelineEntry } from "@/bindings";

interface DueTodayTrayProps {
  entries: TimelineEntry[];
  onStartDrag: (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => void;
  onSelect: (entry: TimelineEntry) => void;
  selectedEntryId: string | null;
}

export function DueTodayTray({
  entries,
  onStartDrag,
  onSelect,
  selectedEntryId,
}: DueTodayTrayProps) {
  if (entries.length === 0) return null;

  return (
    <div className="dashboard__due-today-tray">
      {entries.map((entry) => {
        const meta = entry.metadata as Record<string, unknown> | null;
        const taskId = (meta?.taskId as string) ?? entry.entityId ?? entry.id;
        const estimatedMins = entry.durationSecs ? entry.durationSecs / 60 : 30;
        const isSelected = selectedEntryId === entry.id;
        const cls = `dashboard__due-today-chip${isSelected ? " dashboard__due-today-chip--selected" : ""}`;

        return (
          <button
            key={entry.id}
            type="button"
            className={cls}
            title={entry.title}
            onClick={() => onSelect(entry)}
            onMouseDown={(e) => {
              if (e.button === 0) onStartDrag(e, taskId, estimatedMins);
            }}
          >
            {entry.title}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DueTodayTray.tsx
git commit -m "feat(dashboard): port DueTodayTray"
```

---

## Task 14: Stub `ContextRibbon.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/ContextRibbon.tsx`

**Why:** The backup version reads `useContextTimeline(date)` from a `@features/work-contexts` module that doesn't exist in the current `desktop-ui/`. In Phase 1 we render nothing (the backup also returns `null` when there's no data), preserving the call site so the real implementation can drop in during a future phase.

- [ ] **Step 1: Create the stub**

```tsx
// desktop-ui/src/features/dashboard/components/views/ContextRibbon.tsx

interface Props {
  date: string;
}

/**
 * Phase 1 stub. Real implementation depends on a `useContextTimeline` hook
 * that ships with the work-contexts feature port.
 */
// biome-ignore lint/correctness/noUnusedFunctionParameters: stub — real impl uses date
export function ContextRibbon(_props: Props) {
  return null;
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/ContextRibbon.tsx
git commit -m "feat(dashboard): add ContextRibbon stub for Phase 1"
```

---

## Task 15: Stub Phase-2/3 visual dependencies

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx`
- Create: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx`
- Create: `desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx`

**Why:** The ported `DayColumns.tsx` (Task 16) imports these. Real implementations land in Phase 2 (`CalendarTrack`) and Phase 3 (the rest).

- [ ] **Step 1: Create all four stubs**

```tsx
// desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx
import type { TimelineEntry } from "@/bindings";

export interface SessionBlock {
  id: string;
  startMin: number;
  endMin: number;
}

interface Props {
  entries?: TimelineEntry[];
  pxPerMin?: number;
}

// biome-ignore lint/correctness/noUnusedFunctionParameters: Phase 3 stub
export function ActivityTrack(_props: Props) {
  return null;
}
```

```tsx
// desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx
import type { TimelineEntry } from "@/bindings";

interface Props {
  entries?: TimelineEntry[];
  pxPerMin?: number;
}

// biome-ignore lint/correctness/noUnusedFunctionParameters: Phase 2 stub
export function CalendarTrack(_props: Props) {
  return null;
}
```

```tsx
// desktop-ui/src/features/dashboard/components/SummaryPanel.tsx
interface Props {
  date?: string;
}

// biome-ignore lint/correctness/noUnusedFunctionParameters: Phase 3 stub
export function SummaryPanel(_props: Props) {
  return null;
}
```

```tsx
// desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx
interface Props {
  date?: string;
}

// biome-ignore lint/correctness/noUnusedFunctionParameters: Phase 3 stub
export function ActivityFeed(_props: Props) {
  return null;
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx desktop-ui/src/features/dashboard/components/views/CalendarTrack.tsx desktop-ui/src/features/dashboard/components/SummaryPanel.tsx desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx
git commit -m "feat(dashboard): add Phase 2/3 component stubs"
```

---

## Task 16: Port `DayColumns.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/DayColumns.tsx`

**Why:** This is the largest single file (~742L in backup). The strategy: copy the backup's `DayColumnsView.tsx` content verbatim, then apply a small set of mechanical replacements. The whole file's algorithm is unchanged.

- [ ] **Step 1: Copy the backup file as a starting point**

```bash
cp desktop-ui.bak/src/features/dashboard/components/DayColumnsView.tsx \
   desktop-ui/src/features/dashboard/components/views/DayColumns.tsx
```

- [ ] **Step 2: Apply import replacements**

Open `desktop-ui/src/features/dashboard/components/views/DayColumns.tsx` and replace the import block at the top of the file with:

```ts
import type {
  ActivityTimelineResponse,
  ProductivitySummary,
  TimelineEntry,
  TimelineSummary,
} from "@/bindings";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@/utils/dashboardDates";
import { useTimelineDrag } from "../../hooks/useTimelineDrag";
import { type LayerKey, useEnabledLayers, useSidebarOpen } from "../../lib/layers";
import { computeOverlapLayout } from "../../lib/timeline-utils";
import { ActivityTrack, type SessionBlock } from "./ActivityTrack";
import { CalendarTrack } from "./CalendarTrack";
import { ContextRibbon } from "./ContextRibbon";
import { DraggableTaskBlock } from "./DraggableTaskBlock";
import { DueTodayTray } from "./DueTodayTray";
import { ActivityFeed } from "../productivity/ActivityFeed";
import { SummaryPanel } from "../SummaryPanel";
```

- [ ] **Step 3: Replace the `cn(...)` helper usage**

The backup uses `cn(...)` from `@shared/lib/utils`. Search for `cn(` in this file. For each call site, replace with a template-string concatenation. Example:

```diff
- className={cn("absolute", isDragging && "opacity-50", selected && "ring-1")}
+ className={[
+   "absolute",
+   isDragging ? "opacity-50" : "",
+   selected ? "ring-1" : "",
+ ].filter(Boolean).join(" ")}
```

If a `cn(...)` call is small (≤2 conditional bits), inline it as a template literal:

```diff
- className={cn("dashboard__hour-row", isToday && "dashboard__hour-row--today")}
+ className={`dashboard__hour-row${isToday ? " dashboard__hour-row--today" : ""}`}
```

- [ ] **Step 4: Handle any `useQuery(...)` calls inside DayColumns**

Search for `useQuery(` in this file. For each call:

- **If the result only feeds a stubbed component** (`ActivityTrack`, `CalendarTrack`, `ActivityFeed`, `SummaryPanel`): delete the query block AND any downstream calculations that only feed those stubs. Leave the stub-component invocation in place (it accepts no required props in our stub).
- **If the result feeds task blocks, the gutter, hour rows, or anything visible in Phase 1**: convert it to `useTauriQuery`. Pattern:

  ```ts
  // Before:
  const { data: foo } = useQuery<FooType>("foo_command", args, EMPTY_FOO);
  // After:
  const { data: foo } = useTauriQuery<FooType>({
    queryKey: ["dashboard", "foo", JSON.stringify(args)],
    queryFn: () => invoke<FooType>("foo_command", args),
    fallback: EMPTY_FOO,
  });
  ```

  Use `import { invoke } from "@/api/client";` for the raw command call. Add a one-off `qk.dashboard.<name>` key to `queryKeys.ts` if the query will recur in Phase 2/3 — otherwise the inline array is fine for Phase 1.

**Save a list of every removed query in a code comment at the top of the file** so Phase 3 can re-add them quickly:

```ts
// Phase 1: removed queries pending Phase 3 wiring:
// - productivity_activity_sessions  → ActivityTrack
// - productivity_calendar_events    → CalendarTrack
// (add others as you find them)
```

- [ ] **Step 5: Replace any Tailwind class strings in the JSX**

Search for `className="...flex...` or `className=`...bg-` patterns. Map to BEM classes from `dashboard.css`:

| Tailwind pattern | Class to use |
|---|---|
| `flex flex-col h-full` | `dashboard__day-grid` (or compose with inline style if not exact) |
| `border-r border-border` | `dashboard__day-column` (already has border-right) |
| `text-xs text-muted-foreground` | inline `style={{fontSize: "var(--fs-xs)", color: "var(--ds-text-subtle)"}}` (or add a dedicated class to `dashboard.css`) |

For class strings you can't easily map, prefer adding a new BEM class to `dashboard.css` over inline styles. Goal: zero Tailwind tokens left in the file. To verify:

```bash
cd desktop-ui && grep -E '(text-(xs|sm|muted)|bg-(card|muted|accent)|border-border)' src/features/dashboard/components/views/DayColumns.tsx
```

Expected output: empty.

- [ ] **Step 6: Rename the exported component**

The backup exports `DayColumnsView`. Rename to `DayColumns`:

```diff
- export function DayColumnsView({...}: DayColumnsViewProps) {
+ export function DayColumns({...}: DayColumnsProps) {
```

(Also rename the props interface.)

- [ ] **Step 7: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. If any errors mention `cn`, `@shared/`, or types from `@shared/types`, fix the imports per the cheat sheet at the top of this plan.

- [ ] **Step 8: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DayColumns.tsx
git commit -m "feat(dashboard): port DayColumns timeline grid"
```

---

## Task 17: Port `DayView.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/DayView.tsx`

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/views/DayView.tsx
import { useEffect, useMemo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { todayISO, TZ_OFFSET_MINS } from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";
import { DayColumns } from "./DayColumns";

export function DayView() {
  const { date } = useDashboardState();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const queryClient = useQueryClient();
  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(dateStr, dateStr, sourcesKey),
    queryFn: () => timelineQuery(dateStr, dateStr, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  // Periodic poll for today — catches accumulated activity every 30s
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(() => {
      void queryClient.invalidateQueries({
        queryKey: qk.dashboard.timeline(dateStr, dateStr, sourcesKey),
      });
    }, 30_000);
    return () => clearInterval(id);
  }, [isToday, dateStr, sourcesKey, queryClient]);

  return (
    <DayColumns
      date={dateStr}
      entries={data.entries}
      summary={data.summary}
      isToday={isToday}
      loading={isLoading}
      productivitySummary={null}
    />
  );
}
```

(`productivitySummary={null}` is intentional — the productivity-summary queries are deferred to Phase 3.)

- [ ] **Step 2: Verify `DayColumnsProps` accepts these (look at the props interface in DayColumns.tsx)**

Run: `cd desktop-ui && bun run typecheck`

If the typecheck complains that `productivitySummary` doesn't accept `null`, edit the props interface in `DayColumns.tsx` to allow `null`:

```ts
productivitySummary: ProductivitySummary | null;
```

(It already does in the backup — should be a no-op.)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DayView.tsx
git commit -m "feat(dashboard): add DayView wrapping DayColumns with timelineQuery"
```

---

## Task 18: Create `DashboardTopbar.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx`

**Why:** Lifts the topbar logic out of the backup's `DashboardLayout.tsx` (lines 150–244 — date label, view-pill switcher, layers popover, calendar sync, prev/next/calendar nav-pills, sidebar toggle).

- [ ] **Step 1: Create the file**

```tsx
// desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx
import { Calendar, ChevronLeft, ChevronRight, Layers, PanelRight } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { formatFullDate, formatMonthLabel } from "@/utils/dashboardDates";
import { useDashboardState, type DashboardViewMode } from "../hooks/useDashboardState";
import { LAYERS, useLayerToggle, useSidebarToggle } from "../lib/layers";
import { CalendarSync } from "./CalendarSync";
import { MiniCalendar } from "./MiniCalendar";

const VIEWS: { key: DashboardViewMode; label: string }[] = [
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
];

function formatDateDisplay(mode: DashboardViewMode, date: string): string {
  if (mode === "year") return date;
  if (mode === "day") return formatFullDate(date);
  if (mode === "month") return formatMonthLabel(date.slice(0, 7));
  // Week mode: "Apr 27 – May 3, 2026"
  const d = new Date(`${date}T00:00:00`);
  const end = new Date(d);
  end.setDate(end.getDate() + 6);
  return `${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })} – ${end.toLocaleDateString(
    "en-US",
    { month: "short", day: "numeric", year: "numeric" },
  )}`;
}

interface PopoverPos { top: number; right: number; }

function useClickOutside(ref: React.RefObject<HTMLElement | null>, onOutside: () => void, active: boolean) {
  useEffect(() => {
    if (!active) return;
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onOutside();
    }
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [active, onOutside, ref]);
}

export function DashboardTopbar() {
  const { mode, date, setMode, setDate, navigatePrev, navigateNext } = useDashboardState();

  const { sidebarOpen, toggleSidebar } = useSidebarToggle();
  const { enabled, toggle, reset } = useLayerToggle();

  // Layers popover
  const [layersOpen, setLayersOpen] = useState(false);
  const layersTriggerRef = useRef<HTMLButtonElement | null>(null);
  const layersDropdownRef = useRef<HTMLDivElement | null>(null);
  const [layersPos, setLayersPos] = useState<PopoverPos>({ top: 0, right: 0 });
  useClickOutside(layersDropdownRef, () => setLayersOpen(false), layersOpen);

  const updateLayersPos = useCallback(() => {
    if (!layersTriggerRef.current) return;
    const rect = layersTriggerRef.current.getBoundingClientRect();
    setLayersPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!layersOpen) return;
    updateLayersPos();
    window.addEventListener("resize", updateLayersPos);
    return () => window.removeEventListener("resize", updateLayersPos);
  }, [layersOpen, updateLayersPos]);

  // Mini-calendar popover
  const [calOpen, setCalOpen] = useState(false);
  const calTriggerRef = useRef<HTMLButtonElement | null>(null);
  const calDropdownRef = useRef<HTMLDivElement | null>(null);
  const [calPos, setCalPos] = useState<PopoverPos>({ top: 0, right: 0 });
  useClickOutside(calDropdownRef, () => setCalOpen(false), calOpen);

  const updateCalPos = useCallback(() => {
    if (!calTriggerRef.current) return;
    const rect = calTriggerRef.current.getBoundingClientRect();
    setCalPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!calOpen) return;
    updateCalPos();
    window.addEventListener("resize", updateCalPos);
    return () => window.removeEventListener("resize", updateCalPos);
  }, [calOpen, updateCalPos]);

  const handleDateSelect = (iso: string) => {
    setDate(mode === "year" ? new Date(`${iso}T00:00:00`).getFullYear().toString() : iso);
    setCalOpen(false);
  };

  return (
    <div className="dashboard__topbar">
      <span className="dashboard__topbar-date">{formatDateDisplay(mode, date)}</span>

      {/* View-pill switcher */}
      <div className="dashboard__view-switcher">
        {VIEWS.map((v) => (
          <button
            key={v.key}
            type="button"
            onClick={() => setMode(v.key)}
            className={`dashboard__view-pill${mode === v.key ? " dashboard__view-pill--active" : ""}`}
          >
            {v.label}
          </button>
        ))}
      </div>

      {/* Layers toggle */}
      <button
        ref={layersTriggerRef}
        type="button"
        onClick={() => setLayersOpen((v) => !v)}
        aria-haspopup="dialog"
        aria-expanded={layersOpen}
        aria-label="Toggle layers"
        title="Toggle layers"
        className={`dashboard__icon-button${layersOpen ? " dashboard__icon-button--active" : ""}`}
      >
        <Layers />
      </button>

      <CalendarSync />

      {/* Prev / date-picker / next */}
      <div className="dashboard__nav-pills">
        <button
          type="button"
          onClick={navigatePrev}
          aria-label="Previous"
          className="dashboard__icon-button"
        >
          <ChevronLeft />
        </button>
        <button
          ref={calTriggerRef}
          type="button"
          onClick={() => setCalOpen((v) => !v)}
          aria-haspopup="dialog"
          aria-expanded={calOpen}
          aria-label="Pick date"
          title="Pick date"
          className={`dashboard__icon-button${calOpen ? " dashboard__icon-button--active" : ""}`}
        >
          <Calendar />
        </button>
        <button
          type="button"
          onClick={navigateNext}
          aria-label="Next"
          className="dashboard__icon-button"
        >
          <ChevronRight />
        </button>
      </div>

      {/* Sidebar toggle */}
      <button
        type="button"
        onClick={toggleSidebar}
        title={sidebarOpen ? "Hide summary" : "Show summary"}
        aria-label={sidebarOpen ? "Hide summary" : "Show summary"}
        className={`dashboard__icon-button${sidebarOpen ? " dashboard__icon-button--active" : ""}`}
      >
        <PanelRight />
      </button>

      {/* Layers popover */}
      {layersOpen &&
        createPortal(
          <div
            ref={layersDropdownRef}
            className="dashboard__popover"
            style={{ top: layersPos.top, right: layersPos.right }}
          >
            {LAYERS.map((layer) => (
              <label key={layer.key} className="dashboard__popover-item">
                <input
                  type="checkbox"
                  checked={enabled.has(layer.key)}
                  onChange={() => toggle(layer.key)}
                  style={{ accentColor: "var(--border-accent)", width: 12, height: 12 }}
                />
                <span
                  className="dashboard__layer-swatch"
                  style={{ backgroundColor: layer.color }}
                />
                {layer.label}
              </label>
            ))}
            <button type="button" onClick={reset} className="dashboard__popover-reset">
              Reset to defaults
            </button>
          </div>,
          document.body,
        )}

      {/* MiniCalendar popover */}
      {calOpen &&
        createPortal(
          <div
            ref={calDropdownRef}
            className="dashboard__popover"
            style={{ top: calPos.top, right: calPos.right, padding: 10 }}
          >
            <MiniCalendar
              value={mode === "year" ? null : date}
              onSelect={handleDateSelect}
              showShortcuts={false}
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck and lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx
git commit -m "feat(dashboard): add DashboardTopbar with view switcher + popovers"
```

---

## Task 19: Create `Dashboard.tsx` root + `index.ts`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/Dashboard.tsx`
- Create: `desktop-ui/src/features/dashboard/index.ts`

- [ ] **Step 1: Create the root component**

```tsx
// desktop-ui/src/features/dashboard/components/Dashboard.tsx
import { useDashboardStateImpl, DashboardStateContext } from "../hooks/useDashboardState";
import {
  DataModeContext,
  LayerContext,
  SidebarContext,
  useDataMode,
  useLayerToggle,
  useSidebarToggle,
} from "../lib/layers";
import { DashboardTopbar } from "./DashboardTopbar";
import { DayView } from "./views/DayView";

export function Dashboard() {
  const state = useDashboardStateImpl();
  const { enabled, enabledSources } = useLayerToggle();
  const { sidebarOpen } = useSidebarToggle();
  const { dataMode } = useDataMode();

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

  return (
    <DashboardStateContext.Provider value={state}>
      <DataModeContext.Provider value={dataMode}>
        <LayerContext.Provider value={{ enabled, enabledSources }}>
          <SidebarContext.Provider value={sidebarOpen}>
            <div className="dashboard">
              <DashboardTopbar />
              <div className="dashboard__content">{view}</div>
            </div>
          </SidebarContext.Provider>
        </LayerContext.Provider>
      </DataModeContext.Provider>
    </DashboardStateContext.Provider>
  );
}
```

- [ ] **Step 2: Create the public exports barrel**

```ts
// desktop-ui/src/features/dashboard/index.ts
export { Dashboard } from "./components/Dashboard";
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/Dashboard.tsx desktop-ui/src/features/dashboard/index.ts
git commit -m "feat(dashboard): add Dashboard root + public exports"
```

---

## Task 20: Add `--active` modifier to `sidebar-chat.css`

**Files:**
- Modify: `desktop-ui/src/styles/sidebar-chat.css`

- [ ] **Step 1: Find the existing nav-item rule**

Run: `grep -n "sidebar-chat__nav-item" desktop-ui/src/styles/sidebar-chat.css`
Note the line range of the existing `.sidebar-chat__nav-item` block.

- [ ] **Step 2: Add the modifier rule**

Append to `desktop-ui/src/styles/sidebar-chat.css` (or place near the existing nav-item rule):

```css
.sidebar-chat__nav-item--active {
  background: var(--surface-active);
  color: var(--ds-text-strong);
}
```

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/sidebar-chat.css
git commit -m "feat(dashboard): add active-nav-item modifier to sidebar-chat"
```

---

## Task 21: Modify `SidebarChatLayout.tsx` for active state + Calendar nav

**Files:**
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx`
- Create: `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx`

- [ ] **Step 1: Write failing test for active class + Calendar nav item**

```tsx
// desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx
// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SidebarChatLayout } from "./SidebarChatLayout";

afterEach(() => cleanup());

const baseProps = {
  onOpenSettings: vi.fn(),
  onNewChat: vi.fn(),
  onSelectPlugins: vi.fn(),
  onSelectCalendar: vi.fn(),
  threads: [],
  selectedSessionKey: null,
  onSelectThread: vi.fn(),
  activeNavId: null,
};

describe("SidebarChatLayout", () => {
  it("renders the Calendar nav item", () => {
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("Calendar")).toBeTruthy();
  });

  it("applies the --active modifier class to the matching nav item", () => {
    render(<SidebarChatLayout {...baseProps} activeNavId="calendar" />);
    const calendarBtn = screen.getByText("Calendar").closest("button");
    expect(calendarBtn?.className).toContain("sidebar-chat__nav-item--active");
  });

  it("does not apply --active when activeNavId is null", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const buttons = document.querySelectorAll(".sidebar-chat__nav-item");
    buttons.forEach((b) => {
      expect(b.className).not.toContain("sidebar-chat__nav-item--active");
    });
  });

  it("calls onSelectCalendar when Calendar nav item is clicked", () => {
    const onSelectCalendar = vi.fn();
    render(<SidebarChatLayout {...baseProps} onSelectCalendar={onSelectCalendar} />);
    (screen.getByText("Calendar").closest("button") as HTMLButtonElement).click();
    expect(onSelectCalendar).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run, confirm failure**

Run: `cd desktop-ui && bun run test src/features/app/components/SidebarChatLayout.test.tsx`
Expected: FAIL ("Calendar" not found / `onSelectCalendar` not a prop).

- [ ] **Step 3: Modify `SidebarChatLayout.tsx`**

Replace the existing file contents with:

```tsx
// desktop-ui/src/features/app/components/SidebarChatLayout.tsx
import { Calendar, Clock, FolderPlus, LayoutGrid, Search, Settings, SquarePen } from "lucide-react";
import { memo } from "react";
import type { ChatThread } from "@/features/chat/types";

type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectPlugins: () => void;
  onSelectCalendar: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
  activeNavId: string | null;
};

type NavItem = {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
};

export const SidebarChatLayout = memo(function SidebarChatLayout({
  onOpenSettings,
  onNewChat,
  onSelectPlugins,
  onSelectCalendar,
  threads,
  selectedSessionKey,
  onSelectThread,
  activeNavId,
}: SidebarChatLayoutProps) {
  const navItems: NavItem[] = [
    { id: "new-chat", label: "New chat", icon: <SquarePen aria-hidden />, onClick: onNewChat },
    { id: "search", label: "Search", icon: <Search aria-hidden /> },
    { id: "calendar", label: "Calendar", icon: <Calendar aria-hidden />, onClick: onSelectCalendar },
    { id: "plugins", label: "Plugins", icon: <LayoutGrid aria-hidden />, onClick: onSelectPlugins },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    { id: "project", label: "Project", icon: <FolderPlus aria-hidden /> },
  ];

  return (
    <aside className="sidebar-chat">
      <div className="sidebar-chat__drag-strip" />
      <div className="sidebar-chat__topbar" aria-hidden />

      <nav className="sidebar-chat__nav" aria-label="Primary">
        {navItems.map((item) => {
          const isActive = activeNavId === item.id;
          const cls = `sidebar-chat__nav-item${isActive ? " sidebar-chat__nav-item--active" : ""}`;
          return (
            <button key={item.id} type="button" className={cls} onClick={item.onClick}>
              <span className="sidebar-chat__nav-icon">{item.icon}</span>
              <span className="sidebar-chat__nav-label">{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-chat__chats">
        <div className="sidebar-chat__section-title">Chats</div>
        {threads.length === 0 ? (
          <div className="sidebar-chat__chats-empty">No chats</div>
        ) : (
          <ul className="sidebar-chat__thread-list">
            {threads.map((t) => (
              <li key={t.sessionKey}>
                <button
                  type="button"
                  className={
                    "sidebar-chat__thread-item" +
                    (t.sessionKey === selectedSessionKey
                      ? " sidebar-chat__thread-item--active"
                      : "")
                  }
                  onClick={() => onSelectThread(t.sessionKey)}
                  title={t.title}
                >
                  {t.title || "Untitled"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="sidebar-chat__spacer" />

      <div className="sidebar-chat__footer">
        <button type="button" className="sidebar-chat__settings" onClick={onOpenSettings}>
          <Settings aria-hidden />
          <span>Settings</span>
        </button>
        <button type="button" className="sidebar-chat__upgrade">
          Upgrade
        </button>
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
```

- [ ] **Step 4: Run tests, confirm pass**

Run: `cd desktop-ui && bun run test src/features/app/components/SidebarChatLayout.test.tsx`
Expected: PASS (all 4 tests).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/app/components/SidebarChatLayout.tsx desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx
git commit -m "feat(dashboard): add Calendar nav + activeNavId to SidebarChatLayout"
```

---

## Task 22: Extend `useMainAppLayoutSurfaces.ts` for the calendar view

**Files:**
- Modify: `desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts`

- [ ] **Step 1: Locate the `chatView` shape**

Run: `grep -n "chatView" desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts | head -10`

- [ ] **Step 2: Extend the `appView` union and props**

In `useMainAppLayoutSurfaces.ts`, find the `chatView:` block in the type definition (~line 227). Change:

```diff
   chatView: {
-    appView: "home" | "chat" | "plugins";
+    appView: "home" | "chat" | "plugins" | "calendar";
     selectedSessionKey: string | null;
     onNewChat: () => void;
     onSelectThread: (sessionKey: string) => void;
     onSelectPlugins: () => void;
+    onSelectCalendar: () => void;
+    activeNavId: string | null;
     chatThreads: import("@/features/chat/types").ChatThread[];
     refetchChatThreads: () => Promise<void>;
   };
```

- [ ] **Step 3: Pass through to `sidebarProps` in `buildPrimarySurface`**

Find `sidebarProps:` (~line 352). Add the new props:

```diff
     sidebarProps: {
       onOpenSettings: sidebarHandlers.onOpenSettings,
       onNewChat: chatView.onNewChat,
       onSelectPlugins: chatView.onSelectPlugins,
+      onSelectCalendar: chatView.onSelectCalendar,
+      activeNavId: chatView.activeNavId,
       threads: chatView.chatThreads,
       selectedSessionKey: chatView.selectedSessionKey,
       onSelectThread: chatView.onSelectThread,
     },
```

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: TypeScript will complain in `MainApp.tsx` about missing props — that's the next task. Other files should be clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useMainAppLayoutSurfaces.ts
git commit -m "feat(dashboard): thread onSelectCalendar + activeNavId through layout surfaces"
```

---

## Task 23: Extend `AppLayout.tsx` and `DesktopLayout.tsx` for `dashboardNode`

**Files:**
- Modify: `desktop-ui/src/features/app/components/AppLayout.tsx`
- Modify: `desktop-ui/src/features/layout/components/DesktopLayout.tsx`

- [ ] **Step 1: Add `dashboardNode` to `AppLayoutProps`**

In `desktop-ui/src/features/app/components/AppLayout.tsx`, after `pluginsNode?: ReactNode;` (line 19) add:

```diff
   pluginsNode?: ReactNode;
+  dashboardNode?: ReactNode;
```

Add to the destructured args (after `pluginsNode,` at line 47):

```diff
   pluginsNode,
+  dashboardNode,
```

Pass into `<DesktopLayout>` (after `pluginsNode={pluginsNode}` at line 67):

```diff
       pluginsNode={pluginsNode}
+      dashboardNode={dashboardNode}
```

- [ ] **Step 2: Add `dashboardNode` to `DesktopLayoutProps` and `CenterMode`**

In `desktop-ui/src/features/layout/components/DesktopLayout.tsx`:

Change `CenterMode` (line 5):
```diff
- type CenterMode = "chat" | "diff" | "plugins";
+ type CenterMode = "chat" | "diff" | "plugins" | "calendar";
```

Add prop to `DesktopLayoutProps` (after `pluginsNode?: ReactNode;` at line 63):
```diff
   pluginsNode?: ReactNode;
+  dashboardNode?: ReactNode;
```

Add to destructure (after `pluginsNode,` at line 91):
```diff
   pluginsNode,
+  dashboardNode,
```

Render after the existing pluginsNode line (after line 159 `{centerMode === "plugins" && pluginsNode}`):
```diff
       {centerMode === "plugins" && pluginsNode}
+      {centerMode === "calendar" && dashboardNode}
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: errors in `MainApp.tsx` only — we'll fix those in Task 24.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/app/components/AppLayout.tsx desktop-ui/src/features/layout/components/DesktopLayout.tsx
git commit -m "feat(dashboard): plumb dashboardNode + 'calendar' centerMode through layout"
```

---

## Task 24: Wire dashboard into `MainApp.tsx`

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`

- [ ] **Step 1: Add lazy import for `Dashboard`**

At the top of the file near the existing `PluginsView` import (line 1):

```diff
- import { PluginsView } from "@/features/plugins/components/PluginsView";
+ import { PluginsView } from "@/features/plugins/components/PluginsView";
+ import { Dashboard } from "@/features/dashboard";
```

- [ ] **Step 2: Extend `appView` union (line 327)**

```diff
- const [appView, setAppView] = useState<"home" | "chat" | "plugins">("home");
+ const [appView, setAppView] = useState<"home" | "chat" | "plugins" | "calendar">("home");
```

- [ ] **Step 3: Add the `onSelectCalendar` callback (after `onSelectPlugins`, ~line 338)**

```diff
   const onSelectPlugins = useCallback(() => {
     setAppView("plugins");
   }, []);

+  const onSelectCalendar = useCallback(() => {
+    setAppView("calendar");
+  }, []);
+
   const onSelectThread = useCallback((sessionKey: string) => {
```

- [ ] **Step 4: Compute `activeNavId` near where layout props are assembled (~line 1730)**

Find the block where `chatView` is constructed for the surfaces hook (search for `chatView:` or the assignment that includes `appView,`). Change:

```diff
       chatView: {
         appView,
         selectedSessionKey,
         onNewChat,
         onSelectThread,
         onSelectPlugins,
+        onSelectCalendar,
+        activeNavId:
+          appView === "calendar" ? "calendar" :
+          appView === "plugins" ? "plugins" :
+          null,
         chatThreads,
         refetchChatThreads,
       },
```

- [ ] **Step 5: Update `appLayout` to (a) extend the centerMode logic, (b) suppress home/chat for calendar, (c) pass `dashboardNode` (~line 1813–1837)**

```diff
     appLayout: {
-      showHome: showHome && appView !== "chat" && appView !== "plugins",
-      centerMode: appView === "chat" ? "chat" : appView === "plugins" ? "plugins" : centerMode,
+      showHome: showHome && appView !== "chat" && appView !== "plugins" && appView !== "calendar",
+      centerMode:
+        appView === "chat" ? "chat" :
+        appView === "plugins" ? "plugins" :
+        appView === "calendar" ? "calendar" :
+        centerMode,
       preloadGitDiffs: appSettings.preloadGitDiffs,
       splitChatDiffView: appSettings.splitChatDiffView,
       hasActivePlan: hasActivePlan,
-      activeWorkspace: (Boolean(activeWorkspace) || appView === "chat") && appView !== "plugins",
+      activeWorkspace:
+        (Boolean(activeWorkspace) || appView === "chat") &&
+        appView !== "plugins" &&
+        appView !== "calendar",
       sidebarNode,
       messagesNode: mainMessagesNode,
       composerNode,
       approvalToastsNode,
       updateToastNode,
       errorToastsNode,
       homeNode,
       pluginsNode: appView === "plugins" ? <PluginsView /> : null,
+      dashboardNode: appView === "calendar" ? <Dashboard /> : null,
       gitDiffPanelNode,
       gitDiffViewerNode,
       planPanelNode,
       debugPanelNode,
       terminalDockNode,
       onSidebarResizeStart,
       onChatDiffSplitPositionResizeStart,
       onRightPanelResizeStart,
       onPlanPanelResizeStart,
     },
```

- [ ] **Step 6: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 7: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "feat(dashboard): wire Dashboard into MainApp as appView=calendar"
```

---

## Task 25: Add `Dashboard.test.tsx` (smoke test)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/Dashboard.test.tsx`

- [ ] **Step 1: Write the test**

```tsx
// desktop-ui/src/features/dashboard/components/Dashboard.test.tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    timelineQuery: vi.fn().mockResolvedValue(actual.EMPTY_TIMELINE_RESPONSE),
    taskUpdate: vi.fn(),
    calendarSyncEvents: vi.fn(),
  };
});

import { Dashboard } from "./Dashboard";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("Dashboard", () => {
  it("renders the topbar with view-pill switcher and sync button", () => {
    render(wrap(<Dashboard />));
    expect(screen.getByText("Day")).toBeTruthy();
    expect(screen.getByText("Week")).toBeTruthy();
    expect(screen.getByText("Month")).toBeTruthy();
    expect(screen.getByText("Year")).toBeTruthy();
    expect(screen.getByText("Sync")).toBeTruthy();
  });

  it("active view-pill defaults to Day", () => {
    render(wrap(<Dashboard />));
    const dayPill = screen.getByText("Day").closest("button");
    expect(dayPill?.className).toContain("dashboard__view-pill--active");
  });

  it("renders placeholder for non-day modes", () => {
    render(wrap(<Dashboard />));
    const weekPill = screen.getByText("Week").closest("button") as HTMLButtonElement;
    fireEvent.click(weekPill);
    expect(screen.getByText(/Week view — coming in next phase/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/components/Dashboard.test.tsx`
Expected: PASS (all 3 tests). If the third test fails because the click doesn't update state synchronously, wrap in `act` from `@testing-library/react`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/Dashboard.test.tsx
git commit -m "test(dashboard): smoke test for Dashboard root"
```

---

## Task 26: Add `DayView.test.tsx`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/views/DayView.test.tsx`

- [ ] **Step 1: Write the test**

```tsx
// desktop-ui/src/features/dashboard/components/views/DayView.test.tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineResponse } from "@/bindings";

const mockTimeline: TimelineResponse = {
  entries: [
    {
      id: "task-1",
      source: "todo",
      entryType: "taskDue",
      title: "Write tests",
      description: null,
      startedAt: "2026-04-30T09:00:00Z",
      endedAt: null,
      durationSecs: 1800,
      entityId: "task-1",
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
    {
      id: "task-2",
      source: "todo",
      entryType: "taskDue",
      title: "Review PR",
      description: null,
      startedAt: "2026-04-30T11:00:00Z",
      endedAt: null,
      durationSecs: 900,
      entityId: "task-2",
      entityRoute: null,
      color: "#000",
      metadata: null,
    },
  ],
  summary: {
    totalTrackedSecs: 2700,
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
    timelineQuery: vi.fn().mockResolvedValue(mockTimeline),
    taskUpdate: vi.fn(),
  };
});

import { DashboardStateContext, useDashboardStateImpl } from "../../hooks/useDashboardState";
import { DayView } from "./DayView";

afterEach(() => cleanup());

function StateWrap({ children }: { children: ReactNode }) {
  const state = useDashboardStateImpl({ mode: "day", date: "2026-04-30" });
  return <DashboardStateContext.Provider value={state}>{children}</DashboardStateContext.Provider>;
}

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return (
    <QueryClientProvider client={client}>
      <StateWrap>{node}</StateWrap>
    </QueryClientProvider>
  );
}

describe("DayView", () => {
  it("renders both task blocks from the mocked timeline response", async () => {
    render(wrap(<DayView />));
    await waitFor(() => {
      expect(screen.getByText("Write tests")).toBeTruthy();
      expect(screen.getByText("Review PR")).toBeTruthy();
    });
  });
});
```

- [ ] **Step 2: Run, confirm pass**

Run: `cd desktop-ui && bun run test src/features/dashboard/components/views/DayView.test.tsx`
Expected: PASS.

If the test fails because `DayColumns` doesn't render task blocks for `entryType: "taskDue"` (it might filter on a different entryType), inspect the column-filter logic in `DayColumns.tsx` Task 16 and adjust the test fixture's `entryType` to one that the Tasks column accepts (e.g. `taskDue` per `COLUMNS[3].filter`).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DayView.test.tsx
git commit -m "test(dashboard): DayView renders task blocks from timelineQuery"
```

---

## Task 27: End-to-end gate

**Files:** none

- [ ] **Step 1: Run full typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: 0 errors.

- [ ] **Step 2: Run full lint**

Run: `cd desktop-ui && bun run lint`
Expected: 0 errors, 0 warnings.

- [ ] **Step 3: Run full test suite**

Run: `cd desktop-ui && bun run test`
Expected: All pass. New tests run alongside the existing suite.

- [ ] **Step 4: Run Rust workspace test (sanity — should be unchanged since no Rust touched)**

Run: `cargo nextest run --workspace`
Expected: All pass (no Phase 1 changes affect Rust).

- [ ] **Step 5: Manual smoke (start the desktop app)**

Terminal A: `cd desktop-ui && bun run dev`
Terminal B: `cargo tauri dev`

Step through the acceptance checklist from the spec (`docs/superpowers/specs/2026-04-30-desktop-ui-dashboard-port-design.md` §"Acceptance criteria — Phase 1"):

1. Click **Calendar** in the sidebar → dashboard renders today's day view.
2. Active highlight appears on the Calendar nav item; click Plugins, highlight moves.
3. Date arrows (`<` / `>`) shift the date label.
4. Click the date icon (calendar) → MiniCalendar popover opens; pick a date → day view rerenders with that date.
5. Click view pills (Day/Week/Month/Year) — Day shows the timeline grid; the others show "Coming in next phase" placeholder text.
6. Click a task block, drag it to a new time → block visually moves immediately (optimistic), stays moved after the network roundtrip.
7. Click Layers → toggle a layer → corresponding tracks hide/show.
8. Click the sidebar toggle (PanelRight icon) → toggles the sidebar context. (No panel renders yet — that's Phase 3.)

If any step fails, find the corresponding task above and fix; do **not** ship.

- [ ] **Step 6: Confirm `git status` clean**

Run: `git status`
Expected: nothing to commit.

- [ ] **Step 7: Tag the merge commit**

```bash
git log --oneline -1
```

If everything passes, the branch is mergeable. The PR description should include:
- Link to the spec
- Link to this plan
- Acceptance checklist with each item checked
- Note: "Phase 1 of 3 — Day view only; Week/Month/Year are placeholders. Productivity overlays + SummaryPanel deferred to Phase 3."

---

## Self-review — coverage map

Each spec requirement → task that implements it:

| Spec requirement | Task |
|---|---|
| `endpoints/dashboard.ts` with `timelineQuery`, `taskUpdate` (and `calendarSyncEvents` for the topbar button) | Task 1 |
| `qk.dashboard.*` and `qk.calendarSync.*` with sorted-source normalization | Task 2 |
| Pure helpers ported as-is: `timeline-utils`, `buildContainers`, `layers` | Tasks 3, 4, 5 |
| Date utilities (replaces `@shared/lib/dates`) | Task 6 |
| `useDashboardState` hook + `DashboardStateContext` | Task 7 |
| `useTimelineDrag` adapted to `useTauriMutation` with optimistic invalidation | Task 8 |
| `dashboard.css` with BEM-ish class system + `--fs-*` tokens, no Tailwind, single import | Task 9 |
| `MiniCalendar.tsx` ported and restyled | Task 10 |
| `CalendarSync.tsx` ported with new mutation API | Task 11 |
| `DraggableTaskBlock.tsx` ported and restyled | Task 12 |
| `DueTodayTray.tsx` ported and restyled | Task 13 |
| `ContextRibbon.tsx` (Phase 1 stub) | Task 14 |
| Phase 2/3 stubs (`ActivityTrack`, `CalendarTrack`, `SummaryPanel`, `ActivityFeed`) | Task 15 |
| `DayColumns.tsx` ported (~742L), Tailwind→BEM, `cn`→template strings, `useQuery`→`useTauriQuery` | Task 16 |
| `DayView.tsx` calling `timelineQuery` via `useTauriQuery`, with 30s poll for today | Task 17 |
| `DashboardTopbar.tsx` with view switcher, layers popover, mini-calendar popover, sidebar toggle | Task 18 |
| `Dashboard.tsx` root + `index.ts` barrel | Task 19 |
| `.sidebar-chat__nav-item--active` CSS rule | Task 20 |
| `SidebarChatLayout.tsx` accepts `activeNavId` + `onSelectCalendar`, adds Calendar nav | Task 21 |
| `useMainAppLayoutSurfaces.ts` extends `appView` union and threads new props | Task 22 |
| `AppLayout.tsx` + `DesktopLayout.tsx` accept `dashboardNode` and `centerMode === "calendar"` | Task 23 |
| `MainApp.tsx` line 327: `appView` extended; `onSelectCalendar` added; `dashboardNode` rendered; `activeNavId` computed | Task 24 |
| Tests: `Dashboard.test.tsx`, `DayView.test.tsx`, `useDashboardState.test.ts`, drag mutation test, `SidebarChatLayout.test.tsx` | Tasks 7, 8, 21, 25, 26 |
| Acceptance: typecheck, lint, test suite, manual smoke | Task 27 |

Spec items NOT implemented in Phase 1 (and where they land):
- `productivityWeekly` / `productivityGoals` / `productivityGoalDelete` / `productivityCalendarEvents` / `productivityAutoFocusConfirm` / `flashcardTotalDue` wrappers → Phase 3
- `qk.productivity.*` keys → Phase 3
- `subscribeFocusStateChanged` / `subscribeFocusAutoDetected` / `subscribeFocusAutoStarted` event hubs → Phase 3
- Real `ActivityTrack`, `CalendarTrack`, `SummaryPanel`, `ActivityFeed`, `ContextRibbon`, `FocusStateIndicator`, `AutoFocusToast`, `FocusTrayIndicator` → Phase 2 (`CalendarTrack`) / Phase 3 (rest)
- Week/Month/Year views → Phase 2

---

## Notes for the implementer

- **Task ordering matters once.** Do Task 6 (date utils) before Tasks 3–5 (pure helpers) so the `@/utils/dashboardDates` import resolves. Otherwise the order listed here is a topo sort — each task assumes the prior ones landed.
- **Commit per task.** Don't accumulate. The granularity is calibrated for cheap rollback.
- **`cn(...)` → template strings.** The current UI has no `cn` helper. Don't introduce one; use template strings or filter+join. Keep the diff focused.
- **`@shared/*` aliases don't exist in the current UI.** Every backup file imports from `@shared/...` — this is your most common error class. Replace with `@/...` paths per the cheat sheet.
- **Tests are co-located** in current convention (`Component.test.tsx` next to `Component.tsx`). Follow it.
- **Don't add `optimistic` to the drag mutation** unless you have an `applyTaskUpdate(prev, vars)` reducer ready. The spec's "optimistic" wording is a target; if it adds unbounded scope to Task 8 (rebuilding TimelineResponse with the moved task), defer it — `invalidates` alone gives a working drag with one network roundtrip of latency. Note the deferral in the PR description.
- **If `calendarSyncEvents` binding signature mismatches** the assumption in Task 1 (`commands.calendarSyncEvents([])`), the right fix is to read the actual signature from `desktop-ui/src/bindings.ts` and adapt the wrapper. Do **not** edit the Rust side just to match — adapt to what the binding gives you.
- **Pre-existing CLAUDE.md note about IPC is outdated.** Use `useTauriQuery` / `useTauriMutation` everywhere; that's the live convention. Don't follow the CLAUDE.md "no useQuery wrapper" guidance — it was written before `src/lib/query/` existed.
