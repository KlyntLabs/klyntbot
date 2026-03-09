# Layered Activity Calendar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade day/week calendar views to a layered container model (focused/unfocused containers with nested task, app, and point event layers), add a layer toggle UI, and update month/year views with focus-hours coloring.

**Architecture:** No backend changes. The existing `timeline_query` with `sources` filter provides all data. New client-side logic groups entries into focused/unfocused containers and renders them as nested layers. Layer visibility persists to `localStorage`.

**Tech Stack:** React + TypeScript, Tailwind CSS v4, existing `useQuery` hook, existing `TimelineEntry` / `TimelineSource` types.

---

### Task 1: Add Color Tokens for Layered Calendar

**Files:**
- Modify: `desktop-ui/src/styles/theme.css:L47-L55` (`:root` variables) and `L67-L85` (`@theme inline` block)

**Step 1: Add new CSS variables to `:root`**

Add after the existing `--timeline-system` variable (~L55):

```css
  --timeline-focus-high: oklch(0.58 0.22 290);
  --timeline-focus-low: oklch(0.72 0.12 290);
  --timeline-unfocused: rgba(255, 255, 255, 0.03);
  --timeline-dot-note: oklch(0.70 0.14 250);
  --timeline-dot-task-done: oklch(0.65 0.18 155);
  --timeline-dot-finance: oklch(0.78 0.16 85);
```

**Step 2: Register in `@theme inline`**

Add after the existing `--color-timeline-system` registration:

```css
  --color-timeline-focus-high: var(--timeline-focus-high);
  --color-timeline-focus-low: var(--timeline-focus-low);
  --color-timeline-unfocused: var(--timeline-unfocused);
  --color-timeline-dot-note: var(--timeline-dot-note);
  --color-timeline-dot-task-done: var(--timeline-dot-task-done);
  --color-timeline-dot-finance: var(--timeline-dot-finance);
```

**Step 3: Verify build**

Run: `cd desktop-ui && bun run build`
Expected: Clean build, no errors.

**Step 4: Commit**

```
feat(dashboard): add color tokens for layered calendar
```

---

### Task 2: Create Layer Toggle Hook and Types

**Files:**
- Create: `desktop-ui/src/components/dashboard/layers.ts`

This module exports the layer configuration, types, and a hook for persisting layer visibility to localStorage.

**Step 1: Create the layers module**

```typescript
import { useState } from "react";
import type { TimelineSource } from "../../lib/types";

export type LayerKey = "focus" | "tasks" | "apps" | "events";

export interface LayerConfig {
  key: LayerKey;
  label: string;
  /** TimelineSource values this layer maps to */
  sources: TimelineSource[];
  defaultOn: boolean;
  color: string;
  /** If true, the layer is not yet implemented */
  comingSoon?: boolean;
}

export const LAYERS: LayerConfig[] = [
  { key: "focus", label: "Focus Sessions", sources: ["focus"], defaultOn: true, color: "var(--timeline-focus)" },
  { key: "tasks", label: "Task Time Entries", sources: ["task"], defaultOn: true, color: "var(--timeline-task)" },
  { key: "apps", label: "App Activity", sources: ["productivity"], defaultOn: true, color: "var(--timeline-app-neutral)" },
  { key: "events", label: "Point Events", sources: ["note", "finance", "system"], defaultOn: true, color: "var(--timeline-note)" },
  { key: "calendar" as LayerKey, label: "Calendar Events", sources: [], defaultOn: false, color: "var(--timeline-system)", comingSoon: true },
];

const STORAGE_KEY = "dashboard-layers";

function defaultEnabled(): Set<LayerKey> {
  return new Set(LAYERS.filter((l) => l.defaultOn).map((l) => l.key));
}

function loadEnabled(): Set<LayerKey> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultEnabled();
    const arr = JSON.parse(raw) as LayerKey[];
    return new Set(arr);
  } catch {
    return defaultEnabled();
  }
}

export function useLayerToggle() {
  const [enabled, setEnabled] = useState<Set<LayerKey>>(loadEnabled);

  const toggle = (key: LayerKey) => {
    setEnabled((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      localStorage.setItem(STORAGE_KEY, JSON.stringify([...next]));
      return next;
    });
  };

  const reset = () => {
    const defaults = defaultEnabled();
    setEnabled(defaults);
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...defaults]));
  };

  /** Flat list of TimelineSource values for enabled layers, to pass to timeline_query */
  const enabledSources: TimelineSource[] = LAYERS
    .filter((l) => enabled.has(l.key) && !l.comingSoon)
    .flatMap((l) => l.sources);

  return { enabled, toggle, reset, enabledSources };
}
```

