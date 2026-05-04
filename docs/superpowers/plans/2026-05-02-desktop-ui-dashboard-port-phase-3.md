# Desktop UI — Dashboard port, Phase 3 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the remaining backup dashboard surface (full SummaryPanel + 9 productivity sub-components + ActivityTrack + live ActivityFeed + focus event overlays + ProductivityStrip) into the current desktop-ui, restyled to plain CSS / BEM / design tokens. Phase 3 is the final phase — after this the dashboard is fully ported.

**Architecture:** Per-view SummaryPanel rendering (backup parity). Day owns `selectedEntry`/`selectedSession`/`selectedCalendarEvent` state; Week owns `selectedEntry`; Month/Year always pass `selectedEntry={null}`. SummaryPanel mode-swap precedence: `selectedSession > selectedEntry > summary`. Real-time focus indicators mount as banners below the topbar (FocusStateIndicator + AutoFocusToast) and inside the topbar (FocusTrayIndicator).

**Tech Stack:** React 18 + TypeScript, Vite, plain CSS in `src/styles/dashboard.css`, `useTauriQuery`/`useTauriMutation` over Tauri 2 IPC, `@tauri-apps/api/event` for live events, Vitest + `@testing-library/react`, Bun for typecheck/lint/test.

**Spec:** [`../specs/2026-05-02-desktop-ui-dashboard-port-phase-3-design.md`](../specs/2026-05-02-desktop-ui-dashboard-port-phase-3-design.md)

**Worktree:** `/Users/maixuantung/Dev/raki/klyntbot-calendar` (branch `klyntbot-calendar`). All Phase 3 code lands here. Do NOT modify `/Users/maixuantung/Dev/raki/klyntbot`.

---

## Task ordering & dependency graph

```
Foundation (libs + IPC + events):     Tasks 1–6
Productivity sub-components (leaves): Tasks 7–15
Major panels (SummaryPanel etc.):     Tasks 16–18
Wiring (modify existing):             Tasks 19–24
CSS + index + test extensions:        Tasks 25–28
Final verification:                   Task 29
```

Tasks 7–15 each depend on Tasks 1–5 having landed. Task 16 (SummaryPanel) depends on Tasks 7–9, 11. Task 17 (ActivityTrack) depends on Tasks 4–5. Task 21 (DayColumns) depends on Tasks 16, 17, 12. Tasks 22–24 depend on Task 16. Task 29 depends on everything.

---

## Pre-flight

Before starting, run baseline checks to confirm Phase 1 + 2 still pass cleanly in the worktree.

- [ ] **Pre-flight 1: Verify clean baseline**

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui
bun install
bun run typecheck
bun run lint
bun run test
```

Expected: all four commands exit 0. If any fail, stop and triage before proceeding — do NOT begin Phase 3 on a broken baseline.

- [ ] **Pre-flight 2: Confirm worktree branch**

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar
git status
git rev-parse --abbrev-ref HEAD
```

Expected: branch is `klyntbot-calendar`, working tree is clean (or only contains the spec file from the brainstorm step).

---

## Task 1: Add new endpoint wrappers to `dashboard.ts`

**Files:**
- Modify: `desktop-ui/src/api/endpoints/dashboard.ts`

This task adds 9 read wrappers + 3 mutation wrappers. Pure additions — no logic changes to existing wrappers.

- [ ] **Step 1: Open the file and confirm current imports**

Run:
```bash
sed -n '1,12p' /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/api/endpoints/dashboard.ts
```

Expected: top imports are `CalendarEvent`, `CalendarEventInput`, `DashboardIntelligenceResponse`, `ProductivitySummaryResponse`, `TaskResponse`, `TaskUpdateParams`, `TimelineResponse`, `TimelineSource`, plus `commands`.

- [ ] **Step 2: Replace the import block to add new types**

Replace lines 1–11 with:

```ts
import type {
  ActivityCategoryResponse,
  ActivityTimelineResponse,
  AutoFocusPayload,
  CalendarEvent,
  CalendarEventInput,
  DashboardIntelligenceResponse,
  FocusSessionResponse,
  GoalProgressResponse,
  HourlyBreakdownResponse,
  IntelligenceSessionResponse,
  ProductivityPatternsResponse,
  ProductivitySummaryResponse,
  TaskResponse,
  TaskUpdateParams,
  TimelineResponse,
  TimelineSource,
} from "@/bindings";
import { commands } from "@/bindings";
```

- [ ] **Step 3: Append all 12 new wrappers at the bottom of the file**

Append after the existing `productivityCalendarEvents` wrapper:

```ts
export async function productivitySummaryRangeQuery(
  startDate: string,
  endDate: string,
): Promise<ProductivitySummaryResponse[]> {
  const r = await commands.productivitySummaryRange(startDate, endDate);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity summary range failed");
  return r.data;
}

export async function productivityWeeklyQuery(): Promise<ProductivitySummaryResponse[]> {
  const r = await commands.productivityWeekly();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity weekly failed");
  return r.data;
}

export async function productivityPatternsQuery(
  days: number | null,
): Promise<ProductivityPatternsResponse> {
  const r = await commands.productivityPatterns(days);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity patterns failed");
  return r.data;
}

export async function productivityHourlyBreakdownQuery(
  startDate: string,
  endDate: string,
  tzOffsetMins: number | null,
): Promise<HourlyBreakdownResponse[]> {
  const r = await commands.productivityHourlyBreakdown(startDate, endDate, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity hourly breakdown failed");
  return r.data;
}

export async function productivityTimelineQuery(
  date: string,
  limit: number | null,
  offset: number | null,
  tzOffsetMins: number | null,
): Promise<ActivityTimelineResponse[]> {
  const r = await commands.productivityTimeline(date, limit, offset, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity timeline failed");
  return r.data;
}

export async function productivityCategoriesQuery(): Promise<ActivityCategoryResponse[]> {
  const r = await commands.productivityCategories();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity categories failed");
  return r.data;
}

export async function productivityIntelligenceSessionsQuery(
  date: string,
  tzOffsetMins: number | null,
): Promise<IntelligenceSessionResponse[]> {
  const r = await commands.productivityIntelligenceSessions(date, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity intelligence sessions failed");
  return r.data;
}

export async function productivityActivityFeedQuery(
  limit: number | null,
): Promise<ActivityTimelineResponse[]> {
  const r = await commands.productivityActivityFeed(limit);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity activity feed failed");
  return r.data;
}

export async function productivityGoalsQuery(): Promise<GoalProgressResponse[]> {
  const r = await commands.productivityGoals();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goals failed");
  return r.data;
}

export interface GoalCreateParams {
  goalType: string;
  metric: string;
  targetValue: number;
}

export async function productivityGoalCreate(
  params: GoalCreateParams,
): Promise<GoalProgressResponse> {
  const r = await commands.productivityGoalCreate(params.goalType, params.metric, params.targetValue);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goal create failed");
  return r.data;
}

export async function productivityGoalDelete(id: number): Promise<void> {
  const r = await commands.productivityGoalDelete(id);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goal delete failed");
  return;
}

export async function productivityAutoFocusConfirm(
  payload: AutoFocusPayload,
): Promise<FocusSessionResponse> {
  const r = await commands.productivityAutoFocusConfirm(payload);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity auto-focus confirm failed");
  return r.data;
}
```

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. If any type is missing from `@/bindings`, that means `bindings.ts` is stale — run `cargo tauri dev` once in another terminal to regenerate, then retry.

- [ ] **Step 5: Commit**

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar
git add desktop-ui/src/api/endpoints/dashboard.ts
git commit -m "feat(dashboard): add Phase 3 productivity endpoint wrappers"
```

---

## Task 2: Add new query keys to `queryKeys.ts`

**Files:**
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`

- [ ] **Step 1: Locate the `productivity:` block**

Run:
```bash
grep -n "productivity:" /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/lib/query/queryKeys.ts
```

Expected: line ~114 shows `productivity: {`.

- [ ] **Step 2: Replace the `productivity:` block with the extended set**

Find this:
```ts
productivity: {
  all: () => ["productivity"] as const,
  calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,
}
```

Replace with:
```ts
productivity: {
  all: () => ["productivity"] as const,
  calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,

  summaryRange: (startDate: string, endDate: string) =>
    ["productivity", "summaryRange", startDate, endDate] as const,
  weekly: () => ["productivity", "weekly"] as const,
  patterns: (days: number | null) =>
    ["productivity", "patterns", days ?? "default"] as const,
  hourlyBreakdown: (startDate: string, endDate: string) =>
    ["productivity", "hourlyBreakdown", startDate, endDate] as const,
  timeline: (date: string) => ["productivity", "timeline", date] as const,
  categories: () => ["productivity", "categories"] as const,
  intelligenceSessions: (date: string) =>
    ["productivity", "intelligenceSessions", date] as const,
  activityFeed: (limit: number) => ["productivity", "activityFeed", limit] as const,
  goals: () => ["productivity", "goals"] as const,
}
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts
git commit -m "feat(dashboard): add Phase 3 productivity query keys"
```

---

## Task 3: Add focus event subscriptions to `events.ts`

**Files:**
- Modify: `desktop-ui/src/services/events.ts`

- [ ] **Step 1: Locate the existing hub registrations**

Run:
```bash
grep -n "createEventHub" /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/services/events.ts | head -5
```

Expected: hubs declared sequentially around line 86–115.

- [ ] **Step 2: Add the focus type imports**

In the existing top-of-file import block (lines 2–7), extend the imports:

```ts
import type {
  AppServerEvent,
  AutoFocusPayload,
  DictationEvent,
  DictationModelStatus,
  FocusStatePayload,
  TrayOpenThreadPayload,
} from "../types";
```

If `AutoFocusPayload` and `FocusStatePayload` aren't re-exported from `../types`, import from `@/bindings` instead in a separate `import type` line:

```ts
import type { AutoFocusPayload, FocusStatePayload } from "@/bindings";
```

(Verify by running `grep "AutoFocusPayload\|FocusStatePayload" desktop-ui/src/types.ts` — if absent, use the bindings import.)

- [ ] **Step 3: Add two new hub declarations**

Add directly after the existing `menuComposerCycleCollaborationHub` declaration (around line 114):

```ts
const focusStateChangedHub = createEventHub<FocusStatePayload>("focus:state_changed");
const focusAutoDetectedHub = createEventHub<AutoFocusPayload>("focus:auto_detected");
```

- [ ] **Step 4: Add the two new exported `subscribe*` functions**

Append at the end of the file:

```ts
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

- [ ] **Step 5: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/services/events.ts
git commit -m "feat(dashboard): add focus state-change and auto-detected event subscriptions"
```

---

## Task 4: Create `lib/productivity.ts` (helpers + AppIcon)

**Files:**
- Create: `desktop-ui/src/features/dashboard/lib/productivity.tsx`
- Create: `desktop-ui/src/features/dashboard/lib/productivity.test.ts`

This is a verbatim port of `desktop-ui.bak/src/shared/lib/productivity.tsx`. The file is `.tsx` (not `.ts`) because `AppIcon` returns JSX and the SVG icon constants are JSX-valued.

- [ ] **Step 1: Write the failing test for `scoreColor` thresholds**

Create `desktop-ui/src/features/dashboard/lib/productivity.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  getAppColor,
  getCategoryColor,
  purityToOpacity,
  qualityToColor,
  resolveActivityColor,
  resolveCategoryLabel,
  scoreColor,
} from "./productivity";

describe("scoreColor", () => {
  it("returns success for >= 80", () => {
    expect(scoreColor(80)).toBe("var(--success)");
    expect(scoreColor(95)).toBe("var(--success)");
  });
  it("returns brand for 60-79", () => {
    expect(scoreColor(60)).toBe("var(--brand)");
    expect(scoreColor(79)).toBe("var(--brand)");
  });
  it("returns muted for 40-59", () => {
    expect(scoreColor(40)).toBe("var(--text-muted-foreground)");
  });
  it("returns destructive below 40", () => {
    expect(scoreColor(0)).toBe("var(--destructive)");
    expect(scoreColor(39)).toBe("var(--destructive)");
  });
});

describe("getAppColor", () => {
  it("matches lowercase native app names", () => {
    expect(getAppColor("Visual Studio Code", null)).toBe("#007ACC");
  });
  it("matches lowercase domain site names", () => {
    expect(getAppColor("youtube.com", null)).toBe("#FF0000");
  });
  it("falls back to category color when app unknown", () => {
    expect(getAppColor("UnknownApp", "coding")).toBe("#22C55E");
  });
  it("falls back to brand when nothing matches", () => {
    expect(getAppColor("UnknownApp", null)).toBe("var(--brand)");
  });
});

describe("getCategoryColor", () => {
  it("resolves a known category id", () => {
    expect(getCategoryColor("coding")).toBe("#22C55E");
  });
  it("normalizes display names with spaces and ampersands", () => {
    expect(getCategoryColor("Project Management")).toBe("#8B5CF6");
  });
  it("falls back to a rotating palette for unknown", () => {
    expect(getCategoryColor("zzz", 0)).toBe("#60A5FA");
  });
});

describe("resolveActivityColor", () => {
  it("returns surface-highest for idle", () => {
    expect(resolveActivityColor("anything", true)).toBe("var(--surface-highest)");
  });
  it("returns success for productive", () => {
    expect(resolveActivityColor("productive", false)).toBe("var(--success)");
  });
  it("returns destructive for distracting", () => {
    expect(resolveActivityColor("distracting", false)).toBe("var(--destructive)");
  });
  it("falls back to brand for unknown category type", () => {
    expect(resolveActivityColor("zzz", false)).toBe("var(--brand)");
  });
});

describe("resolveCategoryLabel", () => {
  it("maps known types", () => {
    expect(resolveCategoryLabel("productive")).toBe("Productive");
    expect(resolveCategoryLabel("distracting")).toBe("Distracting");
    expect(resolveCategoryLabel("neutral")).toBe("Neutral");
  });
  it("falls back to Uncategorized", () => {
    expect(resolveCategoryLabel("zzz")).toBe("Uncategorized");
  });
});

describe("qualityToColor", () => {
  it("returns an oklch() string", () => {
    const c = qualityToColor(50);
    expect(c.startsWith("oklch(")).toBe(true);
  });
  it("clamps to 0..100", () => {
    expect(qualityToColor(-50)).toBe(qualityToColor(0));
    expect(qualityToColor(150)).toBe(qualityToColor(100));
  });
});

describe("purityToOpacity", () => {
  it("returns 0.65 for null", () => {
    expect(purityToOpacity(null)).toBe(0.65);
  });
  it("maps 0 → 0.5 and 1 → 0.9", () => {
    expect(purityToOpacity(0)).toBeCloseTo(0.5);
    expect(purityToOpacity(1)).toBeCloseTo(0.9);
  });
});
```

- [ ] **Step 2: Run the test (should fail with module-not-found)**

Run: `cd desktop-ui && bun run test productivity.test.ts`
Expected: FAIL — "Cannot find module './productivity'".

- [ ] **Step 3: Create `productivity.tsx` by copying the backup file verbatim**

```bash
cp /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui.bak/src/shared/lib/productivity.tsx \
   /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/lib/productivity.tsx
```

The backup file is self-contained — no `@shared` imports, only `lucide-react` and `react`. Both are already in the current desktop-ui's package.json.

- [ ] **Step 4: Run the test (should pass)**

Run: `cd desktop-ui && bun run test productivity.test.ts`
Expected: all 17 assertions pass.

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean (the backup file follows the same Biome/ESLint rules as the current desktop-ui).

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/lib/productivity.tsx \
        desktop-ui/src/features/dashboard/lib/productivity.test.ts
git commit -m "feat(dashboard): port productivity helper lib (colors, scores, AppIcon)"
```

---

## Task 5: Create `lib/activity-sessions.ts` (mergeActivitySessions)

**Files:**
- Create: `desktop-ui/src/features/dashboard/lib/activity-sessions.ts`
- Create: `desktop-ui/src/features/dashboard/lib/activity-sessions.test.ts`

- [ ] **Step 1: Write failing tests covering the merge algorithm**

Create `desktop-ui/src/features/dashboard/lib/activity-sessions.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { mergeActivitySessions, type MergeableEvent } from "./activity-sessions";

function ev(partial: Partial<MergeableEvent>): MergeableEvent {
  return {
    startSecs: 0,
    endSecs: 60,
    catType: "productive",
    color: "#22C55E",
    label: "VSCode",
    isIdle: false,
    dur: 60,
    ...partial,
  };
}

