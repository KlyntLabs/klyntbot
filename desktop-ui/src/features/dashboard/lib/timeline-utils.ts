import type { TimelineEntry } from "@shared/types";

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
      activeSecs += e.durationSecs;
    }
  }
  return { activeSecs, focusSecs };
}
