import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type {
  ActivitySwitchPayload,
  ActivityTimeline,
  ProductivitySummary,
  TimelineEntry,
  TimelineSummary,
} from "@shared/types";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type LayerKey, useEnabledLayers, useSidebarOpen } from "../lib/layers";
import { ActivityTrack, type SessionBlock } from "./ActivityTrack";
import { CalendarTrack } from "./CalendarTrack";
import { ContextRibbon } from "./ContextRibbon";
import { ActivityFeed } from "./productivity/ActivityFeed";
import { SummaryPanel } from "./SummaryPanel";

const DEFAULT_HOUR_HEIGHT = 60;
const MIN_HOUR_HEIGHT = 30;
const MAX_HOUR_HEIGHT = 200;
const MIN_BLOCK_HEIGHT = 14;
const HOUR_GUTTER = 48;
const HOURS = Array.from({ length: 24 }, (_, i) => i);

/** Column definition — maps a LayerKey to its rendering config */
interface ColumnDef {
  key: LayerKey;
  label: string;
  icon: string;
  color: string;
  /** flex weight (relative width) */
  flex: number;
  filter: (e: TimelineEntry) => boolean;
}

const COLUMNS: ColumnDef[] = [
  {
    key: "activity",
    label: "Activity",
    icon: "⬡",
    color: "var(--timeline-app-productive)",
    flex: 1.2,
    filter: (e) => e.entryType === "appUsage",
  },
  {
    key: "calendar",
    label: "Calendar",
    icon: "📅",
    color: "var(--timeline-focus)",
    flex: 1.4,
    filter: () => false, // Calendar uses its own data source
  },
  {
    key: "timeEntries",
    label: "Time Entries",
    icon: "☰",
    color: "var(--timeline-task)",
    flex: 1.8,
    filter: (e) => e.entryType === "taskTimeEntry",
  },
  {
    key: "tasks",
    label: "Tasks",
    icon: "☑",
    color: "var(--timeline-todo)",
    flex: 1.8,
    filter: (e) =>
      e.entryType === "taskDue" || e.entryType === "taskCreated" || e.entryType === "taskCompleted",
  },
  {
    key: "transactions",
    label: "Transactions",
    icon: "$",
    color: "var(--timeline-finance)",
    flex: 1.2,
    filter: (e) =>
      e.entryType === "expenseRecorded" ||
      e.entryType === "incomeRecorded" ||
      e.entryType === "transactionRecorded",
  },
  {
    key: "notes",
    label: "Notes",
    icon: "✎",
    color: "var(--timeline-note)",
    flex: 1.2,
    filter: (e) => e.entryType === "noteCreated" || e.entryType === "noteUpdated",
  },
];

interface DayColumnsViewProps {
  date: string;
  entries: TimelineEntry[];
  summary: TimelineSummary | null;
  isToday: boolean;
  loading: boolean;
  productivitySummary?: ProductivitySummary | null;
}