**Step 2: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 3: Commit**

```
feat(dashboard): add layer toggle hook and configuration
```

---

### Task 3: Build Container Logic

**Files:**
- Create: `desktop-ui/src/components/dashboard/buildContainers.ts`

Pure function that groups `TimelineEntry[]` into focused/unfocused containers. This is the core algorithm — no React dependencies, fully testable.

**Step 1: Create the container builder**

```typescript
import { minutesSinceMidnight } from "../../lib/dates";
import type { TimelineEntry } from "../../lib/types";

export interface ActivityContainer {
  id: string;
  type: "focused" | "unfocused";
  startMin: number;
  endMin: number;
  /** The focus session entry (only for focused containers) */
  focusSession?: TimelineEntry;
  /** quality_score from focus session metadata, 0-10 */
  qualityScore: number;
  taskEntries: TimelineEntry[];
  appActivity: TimelineEntry[];
  pointEvents: TimelineEntry[];
}

const POINT_EVENT_TYPES = new Set([
  "taskCreated", "taskCompleted", "taskUpdated",
  "noteCreated", "noteUpdated",
  "transactionRecorded", "expenseRecorded", "incomeRecorded",
  "systemEvent",
]);

function entryRange(entry: TimelineEntry): { start: number; end: number } {
  const start = minutesSinceMidnight(entry.startedAt);
  const end = entry.durationSecs ? start + entry.durationSecs / 60 : start;
  return { start, end };
}

/**
 * Group timeline entries into focused/unfocused containers for layered rendering.
 *
 * Algorithm:
 * 1. Separate focus sessions (containers) from inner entries (task, app, point events)
 * 2. Sort focus sessions by start time, merge overlapping ones
 * 3. Find the day's activity range from all duration entries
 * 4. Fill gaps between focus sessions with unfocused containers
 * 5. Assign each inner entry to its enclosing container by timestamp
 */
export function buildContainers(entries: TimelineEntry[]): ActivityContainer[] {
  // Separate by role
  const focusSessions: TimelineEntry[] = [];
  const taskEntries: TimelineEntry[] = [];
  const appActivity: TimelineEntry[] = [];
  const pointEvents: TimelineEntry[] = [];

  for (const e of entries) {
    if (e.entryType === "focusSession") focusSessions.push(e);
    else if (e.entryType === "taskTimeEntry") taskEntries.push(e);
    else if (e.entryType === "appUsage") appActivity.push(e);
    else if (POINT_EVENT_TYPES.has(e.entryType)) pointEvents.push(e);
  }

  // Sort focus sessions by start time
  focusSessions.sort(
    (a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime(),
  );

  // Build focused containers
  const focused: ActivityContainer[] = focusSessions.map((s, i) => {
    const { start, end } = entryRange(s);
    const quality = typeof s.metadata?.qualityScore === "number"
      ? (s.metadata.qualityScore as number)
      : 5;
    return {
      id: `focus-${i}`,
      type: "focused" as const,
      startMin: start,
      endMin: Math.max(end, start + 1),
      focusSession: s,
      qualityScore: quality,
      taskEntries: [],
      appActivity: [],
      pointEvents: [],
    };
  });

  // Find the day's activity range from all duration entries
  let dayStart = Infinity;
  let dayEnd = 0;
  for (const e of entries) {
    if (!e.durationSecs && !POINT_EVENT_TYPES.has(e.entryType)) continue;
    const { start, end } = entryRange(e);
    if (start < dayStart) dayStart = start;
    const effectiveEnd = e.durationSecs ? end : start;
    if (effectiveEnd > dayEnd) dayEnd = effectiveEnd;
  }

  // If no activity, return empty
  if (dayStart === Infinity) return [];

  // Build unfocused containers for gaps
  const containers: ActivityContainer[] = [];
  let cursor = dayStart;

  for (const fc of focused) {
    if (fc.startMin > cursor) {
      containers.push({
        id: `unfocused-${containers.length}`,
        type: "unfocused",
        startMin: cursor,
        endMin: fc.startMin,
        qualityScore: 0,
        taskEntries: [],
        appActivity: [],
        pointEvents: [],
      });
    }
    containers.push(fc);
    cursor = Math.max(cursor, fc.endMin);
  }

  // Trailing unfocused gap
  if (cursor < dayEnd) {
    containers.push({
      id: `unfocused-${containers.length}`,
      type: "unfocused",
      startMin: cursor,
      endMin: dayEnd,
      qualityScore: 0,
      taskEntries: [],
      appActivity: [],
      pointEvents: [],
    });
  }

  // Assign inner entries to containers
  const innerEntries = [...taskEntries, ...appActivity, ...pointEvents];
  for (const entry of innerEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    // Find enclosing container (last one where startMin >= container.startMin)
    const container = containers.find(
      (c) => startMin >= c.startMin && startMin < c.endMin,
    );
    if (!container) continue;

    if (entry.entryType === "taskTimeEntry") container.taskEntries.push(entry);
    else if (entry.entryType === "appUsage") container.appActivity.push(entry);
    else container.pointEvents.push(entry);
  }

  return containers;
}
```

