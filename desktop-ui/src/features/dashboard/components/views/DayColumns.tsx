import type { QueryKey } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { productivitySummaryRangeQuery, productivityTodayQuery } from "@/api/endpoints/dashboard";
import type {
  CalendarEvent,
  ProductivitySummaryResponse,
  TimelineEntry,
  TimelineSummary,
} from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, minutesSinceMidnight } from "@/utils/dashboardDates";
import { useTimelineDrag } from "../../hooks/useTimelineDrag";
import { type LayerKey, useEnabledLayers, useSidebarOpen } from "../../lib/layers";
import { computeOverlapLayout } from "../../lib/timeline-utils";
import { SummaryPanel } from "../SummaryPanel";
import type { SessionBlock } from "./ActivityTrack";
import { ActivityTrack } from "./ActivityTrack";
import { CalendarTrack } from "./CalendarTrack";
import { ContextRibbon } from "./ContextRibbon";
import { DraggableTaskBlock } from "./DraggableTaskBlock";
import { DueTodayTray } from "./DueTodayTray";

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
    icon: "activity",
    color: "var(--timeline-app-productive)",
    flex: 1.2,
    filter: (e) => e.entryType === "appUsage",
  },
  {
    key: "calendar",
    label: "Calendar",
    icon: "calendar",
    color: "var(--timeline-focus)",
    flex: 1.4,
    filter: () => false,
  },
  {
    key: "timeEntries",
    label: "Time Entries",
    icon: "clock",
    color: "var(--timeline-task)",
    flex: 1.8,
    filter: (e) => e.entryType === "taskTimeEntry",
  },
  {
    key: "tasks",
    label: "Tasks",
    icon: "check-square",
    color: "var(--timeline-todo)",
    flex: 1.8,
    filter: (e) => e.entryType === "taskDue",
  },
  {
    key: "transactions",
    label: "Transactions",
    icon: "dollar-sign",
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
    icon: "file-text",
    color: "var(--timeline-note)",
    flex: 1.2,
    filter: (e) => e.entryType === "noteCreated" || e.entryType === "noteUpdated",
  },
];

export interface DayColumnsProps {
  date: string;
  entries: TimelineEntry[];
  summary: TimelineSummary | null;
  isToday: boolean;
  loading: boolean;
  queryKey: QueryKey;
}

