import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import {
  formatHumanDuration,
  minutesSinceMidnight,
  TZ_OFFSET_MINS,
  todayISO,
  toLocalISO,
} from "../../lib/dates";
import type { TimelineEntry } from "../../lib/types";
import { EMPTY_TIMELINE_RESPONSE } from "../../lib/types";
import { cn } from "../../lib/utils";
import { useEnabledLayers, useSidebarOpen } from "./layers";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 48;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 4;
const HOUR_GUTTER = 40;
const PX_PER_MIN = HOUR_HEIGHT / 60;
const MERGE_GAP_MIN = 3; // merge blocks within 3 minutes

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

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function formatHour(h: number): string {
  if (h === 0) return "";
  if (h < 12) return `${h}a`;
  if (h === 12) return "12p";
  return `${h - 12}p`;
}

/** Assign a hue-based color per app name for visual differentiation */
const APP_COLORS = new Map<string, string>();
const PALETTE = [
  "oklch(0.65 0.15 250)", // blue
  "oklch(0.65 0.15 160)", // teal
  "oklch(0.65 0.15 300)", // purple
  "oklch(0.65 0.15 30)", // orange
  "oklch(0.65 0.15 130)", // green
  "oklch(0.65 0.15 350)", // pink
  "oklch(0.65 0.15 60)", // yellow
  "oklch(0.65 0.15 200)", // cyan
];

function appColor(appName: string): string {
  // Skip idle/system entries — render them dimmer
  const lower = appName.toLowerCase();
  if (lower === "loginwindow" || lower === "idle" || lower === "screensaver") {
    return "var(--surface-highest)";
  }
  let color = APP_COLORS.get(appName);
  if (!color) {
    color = PALETTE[APP_COLORS.size % PALETTE.length];
    APP_COLORS.set(appName, color);
  }
  return color;
}

interface WeekBlock {
  startMin: number;
  endMin: number;
  color: string;
  label: string;
  totalSecs: number;
  type: "activity" | "focus";
}

const IDLE_APPS = new Set(["loginwindow", "idle", "screensaver", "lock screen", "desktop"]);

/** Merge adjacent same-app activity entries into visual blocks */
function buildWeekBlocks(entries: TimelineEntry[]): WeekBlock[] {
  const activityEntries = entries.filter(
    (e) => e.entryType === "appUsage" && e.durationSecs && !IDLE_APPS.has(e.title.toLowerCase()),
  );
  const focusEntries = entries.filter((e) => e.entryType === "focusSession");

  // Sort activity by start time
  const sorted = [...activityEntries].sort(
    (a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime(),
  );

  // Merge adjacent entries for the same app within MERGE_GAP_MIN
  const blocks: WeekBlock[] = [];
  for (const entry of sorted) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const endMin = startMin + (entry.durationSecs ?? 0) / 60;
    const color = appColor(entry.title);

    const last = blocks[blocks.length - 1];
    if (
      last &&
      last.type === "activity" &&
      last.label === entry.title &&
      startMin - last.endMin <= MERGE_GAP_MIN
    ) {
      // Merge into previous block (same app)
      last.endMin = Math.max(last.endMin, endMin);
      last.totalSecs += entry.durationSecs ?? 0;
    } else {
      blocks.push({
        startMin,
        endMin,
        color,
        label: entry.title,
        totalSecs: entry.durationSecs ?? 0,
        type: "activity",
      });
    }
  }

  // Add focus sessions as separate overlay blocks
  for (const entry of focusEntries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const endMin = startMin + (entry.durationSecs ?? 0) / 60;
    blocks.push({
      startMin,
      endMin: Math.max(endMin, startMin + 1),
      color: "var(--timeline-focus)",
      label: entry.title || "Focus",
      totalSecs: entry.durationSecs ?? 0,
      type: "focus",
    });
  }

  return blocks;
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

  // Build visual blocks per day from raw entries
  const blocksByDay = useMemo(() => {
    const entryMap = new Map<string, TimelineEntry[]>();
    for (const day of days) entryMap.set(day, []);
    for (const entry of data.entries) {
      const day = toLocalISO(new Date(entry.startedAt));
      entryMap.get(day)?.push(entry);
    }
    const result = new Map<string, WeekBlock[]>();
    for (const [day, dayEntries] of entryMap) {
      result.set(day, buildWeekBlocks(dayEntries));
    }
    return result;
  }, [data.entries, days]);

  const hours = Array.from({ length: 24 }, (_, i) => i);

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
                const dayBlocks = blocksByDay.get(day) || [];
                const activityBlocks = dayBlocks.filter((b) => b.type === "activity");
                const focusBlocks = dayBlocks.filter((b) => b.type === "focus");

                return (
                  <div key={day} className="flex-1 relative border-r border-border last:border-r-0">
                    {/* Activity blocks — colored by category */}
                    {activityBlocks.map((block, idx) => {
                      const top = block.startMin * PX_PER_MIN;
                      const height = Math.max(
                        (block.endMin - block.startMin) * PX_PER_MIN,
                        MIN_BLOCK_HEIGHT,
                      );

                      return (
                        <button
                          type="button"
                          key={`a-${block.label}-${block.startMin}`}
                          onClick={() => navigate(`/day/${day}`)}
                          className="absolute left-0.5 right-0.5 rounded-sm cursor-pointer hover:brightness-110 transition-all"
                          style={{
                            top,
                            height,
                            backgroundColor: block.color,
                            opacity: 0.8,
                          }}
                          title={`${block.label} · ${formatHumanDuration(block.totalSecs)}`}
                        >
                          {height > 16 && (
                            <span className="text-[8px] text-white/90 font-medium px-0.5 truncate block leading-tight">
                              {block.label}
                            </span>
                          )}
                        </button>
                      );
                    })}

                    {/* Focus session overlays — thin left accent bar */}
                    {focusBlocks.map((block) => {
                      const top = block.startMin * PX_PER_MIN;
                      const height = Math.max(
                        (block.endMin - block.startMin) * PX_PER_MIN,
                        MIN_BLOCK_HEIGHT,
                      );

                      return (
                        <div
                          key={`f-${block.startMin}`}
                          className="absolute left-0 w-[3px] rounded-sm pointer-events-none"
                          style={{
                            top,
                            height,
                            backgroundColor: block.color,
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
  const top = mins * (HOUR_HEIGHT / 60);
  return (
    <div className="absolute w-full pointer-events-none z-10" style={{ top }}>
      <div className="border-t border-red-500" />
    </div>
  );
}