**Step 2: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 3: Commit**

```
feat(dashboard): add container-building algorithm for layered calendar
```

---

### Task 4: Add Layers Toggle UI to DashboardLayout

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DashboardLayout.tsx`
- Read: `desktop-ui/src/components/dashboard/layers.ts` (from Task 2)

**Step 1: Add the Layers icon button and dropdown to the toolbar**

Import `Layers` icon from `lucide-react`, the `useLayerToggle` hook, and `LAYERS` config. Add a layers button between the view switcher and nav pill group. Wire up a popover dropdown (same `glass-dropdown` + `useClickOutside` portal pattern as the calendar picker already in this file).

The dropdown renders a list of `LAYERS` with checkbox toggles:

```tsx
{LAYERS.map((layer) => (
  <label
    key={layer.key}
    className={cn(
      "flex items-center gap-2 px-3 py-1.5 text-xs cursor-pointer rounded-lg transition-colors",
      layer.comingSoon
        ? "text-dim cursor-not-allowed"
        : "text-secondary hover:bg-white/[0.06]",
    )}
  >
    <input
      type="checkbox"
      checked={enabled.has(layer.key)}
      disabled={layer.comingSoon}
      onChange={() => toggle(layer.key)}
      className="accent-brand w-3 h-3"
    />
    <span
      className="w-2 h-2 rounded-full"
      style={{ backgroundColor: layer.color }}
    />
    {layer.label}
    {layer.comingSoon && (
      <span className="text-[9px] text-dim ml-auto">Soon</span>
    )}
  </label>
))}
<button
  type="button"
  onClick={reset}
  className="w-full text-left mt-1 px-3 py-1.5 text-[11px] text-muted hover:text-secondary rounded-lg hover:bg-white/[0.06] transition-colors"
>
  Reset to defaults
</button>
```

**Step 2: Pass `enabledSources` to children**

The `DashboardLayout` wraps its children. To pass `enabledSources` down without prop drilling, create a React context:

In `layers.ts`, add:
```typescript
import { createContext, useContext } from "react";

export const LayerContext = createContext<{
  enabled: Set<LayerKey>;
  enabledSources: TimelineSource[];
}>({ enabled: new Set(), enabledSources: [] });

export function useEnabledLayers() {
  return useContext(LayerContext);
}
```

In `DashboardLayout`, wrap children in `<LayerContext.Provider value={{ enabled, enabledSources }}>`.

**Step 3: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 4: Commit**

```
feat(dashboard): add layers toggle dropdown to toolbar
```

---

### Task 5: Rewrite DayCalendarView with Layered Containers

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DayCalendarView.tsx`
- Read: `desktop-ui/src/components/dashboard/buildContainers.ts` (from Task 3)
- Read: `desktop-ui/src/components/dashboard/layers.ts` (for `useEnabledLayers`)

This is the largest task. The current flat-block rendering is replaced with layered container rendering.

**Step 1: Update query to use enabledSources**

```typescript
const { enabled, enabledSources } = useEnabledLayers();
const queryArgs = useMemo(
  () => ({ startDate: dateStr, endDate: dateStr, sources: enabledSources }),
  [dateStr, enabledSources],
);
```

