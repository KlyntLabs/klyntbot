import { useCallback, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, todayISO, toLocalISO } from "@shared/lib/dates";
import type { TimelineEntry, TimelineSource } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { cn } from "@shared/lib/utils";
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

const SOURCE_COLORS: Record<TimelineSource, string> = {
  productivity: "var(--timeline-app-neutral)",
  focus: "var(--timeline-focus)",
  task: "var(--timeline-task)",
  todo: "var(--timeline-todo)",
  note: "var(--timeline-note)",
  finance: "var(--timeline-finance)",
  system: "var(--timeline-system)",
  calendar: "var(--timeline-calendar)",
};

function focusIntensityBg(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "transparent";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "color-mix(in oklch, var(--timeline-focus) 25%, transparent)";
  if (ratio > 0.5) return "color-mix(in oklch, var(--timeline-focus) 18%, transparent)";
  if (ratio > 0.25) return "color-mix(in oklch, var(--timeline-focus) 10%, transparent)";
  return "color-mix(in oklch, var(--timeline-focus) 5%, transparent)";
}

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

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

  const { focusByDay, maxFocusSecs } = useMemo(() => {
    const map = new Map<string, number>();
    for (const entry of data.entries) {
      if (entry.source !== "focus") continue;
      const day = toLocalISO(new Date(entry.startedAt));
      map.set(day, (map.get(day) || 0) + (entry.durationSecs ?? 0));
    }
    let max = 0;
    for (const v of map.values()) {
      if (v > max) max = v;
    }
    return { focusByDay: map, maxFocusSecs: max };
  }, [data.entries]);

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
                const focusSecs = focusByDay.get(cell.date) || 0;
                return (
                  <button
                    type="button"
                    key={cell.date}
                    onClick={() => navigate(`/day/${cell.date}`)}
                    className={cn(
                      "rounded-lg p-1 flex flex-col items-start text-left transition-colors min-h-[60px]",
                      "hover:bg-white/[0.05] cursor-pointer",
                      cell.isCurrentMonth ? "text-primary" : "text-muted/40",
                      cell.date === today && "ring-1 ring-brand/50",
                      cell.date === focusedDate && cell.date !== today && "ring-1 ring-white/30",
                    )}
                    style={{
                      backgroundColor: focusIntensityBg(focusSecs, maxFocusSecs),
                    }}
                  >
                    <div className="flex items-center justify-between w-full">
                      <span
                        className={cn(
                          "text-[11px] font-medium mb-0.5",
                          cell.date === today && "text-brand",
                        )}
                      >
                        {cell.day}
                      </span>
                      {focusSecs > 0 && (
                        <span className="text-[9px] text-muted/60">
                          {formatHumanDuration(focusSecs)}
                        </span>
                      )}
                    </div>

                    {/* Source color bars */}
                    {cell.entries.length > 0 && (
                      <div className="flex gap-px flex-wrap w-full">
                        {getSourceBars(cell.entries).map(({ source, count }) => (
                          <div
                            key={source}
                            className="h-1 rounded-full"
                            style={{
                              backgroundColor: SOURCE_COLORS[source],
                              width: `${Math.min(count * 3, 100)}%`,
                              minWidth: 4,
                            }}
                            title={`${source}: ${count}`}
                          />
                        ))}
                      </div>
                    )}

                    {/* Entry count badge */}
                    {cell.entries.length > 0 && (
                      <span className="text-[9px] text-muted mt-auto">{cell.entries.length}</span>
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

function getSourceBars(entries: TimelineEntry[]): { source: TimelineSource; count: number }[] {
  const counts = new Map<TimelineSource, number>();
  for (const e of entries) {
    counts.set(e.source, (counts.get(e.source) || 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([source, count]) => ({ source, count }))
    .sort((a, b) => b.count - a.count);
}
