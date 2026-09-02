import { minutesSinceMidnight } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";
import type { OverlapLayout } from "../lib/timeline-utils";

const MIN_BLOCK_HEIGHT = 14;

interface DraggableTaskBlockProps {
  entry: TimelineEntry;
  pxPerMin: number;
  selected: boolean;
  layout?: OverlapLayout;
  isDragging: boolean;
  ghostTopMin?: number;
  ghostEndMin?: number;
  onMouseDownMove: (e: React.MouseEvent) => void;
  onMouseDownResize: (e: React.MouseEvent) => void;
  onClick: () => void;
}

export function DraggableTaskBlock({
  entry,
  pxPerMin,
  selected,
  layout,
  isDragging,
  ghostTopMin,
  ghostEndMin,
  onMouseDownMove,
  onMouseDownResize,
  onClick,
}: DraggableTaskBlockProps) {
  const startMin = minutesSinceMidnight(entry.startedAt);
  const dur = entry.durationSecs ?? 0;
  const endMin = dur > 0 ? startMin + dur / 60 : startMin + 30;

  // Use ghost position when dragging this block
  const displayTop = isDragging && ghostTopMin != null ? ghostTopMin : startMin;
  const displayEnd = isDragging && ghostEndMin != null ? ghostEndMin : endMin;

  const top = displayTop * pxPerMin;
  const height = Math.max((displayEnd - displayTop) * pxPerMin, MIN_BLOCK_HEIGHT);

  // Overlap layout: compute left/width percentages
  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const leftPct = totalCols > 1 ? `${(colIndex / totalCols) * 100}%` : undefined;
  const widthPct = totalCols > 1 ? `${(1 / totalCols) * 100}%` : undefined;

  const posStyle: React.CSSProperties = leftPct
    ? { top, left: leftPct, width: widthPct, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

  const status = entry.metadata?.status as string | undefined;

  return (
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: drag handle — keyboard not applicable */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: drag handle for timeline scheduling */}
      <div
        className={cn(
          "absolute rounded-md px-1.5 py-0.5 text-ui-xs leading-tight overflow-hidden",
          "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/15",
          "hover:bg-[var(--timeline-todo)]/25 transition-colors",
          isDragging && "opacity-50",
          selected && "ring-1 ring-brand",
          !isDragging && "cursor-grab",
          isDragging && "cursor-grabbing",
        )}
        style={{ ...posStyle, height }}
        title={entry.title}
        onMouseDown={(e) => {
          // Check if clicking in the resize zone (bottom 6px)
          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
          if (e.clientY > rect.bottom - 6) {
            onMouseDownResize(e);
          } else {
            onMouseDownMove(e);
          }
        }}
        onClick={(e) => {
          // Only fire click if not dragging
          if (!isDragging) {
            e.stopPropagation();
            onClick();
          }
        }}
      >
        <span className="text-fg-secondary truncate block">{entry.title}</span>
        {status && height > 28 && (
          <span className="text-fg-secondary text-ui-xs truncate block capitalize">{status}</span>
        )}
        {/* Resize handle zone */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: resize handle for timeline block */}
        <div
          className="absolute bottom-0 left-0 right-0 h-1.5 cursor-ns-resize"
          onMouseDown={(e) => {
            e.stopPropagation();
            onMouseDownResize(e);
          }}
        />
      </div>

      {/* Ghost preview during drag */}
      {isDragging && ghostTopMin != null && ghostEndMin != null && (
        <div
          className="absolute rounded-md border-2 border-brand bg-brand/10 pointer-events-none z-10"
          style={{
            top: ghostTopMin * pxPerMin,
            left: leftPct ?? 4,
            right: leftPct ? undefined : 4,
            width: widthPct,
            height: Math.max((ghostEndMin - ghostTopMin) * pxPerMin, MIN_BLOCK_HEIGHT),
          }}
        />
      )}
    </>
  );
}
