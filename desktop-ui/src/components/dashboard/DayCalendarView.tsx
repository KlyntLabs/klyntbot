import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import { minutesSinceMidnight, todayISO } from "../../lib/dates";
import type { TimelineEntry } from "../../lib/types";
import { EMPTY_TIMELINE_RESPONSE } from "../../lib/types";
import { cn } from "../../lib/utils";
import { type ActivityContainer, buildContainers, focusColor } from "./buildContainers";
import { useEnabledLayers } from "./layers";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 60;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 15;
const HOUR_GUTTER = 48;
const PX_PER_MIN = HOUR_HEIGHT / 60;

function eventDotColor(entryType: string): string {
  if (entryType === "taskCompleted") return "var(--timeline-dot-task-done)";
  if (entryType.startsWith("note")) return "var(--timeline-dot-note)";
  if (
    entryType.startsWith("transaction") ||
    entryType.startsWith("expense") ||
    entryType.startsWith("income")
  )
    return "var(--timeline-dot-finance)";
  return "var(--timeline-system)";
}

export function DayCalendarView() {
  const { date } = useParams<{ date: string }>();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const { enabled, enabledSources } = useEnabledLayers();
  const queryArgs = useMemo(
    () => ({ startDate: dateStr, endDate: dateStr, sources: enabledSources }),
    [dateStr, enabledSources],
  );

  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Scroll to current hour on mount (for today) or 8am otherwise.
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-scroll when date changes
  useEffect(() => {
    if (scrollRef.current) {
      const targetHour = isToday ? new Date().getHours() - 1 : 8;
      scrollRef.current.scrollTop = Math.max(0, targetHour * HOUR_HEIGHT);
    }
  }, [isToday, dateStr]);

  const containers = useMemo(() => buildContainers(data.entries), [data.entries]);

  const hours = Array.from({ length: 24 }, (_, i) => i);

  return (
    <div className="flex gap-2 h-full">
      {/* Calendar area */}
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        {loading && <div className="px-4 py-2 text-xs text-muted">Loading...</div>}
        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          <div className="relative" style={{ height: TOTAL_HEIGHT, minWidth: 0 }}>
            {/* Hour lines + labels */}
            {hours.map((h) => (
              <div
                key={h}
                className="absolute w-full flex items-start"
                style={{ top: h * HOUR_HEIGHT }}
              >
                <div
                  className="text-[10px] text-muted text-right pr-2 select-none"
                  style={{ width: HOUR_GUTTER }}
                >
                  {h === 0 ? "" : formatHour(h)}
                </div>
                <div className="flex-1 border-t border-border" />
              </div>
            ))}

            {/* Now line */}
            {isToday && <NowLine />}

            {/* Layered containers */}
            <div className="absolute inset-0" style={{ left: HOUR_GUTTER }}>
              {containers.map((container) => {
                const top = container.startMin * PX_PER_MIN;
                const height = Math.max(
                  (container.endMin - container.startMin) * PX_PER_MIN,
                  MIN_BLOCK_HEIGHT,
                );
                const isFocused = container.type === "focused";

                return (
                  <div
                    key={container.id}
                    className={cn(
                      "absolute left-0 right-0 rounded-lg border overflow-hidden",
                      isFocused ? "border-timeline-focus/30" : "border-white/[0.04]",
                    )}
                    style={{
                      top,
                      height,
                      backgroundColor: isFocused
                        ? focusColor(container.qualityScore)
                        : "var(--timeline-unfocused)",
                    }}
                  >
                    {/* Focus session clickable overlay */}
                    {isFocused && container.focusSession && (
                      <button
                        type="button"
                        onClick={() => {
                          const fs = container.focusSession;
                          if (fs) setSelectedEntry(selectedEntry?.id === fs.id ? null : fs);
                        }}
                        className={cn(
                          "absolute inset-0 cursor-pointer",
                          selectedEntry?.id === container.focusSession.id && "ring-1 ring-brand",
                        )}
                      >
                        <span className="absolute top-0.5 left-1.5 text-[10px] text-secondary/60 truncate max-w-[80%]">
                          {container.focusSession.title}
                        </span>
                      </button>
                    )}

                    {/* Task entries: green-accented inner blocks */}
                    {enabled.has("tasks") &&
                      container.taskEntries.map((entry) => (
                        <TaskBlock
                          key={entry.id}
                          entry={entry}
                          container={container}
                          selected={selectedEntry?.id === entry.id}
                          onClick={() =>
                            setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)
                          }
                        />
                      ))}

                    {/* App activity: thin horizontal bars */}
                    {enabled.has("apps") &&
                      container.appActivity.map((entry) => (
                        <AppBar key={entry.id} entry={entry} container={container} />
                      ))}

                    {/* Point events: colored dots */}
                    {enabled.has("events") &&
                      container.pointEvents.map((entry) => (
                        <EventDot key={entry.id} entry={entry} container={container} />
                      ))}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {/* Summary panel */}
      <SummaryPanel
        summary={data.summary}
        selectedEntry={selectedEntry}
        onClose={() => setSelectedEntry(null)}
      />
    </div>
  );
}

function TaskBlock({
  entry,
  container,
  selected,
  onClick,
}: {
  entry: TimelineEntry;
  container: ActivityContainer;
  selected: boolean;
  onClick: () => void;
}) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const relativeTop = (startMin - container.startMin) * PX_PER_MIN;
  const dur = entry.durationSecs ?? 0;
  const height = Math.max(dur > 0 ? (dur / 60) * PX_PER_MIN : MIN_BLOCK_HEIGHT, MIN_BLOCK_HEIGHT);

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-1 right-1 rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer border-l-2 border-l-timeline-task",
        "bg-timeline-task/15 hover:bg-timeline-task/25 transition-colors",
        selected && "ring-1 ring-brand",
      )}
      style={{ top: relativeTop, height }}
      title={entry.title}
    >
      <span className="text-secondary truncate block">{entry.title}</span>
      {height > 30 && entry.description && (
        <span className="text-muted truncate block text-[10px]">{entry.description}</span>
      )}
    </button>
  );
}

