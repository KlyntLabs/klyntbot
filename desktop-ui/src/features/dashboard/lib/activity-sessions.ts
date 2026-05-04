/**
 * Shared session-merging algorithm for activity events.
 * Used by both the horizontal Timeline bar and the vertical ActivityTrack.
 *
 * Two-pass algorithm:
 *   1. Sort non-idle events by time, merge adjacent events within GAP_THRESHOLD.
 *   2. Color each session by its dominant category, pick the best label.
 */

const GAP_THRESHOLD = 120; // seconds — merge activity within 2 min

/** Minimum fields needed for merging. Consumers can extend with extra data. */
export interface MergeableEvent {
  startSecs: number;
  endSecs: number;
  catType: string;
  color: string;
  label: string;
  isIdle: boolean;
  dur: number;
}

export interface MergedSession<E extends MergeableEvent = MergeableEvent> {
  startSecs: number;
  endSecs: number;
  color: string;
  dominantCategory: string;
  label: string;
  duration: number;
  appBreakdown: { app: string; dur: number; catType: string }[];
  /** The raw events that were merged into this session. */
  events: E[];
}

/**
 * Merge a list of parsed activity events into consolidated sessions.
 * Events should already be parsed (with startSecs, etc.) but need not be sorted.
 */
export function mergeActivitySessions<E extends MergeableEvent>(parsed: E[]): MergedSession<E>[] {
  // Sort by start time
  const sorted = [...parsed].sort((a, b) => a.startSecs - b.startSecs);

  // Pass 1: merge adjacent non-idle events into raw sessions
  interface RawSession {
    startSecs: number;
    endSecs: number;
    events: E[];
  }

  const rawSessions: RawSession[] = [];
  let cur: RawSession | null = null;

  for (const ev of sorted) {
    if (ev.isIdle) continue;

    if (cur && ev.startSecs - cur.endSecs <= GAP_THRESHOLD) {
      cur.endSecs = Math.max(cur.endSecs, ev.endSecs);
      cur.events.push(ev);
    } else {
      if (cur) rawSessions.push(cur);
      cur = { startSecs: ev.startSecs, endSecs: ev.endSecs, events: [ev] };
    }
  }
  if (cur) rawSessions.push(cur);

  // Pass 2: compute dominant category, best label, and app breakdown
  const merged: MergedSession<E>[] = [];
  for (const session of rawSessions) {
    const totalDur = session.endSecs - session.startSecs;
    if (totalDur <= 0) continue;

    const catTally = new Map<string, { dur: number; color: string }>();
    const appTally = new Map<string, { dur: number; catType: string }>();
    let bestLabel = session.events[0]?.label ?? "";
    let bestLabelDur = 0;

    for (const ev of session.events) {
      const existing = catTally.get(ev.catType);
      if (existing) existing.dur += ev.dur;
      else catTally.set(ev.catType, { dur: ev.dur, color: ev.color });

      const appExisting = appTally.get(ev.label);
      if (appExisting) appExisting.dur += ev.dur;
      else appTally.set(ev.label, { dur: ev.dur, catType: ev.catType });

      if (ev.dur > bestLabelDur) {
        bestLabelDur = ev.dur;
        bestLabel = ev.label;
      }
    }

    let dominantColor = "var(--brand)";
    let dominantCategory = "uncategorized";
    let maxDur = 0;
    for (const [cat, val] of catTally) {
      if (val.dur > maxDur) {
        maxDur = val.dur;
        dominantColor = val.color;
        dominantCategory = cat;
      }
    }

    const appBreakdown = [...appTally.entries()]
      .map(([app, { dur, catType }]) => ({ app, dur, catType }))
      .sort((a, b) => b.dur - a.dur)
      .slice(0, 5);

    merged.push({
      startSecs: session.startSecs,
      endSecs: session.endSecs,
      color: dominantColor,
      dominantCategory,
      label: bestLabel,
      duration: totalDur,
      appBreakdown,
      events: session.events,
    });
  }

  return merged;
}
