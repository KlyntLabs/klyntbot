import { useMemo } from "react";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  SHORT_MONTHS,
  TZ_OFFSET_MINS,
  todayISO,
  toLocalISO,
} from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";

const DAY_LABELS = ["M", "", "W", "", "F", "", ""];

function buildMonthGrid(year: number, month: number): (string | null)[][] {
  const first = new Date(year, month, 1);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  // Monday = 0 offset
  const startDay = first.getDay() === 0 ? 6 : first.getDay() - 1;

  const weeks: (string | null)[][] = [];
  let week: (string | null)[] = Array.from({ length: startDay }, () => null);

  for (let d = 1; d <= daysInMonth; d++) {
    week.push(toLocalISO(new Date(year, month, d)));
    if (week.length === 7) {
      weeks.push(week);
      week = [];
    }
  }
  // Pad last week
  if (week.length > 0) {
    while (week.length < 7) week.push(null);
    weeks.push(week);
  }
  return weeks;
}

function intensityStyle(secs: number, maxSecs: number): React.CSSProperties {
  if (secs === 0 || maxSecs === 0) {
    return { background: "color-mix(in oklch, var(--text-muted) 10%, transparent)" };
  }
  const ratio = secs / maxSecs;
  if (ratio > 0.75) {
    return { background: "color-mix(in oklch, var(--timeline-focus) 60%, transparent)" };
  }
  if (ratio > 0.5) {
    return { background: "color-mix(in oklch, var(--timeline-focus) 40%, transparent)" };
  }
  if (ratio > 0.25) {
    return { background: "color-mix(in oklch, var(--timeline-focus) 25%, transparent)" };
  }
  return { background: "color-mix(in oklch, var(--timeline-focus) 10%, transparent)" };
}

export function YearView() {
  const { date, setDate, setMode } = useDashboardState();
  const year = parseInt(date.slice(0, 4), 10) || new Date().getFullYear();

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const startDate = `${year}-01-01`;
  const endDate = `${year}-12-31`;

  const { data, isLoading } = useTauriQuery({
    queryKey: qk.dashboard.timeline(startDate, endDate, sourcesKey),
    queryFn: () => timelineQuery(startDate, endDate, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  // Aggregate focus seconds per day
  const { dayMap, maxSecs } = useMemo(() => {
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
    return { dayMap: map, maxSecs: max };
  }, [data.entries]);

  const today = todayISO();

  return (
    <div className="dashboard__year-grid">
      <div className="dashboard__year-grid-inner">
        {isLoading && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)", marginBottom: 8 }}>
            Loading...
          </div>
        )}

        <div className="dashboard__year-months">
          {Array.from({ length: 12 }, (_, monthIdx) => {
            const weeks = buildMonthGrid(year, monthIdx);
            const monthName = SHORT_MONTHS[monthIdx];
            return (
              <div key={monthName}>
                <div className="dashboard__year-month-name">{monthName}</div>

                {/* Day-of-week labels */}
                <div className="dashboard__year-week" style={{ marginBottom: 2 }}>
                  {DAY_LABELS.map((label, i) => (
                    <div
                      // biome-ignore lint/suspicious/noArrayIndexKey: static 7 day-of-week labels with duplicates
                      key={`${monthName}-label-${i}`}
                      className="dashboard__year-dow-label"
                    >
                      {label}
                    </div>
                  ))}
                </div>

                {/* Weeks */}
                {weeks.map((week, wi) => (
                  <div
                    // biome-ignore lint/suspicious/noArrayIndexKey: week rows within month have no unique ID
                    key={`${monthName}-w${wi}`}
                    className="dashboard__year-week"
                  >
                    {week.map((day, di) =>
                      day ? (
                        <button
                          type="button"
                          key={day}
                          onClick={() => {
                            setDate(day);
                            setMode("day");
                          }}
                          className={
                            day === today
                              ? "dashboard__year-day dashboard__year-day--today"
                              : "dashboard__year-day"
                          }
                          style={intensityStyle(dayMap.get(day) || 0, maxSecs)}
                          title={`${day}: ${formatHumanDuration(dayMap.get(day) || 0)}`}
                        >
                          <span className="dashboard__year-day-num">
                            {parseInt(day.slice(8, 10), 10)}
                          </span>
                        </button>
                      ) : (
                        <div
                          // biome-ignore lint/suspicious/noArrayIndexKey: empty calendar padding cells
                          key={`empty-${monthName}-${wi}-${di}`}
                          className="dashboard__year-day-pad"
                        />
                      ),
                    )}
                  </div>
                ))}
              </div>
            );
          })}
        </div>

        {/* Legend */}
        <div className="dashboard__year-legend">
          <span className="dashboard__year-legend-label">Less focus</span>
          <div
            className="dashboard__year-legend-swatch"
            style={{ background: "color-mix(in oklch, var(--text-muted) 10%, transparent)" }}
          />
          <div
            className="dashboard__year-legend-swatch"
            style={{ background: "color-mix(in oklch, var(--timeline-focus) 10%, transparent)" }}
          />
          <div
            className="dashboard__year-legend-swatch"
            style={{ background: "color-mix(in oklch, var(--timeline-focus) 25%, transparent)" }}
          />
          <div
            className="dashboard__year-legend-swatch"
            style={{ background: "color-mix(in oklch, var(--timeline-focus) 40%, transparent)" }}
          />
          <div
            className="dashboard__year-legend-swatch"
            style={{ background: "color-mix(in oklch, var(--timeline-focus) 60%, transparent)" }}
          />
          <span className="dashboard__year-legend-label">More focus</span>
        </div>
      </div>
    </div>
  );
}
