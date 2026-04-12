import { getEntityTitle } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";
import {
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  endOfWeek,
  format,
  isSameMonth,
  isToday,
  startOfMonth,
  startOfWeek,
  subMonths,
} from "date-fns";
import { useState } from "react";

interface CalendarViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  dateField: string;
  onEntityClick?: (entity: Entity) => void;
}

export function CalendarView({ schema, entities, dateField, onEntityClick }: CalendarViewProps) {
  const [currentMonth, setCurrentMonth] = useState(new Date());

  const monthStart = startOfMonth(currentMonth);
  const monthEnd = endOfMonth(currentMonth);
  const calendarStart = startOfWeek(monthStart, { weekStartsOn: 1 });
  const calendarEnd = endOfWeek(monthEnd, { weekStartsOn: 1 });
  const days = eachDayOfInterval({ start: calendarStart, end: calendarEnd });

  const entityByDate = new Map<string, Entity[]>();
  for (const entity of entities) {
    const dateVal = entity.fields[dateField];
    if (!dateVal) continue;
    try {
      const key = format(new Date(String(dateVal)), "yyyy-MM-dd");
      const list = entityByDate.get(key) ?? [];
      list.push(entity);
      entityByDate.set(key, list);
    } catch {
      /* skip invalid dates */
    }
  }

  return (
    <div className="p-4">
      <div className="mb-4 flex items-center justify-between">
        <button
          type="button"
          onClick={() => setCurrentMonth(subMonths(currentMonth, 1))}
          className="rounded px-2 py-1 text-sm hover:bg-surface-hover"
        >
          &larr;
        </button>
        <h3 className="text-lg font-semibold">{format(currentMonth, "MMMM yyyy")}</h3>
        <button
          type="button"
          onClick={() => setCurrentMonth(addMonths(currentMonth, 1))}
          className="rounded px-2 py-1 text-sm hover:bg-surface-hover"
        >
          &rarr;
        </button>
      </div>
      <div className="grid grid-cols-7 gap-px overflow-hidden rounded bg-border">
        {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((d) => (
          <div
            key={d}
            className="bg-surface-base px-2 py-1 text-center text-xs font-medium text-muted"
          >
            {d}
          </div>
        ))}
        {days.map((day) => {
          const key = format(day, "yyyy-MM-dd");
          const dayEntities = entityByDate.get(key) ?? [];
          const inMonth = isSameMonth(day, currentMonth);
          return (
            <div
              key={key}
              className={`min-h-[80px] bg-surface-base p-1 ${
                !inMonth ? "opacity-40" : ""
              } ${isToday(day) ? "ring-1 ring-accent ring-inset" : ""}`}
            >
              <div className="mb-1 text-xs text-muted">{format(day, "d")}</div>
              <div className="space-y-0.5">
                {dayEntities.slice(0, 3).map((entity) => {
                  const title = getEntityTitle(schema, entity.fields);
                  return (
                    <button
                      key={entity.id}
                      type="button"
                      onClick={() => onEntityClick?.(entity)}
                      className="w-full cursor-pointer truncate rounded bg-accent/10 px-1 py-0.5 text-left text-xs hover:bg-accent/20"
                    >
                      {title}
                    </button>
                  );
                })}
                {dayEntities.length > 3 && (
                  <div className="text-xs text-muted">+{dayEntities.length - 3} more</div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