**Step 2: Replace positionBlocks with buildContainers**

Remove the existing `positionBlocks` function. Replace with:

```typescript
const containers = useMemo(() => buildContainers(data.entries), [data.entries]);
```

**Step 3: Render containers as layered blocks**

Each container renders as a positioned `div` on the hour grid:

```tsx
{containers.map((container) => {
  const top = container.startMin * pxPerMin;
  const height = Math.max((container.endMin - container.startMin) * pxPerMin, MIN_BLOCK_HEIGHT);
  const isFocused = container.type === "focused";

  return (
    <div
      key={container.id}
      className={cn(
        "absolute left-0 right-0 rounded-lg border overflow-hidden",
        isFocused
          ? "border-timeline-focus/30"
          : "border-white/[0.04]",
      )}
      style={{
        top,
        height,
        backgroundColor: isFocused
          ? focusColor(container.qualityScore)
          : "var(--timeline-unfocused)",
      }}
    >
      {/* Task entries: green-accented inner blocks */}
      {enabled.has("tasks") && container.taskEntries.map((entry) => (
        <TaskBlock key={entry.id} entry={entry} container={container} pxPerMin={pxPerMin}
          selected={selectedEntry?.id === entry.id}
          onClick={() => setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)} />
      ))}

      {/* App activity: thin horizontal bars */}
      {enabled.has("apps") && container.appActivity.map((entry) => (
        <AppBar key={entry.id} entry={entry} container={container} pxPerMin={pxPerMin} />
      ))}

      {/* Point events: colored dots */}
      {enabled.has("events") && container.pointEvents.map((entry) => (
        <EventDot key={entry.id} entry={entry} container={container} pxPerMin={pxPerMin} />
      ))}
    </div>
  );
})}
```

**Step 4: Create helper subcomponents inline**

- `focusColor(qualityScore: number): string` — Returns CSS color: high quality (>7) uses `--timeline-focus-high`, low (<4) uses `--timeline-focus-low`, default uses `--timeline-focus`. Apply 25% opacity via `color-mix`.
- `TaskBlock` — Positioned relative to container top. Green left-border accent (`border-l-2`), semi-transparent green background. Clickable for entry detail.
- `AppBar` — Thin horizontal bar (h-3) positioned relative to container. Uses entry's color at low opacity.
- `EventDot` — Small colored circle (w-2 h-2) at the timestamp's vertical position. Color determined by `entryType`: note→blue, taskCompleted→green, finance→yellow.

**Step 5: Keep the click-to-select and SummaryPanel integration**

`selectedEntry` state and `SummaryPanel` props stay the same. Task blocks and focus containers are clickable — clicking a focus container shows the focus session in the detail panel.

**Step 6: Verify build and visual check**

Run: `cd desktop-ui && bun run build`
Run: `cd desktop-ui && bun run dev` — open in browser, check day view renders containers.

**Step 7: Commit**

```
feat(dashboard): rewrite day calendar view with layered containers
```

---

### Task 6: Update WeekCalendarView with Compressed Layers

**Files:**
- Modify: `desktop-ui/src/components/dashboard/WeekCalendarView.tsx`
- Read: `desktop-ui/src/components/dashboard/buildContainers.ts`
- Read: `desktop-ui/src/components/dashboard/layers.ts`

**Step 1: Update query to use enabledSources**

Same pattern as Task 5 — import `useEnabledLayers`, pass `sources: enabledSources` to the query.

**Step 2: Build containers per day**

Replace the current flat `entriesByDay` grouping with container-building per day:

```typescript
const containersByDay = useMemo(() => {
  const map = new Map<string, ActivityContainer[]>();
  for (const day of days) map.set(day, []);
  // Group entries by day, then build containers for each
  const entryMap = new Map<string, TimelineEntry[]>();
  for (const day of days) entryMap.set(day, []);
  for (const entry of data.entries) {
    const day = toLocalISO(new Date(entry.startedAt));
    entryMap.get(day)?.push(entry);
  }
  for (const [day, dayEntries] of entryMap) {
    map.set(day, buildContainers(dayEntries));
  }
  return map;
}, [data.entries, days]);
```

**Step 3: Render compressed containers in day columns**

