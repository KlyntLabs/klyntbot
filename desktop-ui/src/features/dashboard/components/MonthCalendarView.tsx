import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, todayISO, toLocalISO } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { useCallback, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { computeDayStats, DAY_LABELS } from "../lib/timeline-utils";
import { SummaryPanel } from "./SummaryPanel";

function getMonthRange(dateStr: string): {
  start: string;
  end: string;
  year: number;
  month: number;
} {
  const d = new Date(`${dateStr}T00:00:00`);
  const year = d.getFullYear();
  const month = d.getMonth();
  const first = new Date(year, month, 1);
  const last = new Date(year, month + 1, 0);
  return { start: toLocalISO(first), end: toLocalISO(last), year, month };
}

interface DayCell {
  date: string;
  day: number;
  isCurrentMonth: boolean;
  entries: TimelineEntry[];
}

function buildCalendarGrid(year: number, month: number, entries: TimelineEntry[]): DayCell[][] {
  const first = new Date(year, month, 1);
  const startDay = first.getDay() === 0 ? 6 : first.getDay() - 1; // Monday-start
  const daysInMonth = new Date(year, month + 1, 0).getDate();

  // Group entries by date
  const byDate = new Map<string, TimelineEntry[]>();
  for (const entry of entries) {
    const d = toLocalISO(new Date(entry.startedAt));
    if (!byDate.has(d)) byDate.set(d, []);
    byDate.get(d)?.push(entry);
  }

  const cells: DayCell[] = [];

  // Previous month padding
  const prevMonth = new Date(year, month, 0);
  for (let i = startDay - 1; i >= 0; i--) {
    const day = prevMonth.getDate() - i;
    const d = new Date(year, month - 1, day);
    const iso = toLocalISO(d);
    cells.push({ date: iso, day, isCurrentMonth: false, entries: byDate.get(iso) || [] });
  }

  // Current month
  for (let day = 1; day <= daysInMonth; day++) {
    const iso = toLocalISO(new Date(year, month, day));
    cells.push({ date: iso, day, isCurrentMonth: true, entries: byDate.get(iso) || [] });
  }

  // Next month padding (fill to 6 rows)
  const remaining = 42 - cells.length;
  for (let day = 1; day <= remaining; day++) {
    const d = new Date(year, month + 1, day);
    const iso = toLocalISO(d);
    cells.push({ date: iso, day, isCurrentMonth: false, entries: byDate.get(iso) || [] });
  }

  // Split into weeks
  const weeks: DayCell[][] = [];
  for (let i = 0; i < cells.length; i += 7) {
    weeks.push(cells.slice(i, i + 7));
  }
  return weeks;
}

/** Active time → fill ratio for the activity bar (0–1) */
function activeRatio(activeSecs: number, maxActiveSecs: number): number {
  if (maxActiveSecs === 0) return 0;
  return Math.min(1, activeSecs / maxActiveSecs);
}

/** Focus intensity → background tint */
function focusIntensityBg(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "transparent";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "color-mix(in oklch, var(--timeline-focus) 25%, transparent)";
  if (ratio > 0.5) return "color-mix(in oklch, var(--timeline-focus) 18%, transparent)";
  if (ratio > 0.25) return "color-mix(in oklch, var(--timeline-focus) 10%, transparent)";
  return "color-mix(in oklch, var(--timeline-focus) 5%, transparent)";
}

export function MonthCalendarView() {
  const { date } = useParams<{ date: string }>();
  const navigate = useNavigate();
  const dateStr = date || todayISO();
  const today = todayISO();

  const { start, end, year, month } = useMemo(() => getMonthRange(dateStr), [dateStr]);

  const queryArgs = useMemo(() => ({ startDate: start, endDate: end }), [start, end]);
  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  const weeks = useMemo(
    () => buildCalendarGrid(year, month, data.entries),
    [year, month, data.entries],
  );

  const [focusedDate, setFocusedDate] = useState(today);

  const handleGridKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      let delta = 0;
      if (e.key === "ArrowLeft") delta = -1;
      else if (e.key === "ArrowRight") delta = 1;
      else if (e.key === "ArrowUp") delta = -7;
      else if (e.key === "ArrowDown") delta = 7;
      else if (e.key === "Enter") {
        navigate(`/day/${focusedDate}`);
        e.preventDefault();
        return;
      } else return;

      e.preventDefault();
      const next = new Date(`${focusedDate}T00:00:00`);
      next.setDate(next.getDate() + delta);
      setFocusedDate(toLocalISO(next));
    },
    [focusedDate, navigate],
  );

  // Compute per-day stats and find maximums for normalization
  const { dayStats, maxActiveSecs, maxFocusSecs } = useMemo(() => {
    const statsMap = new Map<string, { activeSecs: number; focusSecs: number }>();
    let maxA = 0;
    let maxF = 0;
    for (const week of weeks) {
      for (const cell of week) {
        const stats = computeDayStats(cell.entries);
        statsMap.set(cell.date, stats);
        if (stats.activeSecs > maxA) maxA = stats.activeSecs;
        if (stats.focusSecs > maxF) maxF = stats.focusSecs;
      }
    }
    return { dayStats: statsMap, maxActiveSecs: maxA, maxFocusSecs: maxF };
  }, [weeks]);

  return (
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card p-3 flex flex-col overflow-hidden">
        {loading && <div className="text-xs text-muted mb-1">Loading...</div>}

        {/* Day-of-week header */}
        <div className="grid grid-cols-7 mb-1">
          {DAY_LABELS.map((label) => (
            <div key={label} className="text-center text-[10px] text-muted font-medium py-1">
              {label}
            </div>
          ))}
        </div>

        {/* Calendar grid */}
        {/* biome-ignore lint/a11y/useSemanticElements: CSS grid layout requires div, not table */}
        <div
          className="flex-1 grid grid-rows-6 gap-px outline-none"
          role="grid"
          aria-label="Month calendar"
          tabIndex={0}
          onKeyDown={handleGridKeyDown}
        >
          {weeks.map((week) => (
            <div key={week[0].date} className="grid grid-cols-7 gap-px">
              {week.map((cell) => {
                const stats = dayStats.get(cell.date) || { activeSecs: 0, focusSecs: 0 };
                const aRatio = activeRatio(stats.activeSecs, maxActiveSecs);

                return (
                  <button
                    type="button"
                    key={cell.date}
                    onClick={() => navigate(`/day/${cell.date}`)}
                    className={cn(
                      "rounded-lg p-1.5 flex flex-col items-start text-left transition-colors min-h-[64px]",
                      "hover:bg-accent cursor-pointer",
                      cell.isCurrentMonth ? "text-foreground" : "text-muted-foreground/40",
                      cell.date === today && "ring-1 ring-brand/50",
                      cell.date === focusedDate && cell.date !== today && "ring-1 ring-white/30",
                    )}
                    style={{
                      backgroundColor: focusIntensityBg(stats.focusSecs, maxFocusSecs),
                    }}
                  >
                    {/* Date + focus time */}
                    <div className="flex items-center justify-between w-full">
                      <span
                        className={cn(
                          "text-[11px] font-medium",
                          cell.date === today && "text-brand",
                        )}
                      >
                        {cell.day}
                      </span>
                      {stats.focusSecs > 0 && (
                        <span className="text-[8px] text-muted/60">
                          {formatHumanDuration(stats.focusSecs)}
                        </span>
                      )}
                    </div>

                    {/* Activity bar — proportional to active time */}
                    {stats.activeSecs > 0 && (
                      <div className="w-full mt-auto flex flex-col gap-0.5">
                        <div className="w-full h-[3px] rounded-full bg-accent overflow-hidden">
                          <div
                            className="h-full rounded-full"
                            style={{
                              width: `${Math.max(aRatio * 100, 8)}%`,
                              backgroundColor: "var(--success)",
                              opacity: 0.7 + aRatio * 0.3,
                            }}
                          />
                        </div>
                        <span className="text-[8px] text-muted-foreground/50">
                          {formatHumanDuration(stats.activeSecs)}
                        </span>
                      </div>
                    )}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>

      <SummaryPanel summary={data.summary} selectedEntry={null} onClose={() => {}} />
    </div>
  );
}
