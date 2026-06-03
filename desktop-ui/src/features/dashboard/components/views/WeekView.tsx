import { useEffect, useMemo, useState } from "react";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import type { TimelineEntry } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import {
  formatHumanDuration,
  minutesSinceMidnight,
  shiftDate,
  TZ_OFFSET_MINS,
  todayISO,
  toLocalISO,
  weekStartISO,
} from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers, useSidebarOpen } from "../../lib/layers";
import { computeDayStats, IDLE_APPS } from "../../lib/timeline-utils";
import { SummaryPanel } from "../SummaryPanel";

const HOUR_HEIGHT = 60;
const PX_PER_MIN = HOUR_HEIGHT / 60;
const HOURS = Array.from({ length: 24 }, (_, i) => i);

const SESSION_GAP_MIN = 10;
const MIN_ENTRY_SECS = 30;
const MIN_SESSION_SECS = 120;

interface BuildingSession {
  startMin: number;
  endMin: number;
  totalSecs: number;
  appDurations: Map<string, number>;
}

interface WeekSession {
  startMin: number;
  endMin: number;
  totalSecs: number;
  label: string;
  appCount: number;
  type: "activity" | "focus";
  hasFocus?: boolean;
}

function buildWeekSessions(entries: TimelineEntry[]): WeekSession[] {
  const activityEntries: TimelineEntry[] = [];
  const focusEntries: TimelineEntry[] = [];

  for (const e of entries) {
    if (e.entryType === "focusSession") {
      focusEntries.push(e);
    } else if (
      e.entryType === "appUsage" &&
      (e.durationSecs ?? 0) >= MIN_ENTRY_SECS &&
      !IDLE_APPS.has(e.title.toLowerCase())
    ) {
      activityEntries.push(e);
    }
  }
  activityEntries.sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime());

  const sessions: WeekSession[] = [];
  let cur: BuildingSession | null = null;

  for (const entry of activityEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const dur = (entry.durationSecs ?? 0) / 60;
    const endMin = startMin + dur;

    if (cur && startMin - cur.endMin <= SESSION_GAP_MIN) {
      cur.endMin = Math.max(cur.endMin, endMin);
      cur.totalSecs += entry.durationSecs ?? 0;
      cur.appDurations.set(
        entry.title,
        (cur.appDurations.get(entry.title) || 0) + (entry.durationSecs ?? 0),
      );
    } else {
      if (cur) {
        sessions.push(finishSession(cur));
      }
      const appDurations = new Map<string, number>();
      appDurations.set(entry.title, entry.durationSecs ?? 0);
      cur = { startMin, endMin, totalSecs: entry.durationSecs ?? 0, appDurations };
    }
  }
  if (cur) {
    sessions.push(finishSession(cur));
  }

  const filtered = sessions.filter((s) => s.totalSecs >= MIN_SESSION_SECS);

  // Annotate activity sessions with hasFocus when overlapped by a focus session
  for (const a of filtered) {
    a.hasFocus = focusEntries.some((f) => {
      const fStart = minutesSinceMidnight(f.startedAt);
      const fEnd = fStart + (f.durationSecs ?? 0) / 60;
      return fStart <= a.endMin && fEnd >= a.startMin;
    });
  }

  for (const entry of focusEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const endMin = startMin + (entry.durationSecs ?? 0) / 60;
    filtered.push({
      startMin,
      endMin: Math.max(endMin, startMin + 1),
      totalSecs: entry.durationSecs ?? 0,
      label: entry.title || "Focus",
      appCount: 1,
      type: "focus",
    });
  }

  return filtered;
}

function finishSession(cur: BuildingSession): WeekSession {
  let dominantApp = "Activity";
  let maxDur = 0;
  for (const [app, dur] of cur.appDurations) {
    if (dur > maxDur) {
      maxDur = dur;
      dominantApp = app;
    }
  }
  return {
    startMin: cur.startMin,
    endMin: cur.endMin,
    totalSecs: cur.totalSecs,
    label: dominantApp,
    appCount: cur.appDurations.size,
    type: "activity",
  };
}

function sessionOpacity(totalSecs: number): number {
  const mins = totalSecs / 60;
  if (mins >= 30) return 0.85;
  if (mins >= 15) return 0.75;
  if (mins >= 5) return 0.65;
  return 0.5;
}

