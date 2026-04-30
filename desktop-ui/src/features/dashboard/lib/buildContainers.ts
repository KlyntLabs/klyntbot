import type { TimelineEntry } from "@/bindings";
import { minutesSinceMidnight } from "@/utils/dashboardDates";

/** Maps focus quality score (0-10) to a CSS background color with 25% opacity via color-mix. */
export function focusColor(qualityScore: number): string {
  if (qualityScore > 7) return "color-mix(in oklch, var(--timeline-focus-high) 25%, transparent)";
  if (qualityScore < 4) return "color-mix(in oklch, var(--timeline-focus-low) 25%, transparent)";
  return "color-mix(in oklch, var(--timeline-focus) 25%, transparent)";
}

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
  "taskCreated",
  "taskCompleted",
  "taskUpdated",
  "noteCreated",
  "noteUpdated",
  "transactionRecorded",
  "expenseRecorded",
  "incomeRecorded",
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
  focusSessions.sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime());

  // Build focused containers
  const focused: ActivityContainer[] = focusSessions.map((s) => {
    const { start, end } = entryRange(s);
    const meta = s.metadata as Record<string, unknown> | null;
    const quality = typeof meta?.qualityScore === "number" ? (meta.qualityScore as number) : 5;
    return {
      id: `focus-${s.id}`,
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

  // Assign inner entries to containers (binary search — containers are sorted by startMin)
  const assignEntry = (entry: TimelineEntry) => {
    const startMin = minutesSinceMidnight(entry.startedAt);
    // Binary search: find last container with startMin <= entry's startMin
    let lo = 0;
    let hi = containers.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (containers[mid].startMin <= startMin) lo = mid + 1;
      else hi = mid - 1;
    }
    if (hi < 0 || startMin >= containers[hi].endMin) return;
    const container = containers[hi];
    if (entry.entryType === "taskTimeEntry") container.taskEntries.push(entry);
    else if (entry.entryType === "appUsage") container.appActivity.push(entry);
    else container.pointEvents.push(entry);
  };
  for (const entry of taskEntries) assignEntry(entry);
  for (const entry of appActivity) assignEntry(entry);
  for (const entry of pointEvents) assignEntry(entry);

  return containers;
}
