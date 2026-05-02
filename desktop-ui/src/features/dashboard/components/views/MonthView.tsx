import { useCallback, useMemo, useState } from "react";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import type { TimelineEntry } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, TZ_OFFSET_MINS, todayISO, toLocalISO } from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";
import { computeDayStats, DAY_LABELS } from "../../lib/timeline-utils";

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

export function MonthView() {
  const { date, setDate, setMode } = useDashboardState();
  const dateStr = date || todayISO();
  const today = todayISO();

  const { start, end, year, month } = useMemo(() => getMonthRange(dateStr), [dateStr]);

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(start, end, sourcesKey),
    queryFn: () => timelineQuery(start, end, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

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
        setDate(focusedDate);
        setMode("day");
        e.preventDefault();
        return;
      } else return;

      e.preventDefault();
      const next = new Date(`${focusedDate}T00:00:00`);
      next.setDate(next.getDate() + delta);
      setFocusedDate(toLocalISO(next));
    },
    [focusedDate, setDate, setMode],
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
    <div className="dashboard__month-grid">
      <div className="dashboard__month-grid-inner">
        {isLoading && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)", marginBottom: 4 }}>
            Loading...
          </div>
        )}

        {/* Day-of-week header */}
        <div className="dashboard__month-dow-header">
          {DAY_LABELS.map((label) => (
            <div key={label} className="dashboard__month-dow-label">
              {label}
            </div>
          ))}
        </div>

        {/* Calendar grid */}
        {/* biome-ignore lint/a11y/useSemanticElements: CSS grid layout requires div, not table */}
        <div
          style={{
            flex: 1,
            display: "grid",
            gridTemplateRows: "repeat(6, 1fr)",
            gap: 1,
            outline: "none",
          }}
          role="grid"
          aria-label="Month calendar"
          tabIndex={0}
          onKeyDown={handleGridKeyDown}
        >
          {weeks.map((week) => (
            <div key={week[0].date} className="dashboard__month-week">
              {week.map((cell) => {
                const stats = dayStats.get(cell.date) || { activeSecs: 0, focusSecs: 0 };
                const aRatio = activeRatio(stats.activeSecs, maxActiveSecs);

                const isToday = cell.date === today;
                const isFocused = cell.date === focusedDate && cell.date !== today;

                const dayClasses = [
                  "dashboard__month-day",
                  isToday && "dashboard__month-day--today",
                  isFocused && "dashboard__month-day--focused",
                  !cell.isCurrentMonth && "dashboard__month-day--other",
                ]
                  .filter(Boolean)
                  .join(" ");

                return (
                  <button
                    type="button"
                    key={cell.date}
                    className={dayClasses}
                    onClick={() => {
                      setDate(cell.date);
                      setMode("day");
                    }}
                    style={{
                      backgroundColor: focusIntensityBg(stats.focusSecs, maxFocusSecs),
                    }}
                  >
                    {/* Date + focus time */}
                    <div className="dashboard__month-day-header">
                      <span
                        className={[
                          "dashboard__month-day-number",
                          isToday && "dashboard__month-day-number--today",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                      >
                        {cell.day}
                      </span>
                      {stats.focusSecs > 0 && (
                        <span className="dashboard__month-day-focus">
                          {formatHumanDuration(stats.focusSecs)}
                        </span>
                      )}
                    </div>

                    {/* Activity bar — proportional to active time */}
                    {stats.activeSecs > 0 && (
                      <div
                        style={{
                          width: "100%",
                          marginTop: "auto",
                          display: "flex",
                          flexDirection: "column",
                          gap: 2,
                        }}
                      >
                        <div className="dashboard__month-activity-track">
                          <div
                            className="dashboard__month-activity-fill"
                            style={{
                              width: `${Math.max(aRatio * 100, 8)}%`,
                              opacity: 0.7 + aRatio * 0.3,
                            }}
                          />
                        </div>
                        <span className="dashboard__month-activity-label">
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
    </div>
  );
}
