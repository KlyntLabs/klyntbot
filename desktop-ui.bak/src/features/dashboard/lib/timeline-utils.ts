import { minutesSinceMidnight } from "@shared/lib/dates";
import type { TimelineEntry } from "@shared/types";

export interface OverlapLayout {
  colIndex: number;
  totalCols: number;
}

/**
 * Google Calendar-style overlap layout: items at the same time are placed
 * side-by-side instead of stacking on top of each other.
 */
export function computeOverlapLayout<
  T extends { id: string; startedAt: string; durationSecs?: number | null },
>(items: T[], minBlockMinutes = 7): Map<string, OverlapLayout> {
  const result = new Map<string, OverlapLayout>();
  if (items.length === 0) return result;

  const intervals = items
    .map((item) => {
      const startMin = minutesSinceMidnight(item.startedAt);
      const dur = item.durationSecs ?? 0;
      const endMin = dur > 0 ? startMin + dur / 60 : startMin + minBlockMinutes;
      return { id: item.id, startMin, endMin };
    })
    .sort((a, b) => a.startMin - b.startMin || a.endMin - b.endMin);

  let clusterStart = 0;
  while (clusterStart < intervals.length) {
    let clusterEnd = clusterStart;
    let maxEnd = intervals[clusterStart].endMin;
    while (clusterEnd + 1 < intervals.length && intervals[clusterEnd + 1].startMin < maxEnd) {
      clusterEnd++;
      maxEnd = Math.max(maxEnd, intervals[clusterEnd].endMin);
    }

    const cluster = intervals.slice(clusterStart, clusterEnd + 1);
    const columnEnds: number[] = [];
    for (const iv of cluster) {
      let placed = false;
      for (let c = 0; c < columnEnds.length; c++) {
        if (columnEnds[c] <= iv.startMin) {
          columnEnds[c] = iv.endMin;
          result.set(iv.id, { colIndex: c, totalCols: 0 });
          placed = true;
          break;
        }
      }
      if (!placed) {
        result.set(iv.id, { colIndex: columnEnds.length, totalCols: 0 });
        columnEnds.push(iv.endMin);
      }
    }

    const totalCols = columnEnds.length;
    for (const iv of cluster) {
      result.get(iv.id)!.totalCols = totalCols;
    }

    clusterStart = clusterEnd + 1;
  }

  return result;
}

/** Apps considered idle/inactive — not counted toward active time. */
export const IDLE_APPS = new Set(["loginwindow", "idle", "screensaver", "lock screen", "desktop"]);

/** Monday-start day labels for calendar headers. */
export const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/** Check if a timeline entry represents active (non-idle) app usage. */
export function isActiveAppEntry(e: TimelineEntry): boolean {
  return e.entryType === "appUsage" && !IDLE_APPS.has(e.title.toLowerCase()) && !!e.durationSecs;
}

/** Compute day stats from timeline entries: active seconds and focus seconds. */
export function computeDayStats(entries: TimelineEntry[]): {
  activeSecs: number;
  focusSecs: number;
} {
  let activeSecs = 0;
  let focusSecs = 0;
  for (const e of entries) {
    if (e.source === "focus") {
      focusSecs += e.durationSecs ?? 0;
    }
    if (isActiveAppEntry(e)) {
      activeSecs += e.durationSecs ?? 0;
    }
  }
  return { activeSecs, focusSecs };
}
