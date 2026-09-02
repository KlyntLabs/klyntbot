import { cn } from "@shared/lib/utils";
import type { TimelineEntry } from "@shared/types";

interface DueTodayTrayProps {
  entries: TimelineEntry[];
  onStartDrag: (e: React.MouseEvent, taskId: string, estimatedMinutes: number) => void;
  onSelect: (entry: TimelineEntry) => void;
  selectedEntryId: string | null;
}

export function DueTodayTray({
  entries,
  onStartDrag,
  onSelect,
  selectedEntryId,
}: DueTodayTrayProps) {
  if (entries.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1 px-1.5 py-1 border-b border-separator bg-bg-elevated/50">
      {entries.map((entry) => {
        const meta = entry.metadata as Record<string, unknown> | undefined;
        const taskId = (meta?.taskId as string) ?? entry.entityId ?? entry.id;
        const estimatedMins = entry.durationSecs ? entry.durationSecs / 60 : 30;
        const isSelected = selectedEntryId === entry.id;

        return (
          <button
            key={entry.id}
            type="button"
            className={cn(
              "px-1.5 py-0.5 rounded text-ui-xs truncate max-w-[120px] cursor-grab",
              "bg-[var(--ds-timeline-todo)]/15 text-[var(--ds-timeline-todo)]",
              "hover:bg-[var(--ds-timeline-todo)]/25 transition-colors",
              isSelected && "ring-1 ring-brand",
            )}
            title={entry.title}
            onClick={() => onSelect(entry)}
            onMouseDown={(e) => {
              if (e.button === 0) onStartDrag(e, taskId, estimatedMins);
            }}
          >
            {entry.title}
          </button>
        );
      })}
    </div>
  );
}
