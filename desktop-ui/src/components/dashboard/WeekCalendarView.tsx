import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import { minutesSinceMidnight, todayISO, toLocalISO } from "../../lib/dates";
import type { TimelineEntry } from "../../lib/types";
import { EMPTY_TIMELINE_RESPONSE } from "../../lib/types";
import { cn } from "../../lib/utils";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 48;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 12;
const HOUR_GUTTER = 40;

function getWeekRange(dateStr: string): { start: string; end: string; days: string[] } {
  const d = new Date(`${dateStr}T00:00:00`);
  // Go to Monday
  const dayOfWeek = d.getDay();
  const mondayOffset = dayOfWeek === 0 ? -6 : 1 - dayOfWeek;
  const monday = new Date(d);
  monday.setDate(d.getDate() + mondayOffset);

  const days: string[] = [];
  for (let i = 0; i < 7; i++) {
    const day = new Date(monday);
    day.setDate(monday.getDate() + i);
    days.push(toLocalISO(day));
  }
  return { start: days[0], end: days[6], days };
}

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function formatHour(h: number): string {
  if (h === 0) return "";
  if (h < 12) return `${h}a`;
  if (h === 12) return "12p";
  return `${h - 12}p`;
}

export function WeekCalendarView() {
  const { date } = useParams<{ date: string }>();
  const dateStr = date || todayISO();
  const { start, end, days } = useMemo(() => getWeekRange(dateStr), [dateStr]);
  const today = todayISO();

  const queryArgs = useMemo(() => ({ startDate: start, endDate: end }), [start, end]);
  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-scroll when week changes
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 8 * HOUR_HEIGHT;
    }
  }, [dateStr]);

  // Group entries by day
  const entriesByDay = useMemo(() => {
    const map = new Map<string, TimelineEntry[]>();
    for (const day of days) map.set(day, []);
    for (const entry of data.entries) {
      const day = toLocalISO(new Date(entry.startedAt));
      map.get(day)?.push(entry);
    }
    return map;
  }, [data.entries, days]);

  const hours = Array.from({ length: 24 }, (_, i) => i);
  const pxPerMin = HOUR_HEIGHT / 60;

  return (
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        {/* Day header */}
        <div className="flex border-b border-border" style={{ paddingLeft: HOUR_GUTTER }}>
          {days.map((day, i) => (
            <div
              key={day}
              className={cn(
                "flex-1 text-center py-1.5 text-xs",
                day === today ? "text-brand font-semibold" : "text-muted",
              )}
            >
              <div>{DAY_LABELS[i]}</div>
              <div className="text-[10px]">{new Date(`${day}T00:00:00`).getDate()}</div>
            </div>
          ))}
        </div>

        {loading && <div className="px-4 py-1 text-xs text-muted">Loading...</div>}

        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          <div className="relative" style={{ height: TOTAL_HEIGHT }}>
            {/* Hour lines */}
            {hours.map((h) => (
              <div
                key={h}
                className="absolute w-full flex items-start"
                style={{ top: h * HOUR_HEIGHT }}
              >
                <div
                  className="text-[9px] text-muted text-right pr-1 select-none"
                  style={{ width: HOUR_GUTTER }}
                >
                  {formatHour(h)}
                </div>
                <div className="flex-1 border-t border-border" />
              </div>
            ))}

            {/* Day columns */}
            <div className="absolute inset-0 flex" style={{ left: HOUR_GUTTER }}>
              {days.map((day) => {
                const dayEntries = entriesByDay.get(day) || [];
                return (
                  <div key={day} className="flex-1 relative border-r border-border last:border-r-0">
                    {dayEntries.map((entry) => {
                      const startMin = minutesSinceMidnight(entry.startedAt);
                      const dur = entry.durationSecs ?? 0;
                      const height = Math.max(
                        dur > 0 ? (dur / 60) * pxPerMin : MIN_BLOCK_HEIGHT,
                        MIN_BLOCK_HEIGHT,
                      );
                      const top = startMin * pxPerMin;

                      return (
                        <button
                          type="button"
                          key={entry.id}
                          onClick={() =>
                            setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)
                          }
                          className={cn(
                            "absolute left-0.5 right-0.5 rounded text-[9px] leading-tight overflow-hidden cursor-pointer px-0.5",
                            "hover:opacity-90 border border-white/10",
                            selectedEntry?.id === entry.id && "ring-1 ring-brand",
                          )}
                          style={{
                            top,
                            height,
                            backgroundColor: `color-mix(in oklch, ${entry.color} 30%, transparent)`,
                            borderLeftColor: entry.color,
                            borderLeftWidth: 2,
                          }}
                          title={entry.title}
                        >
                          <span className="text-secondary truncate block">{entry.title}</span>
                        </button>
                      );
                    })}

                    {/* Now line for today */}
                    {day === today && <WeekNowLine />}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      <SummaryPanel
        summary={data.summary}
        selectedEntry={selectedEntry}
        onClose={() => setSelectedEntry(null)}
      />
    </div>
  );
}

function WeekNowLine() {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const mins = now.getHours() * 60 + now.getMinutes();
  const top = mins * (HOUR_HEIGHT / 60);
  return (
    <div className="absolute w-full pointer-events-none z-10" style={{ top }}>
      <div className="border-t border-red-500" />
    </div>
  );
}