export function WeekView() {
  const { date, setDate, setMode } = useDashboardState();
  const { sidebarOpen } = useSidebarOpen();
  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const dateStr = date || todayISO();
  const monday = weekStartISO(dateStr);
  const today = todayISO();

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const days = useMemo(() => {
    const d: string[] = [];
    for (let i = 0; i < 7; i++) {
      d.push(shiftDate(monday, i));
    }
    return d;
  }, [monday]);

  const queryArgs = useMemo(() => ({ startDate: days[0], endDate: days[6] }), [days]);
  const timelineQueryKey = qk.dashboard.timeline(
    queryArgs.startDate,
    queryArgs.endDate,
    sourcesKey,
  );

  const { data, isLoading } = useTauriQuery({
    queryKey: timelineQueryKey,
    queryFn: () =>
      timelineQuery(queryArgs.startDate, queryArgs.endDate, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  const { entriesByDay, activeByDay } = useMemo(() => {
    const entryMap = new Map<string, TimelineEntry[]>();
    const actMap = new Map<string, number>();
    for (const day of days) entryMap.set(day, []);
    for (const entry of data.entries) {
      const day = toLocalISO(new Date(entry.startedAt));
      if (!entryMap.has(day)) entryMap.set(day, []);
      entryMap.get(day)?.push(entry);
    }
    for (const [day, dayEntries] of entryMap) {
      actMap.set(day, computeDayStats(dayEntries).activeSecs);
    }
    return { entriesByDay: entryMap, activeByDay: actMap };
  }, [data.entries, days]);

  return (
    <div className="flex gap-2 h-full w-full">
      <div className="flex gap-2 h-full flex-1">
        <div className="flex-1 flex flex-col overflow-hidden bg-transparent border-none rounded-none">
          {isLoading && <div className="text-ui-2xs text-ds-text-subtle mb-1 px-2 py-1">Loading...</div>}

          {/* Header row */}
          <div className="grid grid-cols-[48px_repeat(7,1fr)] border-b border-border-subtle">
            <div /> {/* gutter */}
            {days.map((day) => {
              const isToday = day === today;
              const d = new Date(`${day}T00:00:00`);
              const activeSecs = activeByDay.get(day) || 0;
              return (
                <button
                  key={day}
                  data-testid="week-day-header"
                  type="button"
                  className={cn(
                    "p-2 text-center text-ui-xs font-medium cursor-pointer transition-colors duration-150 ease-out text-text-muted bg-transparent border-none hover:text-text-strong",
                    isToday && "text-border-accent bg-[color-mix(in_oklch,var(--border-accent)_5%,transparent)]",
                  )}
                  onClick={() => {
                    setDate(day);
                    setMode("day");
                  }}
                >
                  <div className="text-ui-2xs uppercase">{d.toLocaleDateString("en-US", { weekday: "short" })}</div>
                  <div>{d.getDate()}</div>
                  {activeSecs > 0 && (
                    <div className="text-ui-2xs text-ds-text-subtle mt-0.5">{formatHumanDuration(activeSecs)}</div>
                  )}
                </button>
              );
            })}
          </div>

          {/* Scrollable grid */}
          <div className="flex-1 overflow-y-auto">
            <div className="grid grid-cols-[48px_repeat(7,1fr)] relative w-full">
              {/* Hour gutter */}
              <div className="w-12 shrink-0 border-r border-border-subtle">
                {HOURS.map((h) => (
                  <div
                    key={h}
                    data-testid="hour-label"
                    className="text-ui-2xs text-text-muted text-right pr-1.5"
                    style={{ height: HOUR_HEIGHT }}
                  >
                    {h === 0 ? "" : `${h}:00`}
                  </div>
                ))}
              </div>

              {/* Day columns */}
              {days.map((day) => {
                const dayEntries = entriesByDay.get(day) || [];
                const sessions = buildWeekSessions(dayEntries);

                return (
                  <div key={day} className="flex-1 relative border-r border-border-subtle last:border-r-0">
                    {/* Hour lines */}
                    {HOURS.map((h) => (
                      <div
                        key={h}
                        className="absolute w-full flex items-start"
                        style={{ top: h * HOUR_HEIGHT, height: HOUR_HEIGHT }}
                      >
                        <div className="flex-1 border-t border-border-subtle" />
                      </div>
                    ))}

                    {/* Sessions */}
                    {sessions.map((s) => {
                      const top = s.startMin * PX_PER_MIN;
                      const height = Math.max((s.endMin - s.startMin) * PX_PER_MIN, 4);

                      if (s.type === "focus") {
                        return (
                          <div
                            key={`focus-${s.startMin}`}
                            className="absolute left-0 w-[3px] rounded-sm pointer-events-none bg-[var(--timeline-focus)] opacity-90"
                            style={{ top, height }}
                          />
                        );
                      }

                      return (
                        <button
                          key={`activity-${s.startMin}`}
                          type="button"
                          className="absolute rounded-[3px] cursor-pointer transition-[filter] duration-150 ease-out overflow-hidden border-none dashboard__week-session hover:brightness-[1.15]"
                          aria-label={`${s.label}, ${formatHumanDuration(s.totalSecs)}`}
                          style={{
                            top,
                            height,
                            left: s.hasFocus ? 5 : 2,
                            right: 2,
                            opacity: sessionOpacity(s.totalSecs),
                          }}
                          onClick={() => {
                            setSelectedEntry({
                              id: `week-${day}-${s.startMin}`,
                              title: s.label,
                              description: null,
                              startedAt: new Date(`${day}T00:00:00`).toISOString(),
                              endedAt: null,
                              durationSecs: s.totalSecs,
                              source: "productivity",
                              entryType: "appUsage",
                              color: "var(--timeline-app-productive)",
                              metadata: null,
                              entityId: null,
                              entityRoute: null,
                            });
                          }}
                        >
                          <span className="block text-ui-2xs text-text-strong font-medium px-1 mt-0.5 whitespace-nowrap overflow-hidden text-ellipsis leading-tight">
                            {s.label}
                          </span>
                          {s.appCount > 1 && (
                            <span className="block text-ui-3xs text-white/60 px-1 whitespace-nowrap overflow-hidden text-ellipsis">
                              {s.appCount} apps · {Math.round(s.totalSecs / 60)}m
                            </span>
                          )}
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
      {sidebarOpen && (
        <SummaryPanel
          summary={data.summary}
          selectedEntry={selectedEntry}
          onClose={() => setSelectedEntry(null)}
        />
      )}
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
  const top = mins * PX_PER_MIN;
  return (
    <div className="absolute w-full pointer-events-none dashboard__now-line" style={{ top }}>
      <div className="border-t border-destructive" />
    </div>
  );
}

function cn(...inputs: (string | false | undefined)[]) {
  return inputs.filter(Boolean).join(" ");
}