Each container renders as a block with:
- Background: purple (focused) or subtle gray (unfocused)
- Left border: 2px, green if container has task entries, else container color
- Height proportional to duration
- Tooltip on hover showing summary: "2h 15m focus · 1h 40m on 'Task Name' · Chrome 65%"

```tsx
{dayContainers.map((container) => {
  const top = container.startMin * pxPerMin;
  const height = Math.max(
    (container.endMin - container.startMin) * pxPerMin,
    MIN_BLOCK_HEIGHT,
  );
  const isFocused = container.type === "focused";
  const hasTask = container.taskEntries.length > 0;

  return (
    <button
      type="button"
      key={container.id}
      onClick={() => navigate(`/day/${day}`)}
      className={cn(
        "absolute left-0.5 right-0.5 rounded text-[9px] leading-tight overflow-hidden cursor-pointer",
        "hover:opacity-90 border border-white/10",
      )}
      style={{
        top,
        height,
        backgroundColor: isFocused
          ? focusColor(container.qualityScore)
          : "var(--timeline-unfocused)",
        borderLeftColor: hasTask ? "var(--timeline-task)" : (isFocused ? "var(--timeline-focus)" : "transparent"),
        borderLeftWidth: 2,
      }}
      title={buildWeekTooltip(container)}
    >
      {height > 20 && isFocused && (
        <span className="text-secondary truncate block px-0.5">
          {container.focusSession?.title}
        </span>
      )}
    </button>
  );
})}
```

**Step 4: Add tooltip builder**

```typescript
function buildWeekTooltip(container: ActivityContainer): string {
  const parts: string[] = [];
  const durationMin = container.endMin - container.startMin;
  if (container.type === "focused") {
    parts.push(`${formatHumanDuration(durationMin * 60)} focus`);
  } else {
    parts.push(`${formatHumanDuration(durationMin * 60)} unfocused`);
  }
  if (container.taskEntries.length > 0) {
    const taskTime = container.taskEntries.reduce((s, e) => s + (e.durationSecs ?? 0), 0);
    const taskName = container.taskEntries[0].title;
    parts.push(`${formatHumanDuration(taskTime)} on '${taskName}'`);
  }
  if (container.appActivity.length > 0) {
    const topApp = container.appActivity
      .sort((a, b) => (b.durationSecs ?? 0) - (a.durationSecs ?? 0))[0];
    if (topApp) {
      const pct = Math.round(((topApp.durationSecs ?? 0) / (durationMin * 60)) * 100);
      parts.push(`${topApp.title} ${pct}%`);
    }
  }
  return parts.join(" · ");
}
```

**Step 5: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 6: Commit**

```
feat(dashboard): update week view with compressed layered containers
```

---

### Task 7: Update Month and Year Views with Focus-Hours Coloring

**Files:**
- Modify: `desktop-ui/src/components/dashboard/MonthCalendarView.tsx`
- Modify: `desktop-ui/src/components/dashboard/YearHeatmapView.tsx`

These views get minimal changes — color intensity based on focus hours instead of total activity.

**Step 1: MonthCalendarView — color cells by focus time**

Currently, day cells show colored bars per source. Change the cell background to use focus-hours intensity:

In the existing entry grouping, compute focus seconds per day:

```typescript
const focusByDay = useMemo(() => {
  const map = new Map<string, number>();
  for (const entry of data.entries) {
    if (entry.source !== "focus") continue;
    const day = toLocalISO(new Date(entry.startedAt));
    map.set(day, (map.get(day) || 0) + (entry.durationSecs ?? 0));
  }
  return map;
}, [data.entries]);
```

Apply focus-based intensity to each day cell's background, and add a small focus-time label:

```tsx
<div className={cn(
  "min-h-[60px] p-1 rounded-lg border cursor-pointer transition-colors",
  day === today ? "border-brand/30" : "border-white/[0.06]",
  "hover:border-white/[0.12]",
)}
style={{
  backgroundColor: focusIntensityBg(focusByDay.get(day) || 0, maxFocusSecs),
}}
>
  {/* Focus time label in corner */}
  {focusSecs > 0 && (
    <span className="text-[9px] text-muted/60 float-right">
      {formatHumanDuration(focusSecs)}
    </span>
  )}
</div>
```