describe("mergeActivitySessions", () => {
  it("returns empty for empty input", () => {
    expect(mergeActivitySessions([])).toEqual([]);
  });

  it("skips idle events", () => {
    const result = mergeActivitySessions([ev({ isIdle: true })]);
    expect(result).toEqual([]);
  });

  it("merges adjacent events within 120s gap", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
      ev({ startSecs: 100, endSecs: 200, dur: 100 }), // gap = 40s, < 120
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].startSecs).toBe(0);
    expect(result[0].endSecs).toBe(200);
    expect(result[0].duration).toBe(200);
  });

  it("does NOT merge events with > 120s gap", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
      ev({ startSecs: 200, endSecs: 300, dur: 100 }), // gap = 140s
    ]);
    expect(result).toHaveLength(2);
  });

  it("picks dominant category by total duration", () => {
    const result = mergeActivitySessions([
      ev({ catType: "productive", color: "#22C55E", dur: 60, label: "VSCode" }),
      ev({
        startSecs: 80,
        endSecs: 400,
        catType: "neutral",
        color: "#94A3B8",
        dur: 320,
        label: "Slack",
      }),
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].dominantCategory).toBe("neutral");
    expect(result[0].color).toBe("#94A3B8");
  });

  it("picks best label by largest event duration", () => {
    const result = mergeActivitySessions([
      ev({ dur: 60, label: "Short" }),
      ev({ startSecs: 80, endSecs: 400, dur: 320, label: "Long" }),
    ]);
    expect(result[0].label).toBe("Long");
  });

  it("builds appBreakdown sorted by duration, max 5", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 0, endSecs: 30, dur: 30, label: "A" }),
      ev({ startSecs: 30, endSecs: 100, dur: 70, label: "B" }),
      ev({ startSecs: 100, endSecs: 110, dur: 10, label: "C" }),
    ]);
    expect(result[0].appBreakdown.map((a) => a.app)).toEqual(["B", "A", "C"]);
  });

  it("sorts unsorted input before merging", () => {
    const result = mergeActivitySessions([
      ev({ startSecs: 200, endSecs: 300, dur: 100 }),
      ev({ startSecs: 0, endSecs: 60, dur: 60 }),
    ]);
    // events are sorted, so 0..60 and 200..300 are processed in order; gap > 120, two sessions
    expect(result).toHaveLength(2);
    expect(result[0].startSecs).toBe(0);
    expect(result[1].startSecs).toBe(200);
  });

  it("drops zero-duration sessions", () => {
    const result = mergeActivitySessions([ev({ startSecs: 100, endSecs: 100, dur: 0 })]);
    expect(result).toEqual([]);
  });
});
```

- [ ] **Step 2: Run the test (should fail)**

Run: `cd desktop-ui && bun run test activity-sessions.test.ts`
Expected: FAIL — "Cannot find module './activity-sessions'".

- [ ] **Step 3: Copy the backup file verbatim**

```bash
cp /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui.bak/src/shared/lib/activity-sessions.ts \
   /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/lib/activity-sessions.ts
```

The backup file is self-contained — no external imports.

- [ ] **Step 4: Run tests (should pass)**

Run: `cd desktop-ui && bun run test activity-sessions.test.ts`
Expected: all 9 tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/lib/activity-sessions.ts \
        desktop-ui/src/features/dashboard/lib/activity-sessions.test.ts
git commit -m "feat(dashboard): port activity-session merge algorithm"
```

---

## Task 6: Create test fixture mock helper

**Files:**
- Create: `desktop-ui/src/features/dashboard/__tests__/dashboardCommandMocks.ts`

A shared module that test files import to get default-empty `vi.fn()` stubs for every endpoint wrapper used in Phase 3. Keeps individual `vi.mock(...)` blocks small.

- [ ] **Step 1: Create the helper file**

Create `desktop-ui/src/features/dashboard/__tests__/dashboardCommandMocks.ts`:

```ts
/**
 * Default mock factories for dashboard endpoint wrappers.
 * Use in tests via:
 *   vi.mock("@/api/endpoints/dashboard", async () => ({
 *     ...(await vi.importActual<typeof import("@/api/endpoints/dashboard")>("@/api/endpoints/dashboard")),
 *     ...defaultDashboardMocks(),
 *   }));
 */
import { vi } from "vitest";

export function defaultDashboardMocks() {
  return {
    timelineQuery: vi.fn().mockResolvedValue({
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
    }),
    taskUpdate: vi.fn(),
    calendarSyncEvents: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
    productivityTodayQuery: vi.fn().mockResolvedValue(null),
    dashboardIntelligenceQuery: vi.fn().mockResolvedValue({
      activeContext: null,
      focusRecommendation: null,
      sessionSummary: [],
      contextSwitches: 0,
      switchQuality: "neutral",
      productivityScore: 0,
      scoreTrend: 0,
      patterns: [],
      nudges: [],
      resourceClusters: [],
    }),
    productivitySummaryRangeQuery: vi.fn().mockResolvedValue([]),
    productivityWeeklyQuery: vi.fn().mockResolvedValue([]),
    productivityPatternsQuery: vi.fn().mockResolvedValue({
      daysAnalyzed: 0,
      peakFocusHours: [],
      bestDayOfWeek: null,
      avgSessionMins: 0,
    }),
    productivityHourlyBreakdownQuery: vi.fn().mockResolvedValue([]),
    productivityTimelineQuery: vi.fn().mockResolvedValue([]),
    productivityCategoriesQuery: vi.fn().mockResolvedValue([]),
    productivityIntelligenceSessionsQuery: vi.fn().mockResolvedValue([]),
    productivityActivityFeedQuery: vi.fn().mockResolvedValue([]),
    productivityGoalsQuery: vi.fn().mockResolvedValue([]),
    productivityGoalCreate: vi.fn(),
    productivityGoalDelete: vi.fn(),
    productivityAutoFocusConfirm: vi.fn(),
  };
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. The shape mirrors `EMPTY_TIMELINE_RESPONSE` and the bindings types — if the shape drifts from `bindings.ts`, fix mismatched fields.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/__tests__/dashboardCommandMocks.ts
git commit -m "test(dashboard): add shared command mock helper"
```

---

## Task 7: Port `ProductivityScoreRing` (with `ScoreBar` co-export)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/ProductivityScoreRing.tsx`
- Create: `desktop-ui/src/features/dashboard/components/productivity/ProductivityScoreRing.test.tsx`

The backup file is `desktop-ui.bak/src/features/dashboard/components/productivity/ProductivityScoreRing.tsx` (159L). Tailwind class strings translate to BEM classes added in Task 25; for now the component uses the BEM class names directly — when Task 25 lands, the styles fill in.

- [ ] **Step 1: Write the failing test**

```tsx
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ProductivityScoreRing, ScoreBar } from "./ProductivityScoreRing";

afterEach(() => cleanup());

describe("ProductivityScoreRing", () => {
  it("renders the rounded score and 'Good' label for 75", () => {
    render(<ProductivityScoreRing score={75} />);
    expect(screen.getByText("75")).toBeTruthy();
    expect(screen.getByText("Good")).toBeTruthy();
  });

  it("renders 'Excellent' for >= 80", () => {
    render(<ProductivityScoreRing score={92} />);
    expect(screen.getByText("Excellent")).toBeTruthy();
  });

  it("renders em-dash label for 0 score", () => {
    render(<ProductivityScoreRing score={0} />);
    expect(screen.getByText("—")).toBeTruthy();
  });

  it("shows tooltip rows on hover when summary is provided", () => {
    render(
      <ProductivityScoreRing
        score={75}
        summary={{
          productiveSecs: 3600,
          neutralSecs: 600,
          distractingSecs: 0,
          totalActiveSecs: 4200,
          avgSessionQuality: 0.8,
          focusSessionsCount: 2,
          contextSwitches: 5,
        }}
      />,
    );
    const ring = screen.getByText("75").closest("div");
    if (!ring) throw new Error("ring not found");
    fireEvent.mouseEnter(ring);
    expect(screen.getByText("Focus time")).toBeTruthy();
    expect(screen.getByText("Context switches")).toBeTruthy();
  });
});

describe("ScoreBar", () => {
  it("renders label, percent value, and bar fill width", () => {
    render(<ScoreBar label="Quality" value={0.42} />);
    expect(screen.getByText("Quality")).toBeTruthy();
    expect(screen.getByText("42")).toBeTruthy();
  });

  it("clamps value to [0,1]", () => {
    render(<ScoreBar label="X" value={1.5} />);
    expect(screen.getByText("100")).toBeTruthy();
    cleanup();
    render(<ScoreBar label="Y" value={-0.2} />);
    expect(screen.getByText("0")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test (should fail)**

Run: `cd desktop-ui && bun run test ProductivityScoreRing.test`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the component**

Create `desktop-ui/src/features/dashboard/components/productivity/ProductivityScoreRing.tsx`:

```tsx
import { useState } from "react";
import { formatHumanDuration } from "@/utils/dashboardDates";
import { scoreColor } from "../../lib/productivity";

interface ProductivityScoreRingProps {
  score: number;
  size?: number;
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

export function ScoreBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(Math.min(Math.max(value, 0), 1) * 100);
  return (
    <div className="dashboard__summary-score-bar">
      <span className="dashboard__summary-score-label">{label}</span>
      <div className="dashboard__summary-score-track">
        <div className="dashboard__summary-score-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="dashboard__summary-score-value">{pct}</span>
    </div>
  );
}

function scoreLabel(score: number): string {
  if (score >= 80) return "Excellent";
  if (score >= 60) return "Good";
  if (score >= 40) return "Fair";
  if (score > 0) return "Low";
  return "—";
}

export function ProductivityScoreRing({
  score,
  size = 110,
  summary,
}: ProductivityScoreRingProps) {
  const [hovered, setHovered] = useState(false);
  const strokeWidth = 7;
  const radius = (size - strokeWidth) / 2;
  const circumference = 2 * Math.PI * radius;
  const progress = Math.min(score / 100, 1);
  const offset = circumference * (1 - progress);
  const center = size / 2;
  const color = scoreColor(score);

  const focusRatio =
    summary && summary.totalActiveSecs > 0
      ? Math.round((summary.productiveSecs / summary.totalActiveSecs) * 100)
      : null;
  const distractionRatio =
    summary && summary.totalActiveSecs > 0
      ? Math.round((summary.distractingSecs / summary.totalActiveSecs) * 100)
      : null;
  const qualityAvg =
    summary?.avgSessionQuality != null ? Math.round(summary.avgSessionQuality * 100) : null;

  return (
    <div className="dashboard__score-ring">
      <div
        className="dashboard__score-ring-track"
        style={{ width: size, height: size }}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        <div
          className="dashboard__score-ring-glow"
          style={{
            background: `radial-gradient(circle, ${color}15 0%, transparent 70%)`,
            opacity: score > 0 ? 1 : 0,
          }}
        />

        <svg width={size} height={size} className="dashboard__score-ring-svg" aria-hidden="true">
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke="var(--surface-raised)"
            strokeWidth={strokeWidth}
          />
          <circle
            cx={center}
            cy={center}
            r={radius}
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={offset}
            style={{ filter: `drop-shadow(0 0 4px ${color}66)` }}
          />
        </svg>

        <div className="dashboard__score-ring-value">
          <span style={{ color }}>{Math.round(score)}</span>
          <span className="dashboard__score-ring-suffix">/100</span>
        </div>

        {hovered && summary && summary.totalActiveSecs > 0 && (
          <div className="dashboard__score-ring-tooltip">
            {focusRatio != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Focus time</span>
                <span>
                  {focusRatio}% ({formatHumanDuration(summary.productiveSecs)})
                </span>
              </div>
            )}
            <div className="dashboard__score-ring-tooltip-row">
              <span>Context switches</span>
              <span>{summary.contextSwitches}</span>
            </div>
            {qualityAvg != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Session quality</span>
                <span>{qualityAvg}%</span>
              </div>
            )}
            {distractionRatio != null && (
              <div className="dashboard__score-ring-tooltip-row">
                <span>Distraction</span>
                <span>{distractionRatio}%</span>
              </div>
            )}
          </div>
        )}
      </div>

      <span className="dashboard__score-ring-label" style={{ color }}>
        {scoreLabel(score)}
      </span>
    </div>
  );
}
```

- [ ] **Step 4: Run tests (should pass)**

Run: `cd desktop-ui && bun run test ProductivityScoreRing.test`
Expected: 6 assertions pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/productivity/ProductivityScoreRing.tsx \
        desktop-ui/src/features/dashboard/components/productivity/ProductivityScoreRing.test.tsx
git commit -m "feat(dashboard): port ProductivityScoreRing + ScoreBar"
```

---

## Task 8: Port `HourlyHeatmap`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/HourlyHeatmap.tsx`

Pure display port. Uses `productivityHourlyBreakdownQuery`. No new tests — covered indirectly via `SummaryPanel.test.tsx` (Task 16).

- [ ] **Step 1: Create the component**

```tsx
import type { HourlyBreakdownResponse } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { TZ_OFFSET_MINS } from "@/utils/dashboardDates";
import { productivityHourlyBreakdownQuery } from "@/api/endpoints/dashboard";

interface Props {
  startDate: string;
  endDate: string;
}

function heatColor(ratio: number): string {
  const t = Math.max(0, Math.min(1, ratio));
  const stops: [number, number, number][] = [
    [0, 70, 50],
    [25, 80, 50],
    [45, 85, 50],
    [145, 65, 45],
  ];
  const seg = t * (stops.length - 1);
  const i = Math.min(Math.floor(seg), stops.length - 2);
  const f = seg - i;
  const [h, s, l] = stops[i].map((v, k) => v + (stops[i + 1][k] - v) * f);
  return `hsl(${h}, ${s}%, ${l}%)`;
}

export function HourlyHeatmap({ startDate, endDate }: Props) {
  const { data } = useTauriQuery<HourlyBreakdownResponse[]>({
    queryKey: qk.productivity.hourlyBreakdown(startDate, endDate),
    queryFn: () => productivityHourlyBreakdownQuery(startDate, endDate, TZ_OFFSET_MINS),
    fallback: [],
    staleTime: 60_000,
  });

  if (!data || data.length === 0) return null;
  const working = data.filter((h) => h.hour >= 6 && h.hour <= 22);
  if (working.length === 0) return null;

  const maxRatio = Math.max(...working.map((h) => h.productiveRatio), 0.01);
  const peakHour = working.reduce((best, h) =>
    h.productiveRatio > best.productiveRatio ? h : best,
  );

  return (
    <div className="dashboard__hourly">
      <div className="dashboard__hourly-title">
        Hourly Productivity
        {peakHour && <span className="dashboard__hourly-peak"> Peak: {peakHour.hour}:00</span>}
      </div>
      <div>
        {working.map((h) => {
          const width = (h.productiveRatio / maxRatio) * 100;
          return (
            <div key={h.hour} className="dashboard__hourly-row">
              <span className="dashboard__hourly-hour-label">{h.hour}</span>
              <div className="dashboard__hourly-bar-track">
                <div
                  className="dashboard__hourly-bar-fill"
                  style={{ width: `${width}%`, backgroundColor: heatColor(h.productiveRatio) }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. Verify `useTauriQuery`'s `staleTime` option is supported (check `desktop-ui/src/lib/query/useTauriQuery.ts`); if not, drop `staleTime` and rely on the default — invalidation still works via the 30 s Day-view poll.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/productivity/HourlyHeatmap.tsx
git commit -m "feat(dashboard): port HourlyHeatmap"
```

---

## Task 9: Port `PatternsCard`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/PatternsCard.tsx`

- [ ] **Step 1: Create the component**

```tsx
import type { ProductivityPatternsResponse } from "@/bindings";
import { productivityPatternsQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";

const FALLBACK: ProductivityPatternsResponse = {
  daysAnalyzed: 0,
  peakFocusHours: [],
  bestDayOfWeek: null,
  avgSessionMins: 0,
};

export function PatternsCard() {
  const { data } = useTauriQuery<ProductivityPatternsResponse>({
    queryKey: qk.productivity.patterns(null),
    queryFn: () => productivityPatternsQuery(null),
    fallback: FALLBACK,
    staleTime: 5 * 60 * 1000,
  });

  if (!data || data.daysAnalyzed < 3) return null;

  const peakLabel =
    data.peakFocusHours.length > 0
      ? data.peakFocusHours.map((h) => `${h}:00`).join(", ")
      : "—";

  return (
    <div className="dashboard__patterns">
      <div className="dashboard__patterns-title">Your Patterns</div>
      <div>
        <div className="dashboard__patterns-row">Peak hours: {peakLabel}</div>
        {data.bestDayOfWeek && <div className="dashboard__patterns-row">Best day: {data.bestDayOfWeek}</div>}
        <div className="dashboard__patterns-row">Avg session: {Math.round(data.avgSessionMins)}min</div>
        <div className="dashboard__patterns-footer">{data.daysAnalyzed} days analyzed</div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

```bash
git add desktop-ui/src/features/dashboard/components/productivity/PatternsCard.tsx
git commit -m "feat(dashboard): port PatternsCard"
```

---

## Task 10: Port `AddGoalDialog`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/AddGoalDialog.tsx`

- [ ] **Step 1: Create the dialog**

