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
    <div style={{ display: "flex", gap: 8, height: "100%", width: "100%" }}>
      <div className="dashboard__week-grid" style={{ flex: 1 }}>
        <div className="dashboard__week-grid-inner">
          {isLoading && <div className="dashboard__week-loading">Loading...</div>}

          {/* Header row */}
          <div className="dashboard__week-header">
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
                  className={`dashboard__week-header-cell${isToday ? " dashboard__week-header-cell--today" : ""}`}
                  onClick={() => {
                    setDate(day);
                    setMode("day");
                  }}
                >
                  <div className="dashboard__week-header-day">
                    {d.toLocaleDateString("en-US", { weekday: "short" })}
                  </div>
                  <div>{d.getDate()}</div>
                  {activeSecs > 0 && (
                    <div className="dashboard__week-day-active">
                      {formatHumanDuration(activeSecs)}
                    </div>
                  )}
                </button>
              );
            })}
          </div>

          {/* Scrollable grid */}
          <div className="dashboard__week-scroll">
            <div className="dashboard__week-columns">
              {/* Hour gutter */}
              <div className="dashboard__day-gutter">
                {HOURS.map((h) => (
                  <div
                    key={h}
                    data-testid="hour-label"
                    className="dashboard__day-hour-label"
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
                  <div key={day} className="dashboard__day-column">
                    {/* Hour lines */}
                    {HOURS.map((h) => (
                      <div
                        key={h}
                        className="dashboard__hour-row"
                        style={{ top: h * HOUR_HEIGHT, height: HOUR_HEIGHT }}
                      >
                        <div className="dashboard__hour-line" />
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
                            className="dashboard__week-focus-bar"
                            style={{ top, height }}
                          />
                        );
                      }

                      return (
                        <button
                          key={`activity-${s.startMin}`}
                          type="button"
                          className="dashboard__week-session"
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
                          <span className="dashboard__week-session-label">{s.label}</span>
                          {s.appCount > 1 && (
                            <span className="dashboard__week-session-meta">
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
    <div className="dashboard__now-line" style={{ top }}>
      <div style={{ borderTop: "1px solid var(--destructive)" }} />
    </div>
  );
}
