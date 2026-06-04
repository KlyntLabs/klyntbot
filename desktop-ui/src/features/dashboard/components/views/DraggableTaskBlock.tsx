import type { TimelineEntry } from "@/bindings";
import { minutesSinceMidnight } from "@/utils/dashboardDates";
import { cn } from "@/utils/cn";
import type { OverlapLayout } from "../../lib/timeline-utils";

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

  const displayTop = isDragging && ghostTopMin != null ? ghostTopMin : startMin;
  const displayEnd = isDragging && ghostEndMin != null ? ghostEndMin : endMin;

  const top = displayTop * pxPerMin;
  const height = Math.max((displayEnd - displayTop) * pxPerMin, MIN_BLOCK_HEIGHT);

  const colIndex = layout?.colIndex ?? 0;
  const totalCols = layout?.totalCols ?? 1;
  const leftPct = totalCols > 1 ? `${(colIndex / totalCols) * 100}%` : undefined;
  const widthPct = totalCols > 1 ? `${(1 / totalCols) * 100}%` : undefined;

  const posStyle: React.CSSProperties = leftPct
    ? { top, left: leftPct, width: widthPct, paddingLeft: 4, paddingRight: 2 }
    : { top, left: 4, right: 4 };

  const status = (entry.metadata as { status?: string } | null)?.status;

  return (
    <>
      <button
        type="button"
        className={cn(
          "absolute rounded-md px-1.5 py-0.5 text-ui-2xs leading-snug overflow-hidden cursor-grab transition-colors duration-ui-fast ease-out text-left motion-reduce:transition-none focus-visible:outline-2 focus-visible:outline-[var(--border-accent)] focus-visible:outline-offset-1 focus-visible:z-[5]",
          isDragging && "opacity-50 cursor-grabbing",
          selected && "outline outline-1 outline-border-accent",
        )}
        style={{ ...posStyle, height }}
        title={entry.title}
        onMouseDown={(e) => {
          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
          if (e.clientY > rect.bottom - 6) onMouseDownResize(e);
          else onMouseDownMove(e);
        }}
        onClick={(e) => {
          if (!isDragging) {
            e.stopPropagation();
            onClick();
          }
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onClick();
          }
        }}
      >
        <span className="block text-text-muted whitespace-nowrap overflow-hidden text-ellipsis">
          {entry.title}
        </span>
        {status && height > 28 && (
          <span className="block text-text-muted text-ui-2xs capitalize whitespace-nowrap overflow-hidden text-ellipsis">
            {status}
          </span>
        )}
        <div
          className="absolute bottom-0 left-0 right-0 h-1.5 cursor-ns-resize"
          onMouseDown={(e) => {
            e.stopPropagation();
            onMouseDownResize(e);
          }}
          role="separator"
          aria-label="Resize task"
        />
      </button>

      {isDragging && ghostTopMin != null && ghostEndMin != null && (
        <div
          className="absolute border-2 border-border-accent bg-[color-mix(in_srgb,var(--border-accent)_10%,transparent)] rounded-md pointer-events-none z-10"
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
