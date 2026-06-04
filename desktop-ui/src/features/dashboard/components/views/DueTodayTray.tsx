import type { TimelineEntry } from "@/bindings";
import { cn } from "@/utils/cn";

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
    <div className="flex flex-wrap gap-1 px-1.5 py-1 border-b border-border-subtle bg-[color-mix(in_srgb,var(--surface-hover)_50%,transparent)]">
      {entries.map((entry) => {
        const meta = entry.metadata as Record<string, unknown> | null;
        const taskId = (meta?.taskId as string) ?? entry.entityId ?? entry.id;
        const estimatedMins = entry.durationSecs ? entry.durationSecs / 60 : 30;
        const isSelected = selectedEntryId === entry.id;

        return (
          <button
            key={entry.id}
            type="button"
            className={cn(
              "border-none px-1.5 py-0.5 rounded text-ui-2xs cursor-grab max-w-[120px] whitespace-nowrap overflow-hidden text-ellipsis transition-colors duration-ui-fast ease-out motion-reduce:transition-none",
              isSelected && "outline outline-1 outline-border-accent",
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
