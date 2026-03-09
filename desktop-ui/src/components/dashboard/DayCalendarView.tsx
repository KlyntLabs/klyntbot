import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import { minutesSinceMidnight, todayISO } from "../../lib/dates";
import type { TimelineEntry } from "../../lib/types";
import { EMPTY_TIMELINE_RESPONSE } from "../../lib/types";
import { cn } from "../../lib/utils";
import { SummaryPanel } from "./SummaryPanel";

const HOUR_HEIGHT = 60;
const TOTAL_HEIGHT = 24 * HOUR_HEIGHT;
const MIN_BLOCK_HEIGHT = 15;
const HOUR_GUTTER = 48;

interface PositionedBlock {
  entry: TimelineEntry;
  top: number;
  height: number;
  column: number;
  totalColumns: number;
}

function positionBlocks(entries: TimelineEntry[]): PositionedBlock[] {
  const pxPerMin = HOUR_HEIGHT / 60;

  // Separate duration blocks and point events
  const blocks: { entry: TimelineEntry; startMin: number; endMin: number }[] = [];

  for (const entry of entries) {
    const startMin = minutesSinceMidnight(entry.startedAt);
    const dur = entry.durationSecs ?? 0;
    const endMin = dur > 0 ? startMin + dur / 60 : startMin + MIN_BLOCK_HEIGHT / pxPerMin;
    blocks.push({ entry, startMin, endMin });
  }

  // Sort by start time
  blocks.sort((a, b) => a.startMin - b.startMin);

  // Assign columns for overlapping blocks
  const positioned: PositionedBlock[] = [];
  const groups: (typeof blocks)[] = [];
  let currentGroup: typeof blocks = [];

  for (const block of blocks) {
    if (
      currentGroup.length === 0 ||
      block.startMin < Math.max(...currentGroup.map((b) => b.endMin))
    ) {
      currentGroup.push(block);
    } else {
      if (currentGroup.length > 0) groups.push(currentGroup);
      currentGroup = [block];
    }
  }
  if (currentGroup.length > 0) groups.push(currentGroup);

  for (const group of groups) {
    const columns: number[] = [];
    for (const block of group) {
      // Find first available column
      let col = 0;
      while (columns[col] !== undefined && block.startMin < columns[col]) {
        col++;
      }
      columns[col] = block.endMin;

      const top = block.startMin * pxPerMin;
      const rawHeight = (block.endMin - block.startMin) * pxPerMin;
      const height = Math.max(rawHeight, MIN_BLOCK_HEIGHT);

      positioned.push({
        entry: block.entry,
        top,
        height,
        column: col,
        totalColumns: group.length, // Will be refined below
      });
    }
    // Set totalColumns for all blocks in this group to the actual column count
    const maxCols = columns.length;
    for (const p of positioned.slice(positioned.length - group.length)) {
      p.totalColumns = maxCols;
    }
  }

  return positioned;
}

export function DayCalendarView() {
  const { date } = useParams<{ date: string }>();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const queryArgs = useMemo(() => ({ startDate: dateStr, endDate: dateStr }), [dateStr]);

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

  const positioned = useMemo(() => positionBlocks(data.entries), [data.entries]);

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

            {/* Blocks */}
            <div className="absolute inset-0" style={{ left: HOUR_GUTTER }}>
              {positioned.map((block) => (
                <button
                  type="button"
                  key={block.entry.id}
                  onClick={() =>
                    setSelectedEntry(selectedEntry?.id === block.entry.id ? null : block.entry)
                  }
                  className={cn(
                    "absolute rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer transition-opacity border border-white/10",
                    "hover:opacity-90",
                    selectedEntry?.id === block.entry.id && "ring-1 ring-brand",
                  )}
                  style={{
                    top: block.top,
                    height: block.height,
                    left: `${(block.column / block.totalColumns) * 100}%`,
                    width: `${(1 / block.totalColumns) * 100}%`,
                    backgroundColor: `color-mix(in oklch, ${block.entry.color} 30%, transparent)`,
                    borderLeftColor: block.entry.color,
                    borderLeftWidth: 3,
                  }}
                  title={block.entry.title}
                >
                  <span className="text-secondary truncate block">{block.entry.title}</span>
                  {block.height > 30 && block.entry.description && (
                    <span className="text-muted truncate block text-[10px]">
                      {block.entry.description}
                    </span>
                  )}
                </button>
              ))}
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