Where `focusIntensityBg` maps seconds to purple opacity levels:
```typescript
function focusIntensityBg(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "transparent";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "color-mix(in oklch, var(--timeline-focus) 25%, transparent)";
  if (ratio > 0.5) return "color-mix(in oklch, var(--timeline-focus) 18%, transparent)";
  if (ratio > 0.25) return "color-mix(in oklch, var(--timeline-focus) 10%, transparent)";
  return "color-mix(in oklch, var(--timeline-focus) 5%, transparent)";
}
```

Keep the existing source-colored bars for non-focus entries inside cells.

**Step 2: YearHeatmapView — intensity = focus hours**

Change the `dayMap` aggregation to filter for focus entries only:

```typescript
const { dayMap, maxSecs } = useMemo(() => {
  const map = new Map<string, number>();
  for (const entry of data.entries) {
    if (entry.source !== "focus") continue; // ← only focus
    const day = toLocalISO(new Date(entry.startedAt));
    map.set(day, (map.get(day) || 0) + (entry.durationSecs ?? 0));
  }
  let max = 0;
  for (const v of map.values()) {
    if (v > max) max = v;
  }
  return { dayMap: map, maxSecs: max };
}, [data.entries]);
```

Update heatmap `intensityClass` to use `bg-timeline-focus` instead of `bg-brand`:

```typescript
function intensityClass(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "bg-white/[0.03]";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "bg-timeline-focus/60";
  if (ratio > 0.5) return "bg-timeline-focus/40";
  if (ratio > 0.25) return "bg-timeline-focus/25";
  return "bg-timeline-focus/10";
}
```

Update the legend to say "Focus time":

```tsx
<span className="text-[10px] text-muted">Less focus</span>
...
<span className="text-[10px] text-muted">More focus</span>
```

Update legend color swatches from `bg-brand` to `bg-timeline-focus`.

**Step 3: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 4: Commit**

```
feat(dashboard): update month/year views with focus-hours coloring
```

---

### Task 8: Update SummaryPanel with Layer-Aware Breakdown

**Files:**
- Modify: `desktop-ui/src/components/dashboard/SummaryPanel.tsx`

**Step 1: Update DefaultSummary to show layered breakdown**

The existing `DefaultSummary` already shows `focusSecs`, `totalTrackedSecs`, `tasksCompleted`, `notesTouched`, `topApps`, and `sourceBreakdown`. Enhance it:

1. Make **Focus time** the primary stat (first position, larger text)
2. Add a **Focus ratio** line: `focusSecs / totalTrackedSecs * 100`%
3. Group the stat cards by layer:
   - Focus: total focus time, session count (from `sourceBreakdown` where source === "focus"), quality note
   - Tasks: completed count, time tracked on tasks
   - Apps: top apps list (already exists)
   - Events: notes touched, transactions count

```tsx
{/* Focus — primary stat */}
<div className="p-2 rounded-lg bg-timeline-focus/10 border border-timeline-focus/20">
  <div className="text-lg font-semibold text-primary">
    {formatHumanDuration(summary.focusSecs)}
  </div>
  <div className="text-[10px] text-muted">Focus time</div>
  {summary.totalTrackedSecs > 0 && (
    <div className="text-[10px] text-brand mt-0.5">
      {Math.round((summary.focusSecs / summary.totalTrackedSecs) * 100)}% focus ratio
    </div>
  )}
</div>
```

2. Keep the existing `Stat` cards for tasks completed, notes touched, transactions.
3. Keep the existing `topApps` list.
4. Keep the existing `sourceBreakdown` list but reorder: focus first, then task, then productivity.

**Step 2: Verify build**

Run: `cd desktop-ui && bun run build`

**Step 3: Commit**

```
feat(dashboard): update summary panel with focus-first layer breakdown
```

---

## Execution Order

Tasks 1-3 are independent infrastructure (can be parallelized).
Task 4 depends on Task 2.
Task 5 depends on Tasks 2, 3, 4.
Task 6 depends on Tasks 2, 3.
Task 7 is independent (only uses existing query data).
Task 8 is independent.

Suggested batch order:
- **Batch 1:** Tasks 1, 2, 3 (infrastructure, parallel)
- **Batch 2:** Tasks 4, 7, 8 (UI changes, somewhat parallel)
- **Batch 3:** Tasks 5, 6 (main view rewrites, sequential)
