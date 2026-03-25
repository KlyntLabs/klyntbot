/**
 * ActivityTrack — renders merged productivity sessions as full-width vertical blocks
 * inside the day column grid. Activity blocks during focus get a subtle left-edge
 * focus indicator (2px border in timeline-focus color).
 *
 * V2: Intelligence-enriched sessions — quality-based coloring, LLM titles,
 * and purity-based opacity for effective-period intensity.
 */

import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import type { MergeableEvent } from "@shared/lib/activity-sessions";
import { mergeActivitySessions } from "@shared/lib/activity-sessions";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@shared/lib/dates";
import { purityToOpacity, qualityToColor, resolveActivityColor } from "@shared/lib/productivity";
import { cn } from "@shared/lib/utils";
import type {
  ActivityCategory,
  ActivitySwitchPayload,
  ActivityTimeline,
  IntelligenceSession,
  TimelineEntry,
} from "@shared/types";
import { useMemo, useState } from "react";

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
  /** Matched intelligence session (quality, title, etc.) */
  intelligence: IntelligenceSession | null;
}

interface ActivityTrackProps {
  date: string;
  hourHeight: number;
  isToday: boolean;
  onSelectSession: (session: SessionBlock) => void;
  onSelectEntry: (entry: TimelineEntry) => void;
  selectedSession: SessionBlock | null;
  selectedEntryId: string | null;
  /** When provided, skip independent fetch and event listeners — parent owns the data. */
  timelineEntries?: ActivityTimeline[];
}

// ── Intelligence session matching ──────────────────────────────

/** Match a merged session block to the closest overlapping intelligence session. */
function matchIntelligence(
  startMin: number,
  endMin: number,
  sessions: IntelligenceSession[],
): IntelligenceSession | null {
  let best: IntelligenceSession | null = null;
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

  // Require >= 30% overlap to avoid spurious matches
  return best && bestOverlap / sessionDur > 0.3 ? best : null;
}

// ── Component ──────────────────────────────────────────────────

export function ActivityTrack({
  date,
  hourHeight,
  isToday: _isToday,
  onSelectSession,
  onSelectEntry: _onSelectEntry,
  selectedSession,
  selectedEntryId: _selectedEntryId,
  timelineEntries,
}: ActivityTrackProps) {
  const pxPerMin = hourHeight / 60;
  const parentOwnsData = timelineEntries != null;

  // Fallback fetch — only used when parent doesn't provide timeline data.
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

  // Fetch intelligence sessions for quality/title overlay
  // Skip when parent owns data (parent handles refetching)
  const { data: intellSessions } = useQuery<IntelligenceSession[]>(
    "productivity_intelligence_sessions",
    parentOwnsData ? null : { date, tzOffsetMins: TZ_OFFSET_MINS },
    [],
  );

  // Real-time: refetch on app switch and productivity entity changes.
  useEvent<ActivitySwitchPayload>("activity:switch", () => {
    if (!parentOwnsData) refetchEvents();
  });
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (!parentOwnsData && payload?.entityKind === "productivity") refetchEvents();
  });

  const events = parentOwnsData ? timelineEntries : fetchedEvents;

  const categoryMap = useMemo(() => new Map(categories.map((c) => [c.id, c])), [categories]);

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

    // Convert to SessionBlock with focus detection from event metadata
    return merged.map((session) => {
      const sessionStartMin = session.startSecs / 60;
      const sessionEndMin = session.endSecs / 60;
      const duringFocus = session.events.some((ev) => ev.hasFocus);

      const matched = matchIntelligence(sessionStartMin, sessionEndMin, intellSessions);

      // Use quality-based color when available, fallback to category color
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
      {/* Activity session blocks — full-width with focus indicator border when duringFocus */}
      {sessions.map((session, idx) => {
        const top = session.startMin * pxPerMin;
        const height = Math.max((session.endMin - session.startMin) * pxPerMin, 8);
        const isSelected =
          selectedSession?.startMin === session.startMin &&
          selectedSession?.label === session.label;
        const matched = session.intelligence;

        // Purity-based opacity: higher purity = more solid (sustained focus indicator)
        const baseOpacity = purityToOpacity(matched?.categoryPurity);
        const opacity = hoveredIdx !== null && hoveredIdx !== idx ? 0.3 : baseOpacity;

        // Build rich tooltip
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
              "absolute left-0.5 right-0.5 rounded-sm cursor-pointer transition-opacity overflow-hidden",
              isSelected && "ring-1 ring-brand",
              matched && "shadow-sm",
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
            {/* Quality badge — top-right pill */}
            {matched?.qualityScore != null && height > 24 && (
              <span className="absolute top-0.5 right-0.5 text-[7px] font-bold text-foregroundbg-overlay rounded-full px-1 leading-tight">
                {Math.round(matched.qualityScore)}
              </span>
            )}

            {/* Session title (LLM-generated or app name) */}
            {height > 18 && (
              <span className="text-[9px] text-foregroundfont-medium px-1 truncate block leading-tight mt-0.5">
                {session.label}
              </span>
            )}

            {/* Description or duration */}
            {height > 32 && (
              <span className="text-[8px] text-white/60 px-1 truncate block">
                {matched?.description || formatHumanDuration(session.duration)}
              </span>
            )}

            {/* Duration when there's space and description is shown */}
            {height > 48 && matched?.description && (
              <span className="text-[7px] text-white/40 px-1 truncate block">
                {formatHumanDuration(session.duration)}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
