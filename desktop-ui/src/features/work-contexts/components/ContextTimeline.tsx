import type { ContextTimelineBlock } from "@shared/types";
import { useMemo, useState } from "react";
import { contextColor } from "../lib/context-colors";

interface ContextTimelineProps {
  blocks: ContextTimelineBlock[];
  hourHeight: number;
  dayStart: Date;
  onBlockClick?: (block: ContextTimelineBlock) => void;
}

function minuteOffset(iso: string, dayStart: Date): number {
  return (new Date(iso).getTime() - dayStart.getTime()) / 60_000;
}

export function ContextTimeline({
  blocks,
  hourHeight,
  dayStart,
  onBlockClick,
}: ContextTimelineProps) {
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);
  const pxPerMin = hourHeight / 60;

  const rendered = useMemo(() => {
    return blocks.map((block, idx) => {
      const startMin = minuteOffset(block.startTime, dayStart);
      const endMin = minuteOffset(block.endTime, dayStart);
      const top = startMin * pxPerMin;
      const height = Math.max((endMin - startMin) * pxPerMin, 4);
      const color = block.isIdle
        ? "rgba(255,255,255,0.04)"
        : contextColor(block.contextColor ?? undefined, block.contextType ?? undefined);
      const opacity = block.isIdle ? 0.3 : Math.min(0.4 + block.eventCount * 0.06, 1);

      return { block, idx, top, height, color, opacity, startMin, endMin };
    });
  }, [blocks, dayStart, pxPerMin]);

  return (
    <div className="relative w-full" style={{ height: 24 * hourHeight }}>
      {rendered.map(({ block, idx, top, height, color, opacity }) => (
        // biome-ignore lint/a11y/useKeyWithClickEvents: timeline blocks are supplementary click targets
        // biome-ignore lint/a11y/noStaticElementInteractions: timeline blocks
        <div
          key={`${block.startTime}-${block.contextId ?? "idle"}`}
          className="absolute left-0 right-0 rounded-md border border-separator cursor-pointer transition-all hover:brightness-125"
          style={{
            top,
            height,
            backgroundColor: color,
            opacity,
          }}
          onClick={() => onBlockClick?.(block)}
          onMouseEnter={() => setHoveredIdx(idx)}
          onMouseLeave={() => setHoveredIdx(null)}
        >
          {height > 18 && !block.isIdle && (
            <span className="block px-1.5 py-0.5 text-ui-xs text-white truncate font-medium">
              {block.contextTitle ?? "Unknown"}
            </span>
          )}

          {/* Tooltip */}
          {hoveredIdx === idx && (
            <div className="absolute left-full ml-2 top-0 z-50 glass-dropdown px-3 py-2 min-w-[180px] pointer-events-none">
              <p className="text-ui-sm font-medium text-fg">
                {block.isIdle ? "Idle" : (block.contextTitle ?? "Unassigned")}
              </p>
              <p className="text-ui-xs text-fg-secondary mt-0.5">
                {formatTime(block.startTime)} – {formatTime(block.endTime)}
              </p>
              <p className="text-ui-xs text-fg-secondary">
                {block.eventCount} event{block.eventCount !== 1 ? "s" : ""}
              </p>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
