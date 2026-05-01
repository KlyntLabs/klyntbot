import type { TimelineEntry } from "@/bindings";
import { minutesSinceMidnight } from "@/utils/dashboardDates";
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

  const blockClass = [
    "dashboard__task-block",
    isDragging ? "dashboard__task-block--dragging" : "",
    selected ? "dashboard__task-block--selected" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: drag handle — keyboard not applicable */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: drag handle for timeline scheduling */}
      <div
        className={blockClass}
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
      >
        <span className="dashboard__task-block-title">{entry.title}</span>
        {status && height > 28 && <span className="dashboard__task-block-status">{status}</span>}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: resize handle */}
        <div
          className="dashboard__task-block-resize-handle"
          onMouseDown={(e) => {
            e.stopPropagation();
            onMouseDownResize(e);
          }}
        />
      </div>

      {isDragging && ghostTopMin != null && ghostEndMin != null && (
        <div
          className="dashboard__task-ghost"
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
