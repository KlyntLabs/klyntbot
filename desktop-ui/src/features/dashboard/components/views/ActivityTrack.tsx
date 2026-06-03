import { useMemo, useState } from "react";
import {
  productivityCategoriesQuery,
  productivityIntelligenceSessionsQuery,
  productivityTimelineQuery,
} from "@/api/endpoints/dashboard";
import type {
  ActivityCategoryResponse,
  ActivityTimelineResponse,
  IntelligenceSessionResponse,
  TimelineEntry,
} from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@/utils/dashboardDates";
import { cn } from "@/utils/cn";
import { type MergeableEvent, mergeActivitySessions } from "../../lib/activity-sessions";
import { purityToOpacity, qualityToColor, resolveActivityColor } from "../../lib/productivity";

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
    staleTime: 60_000,
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
    staleTime: 60_000,
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

        return (
          <button
            type="button"
            key={`${session.label}-${session.startMin}`}
            className={cn(
              "absolute left-0.5 right-0.5 rounded overflow-hidden border-none text-left transition-opacity duration-200 ease-out min-h-4 cursor-pointer px-1 py-0.5",
              isSelected && "outline outline-1 outline-border-accent",
              matched && "shadow-[0_1px_3px_rgba(0,0,0,0.1)]",
            )}
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
              <span className="absolute top-0.5 right-0.5 text-ui-3xs font-bold rounded-full px-1 bg-black/40 text-white leading-tight">
                {Math.round(matched.qualityScore)}
              </span>
            )}
            {height > 18 && (
              <span className="block text-ui-3xs font-medium whitespace-nowrap overflow-hidden text-ellipsis mt-0.5 text-white">
                {session.label}
              </span>
            )}
            {height > 32 && (
              <span className="block text-ui-3xs whitespace-nowrap overflow-hidden text-ellipsis text-white/60">
                {matched?.description || formatHumanDuration(session.duration)}
              </span>
            )}
            {height > 48 && matched?.description && (
              <span className="block text-ui-3xs whitespace-nowrap overflow-hidden text-ellipsis text-white/40">
                {formatHumanDuration(session.duration)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
