import { useMemo, useState } from "react";
import { useQuery } from "../../hooks/useQuery";
import { formatHumanDuration, todayISO } from "../../lib/dates";
import type { ActivityCategory, ActivityTimeline } from "../../lib/types";

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

/** Full 24-hour range: midnight to midnight. */
const START_HOUR = 0;
const END_HOUR = 24;
const SPAN_HOURS = END_HOUR - START_HOUR;

function resolveColor(categoryType: string | undefined, isIdle: boolean): string {
  if (isIdle) return "var(--surface-highest)";
  if (categoryType === "productive") return "var(--success)";
  if (categoryType === "distracting") return "var(--destructive)";
  if (categoryType === "neutral") return "var(--text-muted)";
  return "var(--brand)";
}

const TICK_LABELS: { hour: number; label: string }[] = [];
for (let h = START_HOUR; h <= END_HOUR; h += 1) {
  const display = h === 0 || h === 24 ? "12a" : h < 12 ? `${h}a` : h === 12 ? "12p" : `${h - 12}p`;
  TICK_LABELS.push({ hour: h, label: display });
}

/** Fraction of the day (0–1) for the current time. */
function nowFraction(): number {
  const now = new Date();
  return (now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds()) / (SPAN_HOURS * 3600);
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

  const blocks: Block[] = useMemo(() => {
    if (events.length === 0) return [];
    const spanSecs = SPAN_HOURS * 3600;

    return events
      .map((e) => {
        const start = new Date(e.startedAt);
        const eSecs = start.getHours() * 3600 + start.getMinutes() * 60 + start.getSeconds();
        const dur = e.durationSecs ?? 0;
        if (eSecs + dur < 0 || eSecs > END_HOUR * 3600) return null;

        const clampedStart = Math.max(eSecs, 0);
        const clampedEnd = Math.min(eSecs + dur, spanSecs);
        const cat = e.categoryId ? categoryMap.get(e.categoryId) : undefined;

        return {
          leftPct: (clampedStart / spanSecs) * 100,
          widthPct: Math.max(((clampedEnd - clampedStart) / spanSecs) * 100, 0.15),
          color: resolveColor(cat?.categoryType, e.isIdle),
          label: e.projectId ?? e.siteName ?? e.appName,
          siteName: e.siteName,
          duration: dur,
        };
      })
      .filter(Boolean) as Block[];
  }, [events, categoryMap]);

  const isToday = date === todayISO();
  const nowPct = isToday ? nowFraction() * 100 : null;

  return (
    <div className="glass-card p-4 flex flex-col gap-2 col-span-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Timeline</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">0:00 – 23:59</span>
      </div>

      {/* Timeline bar */}
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
              borderRadius: b.widthPct > 0.5 ? "2px" : undefined,
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

        {/* Hover tooltip */}
        {hoveredIdx !== null && blocks[hoveredIdx] && (
          <div
            className="absolute -top-8 z-10 px-2 py-1 rounded text-[10px] font-light text-primary whitespace-nowrap pointer-events-none"
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
      </div>

      {/* Time axis — every hour */}
      <div className="relative h-4 mt-1">
        {TICK_LABELS.map(({ hour, label }) => (
          <span
            key={hour}
            className="absolute text-[9px] font-light text-dim tabular-nums -translate-x-1/2"
            style={{ left: `${(hour / SPAN_HOURS) * 100}%` }}
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