export function DayColumns({
  date,
  entries,
  summary,
  isToday,
  loading,
  queryKey,
}: DayColumnsProps) {
  const { enabled } = useEnabledLayers();
  const { sidebarOpen } = useSidebarOpen();
  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const [selectedCalendarEvent, setSelectedCalendarEvent] = useState<CalendarEvent | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionBlock | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const { data: productivitySummary } = useTauriQuery<ProductivitySummaryResponse | null>({
    queryKey: isToday
      ? qk.dashboard.productivityToday(date)
      : qk.productivity.summaryRange(date, date),
    staleTime: 60_000,
    queryFn: async () => {
      if (isToday) return productivityTodayQuery();
      const arr = await productivitySummaryRangeQuery(date, date);
      return arr[0] ?? null;
    },
    fallback: null,
  });

  // Dynamic zoom state
  const [hourHeight, setHourHeight] = useState(DEFAULT_HOUR_HEIGHT);
  const hourHeightRef = useRef(DEFAULT_HOUR_HEIGHT);
  const pxPerMin = hourHeight / 60;
  const totalHeight = 24 * hourHeight;

  // Scroll to current hour on mount
  useEffect(() => {
    if (scrollRef.current) {
      const targetHour = isToday ? new Date().getHours() - 1 : 8;
      scrollRef.current.scrollTop = Math.max(0, targetHour * hourHeightRef.current);
    }
  }, [isToday]);

  // Zoom via Ctrl/Cmd + mouse wheel
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
      const dy = gutterDragRef.current.startY - me.clientY;
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
  } = useTimelineDrag(date, pxPerMin, queryKey);

  useEffect(() => {
    if (!isDragging) return;
    document.addEventListener("mousemove", onDragMouseMove);
    document.addEventListener("mouseup", onDragMouseUp);
    return () => {
      document.removeEventListener("mousemove", onDragMouseMove);
      document.removeEventListener("mouseup", onDragMouseUp);
    };
  }, [isDragging, onDragMouseMove, onDragMouseUp]);

  // Group entries by column and compute overlap layouts
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
    const layoutMap = new Map<LayerKey, Map<string, { colIndex: number; totalCols: number }>>();
    for (const [key, colEntries] of entryMap) {
      layoutMap.set(key, computeOverlapLayout(colEntries));
    }
    return { columnEntries: entryMap, columnLayouts: layoutMap };
  }, [entries]);

  // Split task entries into scheduled and unscheduled
  const { scheduledTaskEntries, trayTaskEntries } = useMemo(() => {
    const taskEntries = columnEntries.get("tasks") ?? [];
    const scheduled: TimelineEntry[] = [];
    const tray: TimelineEntry[] = [];
    for (const entry of taskEntries) {
      const meta = entry.metadata as Record<string, unknown> | undefined;
      if (meta?.scheduled === true) {
        scheduled.push(entry);
      } else {
        const dueMin = minutesSinceMidnight(entry.startedAt);
        if (dueMin > 0) scheduled.push(entry);
        else tray.push(entry);
      }
    }
    return { scheduledTaskEntries: scheduled, trayTaskEntries: tray };
  }, [columnEntries]);

  const visibleColumns = useMemo(() => COLUMNS.filter((col) => enabled.has(col.key)), [enabled]);

  const gridTemplate = useMemo(() => {
    const totalFlex = visibleColumns.reduce((s, c) => s + c.flex, 0);
    const cols = visibleColumns.map((c) => `${(c.flex / totalFlex) * 100}%`).join(" ");
    return `${HOUR_GUTTER}px ${cols}`;
  }, [visibleColumns]);

  const handleSelectEntry = (entry: TimelineEntry) => {
    setSelectedEntry(selectedEntry?.id === entry.id ? null : entry);
  };

  return (
    <div className="flex gap-2 h-full w-full">
      <div className="flex-1 flex flex-col overflow-hidden">
        <ContextRibbon date={date} />

        {loading && (
          <div className="px-4 py-2 text-[var(--fs-sm)] text-ds-text-subtle">Loading...</div>
        )}

        {hourHeight !== DEFAULT_HOUR_HEIGHT && (
          <div className="px-3 py-1 flex items-center justify-between border-b border-ds-border-subtle text-ui-2xs text-ds-text-subtle">
            <span className="tabular-nums">
              Zoom: {Math.round((hourHeight / DEFAULT_HOUR_HEIGHT) * 100)}%
            </span>
            <button
              type="button"
              onClick={resetZoom}
              className="text-border-accent bg-transparent border-none cursor-pointer text-inherit"
              aria-label="Reset zoom to default level"
            >
              Reset
            </button>
          </div>
        )}

        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          <div
            className="sticky top-0 z-20 grid bg-surface-messages rounded-none border-b-none"
            style={{ gridTemplateColumns: gridTemplate }}
          >
            <div />
            {visibleColumns.map((col) => (
              <div key={col.key} className="text-ui-xs text-text-muted font-medium px-1.5 py-1.5 border-r border-border-subtle last:border-r-0 flex items-center gap-1.5 whitespace-nowrap overflow-hidden text-ellipsis min-w-0">
                <span
                  className="shrink-0 rounded-full"
                  style={{ width: 6, height: 6, backgroundColor: col.color }}
                />
                {col.label}
              </div>
            ))}
          </div>

          <div className="relative" style={{ height: totalHeight }}>
            {HOURS.map((h) => (
              <div key={h} className="absolute w-full flex items-start" style={{ top: h * hourHeight }}>
                <div
                  role={h === 0 ? "slider" : "presentation"}
                  aria-label={h === 0 ? "Timeline zoom level" : undefined}
                  aria-valuemin={h === 0 ? MIN_HOUR_HEIGHT : undefined}
                  aria-valuemax={h === 0 ? MAX_HOUR_HEIGHT : undefined}
                  aria-valuenow={h === 0 ? hourHeight : undefined}
                  tabIndex={h === 0 ? 0 : undefined}
                  className="text-ui-2xs text-text-muted text-right pr-1.5 select-none"
                  style={{ width: HOUR_GUTTER, cursor: "ns-resize" }}
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
                <div className="flex-1 border-t border-border-subtle" />
              </div>
            ))}

            {isToday && <NowLine pxPerMin={pxPerMin} />}

            <div
              className="absolute inset-0 grid"
              style={{ gridTemplateColumns: gridTemplate }}
            >
              <div />
              {visibleColumns.map((col) => {
                if (col.key === "activity") {
                  return (
                    <div key={col.key} className="flex-1 relative border-r border-border-subtle last:border-r-0">
                      <ActivityTrack
                        date={date}
                        hourHeight={hourHeight}
                        isToday={isToday}
                        onSelectSession={(s) => setSelectedSession(s)}
                        onSelectEntry={(entry) => setSelectedEntry(entry)}
                        selectedSession={selectedSession}
                        selectedEntryId={selectedEntry?.id ?? null}
                      />
                    </div>
                  );
                }

                if (col.key === "calendar") {
                  return (
                    <div key={col.key} className="flex-1 relative border-r border-border-subtle last:border-r-0">
                      <CalendarTrack
                        date={date}
                        hourHeight={hourHeight}
                        selectedEventId={selectedCalendarEvent?.id ?? null}
                        onSelectEvent={(event) => {
                          setSelectedCalendarEvent(event);
                          if (!event) return;
                          const startedAt = event.startedAt;
                          const endedAt = event.endedAt;
                          const durationSecs =
                            startedAt && endedAt
                              ? Math.max(
                                  0,
                                  Math.floor(
                                    (new Date(endedAt).getTime() - new Date(startedAt).getTime()) /
                                      1000,
                                  ),
                                )
                              : null;
                          setSelectedEntry({
                            id: event.id,
                            title: event.title,
                            description: event.description ?? null,
                            startedAt,
                            endedAt,
                            durationSecs,
                            source: "calendar",
                            entryType: "calendarEvent",
                            color: event.color ?? "var(--timeline-focus)",
                            metadata: null,
                            entityId: event.id,
                            entityRoute: null,
                          });
                        }}
                      />
                    </div>
                  );
                }

                if (col.key === "tasks") {
                  const layouts = columnLayouts.get(col.key);
                  return (
                    <div key={col.key} className="flex-1 relative border-r border-border-subtle last:border-r-0 flex flex-col">
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
                  <div key={col.key} className="flex-1 relative border-r border-border-subtle last:border-r-0">
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
            setSelectedCalendarEvent(null);
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

  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const leftPct = totalCols > 1 ? `${(colIndex / totalCols) * 100}%` : undefined;
  const widthPct = totalCols > 1 ? `${(1 / totalCols) * 100}%` : undefined;

  const posStyle: React.CSSProperties = leftPct
    ? { top, left: leftPct, width: widthPct, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

  if (column.key === "timeEntries") {
    const timeStr = new Date(entry.startedAt).toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
    });
    return (
      <button
        type="button"
        onClick={onClick}
        className="absolute rounded-md px-1.5 py-0.5 text-ui-xs leading-tight overflow-hidden cursor-pointer transition-colors duration-ui-fast ease-out text-left"
        style={{
          ...posStyle,
          height,
          borderLeft: "2px solid var(--timeline-task)",
          background: "color-mix(in srgb, var(--timeline-task) 15%, transparent)",
          outline: selected ? "1px solid var(--border-accent)" : undefined,
        }}
        title={entry.title}
      >
        <span className="block text-ds-text-subtle whitespace-nowrap overflow-hidden text-ellipsis">
          {entry.title}
        </span>
        {height > 28 && (
          <span className="block text-ds-text-subtle text-ui-2xs whitespace-nowrap overflow-hidden text-ellipsis">
            {dur > 0 && `${formatHumanDuration(dur)} · `}
            {timeStr}
          </span>
        )}
      </button>
    );
  }

  if (column.key === "tasks") {
    const isDue = entry.entryType === "taskDue";
    const isCompleted = entry.entryType === "taskCompleted";
    const status = (entry.metadata as Record<string, unknown> | null)?.status as string | undefined;
    return (
      <button
        type="button"
        onClick={onClick}
        className="absolute rounded-md px-1.5 py-0.5 text-ui-xs leading-tight overflow-hidden cursor-pointer transition-colors duration-ui-fast ease-out text-left"
        style={{
          ...posStyle,
          height: Math.max(height, 20),
          borderLeft: `2px solid ${isDue ? "var(--timeline-todo)" : "color-mix(in srgb, var(--timeline-todo) 50%, transparent)"}`,
          background: isDue
            ? "color-mix(in srgb, var(--timeline-todo) 15%, transparent)"
            : "color-mix(in srgb, var(--timeline-todo) 8%, transparent)",
          opacity: isCompleted ? 0.6 : undefined,
          textDecoration: isCompleted ? "line-through" : undefined,
          outline: selected ? "1px solid var(--border-accent)" : undefined,
        }}
        title={entry.title}
      >
        <span className="block text-ds-text-subtle whitespace-nowrap overflow-hidden text-ellipsis">
          {entry.title}
        </span>
        {isDue && status && height > 28 && (
          <span className="block text-ds-text-subtle text-ui-2xs whitespace-nowrap overflow-hidden text-ellipsis capitalize">
            {status}
          </span>
        )}
      </button>
    );
  }

  if (column.key === "transactions") {
    const isExpense = entry.entryType === "expenseRecorded";
    return (
      <button
        type="button"
        onClick={onClick}
        className="absolute rounded-md px-1.5 py-0.5 text-ui-xs leading-tight overflow-hidden cursor-pointer transition-colors duration-ui-fast ease-out text-left"
        style={{
          ...posStyle,
          height: Math.max(height, 18),
          borderLeft: `2px solid var(--timeline-finance-${isExpense ? "expense" : "income"})`,
          background: `color-mix(in srgb, var(--timeline-finance-${isExpense ? "expense" : "income"}) 15%, transparent)`,
          outline: selected ? "1px solid var(--border-accent)" : undefined,
        }}
        title={entry.title}
      >
        <span className="block whitespace-nowrap overflow-hidden text-ellipsis font-medium"
          style={{ color: `var(--timeline-finance-${isExpense ? "expense" : "income"})` }}>
          {entry.title}
        </span>
      </button>
    );
  }

  if (column.key === "notes") {
    return (
      <button
        type="button"
        onClick={onClick}
        className="absolute rounded-md px-1.5 py-0.5 text-ui-xs leading-tight overflow-hidden cursor-pointer transition-colors duration-ui-fast ease-out text-left"
        style={{
          ...posStyle,
          height: Math.max(height, 18),
          borderLeft: "2px solid color-mix(in srgb, var(--timeline-note) 60%, transparent)",
          background: "color-mix(in srgb, var(--timeline-note) 8%, transparent)",
          outline: selected ? "1px solid var(--border-accent)" : undefined,
        }}
        title={entry.title}
      >
        <span className="block text-ds-text-subtle whitespace-nowrap overflow-hidden text-ellipsis">
          {entry.title}
        </span>
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={onClick}
      className="absolute flex items-center gap-1 text-ui-2xs text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out"
      style={posStyle}
      title={entry.title}
    >
      <span
        className="shrink-0 rounded-full"
        style={{ width: 8, height: 8, backgroundColor: column.color }}
      />
      <span className="whitespace-nowrap overflow-hidden text-ellipsis">{entry.title}</span>
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
        <div
          className="w-2 h-2 rounded-full bg-destructive -ml-1"
        />
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
