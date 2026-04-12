import { getEntityTitle } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";
import { addDays, differenceInDays, format, startOfDay } from "date-fns";

interface TimelineViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  startField: string;
  endField?: string;
  onEntityClick?: (entity: Entity) => void;
}

export function TimelineView({
  schema,
  entities,
  startField,
  endField,
  onEntityClick,
}: TimelineViewProps) {
  const dates: Date[] = [];
  for (const entity of entities) {
    const start = entity.fields[startField];
    if (start) dates.push(new Date(String(start)));
    if (endField) {
      const end = entity.fields[endField];
      if (end) dates.push(new Date(String(end)));
    }
  }

  if (dates.length === 0) {
    return <div className="p-4 text-muted">No entities with date values.</div>;
  }

  const minDate = startOfDay(new Date(Math.min(...dates.map((d) => d.getTime()))));
  const maxDate = startOfDay(addDays(new Date(Math.max(...dates.map((d) => d.getTime()))), 1));
  const totalDays = Math.max(differenceInDays(maxDate, minDate), 1);
  const dayWidth = 40;

  return (
    <div className="overflow-x-auto p-4">
      <div style={{ width: totalDays * dayWidth, minWidth: "100%" }}>
        <div className="mb-2 flex border-b border-border">
          {Array.from({ length: totalDays }, (_, i) => {
            const day = addDays(minDate, i);
            return (
              <div
                key={i}
                style={{ width: dayWidth }}
                className="shrink-0 py-1 text-center text-xs text-muted"
              >
                {format(day, "d")}
              </div>
            );
          })}
        </div>

        {/* Entity bars */}
        <div className="space-y-1">
          {entities.map((entity) => {
            const startVal = entity.fields[startField];
            if (!startVal) return null;
            const start = startOfDay(new Date(String(startVal)));
            const end =
              endField && entity.fields[endField]
                ? startOfDay(new Date(String(entity.fields[endField])))
                : addDays(start, 1);
            const offset = differenceInDays(start, minDate);
            const width = Math.max(differenceInDays(end, start), 1);
            const title = getEntityTitle(schema, entity.fields);

            return (
              <div key={entity.id} className="relative h-7">
                <button
                  type="button"
                  onClick={() => onEntityClick?.(entity)}
                  className="absolute flex h-6 cursor-pointer items-center rounded border border-accent/30 bg-accent/20 px-2 transition-colors hover:bg-accent/30"
                  style={{
                    left: offset * dayWidth,
                    width: width * dayWidth,
                  }}
                >
                  <span className="truncate text-xs font-medium">{title}</span>
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
