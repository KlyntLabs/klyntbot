import { useMemo, useState } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import type { MergeableEvent } from "@shared/lib/activity-sessions";
import { mergeActivitySessions } from "@shared/lib/activity-sessions";
import { formatHumanDuration, todayISO } from "@shared/lib/dates";
import type { ActivityCategory, ActivityTimeline } from "@shared/types";
import { resolveActivityColor } from "../lib/constants";

interface TimelineBarProps {
  date: string;
}

interface Block {
  leftPct: number;
  widthPct: number;
  color: string;
  label: string;
  siteName: string | null;
  duration: number;
}

function formatHour(h: number): string {
  if (h === 0 || h === 24) return "12a";
  if (h < 12) return `${h}a`;
  if (h === 12) return "12p";
  return `${h - 12}p`;
}

export function TimelineBar({ date }: TimelineBarProps) {
  const { data: events } = useQuery<ActivityTimeline[]>("productivity_timeline", { date }, []);
  const { data: categories } = useQuery<ActivityCategory[]>(
    "productivity_categories",
    undefined,
    [],
  );
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);

  const categoryMap = useMemo(() => new Map(categories.map((c) => [c.id, c])), [categories]);

  // Compute the visible time range: auto-zoom to activity with 1h padding,
  // or fall back to full 24h when there's no data.
  const { startHour, endHour } = useMemo(() => {
    if (events.length === 0) return { startHour: 0, endHour: 24 };

    let minSecs = Number.POSITIVE_INFINITY;
    let maxSecs = 0;
    for (const e of events) {
      if (e.isIdle) continue;
      const s = new Date(e.startedAt);
      const sec = s.getHours() * 3600 + s.getMinutes() * 60 + s.getSeconds();
      const dur = e.durationSecs ?? 0;
      if (sec < minSecs) minSecs = sec;
      if (sec + dur > maxSecs) maxSecs = sec + dur;
    }

    if (minSecs > maxSecs) return { startHour: 0, endHour: 24 };

    // For today, extend to current time
    const isToday = date === todayISO();
    if (isToday) {
      const now = new Date();
      const nowSecs = now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds();
      if (nowSecs > maxSecs) maxSecs = nowSecs;
    }

    // 1h padding on each side, clamped to 0–24
    const sh = Math.max(0, Math.floor(minSecs / 3600) - 1);
    const eh = Math.min(24, Math.ceil(maxSecs / 3600) + 1);
    // Minimum 3h window for readability
    if (eh - sh < 3) {
      const mid = (sh + eh) / 2;
      return {
        startHour: Math.max(0, Math.floor(mid - 1.5)),
        endHour: Math.min(24, Math.ceil(mid + 1.5)),
      };
    }
    return { startHour: sh, endHour: eh };
  }, [events, date]);

  const spanHours = endHour - startHour;

  // Build tick labels for the visible range
  const tickLabels = useMemo(() => {
    const labels: { hour: number; label: string }[] = [];
    for (let h = startHour; h <= endHour; h += 1) {
      labels.push({ hour: h, label: formatHour(h) });
    }
    return labels;
  }, [startHour, endHour]);

  // Merge adjacent non-idle events into consolidated session blocks.
  const blocks: Block[] = useMemo(() => {
    if (events.length === 0) return [];
    const startSecs = startHour * 3600;
    const spanSecs = spanHours * 3600;

    // Parse events into the shared MergeableEvent shape (with siteName extra)
    interface TimelineEvent extends MergeableEvent {
      siteName: string | null;
    }
    const parsed: TimelineEvent[] = events.map((e) => {
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
        siteName: e.siteName,
        isIdle: e.isIdle,
        dur,
      };
    });

    const sessions = mergeActivitySessions(parsed);

    // Convert to percentage-based Block positions
    return sessions
      .map((session) => {
        const clampedStart = Math.max(session.startSecs - startSecs, 0);
        const clampedEnd = Math.min(session.endSecs - startSecs, spanSecs);
        const totalDur = clampedEnd - clampedStart;
        if (totalDur <= 0) return null;
        return {
          leftPct: (clampedStart / spanSecs) * 100,
          widthPct: Math.max(((clampedEnd - clampedStart) / spanSecs) * 100, 0.5),
          color: session.color,
          label: session.label,
          siteName: session.events[0]?.siteName ?? null,
          duration: totalDur,
        };
      })
      .filter(Boolean) as Block[];
  }, [events, categoryMap, startHour, spanHours]);

  const isToday = date === todayISO();
  const nowPct = useMemo(() => {
    if (!isToday) return null;
    const now = new Date();
    const nowSecs = now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds();
    const startSecs = startHour * 3600;
    const spanSecs = spanHours * 3600;
    const pct = ((nowSecs - startSecs) / spanSecs) * 100;
    return pct >= 0 && pct <= 100 ? pct : null;
  }, [isToday, startHour, spanHours]);

  const rangeLabel = `${formatHour(startHour).replace("a", ":00 AM").replace("p", ":00 PM")} – ${formatHour(endHour).replace("a", ":00 AM").replace("p", ":00 PM")}`;

  return (
    <div className="glass-card p-4 flex flex-col gap-2 col-span-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Timeline</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">{rangeLabel}</span>
      </div>

      {/* Timeline bar — wrapper is relative for tooltip positioning outside overflow */}
      <div className="relative">
        {/* Hover tooltip — outside overflow-hidden so it's not clipped */}
        {hoveredIdx !== null && blocks[hoveredIdx] && (
          <div
            className="absolute -top-7 z-10 px-2 py-1 rounded text-[10px] font-light text-primary whitespace-nowrap pointer-events-none"
            style={{
              left: `${Math.min(blocks[hoveredIdx].leftPct + blocks[hoveredIdx].widthPct / 2, 95)}%`,
              transform: "translateX(-50%)",
              background: "var(--surface-floating)",
              border: "1px solid var(--border)",
              boxShadow: "var(--shadow-tooltip)",
            }}
          >
            {blocks[hoveredIdx].label} · {formatHumanDuration(blocks[hoveredIdx].duration)}
          </div>
        )}

        <div className="relative h-9 rounded-lg bg-white/[0.08] overflow-hidden">
          {blocks.map((b, idx) => (
            <div
              aria-hidden="true"
              // biome-ignore lint/suspicious/noArrayIndexKey: index is a tiebreaker for same-app same-position blocks
              key={`${b.label}-${b.leftPct.toFixed(2)}-${idx}`}
              className="absolute top-0 h-full transition-opacity duration-150"
              style={{
                left: `${b.leftPct}%`,
                width: `${b.widthPct}%`,
                backgroundColor: b.color,
                opacity: hoveredIdx !== null && hoveredIdx !== idx ? 0.3 : 1,
                borderRadius: "3px",
              }}
              onMouseEnter={() => setHoveredIdx(idx)}
              onMouseLeave={() => setHoveredIdx(null)}
            />
          ))}

          {/* Now marker */}
          {nowPct !== null && (
            <div
              className="absolute top-0 h-full w-px pointer-events-none"
              style={{
                left: `${nowPct}%`,
                background: "var(--brand)",
                boxShadow: "0 0 4px var(--brand-glow)",
              }}
            >
              <div
                className="absolute -top-0.5 left-1/2 -translate-x-1/2 w-1.5 h-1.5 rounded-full"
                style={{ background: "var(--brand)" }}
              />
            </div>
          )}
        </div>
      </div>

      {/* Time axis */}
      <div className="relative h-4 mt-1">
        {tickLabels.map(({ hour, label }) => (
          <span
            key={hour}
            className="absolute text-[9px] font-light text-dim tabular-nums -translate-x-1/2"
            style={{ left: `${((hour - startHour) / spanHours) * 100}%` }}
          >
            {label}
          </span>
        ))}
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 text-[10px] font-light text-muted">
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full bg-success" />
          Productive
        </span>
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full bg-brand" />
          Uncategorized
        </span>
        <span className="flex items-center gap-1.5">
          <span className="w-2 h-2 rounded-full bg-destructive" />
          Distracting
        </span>
      </div>
    </div>
  );
}
