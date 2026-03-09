import { useEffect, useMemo, useRef, useState } from "react";
import { formatHumanDuration, minutesSinceMidnight } from "../../lib/dates";
import type { TimelineEntry, TimelineSummary } from "../../lib/types";
import { cn } from "../../lib/utils";
import { focusColor } from "./buildContainers";
import { type LayerKey, useEnabledLayers, useSidebarOpen } from "./layers";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 60;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 14;
const HOUR_GUTTER = 48;
const PX_PER_MIN = HOUR_HEIGHT / 60;
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
    key: "focus",
    label: "Focus",
    icon: "◉",
    color: "var(--timeline-focus)",
    flex: 0.8,
    filter: (e) => e.entryType === "focusSession",
  },
  {
    key: "tasks",
    label: "Time Entries",
    icon: "☰",
    color: "var(--timeline-task)",
    flex: 2,
    filter: (e) => e.entryType === "taskTimeEntry",
  },
  {
    key: "apps",
    label: "Apps",
    icon: "⊞",
    color: "var(--timeline-app-neutral)",
    flex: 1.2,
    filter: (e) => e.entryType === "appUsage",
  },
  {
    key: "events",
    label: "Events",
    icon: "◆",
    color: "var(--timeline-note)",
    flex: 1,
    filter: (e) =>
      e.entryType !== "focusSession" &&
      e.entryType !== "taskTimeEntry" &&
      e.entryType !== "appUsage",
  },
];

interface DayColumnsViewProps {
  entries: TimelineEntry[];
  summary: TimelineSummary | null;
  isToday: boolean;
  loading: boolean;
}

export function DayColumnsView({ entries, summary, isToday, loading }: DayColumnsViewProps) {
  const { enabled } = useEnabledLayers();
  const sidebarOpen = useSidebarOpen();
  const [selectedEntry, setSelectedEntry] = useState<TimelineEntry | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Scroll to current hour on mount
  useEffect(() => {
    if (scrollRef.current) {
      const targetHour = isToday ? new Date().getHours() - 1 : 8;
      scrollRef.current.scrollTop = Math.max(0, targetHour * HOUR_HEIGHT);
    }
  }, [isToday]);

  // Group entries by column
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

  return (
    <div className="flex gap-2 h-full">
      <div className="flex-1 glass-card overflow-hidden flex flex-col">
        {loading && <div className="px-4 py-2 text-xs text-muted">Loading...</div>}

        {/* Scrollable timeline area (headers inside for scrollbar-width alignment) */}
        <div ref={scrollRef} className="flex-1 overflow-y-auto">
          {/* Sticky column headers — CSS grid ensures pixel-perfect alignment with tracks */}
          <div
            className="sticky top-0 z-20 grid border-b border-border bg-[rgba(0,0,0,0.85)]"
            style={{ gridTemplateColumns: gridTemplate }}
          >
            <div />
            {visibleColumns.map((col) => (
              <div
                key={col.key}
                className="text-[11px] text-muted font-medium py-1.5 px-1.5 border-r border-border last:border-r-0 flex items-center gap-1.5 truncate min-w-0"
              >
                <span
                  className="w-1.5 h-1.5 rounded-full shrink-0"
                  style={{ backgroundColor: col.color }}
                />
                {col.label}
              </div>
            ))}
          </div>

          <div className="relative" style={{ height: TOTAL_HEIGHT }}>
            {/* Hour lines + labels */}
            {HOURS.map((h) => (
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

            {/* Column tracks — same CSS grid as header */}
            <div
              className="absolute inset-0 grid"
              style={{ gridTemplateColumns: gridTemplate }}
            >
              <div />
              {visibleColumns.map((col) => {
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
                        selected={selectedEntry?.id === entry.id}
                        onClick={() =>
                          setSelectedEntry(selectedEntry?.id === entry.id ? null : entry)
                        }
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
          onClose={() => setSelectedEntry(null)}
        />
      )}
    </div>
  );
}

/** Renders a single entry block positioned by time within its column. */
function ColumnEntry({
  entry,
  column,
  selected,
  onClick,
}: {
  entry: TimelineEntry;
  column: ColumnDef;
  selected: boolean;
  onClick: () => void;
}) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const top = startMin * PX_PER_MIN;
  const dur = entry.durationSecs ?? 0;
  const height = Math.max(dur > 0 ? (dur / 60) * PX_PER_MIN : MIN_BLOCK_HEIGHT, MIN_BLOCK_HEIGHT);

  // Focus sessions render as colored blocks
  if (column.key === "focus") {
    const quality =
      typeof entry.metadata?.qualityScore === "number"
        ? (entry.metadata.qualityScore as number)
        : 5;
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-0.5 right-0.5 rounded-md cursor-pointer border border-timeline-focus/30 overflow-hidden",
          selected && "ring-1 ring-brand",
        )}
        style={{ top, height, backgroundColor: focusColor(quality) }}
        title={entry.title}
      >
        {height > 24 && (
          <span className="text-[10px] text-secondary/70 px-1 truncate block">{entry.title}</span>
        )}
      </button>
    );
  }

  // Task time entries — rich blocks with title + time
  if (column.key === "tasks") {
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
        <span className="text-secondary truncate block">{entry.title}</span>
        {height > 28 && (
          <span className="text-muted text-[10px] truncate block">
            {dur > 0 && `${formatHumanDuration(dur)} · `}
            {timeStr}
          </span>
        )}
      </button>
    );
  }

  // App activity — colored bars
  if (column.key === "apps") {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "absolute left-0.5 right-0.5 rounded-sm text-[10px] leading-tight overflow-hidden cursor-pointer px-1 py-0.5",
          "hover:brightness-125 transition-all",
          selected && "ring-1 ring-brand",
        )}
        style={{
          top,
          height: Math.max(height, 12),
          backgroundColor: `color-mix(in oklch, ${entry.color} 22%, transparent)`,
          borderLeft: `2px solid ${entry.color}`,
        }}
        title={entry.title}
      >
        <span className="text-secondary/80 truncate block">{entry.title}</span>
      </button>
    );
  }

  // Point events — dot + label
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-1 right-1 flex items-center gap-1 text-[10px] text-muted hover:text-secondary cursor-pointer transition-colors",
        selected && "text-brand",
      )}
      style={{ top }}
      title={entry.title}
    >
      <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: column.color }} />
      <span className="truncate">{entry.title}</span>
    </button>
  );
}

function NowLine() {
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const mins = now.getHours() * 60 + now.getMinutes();
  const top = mins * PX_PER_MIN;
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
