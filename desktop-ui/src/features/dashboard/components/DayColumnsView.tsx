import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, minutesSinceMidnight, TZ_OFFSET_MINS } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type {
  ActivityTimeline,
  ProductivitySummary,
  TimelineEntry,
  TimelineSummary,
} from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTimelineDrag } from "../hooks/useTimelineDrag";
import { type LayerKey, useEnabledLayers, useSidebarOpen } from "../lib/layers";
import { computeOverlapLayout } from "../lib/timeline-utils";
import { ActivityTrack, type SessionBlock } from "./ActivityTrack";
import { CalendarTrack } from "./CalendarTrack";
import { ContextRibbon } from "./ContextRibbon";
import { DraggableTaskBlock } from "./DraggableTaskBlock";
import { DueTodayTray } from "./DueTodayTray";
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
    filter: (e) => e.entryType === "taskDue",
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
  const queryArgs = useMemo(() => ({ date, tzOffsetMins: TZ_OFFSET_MINS }), [date]);
  const { data: activityTimeline } = useQuery<ActivityTimeline[]>(
    "productivity_timeline",
    queryArgs,
    [],
    {
      invalidateOn: ["entity:updated", "activity:switch"],
      invalidateFilter: (p) => {
        const kind = (p as { entityKind?: string })?.entityKind;
        return !kind || kind === "productivity";
      },
    },
  );

  const { enabled } = useEnabledLayers();
  const sidebarOpen = useSidebarOpen();
  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionBlock | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Dynamic zoom state
  const [hourHeight, setHourHeight] = useState(DEFAULT_HOUR_HEIGHT);
  const hourHeightRef = useRef(DEFAULT_HOUR_HEIGHT);
  const pxPerMin = hourHeight / 60;
  const totalHeight = 24 * hourHeight;

  // Scroll to current hour on mount (intentionally excludes hourHeight — don't re-scroll on zoom)
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

  // Timeline drag-and-drop for task scheduling
  const {
    drag: timelineDrag,
    ghost,
    isDragging,
    startMove,
    startResize,
    startTrayDrag,
    onMouseMove: onDragMouseMove,
    onMouseUp: onDragMouseUp,
  } = useTimelineDrag(date, pxPerMin);

  useEffect(() => {
    if (!isDragging) return;
    document.addEventListener("mousemove", onDragMouseMove);
    document.addEventListener("mouseup", onDragMouseUp);
    return () => {
      document.removeEventListener("mousemove", onDragMouseMove);
      document.removeEventListener("mouseup", onDragMouseUp);
    };
  }, [isDragging, onDragMouseMove, onDragMouseUp]);

  // Group entries by column (skip activity — it has its own track) and compute overlap layouts
  const { columnEntries, columnLayouts } = useMemo(() => {
    const entryMap = new Map<LayerKey, TimelineEntry[]>();
    for (const col of COLUMNS) entryMap.set(col.key, []);
    for (const entry of entries) {
      for (const col of COLUMNS) {
        if (col.filter(entry)) {
          entryMap.get(col.key)?.push(entry);
          break;
        }
      }
    }
    // Compute overlap layout for each column
    const layoutMap = new Map<LayerKey, Map<string, { colIndex: number; totalCols: number }>>();
    for (const [key, colEntries] of entryMap) {
      layoutMap.set(key, computeOverlapLayout(colEntries));
    }
    return { columnEntries: entryMap, columnLayouts: layoutMap };
  }, [entries]);

  // Split task entries into scheduled (timeline blocks) and unscheduled (tray chips)
  const { scheduledTaskEntries, trayTaskEntries } = useMemo(() => {
    const taskEntries = columnEntries.get("tasks") ?? [];
    const scheduled: TimelineEntry[] = [];
    const tray: TimelineEntry[] = [];
    for (const entry of taskEntries) {
      const meta = entry.metadata as Record<string, unknown> | undefined;
      if (meta?.scheduled === true) {
        scheduled.push(entry);
      } else {
        // Tasks with a specific due time (non-midnight) show on the timeline;
        // date-only tasks (midnight) go to the drag-and-drop tray.
        const dueMin = minutesSinceMidnight(entry.startedAt);
        if (dueMin > 0) scheduled.push(entry);
        else tray.push(entry);
      }
    }
    return { scheduledTaskEntries: scheduled, trayTaskEntries: tray };
  }, [columnEntries]);

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
                {/* biome-ignore lint/a11y/noStaticElementInteractions: drag-to-zoom gutter; first cell is an accessible slider, rest are presentation */}
                {/* biome-ignore lint/a11y/useAriaPropsSupportedByRole: aria attrs are only rendered for h===0 (slider role) */}
                <div
                  role={h === 0 ? "slider" : "presentation"}
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

                // Tasks column: split into tray (unscheduled) and draggable blocks (scheduled)
                if (col.key === "tasks") {
                  const layouts = columnLayouts.get(col.key);
                  return (
                    <div
                      key={col.key}
                      className="relative border-r border-border last:border-r-0 min-w-0 flex flex-col"
                    >
                      <DueTodayTray
                        entries={trayTaskEntries}
                        onStartDrag={startTrayDrag}
                        onSelect={handleSelectEntry}
                        selectedEntryId={selectedEntry?.id ?? null}
                      />
                      <div className="relative flex-1">
                        {scheduledTaskEntries.map((entry) => {
                          const meta = entry.metadata as Record<string, unknown> | undefined;
                          const taskId = (meta?.taskId as string) ?? entry.entityId ?? entry.id;
                          const startMin = minutesSinceMidnight(entry.startedAt);
                          const dur = entry.durationSecs ?? 0;
                          const endMin = dur > 0 ? startMin + dur / 60 : startMin + 30;
                          const isThisDragging = isDragging && timelineDrag?.taskId === taskId;

                          return (
                            <DraggableTaskBlock
                              key={entry.id}
                              entry={entry}
                              pxPerMin={pxPerMin}
                              selected={selectedEntry?.id === entry.id}
                              layout={layouts?.get(entry.id)}
                              isDragging={isThisDragging}
                              ghostTopMin={isThisDragging ? ghost?.topMin : undefined}
                              ghostEndMin={isThisDragging ? ghost?.endMin : undefined}
                              onMouseDownMove={(e) => startMove(e, taskId, startMin, endMin)}
                              onMouseDownResize={(e) => startResize(e, taskId, startMin, endMin)}
                              onClick={() => handleSelectEntry(entry)}
                            />
                          );
                        })}
                      </div>
                    </div>
                  );
                }

                const colEntries = columnEntries.get(col.key) ?? [];
                const layouts = columnLayouts.get(col.key);
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
                        layout={layouts?.get(entry.id)}
                      />
                    ))}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
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
  layout,
}: {
  entry: TimelineEntry;
  column: ColumnDef;
  pxPerMin: number;
  selected: boolean;
  onClick: () => void;
  layout?: { colIndex: number; totalCols: number };
}) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const top = startMin * pxPerMin;
  const dur = entry.durationSecs ?? 0;
  const height = Math.max(dur > 0 ? (dur / 60) * pxPerMin : MIN_BLOCK_HEIGHT, MIN_BLOCK_HEIGHT);

  // Overlap layout: compute left/width percentages
  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const leftPct = totalCols > 1 ? `${(colIndex / totalCols) * 100}%` : undefined;
  const widthPct = totalCols > 1 ? `${(1 / totalCols) * 100}%` : undefined;

  // Shared positioning style for overlap layout
  const posStyle: React.CSSProperties = leftPct
    ? { top, left: leftPct, width: widthPct, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

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
          "absolute rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer",
          "border-l-2 border-l-timeline-task bg-timeline-task/15 hover:bg-timeline-task/25 transition-colors",
          selected && "ring-1 ring-brand",
        )}
        style={{ ...posStyle, height }}
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
          "absolute rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer transition-colors",
          isDue
            ? "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/15 hover:bg-[var(--timeline-todo)]/25"
            : "border-l-2 border-l-[var(--timeline-todo)]/50 bg-[var(--timeline-todo)]/8 hover:bg-[var(--timeline-todo)]/15",
          isCompleted && "opacity-60 line-through",
          selected && "ring-1 ring-brand",
        )}
        style={{ ...posStyle, height: Math.max(height, 20) }}
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
          "absolute rounded-md px-1.5 py-0.5 text-2xs leading-tight overflow-hidden cursor-pointer transition-colors",
          isExpense
            ? "border-l-2 border-l-[var(--timeline-finance-expense)] bg-[var(--timeline-finance-expense)]/15 hover:bg-[var(--timeline-finance-expense)]/25"
            : "border-l-2 border-l-[var(--timeline-finance-income)] bg-[var(--timeline-finance-income)]/15 hover:bg-[var(--timeline-finance-income)]/25",
          selected && "ring-1 ring-brand",
        )}
        style={{ ...posStyle, height: Math.max(height, 18) }}
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

  // Notes — colored box like tasks
  if (column.key === "notes") {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute rounded-md px-1.5 py-0.5 text-2xs leading-tight overflow-hidden cursor-pointer transition-colors",
          "border-l-2 border-l-[var(--timeline-note)]/60 bg-[var(--timeline-note)]/8 hover:bg-[var(--timeline-note)]/15",
          selected && "ring-1 ring-brand",
        )}
        style={{ ...posStyle, height: Math.max(height, 18) }}
        title={entry.title}
      >
        <span className="text-muted-foreground truncate block">{entry.title}</span>
      </button>
    );
  }

  // Fallback — dot + label
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute flex items-center gap-1 text-2xs text-muted-foreground hover:text-muted-foreground cursor-pointer transition-colors",
        selected && "text-brand",
      )}
      style={posStyle}
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