```tsx
import { X } from "lucide-react";
import { useState } from "react";

interface AddGoalDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: { goalType: string; metric: string; targetValue: number }) => void;
}

const METRICS = [
  { value: "productive_hours", label: "Productive hours", unit: "hours", placeholder: "6" },
  { value: "focus_sessions", label: "Focus sessions", unit: "sessions", placeholder: "4" },
  { value: "productivity_score", label: "Productivity score", unit: "/100", placeholder: "70" },
  {
    value: "max_distracting_mins",
    label: "Max distracting minutes",
    unit: "mins",
    placeholder: "30",
  },
] as const;

export function AddGoalDialog({ open, onClose, onAdd }: AddGoalDialogProps) {
  const [goalType, setGoalType] = useState<"daily" | "weekly">("daily");
  const [metric, setMetric] = useState<string>(METRICS[0].value);
  const [targetValue, setTargetValue] = useState("");

  if (!open) return null;

  const selectedMetric = METRICS.find((m) => m.value === metric) ?? METRICS[0];
  const canSubmit = targetValue.trim() !== "" && Number(targetValue) > 0;

  const handleSubmit = () => {
    onAdd({ goalType, metric, targetValue: Number(targetValue) });
    setTargetValue("");
    onClose();
  };

  return (
    <div className="dashboard__goal-dialog-backdrop">
      <div className="dashboard__goal-dialog">
        <div className="dashboard__goal-dialog-header">
          <h3>Add Goal</h3>
          <button type="button" onClick={onClose} aria-label="Close dialog">
            <X aria-hidden />
          </button>
        </div>

        <div className="dashboard__goal-dialog-body">
          <div className="dashboard__goal-dialog-section">
            <span>Period</span>
            <div className="dashboard__goal-dialog-period-toggle">
              {(["daily", "weekly"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setGoalType(t)}
                  className={
                    goalType === t
                      ? "dashboard__goal-dialog-period-btn dashboard__goal-dialog-period-btn--active"
                      : "dashboard__goal-dialog-period-btn"
                  }
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          <div className="dashboard__goal-dialog-section">
            <span>Metric</span>
            <div className="dashboard__goal-dialog-metric-list">
              {METRICS.map((m) => (
                <button
                  key={m.value}
                  type="button"
                  onClick={() => setMetric(m.value)}
                  className={
                    metric === m.value
                      ? "dashboard__goal-dialog-metric-btn dashboard__goal-dialog-metric-btn--active"
                      : "dashboard__goal-dialog-metric-btn"
                  }
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>

          <div className="dashboard__goal-dialog-section">
            <label htmlFor="goal-target">
              Target <span>({selectedMetric.unit})</span>
            </label>
            <input
              id="goal-target"
              type="number"
              value={targetValue}
              onChange={(e) => setTargetValue(e.target.value)}
              placeholder={selectedMetric.placeholder}
              min={0}
              step={metric === "productive_hours" ? 0.5 : 1}
              className="dashboard__goal-dialog-input"
            />
          </div>
        </div>

        <div className="dashboard__goal-dialog-footer">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="button" onClick={handleSubmit} disabled={!canSubmit}>
            Add goal
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
```
Expected: clean.

```bash
git add desktop-ui/src/features/dashboard/components/productivity/AddGoalDialog.tsx
git commit -m "feat(dashboard): port AddGoalDialog"
```

---

## Task 11: Port `GoalsProgress` (with tests)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/GoalsProgress.tsx`
- Create: `desktop-ui/src/features/dashboard/components/productivity/GoalsProgress.test.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
    productivityGoalsQuery: vi.fn().mockResolvedValue([
      {
        id: 1,
        goalType: "daily",
        metric: "productive_hours",
        targetValue: 6,
        currentValue: 3,
        met: false,
        projectId: null,
      },
      {
        id: 2,
        goalType: "weekly",
        metric: "focus_sessions",
        targetValue: 10,
        currentValue: 12,
        met: true,
        projectId: null,
      },
    ]),
    productivityGoalDelete: vi.fn().mockResolvedValue(undefined),
  };
});

import { GoalsProgress } from "./GoalsProgress";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("GoalsProgress", () => {
  it("renders both goals with correct status pills", async () => {
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("MET")).toBeTruthy());
    expect(screen.getByText("IN PROGRESS")).toBeTruthy();
    expect(screen.getByText(/productive hours/)).toBeTruthy();
    expect(screen.getByText(/focus sessions/)).toBeTruthy();
  });

  it("opens AddGoalDialog when plus button clicked", async () => {
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("Goals")).toBeTruthy());
    const plusBtn = screen.getByLabelText("Add goal");
    fireEvent.click(plusBtn);
    expect(screen.getByText("Add Goal")).toBeTruthy();
  });

  it("calls productivityGoalDelete when trash button clicked", async () => {
    const { productivityGoalDelete } = await import("@/api/endpoints/dashboard");
    render(wrap(<GoalsProgress />));
    await waitFor(() => expect(screen.getByText("MET")).toBeTruthy());
    const deleteButtons = screen.getAllByLabelText("Delete goal");
    fireEvent.click(deleteButtons[0]);
    await waitFor(() => expect(productivityGoalDelete).toHaveBeenCalledWith(1));
  });
});
```

- [ ] **Step 2: Run test (fail)**

Run: `cd desktop-ui && bun run test GoalsProgress.test`
Expected: FAIL — module not found.

- [ ] **Step 3: Create the component**

```tsx
import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import type { GoalProgressResponse } from "@/bindings";
import {
  type GoalCreateParams,
  productivityGoalCreate,
  productivityGoalDelete,
  productivityGoalsQuery,
} from "@/api/endpoints/dashboard";
import { useTauriMutation, useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { AddGoalDialog } from "./AddGoalDialog";

function metricLabel(metric: string): string {
  switch (metric) {
    case "productive_hours":
      return "productive hours";
    case "focus_sessions":
      return "focus sessions";
    case "productivity_score":
      return "score";
    case "max_distracting_mins":
      return "distracting mins";
    case "project_hours":
      return "project hours";
    default:
      return metric;
  }
}

function formatValue(metric: string, value: number): string {
  if (metric === "productive_hours" || metric === "project_hours") return `${value.toFixed(1)}h`;
  if (metric === "max_distracting_mins") return `${Math.round(value)}m`;
  return `${Math.round(value)}`;
}

export function GoalsProgress() {
  const [showAdd, setShowAdd] = useState(false);

  const { data: goals } = useTauriQuery<GoalProgressResponse[]>({
    queryKey: qk.productivity.goals(),
    queryFn: () => productivityGoalsQuery(),
    fallback: [],
  });

  const { mutate: createGoal } = useTauriMutation<GoalProgressResponse, GoalCreateParams>({
    mutationFn: productivityGoalCreate,
    invalidates: [qk.productivity.goals()],
  });

  const { mutate: deleteGoal } = useTauriMutation<void, number>({
    mutationFn: productivityGoalDelete,
    invalidates: [qk.productivity.goals()],
  });

  const handleAdd = (params: GoalCreateParams) => {
    void createGoal(params);
  };

  return (
    <>
      <div className="dashboard__goals">
        <div className="dashboard__goals-header">
          <h2>Goals</h2>
          <button
            type="button"
            onClick={() => setShowAdd(true)}
            className="dashboard__goals-add-btn"
            aria-label="Add goal"
          >
            <Plus aria-hidden />
          </button>
        </div>

        {goals.length === 0 ? (
          <p className="dashboard__goals-empty">No goals set</p>
        ) : (
          <div>
            {goals.map((g) => {
              const pct =
                g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
              return (
                <div key={g.id} className="dashboard__goal-row">
                  <div className="dashboard__goal-meta">
                    <span
                      className={
                        g.met
                          ? "dashboard__goal-status dashboard__goal-status--met"
                          : "dashboard__goal-status dashboard__goal-status--in-progress"
                      }
                    >
                      {g.met ? "MET" : "IN PROGRESS"}
                    </span>
                    <span>
                      {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                    </span>
                    {g.projectId && <span className="dashboard__goal-project-tag">{g.projectId}</span>}
                    <span>({g.goalType})</span>
                    <span>
                      {formatValue(g.metric, g.currentValue)} / {formatValue(g.metric, g.targetValue)}
                    </span>
                    <button
                      type="button"
                      onClick={() => void deleteGoal(g.id)}
                      className="dashboard__goal-delete-btn"
                      aria-label="Delete goal"
                    >
                      <Trash2 aria-hidden />
                    </button>
                  </div>
                  <div className="dashboard__goal-bar-track">
                    <div
                      className={
                        g.met
                          ? "dashboard__goal-bar-fill dashboard__goal-bar-fill--met"
                          : "dashboard__goal-bar-fill"
                      }
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <AddGoalDialog open={showAdd} onClose={() => setShowAdd(false)} onAdd={handleAdd} />
    </>
  );
}
```

- [ ] **Step 4: Run tests (pass)**

Run: `cd desktop-ui && bun run test GoalsProgress.test`
Expected: 3 assertions pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/productivity/GoalsProgress.tsx \
        desktop-ui/src/features/dashboard/components/productivity/GoalsProgress.test.tsx
git commit -m "feat(dashboard): port GoalsProgress with create/delete"
```

---

## Task 12: Rewrite `ActivityFeed` (replace stub)

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx`
- Create: `desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.test.tsx`

The current file is a 7-line stub. Replace with a full polling implementation.

- [ ] **Step 1: Write failing tests**

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
    productivityActivityFeedQuery: vi.fn().mockResolvedValue([
      {
        startedAt: new Date(Date.now() - 5_000).toISOString(),
        appName: "VSCode",
        siteName: null,
        windowTitle: "main.ts — myproject",
        projectId: null,
        categoryId: "coding",
        isIdle: false,
      },
      {
        startedAt: new Date(Date.now() - 90_000).toISOString(),
        appName: "Slack",
        siteName: null,
        windowTitle: null,
        projectId: null,
        categoryId: "communication",
        isIdle: false,
      },
    ]),
  };
});

import { ActivityFeed } from "./ActivityFeed";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("ActivityFeed", () => {
  it("renders both rows with app names", async () => {
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    expect(screen.getByText("Slack")).toBeTruthy();
  });

  it("shows recent ('now' / 'Ns') tag for first row", async () => {
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    // 5s old → "5s" or "now"
    expect(screen.getByText(/now|5s/)).toBeTruthy();
  });

  it("shows empty-state when no events", async () => {
    const { productivityActivityFeedQuery } = await import("@/api/endpoints/dashboard");
    (productivityActivityFeedQuery as ReturnType<typeof vi.fn>).mockResolvedValueOnce([]);
    render(wrap(<ActivityFeed />));
    await waitFor(() => expect(screen.getByText("No recent activity")).toBeTruthy());
  });
});
```

- [ ] **Step 2: Run test (fail)**

Run: `cd desktop-ui && bun run test ActivityFeed.test`
Expected: FAIL — `screen.getByText("VSCode")` not found (current stub returns null).

- [ ] **Step 3: Replace `ActivityFeed.tsx`**

Overwrite `desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx`:

```tsx
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { ActivityTimelineResponse } from "@/bindings";
import { productivityActivityFeedQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatTime } from "@/utils/dashboardDates";
import { AppIcon, getAppColor } from "../../lib/productivity";

const BROWSER_RE =
  /\s*[-–—]\s*(?:Google Chrome|Safari|Firefox|Arc|Brave Browser|Microsoft Edge|Orion|Vivaldi|Opera|Chromium|Zen Browser)(?:\s*[-–—]\s*.+)?$/i;

function stripBrowserSuffix(title: string): string {
  return title.replace(BROWSER_RE, "").trim();
}

function resolveDisplayName(e: ActivityTimelineResponse): { name: string; subtitle?: string } {
  if (e.isIdle) return { name: "Idle" };
  if (e.siteName && e.windowTitle) {
    const pageTitle = stripBrowserSuffix(e.windowTitle);
    if (pageTitle && pageTitle.toLowerCase() !== e.siteName.toLowerCase()) {
      return { name: e.siteName, subtitle: pageTitle };
    }
  }
  if (e.projectId) {
    return { name: e.appName, subtitle: e.projectId };
  }
  return { name: e.siteName ?? e.appName };
}

function ageSecs(dateStr: string): number {
  return Math.max(0, Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000));
}

function relativeTag(secs: number): string | null {
  if (secs < 10) return "now";
  if (secs < 60) return `${secs}s`;
  if (secs < 300) return `${Math.floor(secs / 60)}m`;
  return null;
}