function AppBar({ entry, container }: { entry: TimelineEntry; container: ActivityContainer }) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const relativeTop = (startMin - container.startMin) * PX_PER_MIN;
  const dur = entry.durationSecs ?? 0;
  const width = dur > 0 ? Math.min(Math.max((dur / 3600) * 100, 10), 100) : 10;

  return (
    <div
      className="absolute left-1 h-3 rounded-sm text-[8px] text-muted truncate px-0.5 leading-3"
      style={{
        top: relativeTop,
        width: `${width}%`,
        backgroundColor: `color-mix(in oklch, ${entry.color} 20%, transparent)`,
      }}
      title={entry.title}
    >
      {entry.title}
    </div>
  );
}

function EventDot({ entry, container }: { entry: TimelineEntry; container: ActivityContainer }) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const relativeTop = (startMin - container.startMin) * PX_PER_MIN;

  return (
    <div
      className="absolute right-1.5 w-2 h-2 rounded-full"
      style={{
        top: relativeTop,
        backgroundColor: eventDotColor(entry.entryType),
      }}
      title={entry.title}
    />
  );
}

function NowLine() {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);

  const mins = now.getHours() * 60 + now.getMinutes();
  const top = mins * (HOUR_HEIGHT / 60);

  return (
    <div className="absolute w-full pointer-events-none z-10" style={{ top, left: HOUR_GUTTER }}>
      <div className="flex items-center">
        <div className="w-2 h-2 rounded-full bg-red-500 -ml-1" />
        <div className="flex-1 border-t border-red-500" />
      </div>
    </div>
  );
}

function formatHour(h: number): string {
  if (h === 0) return "12 AM";
  if (h < 12) return `${h} AM`;
  if (h === 12) return "12 PM";
  return `${h - 12} PM`;
}
