import type { TimelineEntry } from "@/bindings";

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
    <div className="dashboard__due-today-tray">
      {entries.map((entry) => {
        const meta = entry.metadata as Record<string, unknown> | null;
        const taskId = (meta?.taskId as string) ?? entry.entityId ?? entry.id;
        const estimatedMins = entry.durationSecs ? entry.durationSecs / 60 : 30;
        const isSelected = selectedEntryId === entry.id;
        const cls = `dashboard__due-today-chip${isSelected ? " dashboard__due-today-chip--selected" : ""}`;

        return (
          <button
            key={entry.id}
            type="button"
            className={cls}
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