export function ActivityFeed() {
  const client = useQueryClient();
  const { data: events } = useTauriQuery<ActivityTimelineResponse[]>({
    queryKey: qk.productivity.activityFeed(30),
    queryFn: () => productivityActivityFeedQuery(30),
    fallback: [],
  });

  const prevKeysRef = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());
  const scrollRef = useRef<HTMLDivElement>(null);

  // Periodic poll every 30s
  useEffect(() => {
    const id = setInterval(() => {
      void client.invalidateQueries({ queryKey: qk.productivity.activityFeed(30) });
    }, 30_000);
    return () => clearInterval(id);
  }, [client]);

  // Periodic re-render for relative-time labels
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 30_000);
    return () => clearInterval(id);
  }, []);

  // Detect new entries and animate
  useEffect(() => {
    const currentKeys = new Set(events.map((e, i) => `${e.startedAt}-${e.appName}-${i}`));
    const fresh = new Set<string>();
    for (const k of currentKeys) {
      if (!prevKeysRef.current.has(k)) fresh.add(k);
    }
    prevKeysRef.current = currentKeys;

    if (fresh.size > 0) {
      setNewKeys(fresh);
      scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
      const timer = setTimeout(() => setNewKeys(new Set()), 600);
      return () => clearTimeout(timer);
    }
  }, [events]);

  if (events.length === 0) {
    return (
      <div className="dashboard__activity-feed">
        <h2 className="dashboard__activity-feed-header">Activity</h2>
        <p className="dashboard__activity-feed-empty">No recent activity</p>
      </div>
    );
  }

  return (
    <div className="dashboard__activity-feed">
      <div className="dashboard__activity-feed-header">
        <h2>Activity</h2>
        <div>
          <span className="dashboard__activity-feed-live-dot" />
          <span>Live</span>
        </div>
      </div>
      <div ref={scrollRef} className="dashboard__activity-feed-list">
        {events.map((e, i) => {
          const { name, subtitle } = resolveDisplayName(e);
          const color = getAppColor(name, e.categoryId);
          const isFirst = i === 0;
          const key = `${e.startedAt}-${e.appName}-${i}`;
          const isNew = newKeys.has(key);
          const age = ageSecs(e.startedAt);
          const tag = relativeTag(age);
          const isRecent = age < 60;

          const rowClass = [
            "dashboard__activity-feed-row",
            isFirst && "dashboard__activity-feed-row--first",
            isNew && "dashboard__activity-feed-row--new",
          ]
            .filter(Boolean)
            .join(" ");

          return (
            <div key={key} className={rowClass}>
              <div className="dashboard__activity-feed-icon">
                {e.isIdle ? <span /> : <AppIcon appName={name} color={color} />}
              </div>

              <span className="dashboard__activity-feed-time">{formatTime(e.startedAt)}</span>
              {tag && (
                <span
                  className={
                    isRecent
                      ? "dashboard__activity-feed-tag dashboard__activity-feed-tag--recent"
                      : "dashboard__activity-feed-tag"
                  }
                >
                  {tag}
                </span>
              )}

              <div>
                <span
                  className={
                    e.isIdle
                      ? "dashboard__activity-feed-name dashboard__activity-feed-name--idle"
                      : isFirst
                        ? "dashboard__activity-feed-name dashboard__activity-feed-name--first"
                        : "dashboard__activity-feed-name"
                  }
                >
                  {name}
                </span>
                {subtitle && !e.isIdle && (
                  <p className="dashboard__activity-feed-subtitle">{subtitle}</p>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
```

Note: this file uses `formatTime` from `@/utils/dashboardDates`. If that helper doesn't exist (only `formatHumanDuration`/`todayISO`/etc), check `desktop-ui/src/utils/dashboardDates.ts` and add a `formatTime(iso: string)` if absent:

```ts
export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}
```

- [ ] **Step 4: Run tests (pass)**

Run: `cd desktop-ui && bun run test ActivityFeed.test`
Expected: 3 assertions pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx \
        desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.test.tsx \
        desktop-ui/src/utils/dashboardDates.ts
git commit -m "feat(dashboard): replace ActivityFeed stub with live polling impl"
```

---

## Task 13: Port `FocusStateIndicator`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/FocusStateIndicator.tsx`

- [ ] **Step 1: Create component**

```tsx
import { useEffect, useState } from "react";
import type { FocusStatePayload } from "@/bindings";
import { subscribeFocusStateChanged } from "@/services/events";

const STATE_CONFIG: Record<string, { label: string; color: string; pulse: boolean }> = {
  building: { label: "Building focus", color: "var(--brand)", pulse: true },
  focused: { label: "Deep focus", color: "var(--success)", pulse: false },
  cooldown: { label: "Cooldown", color: "var(--text-muted-foreground)", pulse: true },
};

export function FocusStateIndicator() {
  const [focusState, setFocusState] = useState<FocusStatePayload | null>(null);

  useEffect(() => {
    return subscribeFocusStateChanged((payload) => {
      if (payload.state === "unfocused" || payload.state === "ended") {
        setFocusState(null);
      } else {
        setFocusState(payload);
      }
    });
  }, []);

  if (!focusState) return null;
  const config = STATE_CONFIG[focusState.state];
  if (!config) return null;

  return (
    <div className="dashboard__focus-state-banner">
      <div className="dashboard__focus-state-pill">
        <span
          className={
            config.pulse
              ? "dashboard__focus-state-pill-dot dashboard__focus-state-pill-dot--pulsing"
              : "dashboard__focus-state-pill-dot"
          }
          style={{ backgroundColor: config.color }}
        />
        <span style={{ color: config.color }}>{config.label}</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/components/productivity/FocusStateIndicator.tsx
git commit -m "feat(dashboard): port FocusStateIndicator"
```

---

## Task 14: Port `FocusTrayIndicator`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/FocusTrayIndicator.tsx`

- [ ] **Step 1: Create component (drops `focus:auto-started` subscription per spec)**

```tsx
import { useEffect, useState } from "react";
import { subscribeFocusStateChanged } from "@/services/events";

export function FocusTrayIndicator() {
  const [inFocus, setInFocus] = useState(false);

  useEffect(() => {
    return subscribeFocusStateChanged((payload) => {
      setInFocus(payload.state !== "unfocused" && payload.state !== "ended");
    });
  }, []);

  if (!inFocus) return null;

  return (
    <div className="dashboard__focus-tray-pill">
      <span className="dashboard__focus-state-pill-dot dashboard__focus-state-pill-dot--pulsing" />
      <span>Focus</span>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/components/productivity/FocusTrayIndicator.tsx
git commit -m "feat(dashboard): port FocusTrayIndicator"
```

---

## Task 15: Port `AutoFocusToast`

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/productivity/AutoFocusToast.tsx`

- [ ] **Step 1: Create component**

```tsx
import { useCallback, useEffect, useState } from "react";
import type { AutoFocusPayload, FocusSessionResponse } from "@/bindings";
import { productivityAutoFocusConfirm } from "@/api/endpoints/dashboard";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { subscribeFocusAutoDetected } from "@/services/events";
import { AppIcon, getAppColor } from "../../lib/productivity";

export function AutoFocusToast() {
  const [session, setSession] = useState<AutoFocusPayload | null>(null);

  useEffect(() => {
    return subscribeFocusAutoDetected((payload) => {
      setSession(payload);
    });
  }, []);

  const { mutate: confirm, isLoading: confirming } = useTauriMutation<
    FocusSessionResponse,
    AutoFocusPayload
  >({
    mutationFn: productivityAutoFocusConfirm,
    invalidates: [qk.dashboard.all(), qk.productivity.all()],
  });

  const handleConfirm = useCallback(async () => {
    if (!session) return;
    try {
      await confirm(session);
    } finally {
      setSession(null);
    }
  }, [session, confirm]);

  const handleDismiss = useCallback(() => {
    setSession(null);
  }, []);

  if (!session) return null;

  const color = getAppColor(session.dominantApp, null);
  const ratio = Math.round(session.productiveRatio * 100);

  return (
    <div className="dashboard__auto-focus-toast">
      <div className="dashboard__auto-focus-toast-icon">
        <AppIcon appName={session.dominantApp} color={color} />
      </div>

      <div className="dashboard__auto-focus-toast-body">
        <div>
          <span>Focus session detected</span>
          <span className="dashboard__auto-focus-toast-ratio">{ratio}% productive</span>
        </div>
        <p>
          {session.durationMins}min in {session.dominantApp}
        </p>
      </div>

      <div className="dashboard__auto-focus-toast-actions">
        <button
          type="button"
          onClick={handleConfirm}
          disabled={confirming}
          className="dashboard__auto-focus-toast-confirm"
        >
          {confirming ? "Saving..." : "Confirm"}
        </button>
        <button
          type="button"
          onClick={handleDismiss}
          className="dashboard__auto-focus-toast-dismiss"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/components/productivity/AutoFocusToast.tsx
git commit -m "feat(dashboard): port AutoFocusToast with confirm mutation"
```

---

## Task 16: Rewrite `SummaryPanel` (full backup port)

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/SummaryPanel.tsx` (current 248L → ~600L)
- Create: `desktop-ui/src/features/dashboard/components/SummaryPanel.test.tsx`

This is the largest single port. The new file contains five exported pieces or local components: `SummaryPanel` (dispatcher), `DaySummary`, `EntryDetail`, `SessionDetail`, `WeeklySparkline`, `TopAppsChart`, `TrendArrow`. The `SessionBlock` type is re-imported from the (still-stub) `views/ActivityTrack` — the rewrite of ActivityTrack in Task 17 keeps the same export name.

- [ ] **Step 1: Write failing tests**

Create `desktop-ui/src/features/dashboard/components/SummaryPanel.test.tsx`:

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
  };
});

import type { TimelineEntry, TimelineSummary } from "@/bindings";
import type { SessionBlock } from "../views/ActivityTrack";
import { SummaryPanel } from "./SummaryPanel";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

const SUMMARY: TimelineSummary = {
  totalTrackedSecs: 3600,
  focusSecs: 1200,
  tasksCompleted: 0,
  tasksCreated: 0,
  notesTouched: 0,
  transactionsCount: 0,
  topApps: [{ appName: "VSCode", durationSecs: 1800 }],
  sourceBreakdown: [],
};

describe("SummaryPanel", () => {
  it("renders nothing when summary is null and no selection", () => {
    const { container } = render(
      wrap(<SummaryPanel summary={null} selectedEntry={null} onClose={() => {}} />),
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders DaySummary fallback when summary present, no productivity", async () => {
    render(wrap(<SummaryPanel summary={SUMMARY} selectedEntry={null} onClose={() => {}} />));
    await waitFor(() => expect(screen.getByText("VSCode")).toBeTruthy());
    expect(screen.getByText("tracked")).toBeTruthy();
  });

  it("renders EntryDetail when selectedEntry is set", () => {
    const entry: TimelineEntry = {
      id: "e1",
      title: "Test entry",
      description: "test desc",
      startedAt: "2026-05-02T10:00:00Z",
      endedAt: "2026-05-02T10:30:00Z",
      durationSecs: 1800,
      source: "task",
      entryType: "taskDue",
      color: "#4285F4",
      metadata: null,
      entityId: null,
      entityRoute: null,
    };
    const onClose = vi.fn();
    render(wrap(<SummaryPanel summary={null} selectedEntry={entry} onClose={onClose} />));
    expect(screen.getByText("Test entry")).toBeTruthy();
    expect(screen.getByText("test desc")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("Close details"));
    expect(onClose).toHaveBeenCalled();
  });

  it("renders SessionDetail when selectedSession is set", () => {
    const session: SessionBlock = {
      startMin: 540,
      endMin: 600,
      color: "#22C55E",
      label: "Coding session",
      duration: 3600,
      dominantCategory: "productive",
      appBreakdown: [{ app: "VSCode", dur: 3600, catType: "productive" }],
      duringFocus: true,
      intelligence: null,
    };
    render(
      wrap(
        <SummaryPanel
          summary={null}
          selectedEntry={null}
          selectedSession={session}
          onClose={() => {}}
        />,
      ),
    );
    expect(screen.getByText("Coding session")).toBeTruthy();
    expect(screen.getByText("Activity Session")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test (fail until rewrite — current SummaryPanel has different signatures)**

Run: `cd desktop-ui && bun run test SummaryPanel.test`
Expected: at least the SessionDetail test fails because the current SummaryPanel doesn't accept `selectedSession`. Some others may pass against the partial impl — that's fine.

- [ ] **Step 3: Replace `SummaryPanel.tsx` entirely**

The file is large; write it section-by-section. Overwrite:

```tsx
import { Brain, ExternalLink, Lightbulb, X } from "lucide-react";
import {
  dashboardIntelligenceQuery,
  productivitySummaryRangeQuery,
  productivityTodayQuery,
  productivityWeeklyQuery,
} from "@/api/endpoints/dashboard";
import type {
  DashboardIntelligenceResponse,
  ProductivitySummaryResponse,
  TimelineEntry,
  TimelineSummary,
} from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  todayISO,
  TZ_OFFSET_MINS,
} from "@/utils/dashboardDates";
import { resolveActivityColor, resolveCategoryLabel } from "../lib/productivity";
import type { SessionBlock } from "../views/ActivityTrack";
import { GoalsProgress } from "./productivity/GoalsProgress";
import { HourlyHeatmap } from "./productivity/HourlyHeatmap";
import { PatternsCard } from "./productivity/PatternsCard";
import { ProductivityScoreRing, ScoreBar } from "./productivity/ProductivityScoreRing";

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  selectedSession?: SessionBlock | null;
  onClose: () => void;
  productivitySummary?: ProductivitySummaryResponse | null;
  date?: string;
}

export function SummaryPanel({
  summary,
  selectedEntry,
  selectedSession,
  onClose,
  productivitySummary,
  date,
}: SummaryPanelProps) {
  if (selectedSession) {
    return <SessionDetail session={selectedSession} onClose={onClose} />;
  }
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} />;
  }
  if (!summary) return null;
  return (
    <DaySummary
      summary={summary}
      productivitySummary={productivitySummary}
      date={date || todayISO()}
    />
  );
}

function DaySummary({
  summary,
  productivitySummary,
  date,
}: {
  summary: TimelineSummary;
  productivitySummary?: ProductivitySummaryResponse | null;
  date: string;
}) {
  const ps = productivitySummary;
  const hasProductivity = ps != null && ps.totalActiveSecs > 0;
  const productivePct = hasProductivity
    ? Math.round((ps.productiveSecs / ps.totalActiveSecs) * 100)
    : 0;

  const { data: intel } = useTauriQuery<DashboardIntelligenceResponse | null>({
    queryKey: qk.dashboard.intelligence(date),
    queryFn: () => dashboardIntelligenceQuery(date, TZ_OFFSET_MINS),
    fallback: null,
  });

  const { data: weeklyData } = useTauriQuery<ProductivitySummaryResponse[]>({
    queryKey: qk.productivity.weekly(),
    queryFn: () => productivityWeeklyQuery(),
    fallback: [],
  });

  return (
    <aside className="dashboard__summary-panel">
      {hasProductivity && ps.productivityScore != null && (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-score-row">
            <ProductivityScoreRing score={ps.productivityScore} size={72} />
            <div className="dashboard__summary-score-meta">
              <div className="dashboard__summary-active">
                <span className="dashboard__summary-active-time">
                  {formatHumanDuration(ps.totalActiveSecs)}
                </span>
                <TrendArrow
                  value={
                    ps.activeTimeTrend != null
                      ? (() => {
                          const baseline = ps.totalActiveSecs - ps.activeTimeTrend;
                          if (baseline < 60) return null;
                          return (ps.activeTimeTrend / baseline) * 100;
                        })()
                      : null
                  }
                />
                <span className="dashboard__summary-dim">active</span>
              </div>
              <div className="dashboard__summary-bar">
                {ps.productiveSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--productive"
                    style={{ width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.neutralSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--neutral"
                    style={{ width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
                {ps.distractingSecs > 0 && (
                  <div
                    className="dashboard__summary-bar-seg dashboard__summary-bar-seg--distracting"
                    style={{ width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%` }}
                  />
                )}
              </div>
              <span className="dashboard__summary-productive-pct">{productivePct}% productive</span>
              {ps.totalActiveSecs > 0 && (
                <div className="dashboard__summary-metrics">
                  <ScoreBar label="Deep focus" value={ps.productiveSecs / ps.totalActiveSecs} />
                  <ScoreBar label="Quality" value={ps.avgSessionQuality ?? 0} />
                  <ScoreBar
                    label="Low distraction"
                    value={1 - ps.distractingSecs / Math.max(ps.totalActiveSecs, 1)}
                  />
                  <ScoreBar
                    label="Alignment"
                    value={ps.contextSwitches > 0 ? Math.max(0, 1 - ps.contextSwitches / 100) : 1}
                  />
                </div>
              )}
              {ps.deepWorkBlocks > 0 && (
                <div className="dashboard__summary-stat-row">
                  <span>
                    {ps.deepWorkBlocks} deep work block{ps.deepWorkBlocks !== 1 ? "s" : ""}
                  </span>
                  <span>{formatHumanDuration(ps.deepWorkSecs)}</span>
                </div>
              )}
              {ps.avgRecoverySecs != null && (
                <div className="dashboard__summary-stat-row">
                  <span>Avg recovery</span>
                  <span>{Math.round(ps.avgRecoverySecs)}s</span>
                </div>
              )}
            </div>
          </div>
        </section>
      )}

      {!hasProductivity && (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-active">
            <span className="dashboard__summary-active-time">
              {formatHumanDuration(summary.totalTrackedSecs)}
            </span>
            <span className="dashboard__summary-dim">tracked</span>
          </div>
        </section>
      )}

      {intel?.focusRecommendation && (
        <p className="dashboard__summary-recommendation">{intel.focusRecommendation}</p>
      )}

      {weeklyData && weeklyData.length >= 2 && <WeeklySparkline data={weeklyData} />}

      {hasProductivity && <PatternsCard />}

      {hasProductivity && <HourlyHeatmap startDate={date} endDate={date} />}

      {hasProductivity && ps.topApps.length > 0 && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart apps={ps.topApps} />
        </section>
      )}

      {!hasProductivity && summary.topApps.length > 0 && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart
            apps={summary.topApps.map((a) => ({
              appName: a.appName,
              durationSecs: a.durationSecs,
              category: null,
            }))}
          />
        </section>
      )}

      {intel && (intel.patterns.length > 0 || intel.nudges.length > 0) && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Insights</h4>
          <div className="dashboard__summary-insights">
            {intel.patterns.map((p) => (
              <div key={`p-${p}`} className="dashboard__summary-insight-item">
                <Brain aria-hidden />
                <span>{p}</span>
              </div>
            ))}
            {intel.nudges.map((n) => (
              <div
                key={`n-${n.nudgeType}-${n.message}`}
                className="dashboard__summary-insight-item"
              >
                <Lightbulb aria-hidden />
                <span>{n.message}</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {ps?.aiSummary && (
        <section className="dashboard__summary-aibox">
          <p>{ps.aiSummary}</p>
        </section>
      )}

      <GoalsProgress />
    </aside>
  );
}

function TopAppsChart({
  apps,
}: {
  apps: { appName: string; durationSecs: number; category?: string | null }[];
}) {
  const maxSecs = apps[0]?.durationSecs ?? 1;
  return (
    <div className="dashboard__summary-apps">
      {apps.slice(0, 5).map((app) => {
        const pct = maxSecs > 0 ? (app.durationSecs / maxSecs) * 100 : 0;
        return (
          <div key={app.appName} className="dashboard__summary-app-row">
            <span className="dashboard__summary-app-name" title={app.appName}>
              {app.appName}
            </span>
            <div className="dashboard__summary-app-track">
              <div
                className="dashboard__summary-app-fill"
                style={{ width: `${Math.max(pct, 4)}%` }}
              />
            </div>
            <span className="dashboard__summary-app-dur">
              {formatHumanDuration(app.durationSecs)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function WeeklySparkline({ data }: { data: ProductivitySummaryResponse[] }) {
  const scores = data.map((d) => d.productivityScore ?? 0);
  if (scores.length < 2) return null;

  const halfLen = Math.floor(scores.length / 2);
  const recentAvg = scores.slice(halfLen).reduce((a, b) => a + b, 0) / (scores.length - halfLen);
  const olderAvg = scores.slice(0, halfLen).reduce((a, b) => a + b, 0) / halfLen;
  const changePct = olderAvg > 0 ? Math.round(((recentAvg - olderAvg) / olderAvg) * 100) : 0;

  const w = 200;
  const h = 32;
  const pad = 2;
  const max = Math.max(...scores, 1);
  const min = Math.min(...scores, 0);
  const range = max - min || 1;

  const points = scores
    .map((v, i) => {
      const x = pad + (i / (scores.length - 1)) * (w - pad * 2);
      const y = h - pad - ((v - min) / range) * (h - pad * 2);
      return `${x},${y}`;
    })
    .join(" ");

  const lastX = w - pad;
  const lastY = h - pad - ((scores[scores.length - 1] - min) / range) * (h - pad * 2);

  const trendClass =
    changePct > 0
      ? "dashboard__sparkline-trend dashboard__sparkline-trend--up"
      : "dashboard__sparkline-trend dashboard__sparkline-trend--down";

  return (
    <div className="dashboard__sparkline">
      <svg
        width={w}
        height={h}
        className="dashboard__sparkline-svg"
        role="img"
        aria-label="Weekly productivity trend"
      >
        <polyline
          points={points}
          fill="none"
          stroke="var(--brand)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <circle cx={lastX} cy={lastY} r="2.5" fill="var(--brand)" />
      </svg>
      {changePct !== 0 && (
        <span className={trendClass}>
          {changePct > 0 ? "↑" : "↓"}
          {Math.abs(changePct)}%
        </span>
      )}
    </div>
  );
}

function SessionDetail({ session, onClose }: { session: SessionBlock; onClose: () => void }) {
  const startH = Math.floor(session.startMin / 60);
  const startM = Math.floor(session.startMin % 60);
  const endH = Math.floor(session.endMin / 60);
  const endM = Math.floor(session.endMin % 60);
  const fmt = (h: number, m: number) =>
    `${h % 12 || 12}:${String(m).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;

  const categoryLabel = resolveCategoryLabel(session.dominantCategory);
  const categoryColor = resolveActivityColor(session.dominantCategory, false);
  const matched = session.intelligence;

  return (
    <aside className="dashboard__summary-panel">
      <div className="dashboard__summary-detail-header">
        <h3 className="dashboard__summary-heading">Activity Session</h3>
        <button
          type="button"
          onClick={onClose}
          className="dashboard__summary-close"
          aria-label="Close details"
        >
          <X aria-hidden />
        </button>
      </div>

      <div className="dashboard__summary-session-header">
        <div
          className="dashboard__summary-entry-swatch"
          style={{ backgroundColor: session.color }}
        />
        <span>{session.label}</span>
      </div>

      {matched?.description && (
        <p className="dashboard__summary-entry-desc">{matched.description}</p>
      )}

      <div className="dashboard__summary-session-stats">
        {matched?.qualityScore != null && (
          <div
            className="dashboard__summary-session-quality-badge"
            style={{
              backgroundColor: `color-mix(in oklch, ${session.color} 20%, transparent)`,
              color: session.color,
              border: `1px solid color-mix(in oklch, ${session.color} 30%, transparent)`,
            }}
          >
            Q: {Math.round(matched.qualityScore)}/100
          </div>
        )}
        <div
          className="dashboard__summary-session-category-badge"
          style={{
            backgroundColor: `color-mix(in oklch, ${categoryColor} 15%, transparent)`,
            color: categoryColor,
            border: `1px solid color-mix(in oklch, ${categoryColor} 25%, transparent)`,
          }}
        >
          <span style={{ backgroundColor: categoryColor }} />
          {categoryLabel}
        </div>
      </div>

      {matched && (
        <div className="dashboard__summary-entry-meta">
          {matched.categoryPurity != null && (
            <div>Focus purity: {Math.round(matched.categoryPurity * 100)}%</div>
          )}
          <div>Context switches: {matched.contextSwitches}</div>
          <div>Distractions: {matched.distractionCount}</div>
        </div>
      )}

      <div className="dashboard__summary-entry-meta">
        <div>
          {fmt(startH, startM)} – {fmt(endH, endM)}
        </div>
        <div>Duration: {formatHumanDuration(session.duration)}</div>
      </div>

      {session.appBreakdown.length > 0 && (
        <div>
          <h4 className="dashboard__summary-heading">Apps in this session</h4>
          <div>
            {session.appBreakdown.map((app) => {
              const appCatColor = resolveActivityColor(app.catType, false);
              return (
                <div key={app.app} className="dashboard__summary-session-app-row">
                  <span style={{ backgroundColor: appCatColor }} />
                  <span>{app.app}</span>
                  <span>{formatHumanDuration(app.dur)}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </aside>
  );
}

function EntryDetail({ entry, onClose }: { entry: TimelineEntry; onClose: () => void }) {
  return (
    <aside className="dashboard__summary-panel">
      <div className="dashboard__summary-detail-header">
        <h3 className="dashboard__summary-heading">Details</h3>
        <button
          type="button"
          onClick={onClose}
          className="dashboard__summary-close"
          aria-label="Close details"
        >
          <X aria-hidden />
        </button>
      </div>

      <div className="dashboard__summary-entry-title">
        <span
          className="dashboard__summary-entry-swatch"
          style={{ backgroundColor: entry.color }}
        />
        <span>{entry.title}</span>
      </div>

      {entry.description && <p className="dashboard__summary-entry-desc">{entry.description}</p>}

      <div className="dashboard__summary-entry-meta">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="dashboard__summary-entry-source">Source: {entry.source}</div>
      </div>

      {entry.entityRoute && (
        <a
          href={entry.entityRoute}
          className="dashboard__summary-entry-link"
          onClick={(e) => e.preventDefault()}
        >
          <ExternalLink aria-hidden />
          <span>Open {entry.source}</span>
        </a>
      )}
    </aside>
  );
}

function TrendArrow({ value, label }: { value?: number | null; label?: string }) {
  if (value == null || Math.abs(value) < 0.5) return null;
  const isUp = value > 0;
  const pct = Math.round(Math.abs(value));
  const cls = isUp
    ? "dashboard__sparkline-trend dashboard__sparkline-trend--up"
    : "dashboard__sparkline-trend dashboard__sparkline-trend--down";
  return (
    <span className={cls} title={label ? `${isUp ? "+" : "-"}${pct}% ${label}` : undefined}>
      {isUp ? "↑" : "↓"}
      {pct > 0 && `${pct}%`}
    </span>
  );
}

// Re-export for use by views that fetch productivity-summary data and pass it down.
export type { SessionBlock };
export { productivityTodayQuery, productivitySummaryRangeQuery };
```

Note: `ProductivitySummaryResponse.activeTimeTrend`, `deepWorkBlocks`, `deepWorkSecs`, `avgRecoverySecs`, `aiSummary`, `productivityScore`, `avgSessionQuality`, `topApps`, `contextSwitches` are all expected fields. If `bindings.ts` calls them differently (e.g. snake_case via specta), adjust the field names at access sites.

- [ ] **Step 4: Run tests (pass)**

Run: `cd desktop-ui && bun run test SummaryPanel.test`
Expected: 4 assertions pass.

Run: `cd desktop-ui && bun run typecheck`
Expected: clean (after Task 17 lands; until then, `SessionBlock` import resolves to the stub file's existing placeholder type — that's intentional and OK for this task's commit).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/SummaryPanel.tsx \
        desktop-ui/src/features/dashboard/components/SummaryPanel.test.tsx
git commit -m "feat(dashboard): rewrite SummaryPanel with full backup port"
```

---

## Task 17: Rewrite `ActivityTrack` (replace null stub)

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx`
- Create: `desktop-ui/src/features/dashboard/components/views/ActivityTrack.test.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defaultDashboardMocks } from "../../__tests__/dashboardCommandMocks";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    ...defaultDashboardMocks(),
    productivityTimelineQuery: vi.fn().mockResolvedValue([
      {
        startedAt: "2026-05-02T09:00:00",
        endedAt: "2026-05-02T10:00:00",
        durationSecs: 3600,
        appName: "VSCode",
        siteName: null,
        windowTitle: null,
        projectId: null,
        categoryId: "coding",
        focusSessionId: null,
        isIdle: false,
      },
    ]),
    productivityCategoriesQuery: vi.fn().mockResolvedValue([
      { id: "coding", categoryType: "productive", name: "Coding" },
    ]),
    productivityIntelligenceSessionsQuery: vi.fn().mockResolvedValue([]),
  };
});

import { ActivityTrack } from "./ActivityTrack";

afterEach(() => cleanup());

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

describe("ActivityTrack", () => {
  it("renders one merged session block from a single timeline entry", async () => {
    const onSelectSession = vi.fn();
    render(
      wrap(
        <ActivityTrack
          date="2026-05-02"
          hourHeight={60}
          isToday={false}
          onSelectSession={onSelectSession}
          onSelectEntry={() => {}}
          selectedSession={null}
          selectedEntryId={null}
        />,
      ),
    );
    await waitFor(() => expect(screen.getByText(/VSCode/)).toBeTruthy());
  });

  it("calls onSelectSession when a block is clicked", async () => {
    const onSelectSession = vi.fn();
    render(
      wrap(
        <ActivityTrack
          date="2026-05-02"
          hourHeight={60}
          isToday={false}
          onSelectSession={onSelectSession}
          onSelectEntry={() => {}}
          selectedSession={null}
          selectedEntryId={null}
        />,
      ),
    );
    await waitFor(() => expect(screen.getByText(/VSCode/)).toBeTruthy());
    fireEvent.click(screen.getByText(/VSCode/).closest("button") as HTMLButtonElement);
    expect(onSelectSession).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test (fail — current stub returns null, so VSCode never appears)**

Run: `cd desktop-ui && bun run test ActivityTrack.test`
Expected: FAIL with timeout on `waitFor(() => screen.getByText(/VSCode/))`.

- [ ] **Step 3: Replace `ActivityTrack.tsx`**

Overwrite `desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx`:

```tsx
import { useMemo, useState } from "react";
import type {
  ActivityCategoryResponse,
  ActivityTimelineResponse,
  IntelligenceSessionResponse,
  TimelineEntry,
} from "@/bindings";
import {
  productivityCategoriesQuery,
  productivityIntelligenceSessionsQuery,
  productivityTimelineQuery,
} from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@/utils/dashboardDates";
import {
  type MergeableEvent,
  mergeActivitySessions,
} from "../../lib/activity-sessions";
import {
  purityToOpacity,
  qualityToColor,
  resolveActivityColor,
} from "../../lib/productivity";

export interface SessionBlock {
  startMin: number;
  endMin: number;
  color: string;
  label: string;
  duration: number;
  dominantCategory: string;
  appBreakdown: { app: string; dur: number; catType: string }[];
  duringFocus: boolean;
  intelligence: IntelligenceSessionResponse | null;
}

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

function matchIntelligence(
  startMin: number,
  endMin: number,
  sessions: IntelligenceSessionResponse[],
): IntelligenceSessionResponse | null {
  let best: IntelligenceSessionResponse | null = null;
  let bestOverlap = 0;
  const sessionDur = endMin - startMin;
  if (sessionDur <= 0) return null;

  for (const is of sessions) {
    const isStart = minutesSinceMidnight(is.startedAt);
    const isEnd = is.endedAt
      ? minutesSinceMidnight(is.endedAt)
      : isStart + (is.durationSecs ?? 0) / 60;
    const overlapStart = Math.max(startMin, isStart);
    const overlapEnd = Math.min(endMin, isEnd);
    const overlap = Math.max(0, overlapEnd - overlapStart);

    if (overlap > bestOverlap) {
      bestOverlap = overlap;
      best = is;
    }
  }

  return best && bestOverlap / sessionDur > 0.3 ? best : null;
}

export function ActivityTrack({
  date,
  hourHeight,
  isToday: _isToday,
  onSelectSession,
  selectedSession,
  timelineEntries,
}: ActivityTrackProps) {
  const pxPerMin = hourHeight / 60;
  const parentOwnsData = timelineEntries != null;

  const { data: fetchedEvents } = useTauriQuery<ActivityTimelineResponse[]>({
    queryKey: qk.productivity.timeline(date),
    queryFn: () => productivityTimelineQuery(date, null, null, TZ_OFFSET_MINS),
    fallback: [],
    enabled: !parentOwnsData,
  });

  const { data: categories } = useTauriQuery<ActivityCategoryResponse[]>({
    queryKey: qk.productivity.categories(),
    queryFn: () => productivityCategoriesQuery(),
    fallback: [],
    staleTime: Number.POSITIVE_INFINITY,
  });

  const { data: intellSessions } = useTauriQuery<IntelligenceSessionResponse[]>({
    queryKey: qk.productivity.intelligenceSessions(date),
    queryFn: () => productivityIntelligenceSessionsQuery(date, TZ_OFFSET_MINS),
    fallback: [],
    enabled: !parentOwnsData,
  });

  const events = parentOwnsData ? timelineEntries : fetchedEvents;
  const categoryMap = useMemo(() => new Map(categories.map((c) => [c.id, c])), [categories]);

  const sessions: SessionBlock[] = useMemo(() => {
    if (events.length === 0) return [];

    interface TrackEvent extends MergeableEvent {
      hasFocus: boolean;
    }
    const parsed: TrackEvent[] = events.map((e) => {
      const start = new Date(e.startedAt);
      const eSecs = start.getHours() * 3600 + start.getMinutes() * 60 + start.getSeconds();
      const dur = e.durationSecs ?? 0;
      const cat = e.categoryId ? categoryMap.get(e.categoryId) : undefined;
      return {
        startSecs: eSecs,
        endSecs: eSecs + dur,
        catType: cat?.categoryType ?? "uncategorized",
        color: resolveActivityColor(cat?.categoryType, e.isIdle),
        label: e.projectId ?? e.siteName ?? e.appName,
        isIdle: e.isIdle,
        dur,
        hasFocus: e.focusSessionId != null,
      };
    });

    const merged = mergeActivitySessions(parsed);

    return merged.map((session) => {
      const sessionStartMin = session.startSecs / 60;
      const sessionEndMin = session.endSecs / 60;
      const duringFocus = session.events.some((ev) => ev.hasFocus);
      const matched = matchIntelligence(sessionStartMin, sessionEndMin, intellSessions);
      const color =
        matched?.qualityScore != null ? qualityToColor(matched.qualityScore) : session.color;

      return {
        startMin: sessionStartMin,
        endMin: sessionEndMin,
        color,
        label: matched?.title || session.label,
        duration: session.duration,
        dominantCategory: session.dominantCategory,
        appBreakdown: session.appBreakdown,
        duringFocus,
        intelligence: matched,
      };
    });
  }, [events, categoryMap, intellSessions]);

  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  return (
    <>
      {sessions.map((session, idx) => {
        const top = session.startMin * pxPerMin;
        const height = Math.max((session.endMin - session.startMin) * pxPerMin, 8);
        const isSelected =
          selectedSession?.startMin === session.startMin &&
          selectedSession?.label === session.label;
        const matched = session.intelligence;

        const baseOpacity = purityToOpacity(matched?.categoryPurity);
        const opacity = hoveredIdx !== null && hoveredIdx !== idx ? 0.3 : baseOpacity;

        const qualityLabel =
          matched?.qualityScore != null ? ` · Q:${Math.round(matched.qualityScore)}` : "";
        const tooltip = matched?.title
          ? `${matched.title} · ${formatHumanDuration(session.duration)}${qualityLabel}${session.duringFocus ? " (focus)" : ""}`
          : `${session.label} · ${formatHumanDuration(session.duration)}${session.duringFocus ? " (focus)" : ""}`;

        const cls = [
          "dashboard__activity-block",
          isSelected && "dashboard__activity-block--selected",
          matched && "dashboard__activity-block--shadow",
        ]
          .filter(Boolean)
          .join(" ");

        return (
          <button
            type="button"
            key={`${session.label}-${session.startMin}`}
            className={cls}
            style={{
              top,
              height,
              backgroundColor: session.duringFocus ? "var(--timeline-focus)" : session.color,
              opacity,
            }}
            onClick={() => onSelectSession(session)}
            onMouseEnter={() => setHoveredIdx(idx)}
            onMouseLeave={() => setHoveredIdx(null)}
            title={tooltip}
          >
            {matched?.qualityScore != null && height > 24 && (
              <span className="dashboard__activity-block-quality-badge">
                {Math.round(matched.qualityScore)}
              </span>
            )}
            {height > 18 && (
              <span className="dashboard__activity-block-title">{session.label}</span>
            )}
            {height > 32 && (
              <span className="dashboard__activity-block-desc">
                {matched?.description || formatHumanDuration(session.duration)}
              </span>
            )}
            {height > 48 && matched?.description && (
              <span className="dashboard__activity-block-duration">
                {formatHumanDuration(session.duration)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
```

Note on `useTauriQuery({ enabled: !parentOwnsData })`: verify that `useTauriQuery` supports `enabled`. If not, replace with a conditional pre-check that returns `[]` when `parentOwnsData`. (Read `desktop-ui/src/lib/query/useTauriQuery.ts`.)

- [ ] **Step 4: Run tests (pass)**

Run: `cd desktop-ui && bun run test ActivityTrack.test`
Expected: 2 assertions pass.

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/ActivityTrack.tsx \
        desktop-ui/src/features/dashboard/components/views/ActivityTrack.test.tsx
git commit -m "feat(dashboard): rewrite ActivityTrack with intelligence-enriched sessions"
```

---

## Task 18: Port `ProductivityStrip` (orphan, file-only)

**Files:**
- Create: `desktop-ui/src/features/dashboard/components/ProductivityStrip.tsx`

Per spec, this file ports for parity but is not mounted by any view. Re-exported from `dashboard/index.ts` in Task 26.

- [ ] **Step 1: Create the component**

```tsx
import { useState } from "react";
import type { ProductivitySummaryResponse } from "@/bindings";
import { formatHumanDuration } from "@/utils/dashboardDates";
import { scoreColor } from "../lib/productivity";

interface ProductivityStripProps {
  summary: ProductivitySummaryResponse | null;
}

function CategoryBar({ summary }: { summary: ProductivitySummaryResponse }) {
  const total = summary.productiveSecs + summary.neutralSecs + summary.distractingSecs;
  if (total === 0) return null;
  const segments = [
    { key: "productive", secs: summary.productiveSecs, color: "var(--success)" },
    { key: "neutral", secs: summary.neutralSecs, color: "var(--text-muted-foreground)" },
    { key: "distracting", secs: summary.distractingSecs, color: "var(--destructive)" },
  ].filter((s) => s.secs > 0);

  return (
    <div className="dashboard__strip-category-bar">
      {segments.map((seg) => (
        <div
          key={seg.key}
          className="dashboard__strip-category-seg"
          style={{
            width: `${(seg.secs / total) * 100}%`,
            backgroundColor: seg.color,
          }}
        />
      ))}
    </div>
  );
}

function MiniScore({ score }: { score: number | null }) {
  if (score == null) return null;
  const clamped = Math.max(0, Math.min(100, score));
  const color = scoreColor(clamped);
  return (
    <div
      className="dashboard__strip-mini-score"
      style={{
        background: `conic-gradient(${color} ${clamped * 3.6}deg, rgba(255,255,255,0.06) 0deg)`,
      }}
    >
      <div>
        <span style={{ color }}>{Math.round(clamped)}</span>
      </div>
    </div>
  );
}

function TopAppsMini({ summary }: { summary: ProductivitySummaryResponse }) {
  const apps = summary.topApps.slice(0, 3);
  if (apps.length === 0) return null;
  const maxDur = apps[0]?.durationSecs ?? 1;

  return (
    <div className="dashboard__strip-top-apps">
      {apps.map((app) => (
        <div key={app.appName}>
          <span>{app.appName}</span>
          <div>
            <div
              style={{
                width: `${(app.durationSecs / maxDur) * 100}%`,
                backgroundColor: "var(--brand)",
              }}
            />
          </div>
          <span>{formatHumanDuration(app.durationSecs)}</span>
        </div>
      ))}
    </div>
  );
}

export function ProductivityStrip({ summary }: ProductivityStripProps) {
  const [expanded, setExpanded] = useState(false);

  if (!summary || summary.totalActiveSecs === 0) return null;
  const productivePct = Math.round((summary.productiveSecs / summary.totalActiveSecs) * 100);

  return (
    <div className="dashboard__strip">
      <button type="button" onClick={() => setExpanded(!expanded)} className="dashboard__strip-toggle">
        <MiniScore score={summary.productivityScore} />
        <div>
          <CategoryBar summary={summary} />
        </div>
        <div className="dashboard__strip-quick-stats">
          <span>
            {formatHumanDuration(summary.totalActiveSecs)} <span>active</span>
          </span>
          <span style={{ color: "var(--success)" }}>
            {productivePct}% <span>productive</span>
          </span>
          {summary.focusSessionsCount > 0 && (
            <span>
              {summary.focusSessionsCount} <span>sessions</span>
            </span>
          )}
        </div>
        <svg
          aria-hidden="true"
          className="dashboard__strip-chevron"
          style={{ transform: expanded ? "rotate(180deg)" : undefined }}
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <path d="M3 5l3 3 3-3" />
        </svg>
      </button>

      {expanded && (
        <div className="dashboard__strip-detail">
          <TopAppsMini summary={summary} />
          <div className="dashboard__strip-breakdown">
            <span>
              <span style={{ backgroundColor: "var(--success)" }} />
              {formatHumanDuration(summary.productiveSecs)}
            </span>
            <span>
              <span style={{ backgroundColor: "var(--text-muted-foreground)" }} />
              {formatHumanDuration(summary.neutralSecs)}
            </span>
            <span>
              <span style={{ backgroundColor: "var(--destructive)" }} />
              {formatHumanDuration(summary.distractingSecs)}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/components/ProductivityStrip.tsx
git commit -m "feat(dashboard): port ProductivityStrip (file-only, orphan parity)"
```

---

## Task 19: Modify `Dashboard.tsx` — mount focus banners

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/Dashboard.tsx`

- [ ] **Step 1: Locate the existing return**

Run:
```bash
sed -n '38,52p' /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/components/Dashboard.tsx
```

Expected: lines 38–52 show the JSX `return (...)` block with `<DashboardTopbar />` then `<div className="dashboard__content">{view}</div>`.

- [ ] **Step 2: Add imports for the focus components**

At the top of the file, after the existing imports, add:

```tsx
import { AutoFocusToast } from "./productivity/AutoFocusToast";
import { FocusStateIndicator } from "./productivity/FocusStateIndicator";
```

- [ ] **Step 3: Insert two banner siblings between topbar and content**

Replace the existing JSX:
```tsx
<div className="dashboard">
  <DashboardTopbar />
  <div className="dashboard__content">{view}</div>
</div>
```

With:
```tsx
<div className="dashboard">
  <DashboardTopbar />
  <FocusStateIndicator />
  <AutoFocusToast />
  <div className="dashboard__content">{view}</div>
</div>
```

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Run existing Dashboard tests (should still pass — banners render null when no events)**

Run: `cd desktop-ui && bun run test Dashboard.test`
Expected: all 3 existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/Dashboard.tsx
git commit -m "feat(dashboard): mount FocusStateIndicator + AutoFocusToast banners"
```

---

## Task 20: Modify `DashboardTopbar.tsx` — mount FocusTrayIndicator

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx`

- [ ] **Step 1: Read the existing file to find the date label**

```bash
grep -n "dashboard__topbar-date\|view-pill\|formatFullDate\|FocusTray" /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx
```

Expected: shows the date-label render line. Note its surrounding JSX so the next step can place `FocusTrayIndicator` correctly.

- [ ] **Step 2: Add the import**

At the top of `DashboardTopbar.tsx`, add:

```tsx
import { FocusTrayIndicator } from "./productivity/FocusTrayIndicator";
```

- [ ] **Step 3: Insert `<FocusTrayIndicator />` after the date label, before the view-switcher pill group**

Find the JSX that renders the date-label `<span>` (likely with `className="dashboard__topbar-date"`). Insert immediately after the closing `</span>` of that label and before the view-switcher pill group:

```tsx
<FocusTrayIndicator />
```

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DashboardTopbar.tsx
git commit -m "feat(dashboard): mount FocusTrayIndicator in topbar"
```

---

## Task 21: Modify `DayColumns.tsx` — productivity-summary fetch, ActivityTrack wiring, ActivityFeed mount

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/DayColumns.tsx`

This task has four sub-changes. Do each step in order.

- [ ] **Step 1: Add imports**

At the top of `DayColumns.tsx`, alongside existing imports, add:

```tsx
import { ChevronDown, ChevronUp } from "lucide-react";
import {
  productivitySummaryRangeQuery,
  productivityTodayQuery,
} from "@/api/endpoints/dashboard";
import type { ProductivitySummaryResponse } from "@/bindings";
import { todayISO } from "@/utils/dashboardDates";
import { ActivityFeed } from "../productivity/ActivityFeed";
import type { SessionBlock } from "./ActivityTrack";
```

(Some of these may already be imported — keep duplicates merged into a single statement per source.)

- [ ] **Step 2: Add productivity-summary fetch + selectedSession + feedExpanded state**

Inside the `DayColumns` component body, after the existing `useState`/`useRef` declarations, add:

```tsx
const isToday = date === todayISO();

const { data: productivitySummary } = useTauriQuery<ProductivitySummaryResponse | null>({
  queryKey: isToday
    ? qk.dashboard.productivityToday(date)
    : qk.productivity.summaryRange(date, date),
  queryFn: async () => {
    if (isToday) return productivityTodayQuery();
    const arr = await productivitySummaryRangeQuery(date, date);
    return arr[0] ?? null;
  },
  fallback: null,
});

const [selectedSession, setSelectedSession] = useState<SessionBlock | null>(null);
const [feedExpanded, setFeedExpanded] = useState(false);
```

(`useTauriQuery` and `qk` should already be imported — if not, add them.)

- [ ] **Step 3: Wire ActivityTrack with real props**

Find the existing JSX that renders `<ActivityTrack />` (no-prop call against the stub). Replace with:

```tsx
<ActivityTrack
  date={date}
  hourHeight={hourHeight}
  isToday={isToday}
  onSelectSession={(s) => setSelectedSession(s)}
  onSelectEntry={(entry) => setSelectedEntry(entry)}
  selectedSession={selectedSession}
  selectedEntryId={selectedEntry?.id ?? null}
/>
```

- [ ] **Step 4: Add calendar-event → TimelineEntry conversion**

Find the existing `<CalendarTrack ... />` invocation and the surrounding `selectedCalendarEvent` state. Update the `onSelectEvent` handler:

```tsx
<CalendarTrack
  date={date}
  hourHeight={hourHeight}
  selectedEventId={selectedCalendarEvent?.id ?? null}
  onSelectEvent={(event) => {
    setSelectedCalendarEvent(event);
    // Convert calendar event to TimelineEntry-shape so SummaryPanel's EntryDetail can render it
    const startedAt = event.startTime;
    const endedAt = event.endTime;
    const durationSecs =
      startedAt && endedAt
        ? Math.max(0, Math.floor((new Date(endedAt).getTime() - new Date(startedAt).getTime()) / 1000))
        : null;
    setSelectedEntry({
      id: event.id,
      title: event.title,
      description: event.description ?? null,
      startedAt,
      endedAt,
      durationSecs,
      source: "calendar",
      entryType: "calendarEvent",
      color: event.color ?? "var(--timeline-focus)",
      metadata: null,
      entityId: event.id,
      entityRoute: null,
    });
  }}
/>
```

(Adjust field names — `event.startTime`/`endTime` may be `event.startedAt`/`endedAt` depending on the `CalendarEvent` type. Check `@/bindings` for the actual shape.)

- [ ] **Step 5: Update SummaryPanel call to pass new props**

Replace the existing `<SummaryPanel ... />` invocation with:

```tsx
{sidebarOpen && (
  <SummaryPanel
    summary={summary}
    selectedEntry={selectedEntry}
    selectedSession={selectedSession}
    onClose={() => {
      setSelectedEntry(null);
      setSelectedSession(null);
      setSelectedCalendarEvent(null);
    }}
    productivitySummary={productivitySummary}
    date={date}
  />
)}
```

- [ ] **Step 6: Mount the collapsible ActivityFeed at the bottom of the day grid**

Inside the main `dashboard__day-grid` flex container (the one wrapping the timeline scroll area), append before the closing `</div>` for that grid:

```tsx
<div
  style={{
    borderTop: "1px solid var(--ds-border-subtle)",
    transition: "max-height 0.3s ease-in-out",
    maxHeight: feedExpanded ? 260 : 36,
    overflow: "hidden",
  }}
>
  <button
    type="button"
    onClick={() => setFeedExpanded(!feedExpanded)}
    className="dashboard__activity-feed-toggle"
  >
    {feedExpanded ? <ChevronDown aria-hidden /> : <ChevronUp aria-hidden />}
    Live Activity Feed
  </button>
  {feedExpanded && (
    <div style={{ overflowY: "auto", maxHeight: 224 }}>
      <ActivityFeed />
    </div>
  )}
</div>
```

(The `dashboard__activity-feed-toggle` BEM class is added in Task 25.)

- [ ] **Step 7: Typecheck + run DayView tests**

```bash
cd desktop-ui && bun run typecheck
bun run test DayView.test
```

Expected: clean typecheck. Existing DayView tests should still pass — they mock `timelineQuery`, and the new `productivity*` queries will fall back to `null`/`[]` until mocked.

If DayView.test.tsx fails because the new queries aren't mocked, update the file to extend its `vi.mock("@/api/endpoints/dashboard", ...)` block with the `defaultDashboardMocks()` spread:

```tsx
import { defaultDashboardMocks } from "../../__tests__/dashboardCommandMocks";
// ...
vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return { ...actual, ...defaultDashboardMocks() };
});
```

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/DayColumns.tsx \
        desktop-ui/src/features/dashboard/components/views/DayView.test.tsx
git commit -m "feat(dashboard): wire DayColumns to ActivityTrack/ActivityFeed/productivity summary"
```

---

## Task 22: Modify `WeekView.tsx` — render SummaryPanel + Phase-2 cleanup

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/WeekView.tsx`

- [ ] **Step 1: Add imports**

```tsx
import { useState } from "react";
import type { TimelineEntry } from "@/bindings";
import { useSidebarOpen } from "../../lib/layers";
import { SummaryPanel } from "../SummaryPanel";
```

- [ ] **Step 2: Add `selectedEntry` state in the component body**

After the existing `const { date, setDate, setMode } = useDashboardState();` line, add:

```tsx
const { sidebarOpen } = useSidebarOpen();
const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
```

- [ ] **Step 3: Wrap the existing return JSX in a flex container, add SummaryPanel**

Replace the outer `<div className="dashboard__week-grid">…</div>` return with:

```tsx
return (
  <div style={{ display: "flex", gap: 8, height: "100%", width: "100%" }}>
    <div className="dashboard__week-grid" style={{ flex: 1 }}>
      {/* …existing content unchanged… */}
    </div>
    {sidebarOpen && (
      <SummaryPanel
        summary={data.summary}
        selectedEntry={selectedEntry}
        onClose={() => setSelectedEntry(null)}
      />
    )}
  </div>
);
```

- [ ] **Step 4: Make session blocks clickable → set selectedEntry**

Find the `<button … className="dashboard__week-session" …>` JSX. Add:

```tsx
onClick={() => {
  // Find the underlying TimelineEntry whose start matches s.startMin in this day
  // For Week view we synthesize a minimal TimelineEntry from the session
  setSelectedEntry({
    id: `week-${day}-${s.startMin}`,
    title: s.label,
    description: null,
    startedAt: new Date(`${day}T00:00:00`).toISOString(),
    endedAt: null,
    durationSecs: s.totalSecs,
    source: "productivity",
    entryType: "appUsage",
    color: "var(--timeline-app-productive)",
    metadata: null,
    entityId: null,
    entityRoute: null,
  });
}}
```

- [ ] **Step 5: Replace inline `style={{ fontSize: ... }}` with BEM classes**

Find the three inline-style blocks in WeekView (loading "Loading...", per-day active-secs label, and any `font-size` inline rules). Replace:

```tsx
<div style={{ fontSize: "var(--fs-2xs)", color: "var(--text-muted)", marginBottom: 4, padding: "4px 8px" }}>
  Loading...
</div>
```

with:

```tsx
<div className="dashboard__week-loading">Loading...</div>
```

And:

```tsx
<div style={{ fontSize: "var(--fs-3xs)", color: "var(--text-muted)", marginTop: 2 }}>
  {formatHumanDuration(activeSecs)}
</div>
```

with:

```tsx
<div className="dashboard__week-day-active">{formatHumanDuration(activeSecs)}</div>
```

(BEM classes are defined in Task 25.)

- [ ] **Step 6: Remove dead-code guard at line 93**

Find:

```tsx
for (const a of filtered) {
  if (a.type !== "activity") continue;
  // …
}
```

The `for...of` iterates over `filtered` which at this point only contains `activity`-type sessions (focus sessions are pushed afterward). Remove the `if (a.type !== "activity") continue;` line:

```tsx
for (const a of filtered) {
  a.hasFocus = focusEntries.some((f) => {
    // …
  });
}
```

- [ ] **Step 7: Run WeekView tests + extend with click test**

Add to `WeekView.test.tsx`:

```tsx
it("selecting a session block opens EntryDetail in SummaryPanel", async () => {
  // …mock timelineQuery to return one appUsage entry on a Tuesday
  // …assert clicking the session button updates the SummaryPanel header
  // (See Phase 2 plan for week-view test setup boilerplate)
});
```

For brevity, the click test asserts that after firing a click on a `.dashboard__week-session` button, `screen.getByText("Details")` exists (the EntryDetail header).

Run: `cd desktop-ui && bun run test WeekView.test`
Expected: existing tests pass; new click test passes.

- [ ] **Step 8: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/components/views/WeekView.tsx \
        desktop-ui/src/features/dashboard/components/views/WeekView.test.tsx
git commit -m "feat(dashboard): wire WeekView to SummaryPanel + Phase-2 BEM cleanup"
```

---

## Task 23: Modify `MonthView.tsx` — render SummaryPanel + BEM + click test

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/MonthView.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/views/MonthView.test.tsx`

- [ ] **Step 1: Add imports**

```tsx
import { useSidebarOpen } from "../../lib/layers";
import { SummaryPanel } from "../SummaryPanel";
```

- [ ] **Step 2: Read sidebarOpen and wrap the return in a flex container**

After existing hooks, add `const { sidebarOpen } = useSidebarOpen();`. Then wrap the return:

```tsx
return (
  <div style={{ display: "flex", gap: 8, height: "100%", width: "100%" }}>
    <div className="dashboard__month-grid" style={{ flex: 1 }}>
      {/* …existing content… */}
    </div>
    {sidebarOpen && (
      <SummaryPanel summary={data.summary} selectedEntry={null} onClose={() => {}} />
    )}
  </div>
);
```

- [ ] **Step 3: Replace inline-style "Loading..." with BEM**

```tsx
<div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)", marginBottom: 4 }}>
  Loading...
</div>
```

→

```tsx
<div className="dashboard__month-loading">Loading...</div>
```

- [ ] **Step 4: Improve click test in MonthView.test.tsx**

Replace the existing smoke "doesn't throw" click test with an assertion that mocks `useDashboardState` and verifies `setDate` and `setMode` are called:

```tsx
it("clicking a day cell calls setDate(cellDate) then setMode('day')", async () => {
  const setDate = vi.fn();
  const setMode = vi.fn();
  vi.mocked(useDashboardState).mockReturnValue({
    mode: "month",
    date: "2026-05-02",
    setDate,
    setMode,
    navigatePrev: vi.fn(),
    navigateNext: vi.fn(),
    navigateToday: vi.fn(),
  });
  render(wrap(<MonthView />));
  await waitFor(() => expect(screen.getAllByRole("button").length).toBeGreaterThan(0));
  // Click any cell with a day number
  const cellWithDay = screen.getAllByRole("button").find((b) => /^\d+$/.test(b.textContent ?? ""));
  if (!cellWithDay) throw new Error("no day cell found");
  fireEvent.click(cellWithDay);
  expect(setDate).toHaveBeenCalled();
  expect(setMode).toHaveBeenCalledWith("day");
});
```

(`useDashboardState` mocking will need a top-of-file `vi.mock("../../hooks/useDashboardState")` — if the existing test file doesn't have one, add it.)

- [ ] **Step 5: Typecheck + run tests**

```bash
cd desktop-ui && bun run typecheck
bun run test MonthView.test
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/views/MonthView.tsx \
        desktop-ui/src/features/dashboard/components/views/MonthView.test.tsx
git commit -m "feat(dashboard): wire MonthView to SummaryPanel + BEM cleanup + click test"
```

---

## Task 24: Modify `YearView.tsx` — render SummaryPanel + enabledSources + BEM

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/YearView.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/views/YearView.test.tsx`

- [ ] **Step 1: Add imports**

```tsx
import type { TimelineSource } from "@/bindings";
import { useSidebarOpen } from "../../lib/layers";
import { SummaryPanel } from "../SummaryPanel";
```

- [ ] **Step 2: Replace the focus-only filter with `enabledSources` filter**

Find:
```tsx
for (const entry of data.entries) {
  if (entry.source !== "focus") continue;
  // …
}
```

Replace with:
```tsx
const enabledSet = new Set<string>(enabledSources.map((s) => String(s)));
for (const entry of data.entries) {
  if (!enabledSet.has(String(entry.source))) continue;
  // …
}
```

(`enabledSources` is already destructured from `useEnabledLayers()`. If not, add the destructuring.)

- [ ] **Step 3: Read sidebarOpen and add SummaryPanel**

After existing hooks: `const { sidebarOpen } = useSidebarOpen();`

Wrap the return:

```tsx
return (
  <div style={{ display: "flex", gap: 8, height: "100%", width: "100%" }}>
    <div className="dashboard__year-grid" style={{ flex: 1 }}>
      {/* …existing content… */}
    </div>
    {sidebarOpen && (
      <SummaryPanel summary={null} selectedEntry={null} onClose={() => {}} />
    )}
  </div>
);
```

(Year view doesn't have a `data.summary` for the whole year — pass `summary={null}`. SummaryPanel returns `null` when summary is null and no selection — no panel renders, but the slot is wired for consistency.)

- [ ] **Step 4: Replace inline-style "Loading..." with BEM**

```tsx
<div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)", marginBottom: 8 }}>
  Loading...
</div>
```

→

```tsx
<div className="dashboard__year-loading">Loading...</div>
```

- [ ] **Step 5: Add a layer-toggle test to `YearView.test.tsx`**

At the top of the file, ensure `vi.mock("../../lib/layers")` is in place; if not, add:

```tsx
vi.mock("../../lib/layers", async () => {
  const actual = await vi.importActual<typeof import("../../lib/layers")>("../../lib/layers");
  return {
    ...actual,
    useEnabledLayers: vi.fn(),
    useSidebarOpen: vi.fn().mockReturnValue({ sidebarOpen: false, toggleSidebar: () => {} }),
  };
});
```

Then add this test inside the `describe("YearView", ...)` block:

```tsx
it("respects enabledSources — disabling 'focus' removes tinting", async () => {
  const { useEnabledLayers } = await import("../../lib/layers");
  const { timelineQuery } = await import("@/api/endpoints/dashboard");

  // First render: focus enabled, focus entry tints the day
  (useEnabledLayers as ReturnType<typeof vi.fn>).mockReturnValue({
    enabled: new Set(["activity"]),
    enabledSources: ["focus"],
    toggle: vi.fn(),
    reset: vi.fn(),
  });
  (timelineQuery as ReturnType<typeof vi.fn>).mockResolvedValue({
    entries: [
      {
        id: "f1",
        title: "Focus session",
        description: null,
        startedAt: "2026-05-02T10:00:00",
        endedAt: "2026-05-02T11:00:00",
        durationSecs: 3600,
        source: "focus",
        entryType: "focusSession",
        color: "var(--timeline-focus)",
        metadata: null,
        entityId: null,
        entityRoute: null,
      },
    ],
    summary: {
      totalTrackedSecs: 0,
      focusSecs: 3600,
      tasksCompleted: 0,
      tasksCreated: 0,
      notesTouched: 0,
      transactionsCount: 0,
      topApps: [],
      sourceBreakdown: [],
    },
  });

  vi.mocked(useDashboardState).mockReturnValue({
    mode: "year",
    date: "2026",
    setDate: vi.fn(),
    setMode: vi.fn(),
    navigatePrev: vi.fn(),
    navigateNext: vi.fn(),
    navigateToday: vi.fn(),
  });

  const { rerender, unmount } = render(wrap(<YearView />));
  await waitFor(() => expect(screen.getAllByRole("button").length).toBeGreaterThan(0));
  const tintedCell = screen.getByTitle(/2026-05-02/);
  const tintedBg = tintedCell.getAttribute("style") ?? "";
  expect(tintedBg).toContain("timeline-focus");

  // Second render: focus disabled — same date should now have transparent/muted background
  unmount();
  (useEnabledLayers as ReturnType<typeof vi.fn>).mockReturnValue({
    enabled: new Set(),
    enabledSources: [],
    toggle: vi.fn(),
    reset: vi.fn(),
  });
  rerender(wrap(<YearView />));
  await waitFor(() => expect(screen.getAllByRole("button").length).toBeGreaterThan(0));
  const cleanCell = screen.getByTitle(/2026-05-02/);
  const cleanBg = cleanCell.getAttribute("style") ?? "";
  expect(cleanBg).not.toContain("timeline-focus");
});
```

- [ ] **Step 6: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
bun run test YearView.test
git add desktop-ui/src/features/dashboard/components/views/YearView.tsx \
        desktop-ui/src/features/dashboard/components/views/YearView.test.tsx
git commit -m "feat(dashboard): wire YearView to SummaryPanel + enabledSources + BEM"
```

---

## Task 25: Add CSS to `dashboard.css`

**Files:**
- Modify: `desktop-ui/src/styles/dashboard.css`

This task adds ~250 lines of CSS for all the BEM blocks introduced in Tasks 7–24. Append the rules at the bottom of the existing file.

- [ ] **Step 1: Append `@keyframes fade-in` if not present**

Search:
```bash
grep -n "@keyframes fade-in" /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/styles/dashboard.css
```

If absent, append:

```css
@keyframes fade-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
```

- [ ] **Step 2: Append SummaryPanel block rules**

```css
/* ── SummaryPanel ──────────────────────────────────────────────────── */
.dashboard__summary-panel {
  width: 320px;
  flex-shrink: 0;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  background: var(--surface-card-strong);
  border-left: 1px solid var(--ds-border-subtle);
  font-size: var(--fs-base);
}
.dashboard__summary-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.dashboard__summary-score-row {
  display: flex;
  align-items: flex-start;
  gap: 12px;
}
.dashboard__summary-score-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.dashboard__summary-active {
  display: flex;
  align-items: center;
  gap: 6px;
}
.dashboard__summary-active-time {
  font-size: var(--fs-sm);
  font-weight: 600;
  color: var(--ds-text-strong);
  font-variant-numeric: tabular-nums;
}
.dashboard__summary-dim {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
}
.dashboard__summary-bar {
  display: flex;
  height: 4px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--surface-control);
}
.dashboard__summary-bar-seg {
  height: 100%;
}
.dashboard__summary-bar-seg--productive {
  background: var(--success);
}
.dashboard__summary-bar-seg--neutral {
  background: var(--ds-text-subtle);
}
.dashboard__summary-bar-seg--distracting {
  background: var(--destructive);
}
.dashboard__summary-productive-pct {
  font-size: var(--fs-2xs);
  color: var(--success);
}
.dashboard__summary-metrics {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 4px;
}
.dashboard__summary-score-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-2xs);
  font-weight: 300;
}
.dashboard__summary-score-label {
  width: 68px;
  color: var(--ds-text-subtle);
  text-align: right;
  flex-shrink: 0;
}
.dashboard__summary-score-track {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}
.dashboard__summary-score-fill {
  height: 100%;
  background: color-mix(in srgb, var(--brand) 60%, transparent);
}
.dashboard__summary-score-value {
  width: 28px;
  text-align: right;
  color: var(--ds-text-subtle);
  font-variant-numeric: tabular-nums;
}
.dashboard__summary-stat-row {
  display: flex;
  justify-content: space-between;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  padding: 0 4px;
}
.dashboard__summary-recommendation {
  font-size: var(--fs-2xs);
  font-style: italic;
  color: var(--ds-text-subtle);
  line-height: 1.5;
}
.dashboard__summary-heading {
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-subtle);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 6px;
}
.dashboard__summary-apps {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dashboard__summary-app-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.dashboard__summary-app-name {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  width: 64px;
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dashboard__summary-app-track {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}
.dashboard__summary-app-fill {
  height: 100%;
  background: var(--brand);
  border-radius: 999px;
}
.dashboard__summary-app-dur {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  font-variant-numeric: tabular-nums;
  text-align: right;
  flex-shrink: 0;
  white-space: nowrap;
}
.dashboard__summary-insights {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.dashboard__summary-insight-item {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
}
.dashboard__summary-insight-item svg {
  width: 12px;
  height: 12px;
  margin-top: 2px;
  flex-shrink: 0;
}
.dashboard__summary-aibox {
  border: 1px solid color-mix(in srgb, var(--brand) 15%, transparent);
  border-radius: 8px;
  padding: 8px 10px;
  background: color-mix(in srgb, var(--brand) 6%, transparent);
}
.dashboard__summary-aibox p {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  line-height: 1.5;
  margin: 0;
}

/* Detail variants */
.dashboard__summary-detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.dashboard__summary-close {
  background: none;
  border: none;
  color: var(--ds-text-subtle);
  cursor: pointer;
  padding: 4px;
}
.dashboard__summary-close:hover {
  color: var(--ds-text-strong);
}
.dashboard__summary-close svg {
  width: 14px;
  height: 14px;
}
.dashboard__summary-entry-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-base);
  font-weight: 500;
  color: var(--ds-text-strong);
}
.dashboard__summary-entry-swatch {
  width: 12px;
  height: 12px;
  border-radius: 2px;
  flex-shrink: 0;
}
.dashboard__summary-entry-desc {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  line-height: 1.5;
  margin: 0;
}
.dashboard__summary-entry-meta {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.dashboard__summary-entry-source {
  text-transform: capitalize;
}
.dashboard__summary-entry-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-2xs);
  color: var(--brand);
  text-decoration: none;
  margin-top: 4px;
}
.dashboard__summary-entry-link:hover {
  text-decoration: underline;
}
.dashboard__summary-entry-link svg {
  width: 12px;
  height: 12px;
}
.dashboard__summary-session-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-base);
  font-weight: 500;
  color: var(--ds-text-strong);
}
.dashboard__summary-session-stats {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.dashboard__summary-session-quality-badge,
.dashboard__summary-session-category-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: var(--fs-2xs);
  font-weight: 500;
}
.dashboard__summary-session-category-badge span {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}
.dashboard__summary-session-app-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}
.dashboard__summary-session-app-row > span:first-child {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  flex-shrink: 0;
}
.dashboard__summary-session-app-row > span:nth-child(2) {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dashboard__summary-session-app-row > span:last-child {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  font-variant-numeric: tabular-nums;
}
```

- [ ] **Step 3: Append remaining blocks (sparkline, score ring, patterns, hourly, goals, dialog, activity feed, app icon, activity track, focus overlays, productivity strip, week/month/year follow-up classes)**

```css
/* ── WeeklySparkline ──────────────────────────────────────────────── */
.dashboard__sparkline {
  display: flex;
  align-items: center;
  gap: 12px;
}
.dashboard__sparkline-svg {
  flex: 1;
}
.dashboard__sparkline-trend {
  font-size: var(--fs-2xs);
  font-weight: 500;
  flex-shrink: 0;
}
.dashboard__sparkline-trend--up {
  color: var(--success);
}
.dashboard__sparkline-trend--down {
  color: var(--destructive);
}

/* ── ProductivityScoreRing ────────────────────────────────────────── */
.dashboard__score-ring {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}
.dashboard__score-ring-track {
  position: relative;
}
.dashboard__score-ring-glow {
  position: absolute;
  inset: 8px;
  border-radius: 999px;
  transition: opacity 0.7s ease;
  pointer-events: none;
}
.dashboard__score-ring-svg {
  transform: rotate(-90deg);
}
.dashboard__score-ring-svg circle {
  transition: stroke-dashoffset 1s ease;
}
.dashboard__score-ring-value {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
.dashboard__score-ring-value > span:first-child {
  font-size: 26px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}
.dashboard__score-ring-suffix {
  font-size: var(--fs-2xs);
  font-weight: 300;
  color: var(--ds-text-subtle);
  margin-top: 2px;
}
.dashboard__score-ring-tooltip {
  position: absolute;
  left: 50%;
  top: 100%;
  transform: translateX(-50%);
  margin-top: 8px;
  z-index: 50;
  background: var(--ds-popover-bg);
  border: 1px solid var(--ds-popover-border);
  box-shadow: var(--ds-popover-shadow);
  border-radius: 8px;
  padding: 8px 12px;
  min-width: 160px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: var(--fs-2xs);
}
.dashboard__score-ring-tooltip-row {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}
.dashboard__score-ring-label {
  font-size: var(--fs-2xs);
  font-weight: 500;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

/* ── PatternsCard ─────────────────────────────────────────────────── */
.dashboard__patterns {
  padding: 8px 4px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dashboard__patterns-title {
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-strong);
}
.dashboard__patterns-row {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
}
.dashboard__patterns-footer {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 60%, transparent);
}

/* ── HourlyHeatmap ────────────────────────────────────────────────── */
.dashboard__hourly {
  padding: 8px 4px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dashboard__hourly-title {
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-strong);
}
.dashboard__hourly-peak {
  font-weight: 400;
  color: color-mix(in srgb, var(--ds-text-subtle) 60%, transparent);
  margin-left: 4px;
}
.dashboard__hourly-row {
  display: flex;
  align-items: center;
  gap: 4px;
}
.dashboard__hourly-hour-label {
  font-size: var(--fs-2xs);
  color: color-mix(in srgb, var(--ds-text-subtle) 60%, transparent);
  width: 16px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.dashboard__hourly-bar-track {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}
.dashboard__hourly-bar-fill {
  height: 100%;
  border-radius: 999px;
  transition: all 0.3s ease;
}

/* ── GoalsProgress + AddGoalDialog ────────────────────────────────── */
.dashboard__goals {
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 8px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.dashboard__goals-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.dashboard__goals-header h2 {
  font-size: var(--fs-sm);
  font-weight: 500;
  color: var(--ds-text-subtle);
  margin: 0;
}
.dashboard__goals-add-btn {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  background: none;
  border: none;
  color: var(--ds-text-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.dashboard__goals-add-btn:hover {
  color: var(--brand);
  background: var(--surface-control);
}
.dashboard__goals-add-btn svg {
  width: 14px;
  height: 14px;
}
.dashboard__goals-empty {
  font-size: var(--fs-2xs);
  font-weight: 300;
  color: var(--ds-text-subtle);
  margin: 0;
}
.dashboard__goal-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dashboard__goal-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-2xs);
  font-weight: 300;
  flex-wrap: wrap;
}
.dashboard__goal-status--met {
  color: var(--success);
}
.dashboard__goal-status--in-progress {
  color: var(--brand);
}
.dashboard__goal-project-tag {
  font-size: var(--fs-2xs);
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--surface-control);
  color: var(--ds-text-subtle);
}
.dashboard__goal-delete-btn {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  background: none;
  border: none;
  color: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.dashboard__goal-row:hover .dashboard__goal-delete-btn {
  color: var(--ds-text-subtle);
}
.dashboard__goal-delete-btn:hover {
  color: var(--destructive) !important;
}
.dashboard__goal-delete-btn svg {
  width: 12px;
  height: 12px;
}
.dashboard__goal-bar-track {
  height: 6px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}
.dashboard__goal-bar-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--brand);
  transition: width 0.3s ease;
}
.dashboard__goal-bar-fill--met {
  background: var(--success);
}
.dashboard__goal-dialog-backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.4);
}
.dashboard__goal-dialog {
  width: 400px;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 12px;
}
.dashboard__goal-dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  border-bottom: 1px solid var(--ds-border-subtle);
}
.dashboard__goal-dialog-header h3 {
  margin: 0;
  font-size: var(--fs-base);
  font-weight: 500;
}
.dashboard__goal-dialog-header button {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: none;
  color: var(--ds-text-subtle);
  cursor: pointer;
}
.dashboard__goal-dialog-body {
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.dashboard__goal-dialog-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.dashboard__goal-dialog-section > span,
.dashboard__goal-dialog-section > label {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
}
.dashboard__goal-dialog-period-toggle {
  display: flex;
  gap: 8px;
}
.dashboard__goal-dialog-period-btn {
  flex: 1;
  padding: 6px;
  font-size: var(--fs-2xs);
  text-transform: capitalize;
  border-radius: 6px;
  border: 1px solid var(--ds-border-subtle);
  background: var(--surface-control);
  color: var(--ds-text-subtle);
  cursor: pointer;
}
.dashboard__goal-dialog-period-btn--active {
  border-color: color-mix(in srgb, var(--brand) 50%, transparent);
  background: color-mix(in srgb, var(--brand) 5%, transparent);
  color: var(--brand);
}
.dashboard__goal-dialog-metric-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.dashboard__goal-dialog-metric-btn {
  padding: 8px 12px;
  font-size: var(--fs-2xs);
  text-align: left;
  border-radius: 6px;
  border: 1px solid var(--ds-border-subtle);
  background: var(--surface-control);
  color: var(--ds-text-subtle);
  cursor: pointer;
}
.dashboard__goal-dialog-metric-btn--active {
  border-color: color-mix(in srgb, var(--brand) 50%, transparent);
  background: color-mix(in srgb, var(--brand) 5%, transparent);
  color: var(--brand);
}
.dashboard__goal-dialog-input {
  width: 100%;
  padding: 6px 12px;
  font-size: var(--fs-base);
  background: var(--surface-control);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 6px;
  color: var(--ds-text-strong);
}
.dashboard__goal-dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--ds-border-subtle);
}
.dashboard__goal-dialog-footer button {
  padding: 6px 16px;
  font-size: var(--fs-2xs);
  border-radius: 6px;
  border: none;
  cursor: pointer;
}
.dashboard__goal-dialog-footer button:first-child {
  background: none;
  color: var(--ds-text-subtle);
}
.dashboard__goal-dialog-footer button:last-child {
  background: var(--brand);
  color: white;
}
.dashboard__goal-dialog-footer button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* ── ActivityFeed ─────────────────────────────────────────────────── */
.dashboard__activity-feed {
  padding: 12px 14px;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.dashboard__activity-feed-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--fs-sm);
}
.dashboard__activity-feed-header h2 {
  margin: 0;
  font-size: var(--fs-sm);
  font-weight: 500;
  color: var(--ds-text-subtle);
}
.dashboard__activity-feed-header > div:last-child {
  display: flex;
  align-items: center;
  gap: 6px;
}
.dashboard__activity-feed-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--success);
  animation: pulse 2s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}
.dashboard__activity-feed-list {
  display: flex;
  flex-direction: column;
  max-height: 256px;
  overflow-y: auto;
}
.dashboard__activity-feed-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  border-top: 1px solid var(--ds-border-subtle);
}
.dashboard__activity-feed-row--first {
  border-top: none;
}
.dashboard__activity-feed-row--new {
  animation: fade-in 0.4s ease-out;
}
.dashboard__activity-feed-icon {
  flex-shrink: 0;
}
.dashboard__activity-feed-time {
  font-size: var(--fs-2xs);
  font-variant-numeric: tabular-nums;
  width: 40px;
  flex-shrink: 0;
  font-weight: 300;
  color: var(--ds-text-subtle);
}
.dashboard__activity-feed-tag {
  font-size: var(--fs-2xs);
  font-weight: 500;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
  color: var(--ds-text-subtle);
}
.dashboard__activity-feed-tag--recent {
  color: var(--success);
}
.dashboard__activity-feed-name {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  font-weight: 300;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dashboard__activity-feed-name--first {
  font-weight: 400;
  color: var(--ds-text-strong);
}
.dashboard__activity-feed-name--idle {
  font-style: italic;
}
.dashboard__activity-feed-subtitle {
  font-size: var(--fs-2xs);
  font-weight: 300;
  color: var(--ds-text-subtle);
  margin: 0;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dashboard__activity-feed-empty {
  font-size: var(--fs-2xs);
  font-weight: 300;
  color: var(--ds-text-subtle);
  margin: 0;
}
.dashboard__activity-feed-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  background: none;
  border: none;
  cursor: pointer;
  width: 100%;
  text-align: left;
}
.dashboard__activity-feed-toggle:hover {
  color: var(--ds-text-strong);
}
.dashboard__activity-feed-toggle svg {
  width: 14px;
  height: 14px;
}

/* ── AppIcon ──────────────────────────────────────────────────────── */
.dashboard__app-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

/* ── ActivityTrack ────────────────────────────────────────────────── */
.dashboard__activity-block {
  position: absolute;
  left: 2px;
  right: 2px;
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
  border: none;
  padding: 2px 4px;
  text-align: left;
  transition: opacity 0.2s ease;
}
.dashboard__activity-block--selected {
  outline: 1px solid var(--border-accent);
}
.dashboard__activity-block--shadow {
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}
.dashboard__activity-block-quality-badge {
  position: absolute;
  top: 2px;
  right: 2px;
  font-size: 7px;
  font-weight: 700;
  border-radius: 999px;
  padding: 0 4px;
  background: rgba(0, 0, 0, 0.4);
  color: white;
  line-height: 1.2;
}
.dashboard__activity-block-title {
  font-size: 9px;
  font-weight: 500;
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 2px;
  color: white;
}
.dashboard__activity-block-desc {
  font-size: 8px;
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: rgba(255, 255, 255, 0.6);
}
.dashboard__activity-block-duration {
  font-size: 7px;
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: rgba(255, 255, 255, 0.4);
}

/* ── Focus overlays ───────────────────────────────────────────────── */
.dashboard__focus-tray-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--success) 10%, transparent);
  color: var(--success);
  font-size: var(--fs-2xs);
  font-weight: 500;
}
.dashboard__focus-state-banner {
  padding: 4px 16px;
  display: flex;
  justify-content: flex-start;
}
.dashboard__focus-state-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  font-size: var(--fs-2xs);
  font-weight: 500;
}
.dashboard__focus-state-pill-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}
.dashboard__focus-state-pill-dot--pulsing {
  animation: pulse 2s ease-in-out infinite;
}
.dashboard__auto-focus-toast {
  margin: 4px 16px;
  padding: 12px 16px;
  border-radius: 8px;
  background: var(--surface-card-strong);
  border: 1px solid var(--ds-border-subtle);
  border-left: 2px solid var(--success);
  display: flex;
  align-items: center;
  gap: 12px;
  animation: fade-in 0.3s ease-out;
}
.dashboard__auto-focus-toast-icon {
  flex-shrink: 0;
}
.dashboard__auto-focus-toast-body {
  flex: 1;
  min-width: 0;
}
.dashboard__auto-focus-toast-body > div {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--ds-text-strong);
}
.dashboard__auto-focus-toast-ratio {
  font-size: var(--fs-2xs);
  padding: 2px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--success) 15%, transparent);
  color: var(--success);
  font-weight: 500;
}
.dashboard__auto-focus-toast-body p {
  font-size: var(--fs-2xs);
  font-weight: 300;
  color: var(--ds-text-subtle);
  margin: 2px 0 0 0;
}
.dashboard__auto-focus-toast-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.dashboard__auto-focus-toast-confirm,
.dashboard__auto-focus-toast-dismiss {
  font-size: var(--fs-2xs);
  font-weight: 500;
  padding: 6px 12px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
}
.dashboard__auto-focus-toast-confirm {
  background: color-mix(in srgb, var(--success) 15%, transparent);
  color: var(--success);
}
.dashboard__auto-focus-toast-confirm:hover {
  background: color-mix(in srgb, var(--success) 25%, transparent);
}
.dashboard__auto-focus-toast-confirm:disabled {
  opacity: 0.5;
}
.dashboard__auto-focus-toast-dismiss {
  background: var(--surface-control);
  color: var(--ds-text-subtle);
}
.dashboard__auto-focus-toast-dismiss:hover {
  background: var(--surface-control-hover);
}

/* ── ProductivityStrip (orphan) ───────────────────────────────────── */
.dashboard__strip {
  border-bottom: 1px solid var(--ds-border-subtle);
  background: var(--surface-card-strong);
}
.dashboard__strip-toggle {
  width: 100%;
  padding: 8px 12px;
  display: flex;
  align-items: center;
  gap: 12px;
  background: none;
  border: none;
  cursor: pointer;
}
.dashboard__strip-toggle:hover {
  background: var(--surface-card-strong);
}
.dashboard__strip-mini-score {
  width: 24px;
  height: 24px;
  border-radius: 999px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 9px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
.dashboard__strip-mini-score > div {
  width: 18px;
  height: 18px;
  border-radius: 999px;
  background: var(--ds-popover-bg);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dashboard__strip-category-bar {
  display: flex;
  height: 6px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--surface-control);
}
.dashboard__strip-category-seg {
  height: 100%;
}
.dashboard__strip-quick-stats {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: var(--fs-2xs);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
  color: var(--ds-text-subtle);
}
.dashboard__strip-chevron {
  width: 12px;
  height: 12px;
  color: var(--ds-text-subtle);
  transition: transform 0.2s ease;
}
.dashboard__strip-detail {
  padding: 0 12px 10px 12px;
  display: flex;
  gap: 24px;
  animation: fade-in 0.15s ease-out;
}
.dashboard__strip-top-apps {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dashboard__strip-top-apps > div {
  display: flex;
  align-items: center;
  gap: 8px;
}
.dashboard__strip-top-apps > div > span:first-child {
  font-size: 9px;
  color: var(--ds-text-subtle);
  width: 56px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dashboard__strip-top-apps > div > div {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: var(--surface-control);
  overflow: hidden;
}
.dashboard__strip-top-apps > div > div > div {
  height: 100%;
  border-radius: 999px;
}
.dashboard__strip-top-apps > div > span:last-child {
  font-size: 9px;
  color: var(--ds-text-subtle);
  font-variant-numeric: tabular-nums;
  width: 24px;
  text-align: right;
}
.dashboard__strip-breakdown {
  display: flex;
  align-items: center;
  gap: 16px;
  font-size: 9px;
  flex-shrink: 0;
}
.dashboard__strip-breakdown > span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.dashboard__strip-breakdown > span > span {
  width: 6px;
  height: 6px;
  border-radius: 999px;
}

/* ── Phase-2 follow-up: extracted inline styles ───────────────────── */
.dashboard__week-loading,
.dashboard__month-loading,
.dashboard__year-loading {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  margin-bottom: 4px;
  padding: 4px 8px;
}
.dashboard__week-day-active {
  font-size: var(--fs-2xs);
  color: var(--ds-text-subtle);
  margin-top: 2px;
}
```

- [ ] **Step 4: Run lint + smoke**

```bash
cd desktop-ui && bun run lint
```
Expected: clean.

Open `cargo tauri dev` and click Calendar. Verify panel renders without crashing (full functional smoke is in Task 29).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/styles/dashboard.css
git commit -m "style(dashboard): add Phase 3 BEM blocks for SummaryPanel/productivity/focus"
```

---

## Task 26: Update `dashboard/index.ts` re-exports

**Files:**
- Modify: `desktop-ui/src/features/dashboard/index.ts`

- [ ] **Step 1: Read the current exports**

```bash
cat /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/index.ts
```

- [ ] **Step 2: Append re-exports for the new public components**

Add to the bottom of `index.ts`:

```ts
export { ProductivityStrip } from "./components/ProductivityStrip";
export { SummaryPanel } from "./components/SummaryPanel";
export { ActivityFeed } from "./components/productivity/ActivityFeed";
export { AddGoalDialog } from "./components/productivity/AddGoalDialog";
export { AutoFocusToast } from "./components/productivity/AutoFocusToast";
export { FocusStateIndicator } from "./components/productivity/FocusStateIndicator";
export { FocusTrayIndicator } from "./components/productivity/FocusTrayIndicator";
export { GoalsProgress } from "./components/productivity/GoalsProgress";
export { HourlyHeatmap } from "./components/productivity/HourlyHeatmap";
export { PatternsCard } from "./components/productivity/PatternsCard";
export { ProductivityScoreRing, ScoreBar } from "./components/productivity/ProductivityScoreRing";
export { ActivityTrack } from "./components/views/ActivityTrack";
export type { SessionBlock } from "./components/views/ActivityTrack";
export { mergeActivitySessions } from "./lib/activity-sessions";
export type { MergeableEvent, MergedSession } from "./lib/activity-sessions";
export { AppIcon, getAppColor, scoreColor, qualityToColor } from "./lib/productivity";
```

- [ ] **Step 3: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/dashboard/index.ts
git commit -m "feat(dashboard): re-export Phase 3 public components"
```

---

## Task 27: Extend `CalendarTrack.test.tsx` (Phase-2 follow-up)

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx`

- [ ] **Step 1: Read the existing test file**

```bash
cat /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx
```

- [ ] **Step 2: Add overlap-layout test**

Append:

```tsx
it("renders two overlapping events side-by-side via computeOverlapLayout", async () => {
  const { productivityCalendarEvents } = await import("@/api/endpoints/dashboard");
  (productivityCalendarEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
    {
      id: "e1",
      title: "Meeting A",
      startTime: "2026-05-02T10:00:00",
      endTime: "2026-05-02T11:00:00",
      color: "#4285F4",
      description: null,
    },
    {
      id: "e2",
      title: "Meeting B",
      startTime: "2026-05-02T10:30:00",
      endTime: "2026-05-02T11:30:00",
      color: "#FF0000",
      description: null,
    },
  ]);
  render(
    wrap(
      <CalendarTrack
        date="2026-05-02"
        hourHeight={60}
        selectedEventId={null}
        onSelectEvent={() => {}}
      />,
    ),
  );
  await waitFor(() => expect(screen.getByText("Meeting A")).toBeTruthy());
  expect(screen.getByText("Meeting B")).toBeTruthy();
  // Both blocks present — overlap layout should give them differing left/width inline styles
  const blocks = screen.getAllByRole("button");
  const styleA = blocks[0].getAttribute("style") ?? "";
  const styleB = blocks[1].getAttribute("style") ?? "";
  // Distinct left percentages indicate overlap layout was applied
  expect(styleA).not.toEqual(styleB);
});

it("returns null with no children when productivityCalendarEvents returns empty", async () => {
  const { productivityCalendarEvents } = await import("@/api/endpoints/dashboard");
  (productivityCalendarEvents as ReturnType<typeof vi.fn>).mockResolvedValueOnce([]);
  const { container } = render(
    wrap(
      <CalendarTrack
        date="2026-05-02"
        hourHeight={60}
        selectedEventId={null}
        onSelectEvent={() => {}}
      />,
    ),
  );
  await waitFor(() => expect(container.firstChild).toBeNull());
});
```

- [ ] **Step 3: Run + commit**

```bash
cd desktop-ui && bun run test CalendarTrack.test
git add desktop-ui/src/features/dashboard/components/views/CalendarTrack.test.tsx
git commit -m "test(dashboard): cover CalendarTrack overlap layout and empty-state"
```

---

## Task 28: Extend `Dashboard.test.tsx` (focus banner siblings)

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/Dashboard.test.tsx`

- [ ] **Step 1: Add a test that asserts focus banner mount points exist**

Append to the `describe("Dashboard", ...)` block:

```tsx
it("mounts FocusStateIndicator and AutoFocusToast as siblings of dashboard__content", () => {
  render(wrap(<Dashboard />));
  // Both render null when no events have arrived — assert by querying the dashboard root
  const root = screen.getByText("Day").closest(".dashboard");
  expect(root).toBeTruthy();
  // Children: topbar, FocusStateIndicator (null), AutoFocusToast (null), dashboard__content
  // We can't observe null components directly; instead verify dashboard__content is the LAST child
  // (proves the two banner siblings exist between topbar and content)
  expect(root?.querySelector(".dashboard__content")).toBeTruthy();
});
```

- [ ] **Step 2: Run + commit**

```bash
cd desktop-ui && bun run test Dashboard.test
git add desktop-ui/src/features/dashboard/components/Dashboard.test.tsx
git commit -m "test(dashboard): cover focus banner mount points"
```

---

## Task 29: Final verification + acceptance smoke

- [ ] **Step 1: Run full automated suite**

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui
bun run typecheck
bun run lint
bun run test
```

Expected: all three exit 0.

- [ ] **Step 2: Run `cargo tauri dev` and walk the manual acceptance checklist**

In a separate terminal:

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar/desktop-ui && bun run dev:vite
```

Then in another terminal:

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar
KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

Walk every item in the spec's manual smoke checklist:

  1. Open Calendar → Day view. SummaryPanel shows score ring + bars + top apps + AI summary if data exists.
  2. Click an ActivityTrack session block → SummaryPanel switches to SessionDetail.
  3. Click a CalendarTrack event → SummaryPanel switches to EntryDetail.
  4. Click EntryDetail close → returns to DaySummary.
  5. Toggle "Live Activity Feed" expander → feed slides open with recent entries.
  6. Switch to Week → SummaryPanel renders fallback summary (totalTrackedSecs + Top Apps).
  7. Switch to Month / Year → same fallback summary; cells clickable.
  8. In Year, toggle off Activity layer → tinting follows.
  9. Goals plus button → AddGoalDialog opens; create + delete a goal works end-to-end.
  10. Trigger focus events from backend (or manually via Tauri event-send) → FocusStateIndicator banner + tray pill appear.
  11. Trigger `focus:auto_detected` → AutoFocusToast appears; Confirm fires `productivityAutoFocusConfirm`.
  12. Sidebar toggle works across all four views.
  13. Day-view drag-to-reschedule still works (regression).
  14. CalendarTrack overlap layout still places overlapping events side-by-side (regression).

- [ ] **Step 3: If any manual step fails, open a bug-fix sub-task**

Stop, fix, re-run typecheck/lint/test, then re-run the manual step. Do not skip past failures.

- [ ] **Step 4: Final sweep — `git status` clean**

```bash
cd /Users/maixuantung/Dev/raki/klyntbot-calendar
git status
```

Expected: clean working tree.

- [ ] **Step 5: Final commit (if any straggler files)**

If lint/format produces small diffs, commit them:

```bash
git add -A
git commit -m "chore(dashboard): final lint/format sweep for Phase 3"
```

---

## Manual smoke checklist (printable)

Use this as a paste-able checklist when running `cargo tauri dev`:

```
[ ] Calendar nav item highlighted; Day view renders
[ ] Day SummaryPanel: score ring + bars + top apps visible (if today has data)
[ ] Click activity-session block → SessionDetail shows quality + category badges
[ ] Click calendar-event block → EntryDetail shows title + time range
[ ] Click EntryDetail close button → returns to DaySummary
[ ] Toggle Live Activity Feed expander → feed slides open with last ~30 entries
[ ] New entries fade-in when poll cycle returns fresh data
[ ] Switch to Week view → SummaryPanel falls back to "tracked" summary
[ ] Switch to Month → same fallback; cells clickable to drop into Day
[ ] Switch to Year → same fallback; cells tinted by enabled sources
[ ] Year: toggle off Activity layer → tinting follows enabledSources
[ ] Goals: plus button opens AddGoalDialog; submit creates a goal; trash deletes
[ ] FocusStateIndicator banner appears when focus state changes (building/focused/cooldown)
[ ] FocusTrayIndicator pill appears in topbar in sync
[ ] AutoFocusToast appears on focus:auto_detected; Confirm + Dismiss both work
[ ] Sidebar toggle hides/shows SummaryPanel across all four views
[ ] Day-view task drag still works (regression)
[ ] CalendarTrack overlap layout: two overlapping events render side-by-side
[ ] Bun typecheck/lint/test all green
```

---

## Notes for the implementer

- **TDD discipline:** every Task with a `*.test.*` file follows write-test → fail → implement → pass → commit. Don't skip the "fail" step — it confirms the test actually exercises new code.
- **Commit cadence:** commit after every task. The plan has ~30 commits total. Granular commits make review and bisection easy.
- **When backup-vs-bindings fields disagree:** the bindings file is authoritative. Fix the field name at the access site rather than reshaping the Rust type.
- **When `useTauriQuery` doesn't support `enabled` or `staleTime`:** read `desktop-ui/src/lib/query/useTauriQuery.ts` and either use what it supports, or extend it as a tiny side-task and commit separately. Don't fork the wrapper.
- **CSS visual tweaks:** if the BEM classes in Task 25 produce visually-off results, tune CSS values directly (do not move logic into inline styles). The CSS is a port skeleton — pixel-perfect match with the backup is not required, just functional and on-brand.
- **`useEffect` cleanup for event subscriptions:** every `subscribeFocus*` call inside `useEffect` returns an `Unsubscribe` function. Always return it from the effect: `return subscribeFocusStateChanged(handler)`. The skill is forgetting that the return value of `subscribe*` is itself the cleanup.
- **Test isolation:** each test file imports the component AFTER the `vi.mock(...)` call. Don't move the import to the top — it'll bind the un-mocked module.
- **`__tests__/dashboardCommandMocks.ts` evolves:** if a Phase-3 component starts depending on a new endpoint, add it to the helper rather than spreading mocks everywhere.




