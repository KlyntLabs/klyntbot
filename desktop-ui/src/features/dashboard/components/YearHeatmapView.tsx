import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, todayISO, toLocalISO } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { useMemo } from "react";
import { useNavigate, useParams } from "react-router";
import { SummaryPanel } from "./SummaryPanel";

const MONTH_NAMES = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];
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

function intensityClass(secs: number, maxSecs: number): string {
  if (secs === 0 || maxSecs === 0) return "bg-white/[0.03]";
  const ratio = secs / maxSecs;
  if (ratio > 0.75) return "bg-timeline-focus/60";
  if (ratio > 0.5) return "bg-timeline-focus/40";
  if (ratio > 0.25) return "bg-timeline-focus/25";
  return "bg-timeline-focus/10";
}

export function YearHeatmapView() {
  const { year: yearParam } = useParams<{ year: string }>();
  const navigate = useNavigate();
  const year = Number(yearParam) || new Date().getFullYear();

  const queryArgs = useMemo(
    () => ({ startDate: `${year}-01-01`, endDate: `${year}-12-31` }),
    [year],
  );
  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

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
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card p-4 overflow-y-auto">
        {loading && <div className="text-xs text-muted mb-2">Loading...</div>}

        <div className="grid grid-cols-3 gap-4">
          {Array.from({ length: 12 }, (_, monthIdx) => {
            const weeks = buildMonthGrid(year, monthIdx);
            return (
              <div key={monthIdx}>
                <div className="text-xs font-medium text-muted mb-1.5">{MONTH_NAMES[monthIdx]}</div>

                {/* Day-of-week labels */}
                <div className="grid grid-cols-7 gap-px mb-0.5">
                  {DAY_LABELS.map((label, i) => (
                    <div
                      key={`${monthIdx}-label-${i}`}
                      className="text-[8px] text-muted/50 text-center"
                    >
                      {label}
                    </div>
                  ))}
                </div>

                {/* Weeks */}
                {weeks.map((week, wi) => (
                  <div key={`${monthIdx}-w${wi}`} className="grid grid-cols-7 gap-px">
                    {week.map((day, di) =>
                      day ? (
                        <button
                          type="button"
                          key={day}
                          onClick={() => navigate(`/day/${day}`)}
                          className={cn(
                            "aspect-square rounded-[2px] transition-colors cursor-pointer",
                            intensityClass(dayMap.get(day) || 0, maxSecs),
                            day === today && "ring-1 ring-brand/60",
                          )}
                          title={`${day}: ${formatHumanDuration(dayMap.get(day) || 0)}`}
                        />
                      ) : (
                        <div key={`empty-${monthIdx}-${wi}-${di}`} className="aspect-square" />
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
          <span className="text-[10px] text-muted">Less focus</span>
          <div className="w-3 h-3 rounded-[2px] bg-white/[0.03]" />
          <div className="w-3 h-3 rounded-[2px] bg-timeline-focus/10" />
          <div className="w-3 h-3 rounded-[2px] bg-timeline-focus/25" />
          <div className="w-3 h-3 rounded-[2px] bg-timeline-focus/40" />
          <div className="w-3 h-3 rounded-[2px] bg-timeline-focus/60" />
          <span className="text-[10px] text-muted">More focus</span>
        </div>
      </div>

      <SummaryPanel summary={data.summary} selectedEntry={null} onClose={() => {}} />
    </div>
  );
}
