import { useQuery } from "@shared/hooks/useQuery";
import {
  formatHumanDuration,
  minutesSinceMidnight,
  TZ_OFFSET_MINS,
  todayISO,
  toLocalISO,
} from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useEnabledLayers, useSidebarOpen } from "../lib/layers";
import { computeDayStats, DAY_LABELS, IDLE_APPS } from "../lib/timeline-utils";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 48;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 4;
const HOUR_GUTTER = 40;
const PX_PER_MIN = HOUR_HEIGHT / 60;
const SESSION_GAP_MIN = 10; // aggressive merge for week overview (day view has 2min precision)
const MIN_ENTRY_SECS = 30; // ignore entries shorter than 30 seconds
const MIN_SESSION_SECS = 120; // hide merged sessions shorter than 2 minutes in week overview
const HOURS = Array.from({ length: 24 }, (_, i) => i);

function getWeekRange(dateStr: string): { start: string; end: string; days: string[] } {
  const d = new Date(`${dateStr}T00:00:00`);
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

function formatHour(h: number): string {
  if (h === 0) return "";
  if (h < 12) return `${h}a`;
  if (h === 12) return "12p";
  return `${h - 12}p`;
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

interface BuildingSession {
  startMin: number;
  endMin: number;
  totalSecs: number;
  appDurations: Map<string, number>;
}

/**
 * Merge ALL adjacent activity entries into consolidated sessions.
 * Unlike the old approach (same-app only), this merges across apps
 * to produce clean, unfragmented session blocks.
 */
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

  // Merge adjacent activity regardless of app within SESSION_GAP_MIN
  const sessions: WeekSession[] = [];
  let cur: BuildingSession | null = null;

  for (const entry of activityEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const dur = (entry.durationSecs ?? 0) / 60;
    const endMin = startMin + dur;

    if (cur && startMin - cur.endMin <= SESSION_GAP_MIN) {
      // Extend current session
      cur.endMin = Math.max(cur.endMin, endMin);
      cur.totalSecs += entry.durationSecs ?? 0;
      cur.appDurations.set(
        entry.title,
        (cur.appDurations.get(entry.title) || 0) + (entry.durationSecs ?? 0),
      );
    } else {
      // Flush previous session
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

  // Filter out very short sessions that clutter the week overview
  const filtered = sessions.filter((s) => s.totalSecs >= MIN_SESSION_SECS);

  // Add focus sessions as separate overlay blocks
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

function finishSession(cur: {
  startMin: number;
  endMin: number;
  totalSecs: number;
  appDurations: Map<string, number>;
}): WeekSession {
  // Find dominant app (longest duration)
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

/**
 * Session density → opacity. Longer sessions = more opaque.
 * Short sessions (<5min) are lighter, long sessions (>30min) are solid.
 */
function sessionOpacity(totalSecs: number): number {
  const mins = totalSecs / 60;
  if (mins >= 30) return 0.85;
  if (mins >= 15) return 0.75;
  if (mins >= 5) return 0.65;
  return 0.5;
}

export function WeekCalendarView() {
  const { date } = useParams<{ date: string }>();
  const navigate = useNavigate();
  const dateStr = date || todayISO();
  const { start, end, days } = useMemo(() => getWeekRange(dateStr), [dateStr]);
  const today = todayISO();

  const { enabledSources } = useEnabledLayers();
  const sidebarOpen = useSidebarOpen();
  const queryArgs = useMemo(
    () => ({
      startDate: start,
      endDate: end,
      sources: enabledSources,
      tzOffsetMins: TZ_OFFSET_MINS,
    }),
    [start, end, enabledSources],
  );
  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-scroll when week changes
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 8 * HOUR_HEIGHT;
    }
  }, [dateStr]);

  // Group entries by day, build merged sessions, pre-split by type, compute active time
  const { dayData, activeByDay } = useMemo(() => {
    const entryMap = new Map<string, TimelineEntry[]>();
    for (const day of days) entryMap.set(day, []);
    for (const entry of data.entries) {
      const day = toLocalISO(new Date(entry.startedAt));
      entryMap.get(day)?.push(entry);
    }
    const dMap = new Map<string, { activity: WeekSession[]; focus: WeekSession[] }>();
    const actMap = new Map<string, number>();
    for (const [day, dayEntries] of entryMap) {
      const sessions = buildWeekSessions(dayEntries);
      const activity = sessions.filter((s) => s.type === "activity");
      const focus = sessions.filter((s) => s.type === "focus");
      // Pre-compute hasFocus per activity session
      for (const a of activity) {
        a.hasFocus = focus.some((f) => f.startMin <= a.endMin && f.endMin >= a.startMin);
      }
      dMap.set(day, { activity, focus });
      actMap.set(day, computeDayStats(dayEntries).activeSecs);
    }
    return { dayData: dMap, activeByDay: actMap };
  }, [data.entries, days]);

  return (
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        {/* Day header with active time */}
        <div className="flex border-b border-border" style={{ paddingLeft: HOUR_GUTTER }}>
          {days.map((day, i) => {
            const activeSecs = activeByDay.get(day) || 0;
            return (
              <button
                type="button"
                key={day}
                onClick={() => navigate(`/day/${day}`)}
                className={cn(
                  "flex-1 text-center py-1.5 text-xs cursor-pointer hover:bg-surface-lowest transition-colors",
                  day === today ? "text-brand font-semibold" : "text-muted",
                )}
              >
                <div>{DAY_LABELS[i]}</div>
                <div className="text-[10px]">{new Date(`${day}T00:00:00`).getDate()}</div>
                {activeSecs > 0 && (
                  <div className="text-[9px] text-secondary/60 mt-0.5">
                    {formatHumanDuration(activeSecs)}
                  </div>
                )}
              </button>
            );
          })}
        </div>

        {loading && <div className="px-4 py-1 text-xs text-muted">Loading...</div>}

        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          <div className="relative" style={{ height: TOTAL_HEIGHT }}>
            {/* Hour lines */}
            {HOURS.map((h) => (
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
                const { activity: activitySessions, focus: focusSessions } = dayData.get(day) || {
                  activity: [],
                  focus: [],
                };

                return (
                  <div key={day} className="flex-1 relative border-r border-border last:border-r-0">
                    {/* Activity session blocks — clean, merged bars */}
                    {activitySessions.map((session) => {
                      const top = session.startMin * PX_PER_MIN;
                      const height = Math.max(
                        (session.endMin - session.startMin) * PX_PER_MIN,
                        MIN_BLOCK_HEIGHT,
                      );
                      const leftOffset = session.hasFocus ? 5 : 2;
                      const durationLabel = formatHumanDuration(session.totalSecs);
                      const appSuffix = session.appCount > 1 ? ` +${session.appCount - 1}` : "";

                      return (
                        <button
                          type="button"
                          key={`s-${session.startMin}`}
                          onClick={() => navigate(`/day/${day}`)}
                          className="absolute rounded-[3px] cursor-pointer hover:brightness-125 transition-all overflow-hidden"
                          style={{
                            top,
                            height,
                            left: leftOffset,
                            right: 2,
                            backgroundColor: "var(--success)",
                            opacity: sessionOpacity(session.totalSecs),
                          }}
                          title={`${session.label}${appSuffix} · ${durationLabel}`}
                        >
                          {height > 20 && (
                            <span className="text-[8px] text-primary font-medium px-1 truncate block leading-tight mt-0.5">
                              {session.label}
                            </span>
                          )}
                          {height > 32 && (
                            <span className="text-[7px] text-white/60 px-1 truncate block">
                              {durationLabel}
                            </span>
                          )}
                        </button>
                      );
                    })}

                    {/* Focus session overlays — thin left accent bar */}
                    {focusSessions.map((session) => {
                      const top = session.startMin * PX_PER_MIN;
                      const height = Math.max(
                        (session.endMin - session.startMin) * PX_PER_MIN,
                        MIN_BLOCK_HEIGHT,
                      );

                      return (
                        <div
                          key={`f-${session.startMin}`}
                          className="absolute left-0 w-[3px] rounded-sm pointer-events-none"
                          style={{
                            top,
                            height,
                            backgroundColor: "var(--timeline-focus)",
                            opacity: 0.9,
                          }}
                        />
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
    <div className="absolute w-full pointer-events-none z-10" style={{ top }}>
      <div className="border-t border-red-500" />
    </div>
  );
}
