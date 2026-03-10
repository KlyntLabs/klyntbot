/**
 * ActivityTrack — renders merged productivity sessions as vertical blocks
 * inside the day column grid. Focus sessions render as thin left-edge
 * indicator bars, and activity blocks during focus get a focus border.
 */

import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import type { MergeableEvent } from "@shared/lib/activity-sessions";
import { mergeActivitySessions } from "@shared/lib/activity-sessions";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type {
  ActivityCategory,
  ActivitySwitchPayload,
  ActivityTimeline,
  TimelineEntry,
} from "@shared/types";
import { useEffect, useMemo, useState } from "react";
import { resolveActivityColor } from "../productivity/shared";

const FOCUS_BAR_WIDTH = 4; // px — left-edge focus indicator

export interface SessionBlock {
  startMin: number;
  endMin: number;
  color: string;
  label: string;
  duration: number;
  dominantCategory: string;
  appBreakdown: { app: string; dur: number; catType: string }[];
  /** True if this session overlaps with a focus period */
  duringFocus: boolean;
}

interface FocusRange {
  startMin: number;
  endMin: number;
  entryId: string;
  title: string;
  isLive: boolean;
}

interface ActivityTrackProps {
  date: string;
  hourHeight: number;
  isToday: boolean;
  focusEntries: TimelineEntry[];
  onSelectSession: (session: SessionBlock) => void;
  onSelectEntry: (entry: TimelineEntry) => void;
  selectedSession: SessionBlock | null;
  selectedEntryId: string | null;
  /** When provided, skip independent fetch and event listeners — parent owns the data. */
  timelineEntries?: ActivityTimeline[];
}

export function ActivityTrack({
  date,
  hourHeight,
  isToday,
  focusEntries,
  onSelectSession,
  onSelectEntry,
  selectedSession,
  selectedEntryId,
  timelineEntries,
}: ActivityTrackProps) {
  const pxPerMin = hourHeight / 60;
  const parentOwnsData = timelineEntries != null;

  // Fallback fetch — only used when parent doesn't provide timeline data.
  // Pass `null` args to skip the IPC call when parent owns the data.
  const { data: fetchedEvents, refetch: refetchEvents } = useQuery<ActivityTimeline[]>(
    "productivity_timeline",
    parentOwnsData ? null : { date, tzOffsetMins: TZ_OFFSET_MINS },
    [],
  );
  const { data: categories } = useQuery<ActivityCategory[]>(
    "productivity_categories",
    undefined,
    [],
  );

  // Real-time: refetch on app switch and productivity entity changes.
  // No-op handlers when parent owns the data — parent handles refetching.
  useEvent<ActivitySwitchPayload>("activity:switch", () => {
    if (!parentOwnsData) refetchEvents();
  });
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (!parentOwnsData && payload?.entityKind === "productivity") refetchEvents();
  });

  const events = parentOwnsData ? timelineEntries : fetchedEvents;

  const categoryMap = useMemo(() => new Map(categories.map((c) => [c.id, c])), [categories]);

  // Compute focus time ranges from focus entries
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(() => setNow(new Date()), 10_000);
    return () => clearInterval(id);
  }, [isToday]);

  const focusRanges: FocusRange[] = useMemo(() => {
    return focusEntries.map((e) => {
      const startMin = minutesSinceMidnight(e.startedAt);
      const isLive = isToday && !e.endedAt;
      let endMin: number;
      if (isLive) {
        endMin = now.getHours() * 60 + now.getMinutes() + now.getSeconds() / 60;
      } else {
        const dur = e.durationSecs ?? 0;
        endMin = startMin + dur / 60;
      }
      return { startMin, endMin, entryId: e.id, title: e.title, isLive };
    });
  }, [focusEntries, isToday, now]);

  const sessions: SessionBlock[] = useMemo(() => {
    if (events.length === 0) return [];

    // Parse events with extra hasFocus field
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

    // Convert to SessionBlock with focus overlap detection
    return merged.map((session) => {
      const sessionStartMin = session.startSecs / 60;
      const sessionEndMin = session.endSecs / 60;
      const duringFocus =
        session.events.some((ev) => ev.hasFocus) ||
        focusRanges.some((f) => sessionStartMin < f.endMin && sessionEndMin > f.startMin);

      return {
        startMin: sessionStartMin,
        endMin: sessionEndMin,
        color: session.color,
        label: session.label,
        duration: session.duration,
        dominantCategory: session.dominantCategory,
        appBreakdown: session.appBreakdown,
        duringFocus,
      };
    });
  }, [events, categoryMap, focusRanges]);

  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  return (
    <>
      {/* Focus indicator bars — thin left-edge bars showing focus periods */}
      {focusRanges.map((f) => {
        const top = f.startMin * pxPerMin;
        const height = Math.max((f.endMin - f.startMin) * pxPerMin, 8);
        const focusEntry = focusEntries.find((e) => e.id === f.entryId);
        const isSelected = selectedEntryId === f.entryId;

        return (
          <button
            type="button"
            key={f.entryId}
            className={cn(
              "absolute left-0 rounded-sm cursor-pointer z-10",
              isSelected && "ring-1 ring-brand",
            )}
            style={{
              top,
              height,
              width: FOCUS_BAR_WIDTH,
              backgroundColor: "var(--timeline-focus)",
              opacity: f.isLive ? 1 : 0.8,
              boxShadow: f.isLive
                ? "0 0 6px color-mix(in oklch, var(--timeline-focus) 50%, transparent)"
                : undefined,
            }}
            onClick={() => focusEntry && onSelectEntry(focusEntry)}
            aria-label={`Focus session: ${f.title || "Untitled"}${f.isLive ? " (in progress)" : ""}, ${formatHumanDuration(Math.round((f.endMin - f.startMin) * 60))}`}
            title={`${f.title}${f.isLive ? " (in progress)" : ""}`}
          >
            {f.isLive && (
              <span
                className="absolute -top-0.5 -left-0.5 w-[5px] h-[5px] rounded-full animate-pulse"
                style={{ backgroundColor: "var(--timeline-focus)" }}
              />
            )}
          </button>
        );
      })}

      {/* Activity session blocks — offset left to make room for focus bar */}
      {sessions.map((session, idx) => {
        const top = session.startMin * pxPerMin;
        const height = Math.max((session.endMin - session.startMin) * pxPerMin, 8);
        const isSelected =
          selectedSession?.startMin === session.startMin &&
          selectedSession?.label === session.label;
        const leftOffset = session.duringFocus ? FOCUS_BAR_WIDTH + 2 : 2;

        return (
          <button
            type="button"
            key={`${session.label}-${session.startMin}`}
            className={cn(
              "absolute right-0.5 rounded-sm cursor-pointer transition-opacity overflow-hidden",
              isSelected && "ring-1 ring-brand",
            )}
            style={{
              top,
              height,
              left: leftOffset,
              backgroundColor: session.color,
              opacity: hoveredIdx !== null && hoveredIdx !== idx ? 0.3 : 0.75,
            }}
            onClick={() => onSelectSession(session)}
            onMouseEnter={() => setHoveredIdx(idx)}
            onMouseLeave={() => setHoveredIdx(null)}
            title={`${session.label} · ${formatHumanDuration(session.duration)}${session.duringFocus ? " (focus)" : ""}`}
          >
            {height > 18 && (
              <span className="text-[9px] text-white/90 font-medium px-1 truncate block leading-tight mt-0.5">
                {session.label}
              </span>
            )}
            {height > 32 && (
              <span className="text-[8px] text-white/60 px-1 truncate block">
                {formatHumanDuration(session.duration)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
