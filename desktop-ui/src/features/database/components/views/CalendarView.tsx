import {
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { CSS } from "@dnd-kit/utilities";
import { useUpdateEntity } from "@features/database/hooks/useEntity";
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
  const { mutate: updateEntity } = useUpdateEntity(schema.id);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

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

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const dayKey = String(over.id);
    void updateEntity(String(active.id), {
      fields: { [dateField]: `${dayKey}T00:00:00Z` },
    });
  };

  return (
    <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
      <div className="flex h-full w-full flex-col px-4 pt-3 pb-6">
        <div className="mb-3 flex shrink-0 items-center gap-2">
          <h3 className="text-[15px] font-semibold text-foreground">
            {format(currentMonth, "MMMM yyyy")}
          </h3>
          <div className="ml-auto flex items-center gap-1">
            <button
              type="button"
              onClick={() => setCurrentMonth(subMonths(currentMonth, 1))}
              className="rounded-md px-2 py-1 text-[13px] text-foreground/70 hover:bg-accent hover:text-foreground transition-colors"
            >
              &larr;
            </button>
            <button
              type="button"
              onClick={() => setCurrentMonth(new Date())}
              className="rounded-md px-2.5 py-1 text-[13px] text-foreground/70 hover:bg-accent hover:text-foreground transition-colors"
            >
              Today
            </button>
            <button
              type="button"
              onClick={() => setCurrentMonth(addMonths(currentMonth, 1))}
              className="rounded-md px-2 py-1 text-[13px] text-foreground/70 hover:bg-accent hover:text-foreground transition-colors"
            >
              &rarr;
            </button>
          </div>
        </div>
        <div className="flex min-h-0 w-full flex-1 flex-col overflow-hidden rounded-lg border border-border">
          <div className="grid shrink-0 grid-cols-7 border-b border-border bg-accent/20">
            {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((d) => (
              <div
                key={d}
                className="px-2 py-1.5 text-center text-[11px] font-semibold uppercase tracking-wide text-foreground/60"
              >
                {d}
              </div>
            ))}
          </div>
          <div
            className="grid min-h-0 w-full flex-1 grid-cols-7 bg-border/60"
            style={{ gridAutoRows: "minmax(100px, 1fr)", gap: "1px" }}
          >
            {days.map((day) => {
              const key = format(day, "yyyy-MM-dd");
              const dayEntities = entityByDate.get(key) ?? [];
              return (
                <DayCell
                  key={key}
                  dayKey={key}
                  day={day}
                  inMonth={isSameMonth(day, currentMonth)}
                  today={isToday(day)}
                  entities={dayEntities}
                  schema={schema}
                  onEntityClick={onEntityClick}
                />
              );
            })}
          </div>
        </div>
      </div>
    </DndContext>
  );
}

interface DayCellProps {
  dayKey: string;
  day: Date;
  inMonth: boolean;
  today: boolean;
  entities: Entity[];
  schema: DatabaseSchema;
  onEntityClick?: (entity: Entity) => void;
}

function DayCell({ dayKey, day, inMonth, today, entities, schema, onEntityClick }: DayCellProps) {
  const { setNodeRef, isOver } = useDroppable({ id: dayKey });
  return (
    <div
      ref={setNodeRef}
      className={`flex flex-col bg-background p-1.5 transition-colors ${
        !inMonth ? "bg-background/40" : ""
      } ${isOver ? "ring-2 ring-inset ring-accent" : ""}`}
    >
      <div
        className={`mb-1 text-[12px] tabular-nums ${
          today
            ? "inline-flex h-5 w-5 items-center justify-center rounded-full bg-brand font-semibold text-primary-foreground"
            : inMonth
              ? "text-foreground/80"
              : "text-foreground/35"
        }`}
      >
        {format(day, "d")}
      </div>
      <div className="flex flex-col gap-0.5">
        {entities.slice(0, 3).map((entity) => (
          <DraggableChip
            key={entity.id}
            entity={entity}
            title={getEntityTitle(schema, entity.fields)}
            onClick={() => onEntityClick?.(entity)}
          />
        ))}
        {entities.length > 3 && (
          <div className="px-1.5 text-[11px] text-foreground/55">+{entities.length - 3} more</div>
        )}
      </div>
    </div>
  );
}

function DraggableChip({
  entity,
  title,
  onClick,
}: {
  entity: Entity;
  title: string;
  onClick: () => void;
}) {
  // Calendar chips use useDraggable (not useSortable) because dropping on a
  // day cell changes the date field, not the list order — there is no sort
  // index within a cell.
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: entity.id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Translate.toString(transform),
    opacity: isDragging ? 0.4 : 1,
  };
  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={onClick}
      className="w-full cursor-pointer truncate rounded bg-brand/15 px-1.5 py-0.5 text-left text-[11px] font-medium text-foreground hover:bg-brand/25 transition-colors"
      {...attributes}
      {...listeners}
    >
      {title}
    </button>
  );
}
