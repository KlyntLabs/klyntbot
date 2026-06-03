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
import { cn } from "@/utils/cn";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers, useSidebarOpen } from "../../lib/layers";
import { SummaryPanel } from "../SummaryPanel";

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
  const { sidebarOpen } = useSidebarOpen();
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
    const enabledSet = new Set<string>(enabledSources.map((s) => String(s)));
    const map = new Map<string, number>();
    for (const entry of data.entries) {
      if (!enabledSet.has(String(entry.source))) continue;
      const day = toLocalISO(new Date(entry.startedAt));
      map.set(day, (map.get(day) || 0) + (entry.durationSecs ?? 0));
    }
    let max = 0;
    for (const v of map.values()) {
      if (v > max) max = v;
    }
    return { dayMap: map, maxSecs: max };
  }, [data.entries, enabledSources]);

  const today = todayISO();

  return (
    <div className="flex gap-2 h-full w-full">
      <div className="flex gap-2 h-full flex-1">
        <div className="flex-1 bg-transparent border-none rounded-none p-4 overflow-y-auto">
          {isLoading && <div className="text-ui-2xs text-ds-text-subtle mb-1 px-2 py-1">Loading...</div>}

          <div className="grid grid-cols-3 gap-4">
            {Array.from({ length: 12 }, (_, monthIdx) => {
              const weeks = buildMonthGrid(year, monthIdx);
              const monthName = SHORT_MONTHS[monthIdx];
              return (
                <div key={monthName}>
                  <div className="text-ui-xs font-medium text-text-muted mb-1.5">{monthName}</div>

                  {/* Day-of-week labels */}
                  <div className="grid grid-cols-7 gap-0.5 mb-0.5">
                    {DAY_LABELS.map((label, i) => (
                      <div
                        // biome-ignore lint/suspicious/noArrayIndexKey: static 7 day-of-week labels with duplicates
                        key={`${monthName}-label-${i}`}
                        className="text-ui-3xs text-[color-mix(in_oklch,var(--text-muted)_50%,transparent)] text-center"
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
                      className="grid grid-cols-7 gap-0.5 mb-0.5"
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
                            className={cn(
                              "aspect-square rounded-sm transition-colors duration-150 ease-out cursor-pointer border-none p-0.5 flex items-start justify-start hover:brightness-[1.2]",
                              day === today && "outline outline-1 outline-[color-mix(in_oklch,var(--border-accent)_60%,transparent)]",
                            )}
                            style={intensityStyle(dayMap.get(day) || 0, maxSecs)}
                            title={`${day}: ${formatHumanDuration(dayMap.get(day) || 0)}`}
                          >
                            <span className="text-ui-2xs text-[color-mix(in_oklch,var(--text-muted)_80%,transparent)] leading-none tabular-nums">
                              {parseInt(day.slice(8, 10), 10)}
                            </span>
                          </button>
                        ) : (
                          <div
                            // biome-ignore lint/suspicious/noArrayIndexKey: empty calendar padding cells
                            key={`empty-${monthName}-${wi}-${di}`}
                            className="aspect-square rounded-sm bg-transparent"
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
          <div className="flex items-center gap-2 mt-4 justify-center">
            <span className="text-ui-2xs text-text-muted">Less focus</span>
            <div
              className="w-3 h-3 rounded-sm"
              style={{ background: "color-mix(in oklch, var(--text-muted) 10%, transparent)" }}
            />
            <div
              className="w-3 h-3 rounded-sm"
              style={{ background: "color-mix(in oklch, var(--timeline-focus) 10%, transparent)" }}
            />
            <div
              className="w-3 h-3 rounded-sm"
              style={{ background: "color-mix(in oklch, var(--timeline-focus) 25%, transparent)" }}
            />
            <div
              className="w-3 h-3 rounded-sm"
              style={{ background: "color-mix(in oklch, var(--timeline-focus) 40%, transparent)" }}
            />
            <div
              className="w-3 h-3 rounded-sm"
              style={{ background: "color-mix(in oklch, var(--timeline-focus) 60%, transparent)" }}
            />
            <span className="text-ui-2xs text-text-muted">More focus</span>
          </div>
        </div>
      </div>
      {sidebarOpen && <SummaryPanel summary={null} selectedEntry={null} onClose={() => {}} />}
    </div>
  );
}