export function DayColumnsView({
  date,
  entries,
  summary,
  isToday,
  loading,
  productivitySummary,
}: DayColumnsViewProps) {
  // Centralized activity timeline fetch — passed to ActivityTrack to avoid duplicate IPC
  const { data: activityTimeline, refetch: refetchTimeline } = useQuery<ActivityTimeline[]>(
    "productivity_timeline",
    { date, tzOffsetMins: TZ_OFFSET_MINS },
    [],
  );
  useEvent<ActivitySwitchPayload>("activity:switch", () => refetchTimeline());
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetchTimeline();
  });

  const { enabled } = useEnabledLayers();
  const sidebarOpen = useSidebarOpen();
  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionBlock | null>(null);
  const [feedExpanded, setFeedExpanded] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Dynamic zoom state
  const [hourHeight, setHourHeight] = useState(DEFAULT_HOUR_HEIGHT);
  const hourHeightRef = useRef(DEFAULT_HOUR_HEIGHT);
  const pxPerMin = hourHeight / 60;
  const totalHeight = 24 * hourHeight;

  // Scroll to current hour on mount (intentionally excludes hourHeight — don't re-scroll on zoom)
  // biome-ignore lint/correctness/useExhaustiveDependencies: only scroll on mount/date change
  useEffect(() => {
    if (scrollRef.current) {
      const targetHour = isToday ? new Date().getHours() - 1 : 8;
      scrollRef.current.scrollTop = Math.max(0, targetHour * hourHeightRef.current);
    }
  }, [isToday]);

  // Zoom via Ctrl/Cmd + mouse wheel (preserves scroll position under cursor)
  useEffect(() => {
    const container = scrollRef.current;
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();

      const rect = container.getBoundingClientRect();
      const offsetFromTop = e.clientY - rect.top;
      const currentHH = hourHeightRef.current;
      const minuteAtCursor = (container.scrollTop + offsetFromTop) / (currentHH / 60);

      const delta = e.deltaY > 0 ? -4 : 4;
      const next = Math.min(MAX_HOUR_HEIGHT, Math.max(MIN_HOUR_HEIGHT, currentHH + delta));
      if (next === currentHH) return;

      hourHeightRef.current = next;
      setHourHeight(next);

      // Keep the minute under cursor at the same screen position
      requestAnimationFrame(() => {
        container.scrollTop = Math.max(0, minuteAtCursor * (next / 60) - offsetFromTop);
      });
    };

    container.addEventListener("wheel", handleWheel, { passive: false });
    return () => container.removeEventListener("wheel", handleWheel);
  }, []);

  // Drag on hour gutter to zoom
  const gutterDragRef = useRef<{ startY: number; startHH: number } | null>(null);
  const handleGutterMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    gutterDragRef.current = { startY: e.clientY, startHH: hourHeightRef.current };

    const handleMouseMove = (me: MouseEvent) => {
      if (!gutterDragRef.current) return;
      const dy = gutterDragRef.current.startY - me.clientY; // drag up = zoom in
      const next = Math.min(
        MAX_HOUR_HEIGHT,
        Math.max(MIN_HOUR_HEIGHT, gutterDragRef.current.startHH + dy * 0.5),
      );
      hourHeightRef.current = next;
      setHourHeight(next);
    };

    const handleMouseUp = () => {
      gutterDragRef.current = null;
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }, []);

  const resetZoom = useCallback(() => {
    hourHeightRef.current = DEFAULT_HOUR_HEIGHT;
    setHourHeight(DEFAULT_HOUR_HEIGHT);
  }, []);

  // Group entries by column (skip activity — it has its own track)
  const columnEntries = useMemo(() => {
    const map = new Map<LayerKey, TimelineEntry[]>();
    for (const col of COLUMNS) map.set(col.key, []);
    for (const entry of entries) {
      for (const col of COLUMNS) {
        if (col.filter(entry)) {
          map.get(col.key)?.push(entry);
          break;
        }
      }
    }
    return map;
  }, [entries]);

  // Only show columns whose layer is enabled
  const visibleColumns = useMemo(() => COLUMNS.filter((col) => enabled.has(col.key)), [enabled]);

  // Shared grid template so header and tracks have identical column widths
  const gridTemplate = useMemo(() => {
    const totalFlex = visibleColumns.reduce((s, c) => s + c.flex, 0);
    const cols = visibleColumns.map((c) => `${(c.flex / totalFlex) * 100}%`).join(" ");
    return `${HOUR_GUTTER}px ${cols}`;
  }, [visibleColumns]);

  const handleSelectSession = (session: SessionBlock) => {
    setSelectedEntry(null);
    setSelectedSession(
      selectedSession?.startMin === session.startMin && selectedSession?.label === session.label
        ? null
        : session,
    );
  };

  const handleSelectEntry = (entry: TimelineEntry) => {
    setSelectedSession(null);
    setSelectedEntry(selectedEntry?.id === entry.id ? null : entry);
  };

  return (
    <div className="flex gap-2 h-full w-full">
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        {/* Context color ribbon — subtle work context indicator per hour */}
        <ContextRibbon date={date} />

        {loading && <div className="px-4 py-2 text-xs text-muted-foreground">Loading...</div>}

        {/* Zoom indicator — shown when zoomed away from default */}
        {hourHeight !== DEFAULT_HOUR_HEIGHT && (
          <div className="px-3 py-1 flex items-center justify-between border-b border-border text-2xs text-muted-foreground">
            <span className="tabular-nums">
              Zoom: {Math.round((hourHeight / DEFAULT_HOUR_HEIGHT) * 100)}%
            </span>
            <button
              type="button"
              onClick={resetZoom}
              className="text-brand hover:underline"
              aria-label="Reset zoom to default level"
            >
              Reset
            </button>
          </div>
        )}

        {/* Scrollable timeline area (headers inside for scrollbar-width alignment) */}
        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          {/* Sticky column headers — CSS grid ensures pixel-perfect alignment with tracks */}
          <div
            className="sticky top-0 z-20 grid border-b border-border bg-popover"
            style={{ gridTemplateColumns: gridTemplate }}
          >
            <div />
            {visibleColumns.map((col) => (
              <div
                key={col.key}
                className="text-[11px] text-muted-foreground font-medium py-1.5 px-1.5 border-r border-border last:border-r-0 flex items-center gap-1.5 truncate min-w-0"
              >
                <span
                  className="w-1.5 h-1.5 rounded-full shrink-0"
                  style={{ backgroundColor: col.color }}
                />
                {col.label}
              </div>
            ))}
          </div>

          <div className="relative" style={{ height: totalHeight }}>
            {/* Hour lines + labels (gutter is draggable for zoom) */}
            {HOURS.map((h) => (
              <div
                key={h}
                className="absolute w-full flex items-start"
                style={{ top: h * hourHeight }}
              >
                <div
                  role={h === 0 ? "slider" : undefined}
                  aria-label={h === 0 ? "Timeline zoom level" : undefined}
                  aria-valuemin={h === 0 ? MIN_HOUR_HEIGHT : undefined}
                  aria-valuemax={h === 0 ? MAX_HOUR_HEIGHT : undefined}
                  aria-valuenow={h === 0 ? hourHeight : undefined}
                  tabIndex={h === 0 ? 0 : undefined}
                  className="text-2xs text-muted-foreground text-right pr-2 select-none cursor-ns-resize"
                  style={{ width: HOUR_GUTTER }}
                  onMouseDown={handleGutterMouseDown}
                  onKeyDown={
                    h === 0
                      ? (e) => {
                          if (e.key === "ArrowUp") {
                            e.preventDefault();
                            const next = Math.min(MAX_HOUR_HEIGHT, hourHeightRef.current + 10);
                            hourHeightRef.current = next;
                            setHourHeight(next);
                          } else if (e.key === "ArrowDown") {
                            e.preventDefault();
                            const next = Math.max(MIN_HOUR_HEIGHT, hourHeightRef.current - 10);
                            hourHeightRef.current = next;
                            setHourHeight(next);
                          }
                        }
                      : undefined
                  }
                >
                  {h === 0 ? "" : formatHour(h)}
                </div>
                <div className="flex-1 border-t border-border" />
              </div>
            ))}

            {/* Now line */}
            {isToday && <NowLine pxPerMin={pxPerMin} />}

            {/* Column tracks — same CSS grid as header */}
            <div className="absolute inset-0 grid" style={{ gridTemplateColumns: gridTemplate }}>
              <div />
              {visibleColumns.map((col) => {
                // Activity column: merged app sessions with unified focus rendering
                if (col.key === "activity") {
                  return (
                    <div
                      key={col.key}
                      className="relative border-r border-border last:border-r-0 min-w-0"
                    >
                      <ActivityTrack
                        date={date}
                        hourHeight={hourHeight}
                        isToday={isToday}
                        onSelectSession={handleSelectSession}
                        onSelectEntry={handleSelectEntry}
                        selectedSession={selectedSession}
                        selectedEntryId={selectedEntry?.id ?? null}
                        timelineEntries={activityTimeline}
                      />
                    </div>
                  );
                }

                // Calendar column: fetches its own data
                if (col.key === "calendar") {
                  return (
                    <div
                      key={col.key}
                      className="relative border-r border-border last:border-r-0 min-w-0"
                    >
                      <CalendarTrack
                        date={date}
                        hourHeight={hourHeight}
                        selectedEventId={selectedEntry?.id ?? null}
                        onSelectEvent={(event) =>
                          handleSelectEntry({
                            id: event.id,
                            title: event.title,
                            description: event.description ?? undefined,
                            startedAt: event.startedAt,
                            endedAt: event.endedAt,
                            durationSecs: Math.round(
                              (new Date(event.endedAt).getTime() -
                                new Date(event.startedAt).getTime()) /
                                1000,
                            ),
                            source: "calendar",
                            entryType: "calendarEvent",
                            color: event.color ?? "var(--timeline-focus)",
                          } as TimelineEntry)
                        }
                      />
                    </div>
                  );
                }

                const colEntries = columnEntries.get(col.key) ?? [];
                return (
                  <div
                    key={col.key}
                    className="relative border-r border-border last:border-r-0 min-w-0"
                  >
                    {colEntries.map((entry) => (
                      <ColumnEntry
                        key={entry.id}
                        entry={entry}
                        column={col}
                        pxPerMin={pxPerMin}
                        selected={selectedEntry?.id === entry.id}
                        onClick={() => handleSelectEntry(entry)}
                      />
                    ))}
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {/* Collapsible activity feed — only for today */}
        {isToday && (
          <div
            className="border-t border-border transition-[max-height] duration-300 ease-in-out"
            style={{ maxHeight: feedExpanded ? 260 : 36, overflow: "hidden" }}
          >
            <button
              type="button"
              onClick={() => setFeedExpanded(!feedExpanded)}
              className="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors w-full"
            >
              {feedExpanded ? (
                <ChevronDown className="size-3.5" />
              ) : (
                <ChevronUp className="size-3.5" />
              )}
              Live Activity Feed
            </button>
            {feedExpanded && (
              <div className="overflow-y-auto" style={{ maxHeight: 224 }}>
                <ActivityFeed />
              </div>
            )}
          </div>
        )}
      </div>

      {sidebarOpen && (
        <SummaryPanel
          summary={summary}
          selectedEntry={selectedEntry}
          selectedSession={selectedSession}
          onClose={() => {
            setSelectedEntry(null);
            setSelectedSession(null);
          }}
          productivitySummary={productivitySummary}
          date={date}
        />
      )}
    </div>
  );
}

/** Renders a single entry block positioned by time within its column. */
function ColumnEntry({
  entry,
  column,
  pxPerMin,
  selected,
  onClick,
}: {
  entry: TimelineEntry;
  column: ColumnDef;
  pxPerMin: number;
  selected: boolean;
  onClick: () => void;
}) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const top = startMin * pxPerMin;
  const dur = entry.durationSecs ?? 0;
  const height = Math.max(dur > 0 ? (dur / 60) * pxPerMin : MIN_BLOCK_HEIGHT, MIN_BLOCK_HEIGHT);

  // Time entries — rich blocks with title + time
  if (column.key === "timeEntries") {
    const timeStr = new Date(entry.startedAt).toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
    });
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-1 right-1 rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer",
          "border-l-2 border-l-timeline-task bg-timeline-task/15 hover:bg-timeline-task/25 transition-colors",
          selected && "ring-1 ring-brand",
        )}
        style={{ top, height }}
        title={entry.title}
      >
        <span className="text-muted-foreground truncate block">{entry.title}</span>
        {height > 28 && (
          <span className="text-muted-foreground text-2xs truncate block">
            {dur > 0 && `${formatHumanDuration(dur)} · `}
            {timeStr}
          </span>
        )}
      </button>
    );
  }

  // Tasks — due/created/completed
  if (column.key === "tasks") {
    const isDue = entry.entryType === "taskDue";
    const isCompleted = entry.entryType === "taskCompleted";
    const status = entry.metadata?.status as string | undefined;
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-1 right-1 rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer transition-colors",
          isDue
            ? "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/15 hover:bg-[var(--timeline-todo)]/25"
            : "border-l border-border bg-card hover:bg-muted",
          isCompleted && "opacity-60 line-through",
          selected && "ring-1 ring-brand",
        )}
        style={{ top, height: Math.max(height, 20) }}
        title={entry.title}
      >
        <span className="text-muted-foreground truncate block">{entry.title}</span>
        {isDue && status && height > 28 && (
          <span className="text-muted-foreground text-2xs truncate block capitalize">{status}</span>
        )}
      </button>
    );
  }

  // Transactions — expense/income
  if (column.key === "transactions") {
    const isExpense = entry.entryType === "expenseRecorded";
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-0.5 right-0.5 rounded-md px-1.5 py-0.5 text-2xs leading-tight overflow-hidden cursor-pointer transition-colors",
          isExpense
            ? "border-l-2 border-l-[var(--timeline-finance-expense)] bg-[var(--timeline-finance-expense)]/15 hover:bg-[var(--timeline-finance-expense)]/25"
            : "border-l-2 border-l-[var(--timeline-finance-income)] bg-[var(--timeline-finance-income)]/15 hover:bg-[var(--timeline-finance-income)]/25",
          selected && "ring-1 ring-brand",
        )}
        style={{ top, height: Math.max(height, 18) }}
        title={entry.title}
      >
        <span
          className={cn(
            "truncate block font-medium",
            isExpense
              ? "text-[var(--timeline-finance-expense)]"
              : "text-[var(--timeline-finance-income)]",
          )}
        >
          {entry.title}
        </span>
      </button>
    );
  }

  // Notes — dot + label
  if (column.key === "notes") {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-1 right-1 flex items-center gap-1 text-2xs cursor-pointer transition-colors",
          "text-muted-foreground hover:text-muted-foreground",
          selected && "text-brand",
        )}
        style={{ top }}
        title={entry.title}
      >
        <span
          className="size-2 rounded-full shrink-0"
          style={{ backgroundColor: "var(--timeline-note)" }}
        />
        <span className="truncate">{entry.title}</span>
      </button>
    );
  }

  // Fallback — dot + label
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-1 right-1 flex items-center gap-1 text-2xs text-muted-foreground hover:text-muted-foreground cursor-pointer transition-colors",
        selected && "text-brand",
      )}
      style={{ top }}
      title={entry.title}
    >
      <span className="size-2 rounded-full shrink-0" style={{ backgroundColor: column.color }} />
      <span className="truncate">{entry.title}</span>
    </button>
  );
}

function NowLine({ pxPerMin }: { pxPerMin: number }) {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const mins = now.getHours() * 60 + now.getMinutes();
  const top = mins * pxPerMin;
  return (
    <div className="absolute w-full pointer-events-none z-10" style={{ top, left: HOUR_GUTTER }}>
      <div className="flex items-center">
        <div className="size-2 rounded-full bg-destructive -ml-1" />
        <div className="flex-1 border-t border-destructive" />
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
