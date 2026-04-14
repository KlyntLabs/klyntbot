import {
  closestCorners,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useDroppable,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { useUpdateEntity } from "@features/database/hooks/useEntity";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { getEntityTitle } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import { tagColor } from "@shared/lib/tagColor";
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";
import { useMemo } from "react";

interface BoardViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  groupByField: string;
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

const EMPTY_GROUP = "\u2014";

function isEmptyValue(v: unknown): boolean {
  return v == null || v === "" || (Array.isArray(v) && v.length === 0);
}

export function BoardView({
  schema,
  entities,
  groupByField,
  cardFields,
  onEntityClick,
}: BoardViewProps) {
  const groupField = schema.fields.find((f) => f.slug === groupByField);
  const reorder = useEntityReorder(schema.id);
  const { mutate: updateEntity } = useUpdateEntity(schema.id);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const entityById = useMemo(() => {
    const m = new Map<string, Entity>();
    for (const e of entities) m.set(e.id, e);
    return m;
  }, [entities]);

  if (!groupField || groupField.fieldType !== "select") {
    return (
      <div className="p-6 text-[13px] text-foreground/55">
        Select a "select" field to group this board by.
      </div>
    );
  }

  const options: string[] = Array.isArray(groupField.options) ? groupField.options : [];
  const columns = [...options, EMPTY_GROUP];

  const displayFields: FieldDefinition[] =
    cardFields && cardFields.length > 0
      ? schema.fields.filter((f) => cardFields.includes(f.slug) && f.slug !== groupByField)
      : schema.fields.filter((f) => !f.hidden && f.slug !== groupByField).slice(0, 3);

  const groupedEntities = new Map<string, Entity[]>();
  for (const col of columns) groupedEntities.set(col, []);
  for (const entity of entities) {
    const val = String(entity.fields[groupByField] ?? EMPTY_GROUP);
    const bucket = groupedEntities.get(val) ?? groupedEntities.get(EMPTY_GROUP)!;
    bucket.push(entity);
  }

  const findColumnOfEntity = (entityId: string): string | null => {
    for (const [col, items] of groupedEntities) {
      if (items.some((e) => e.id === entityId)) return col;
    }
    return null;
  };

  const handleDragEnd = async ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const sourceId = String(active.id);
    const source = entityById.get(sourceId);
    if (!source) return;

    const overId = String(over.id);
    const overType = over.data.current?.type as "column" | "entity" | undefined;
    let targetCol: string;
    let targetEntityId: string | null = null;
    if (overType === "column") {
      targetCol = (over.data.current?.column as string) ?? EMPTY_GROUP;
    } else {
      targetEntityId = overId;
      targetCol = findColumnOfEntity(overId) ?? EMPTY_GROUP;
    }

    const sourceCol = findColumnOfEntity(sourceId);
    const crossColumn = sourceCol !== targetCol;
    const targetValue = targetCol === EMPTY_GROUP ? null : targetCol;

    if (crossColumn) {
      await updateEntity(sourceId, { fields: { [groupByField]: targetValue } });
    }

    const colItems = groupedEntities.get(targetCol) ?? [];
    let beforeId: string | undefined;
    let afterId: string | undefined;
    if (targetEntityId && targetEntityId !== sourceId) {
      const idx = colItems.findIndex((e) => e.id === targetEntityId);
      if (idx >= 0) {
        // When sort is same-column and moving forward, place after target; else before.
        const srcIdx = colItems.findIndex((e) => e.id === sourceId);
        const movingForward = !crossColumn && srcIdx >= 0 && srcIdx < idx;
        if (movingForward) {
          beforeId = targetEntityId;
          afterId = colItems[idx + 1]?.id;
        } else {
          afterId = targetEntityId;
          beforeId = colItems[idx - 1]?.id;
        }
      }
    } else {
      // Drop on empty column body → end of column
      const last = colItems[colItems.length - 1];
      if (last && last.id !== sourceId) beforeId = last.id;
    }

    if (beforeId || afterId) {
      await reorder.mutate({ entityId: sourceId, beforeId, afterId });
    }
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCorners} onDragEnd={handleDragEnd}>
      <div className="flex h-full gap-4 overflow-x-auto px-4 pt-3 pb-6">
        {columns.map((col) => {
          const items = groupedEntities.get(col) ?? [];
          const isEmptyCol = col === EMPTY_GROUP;
          const dotColor = isEmptyCol ? "var(--color-dim)" : tagColor(col);
          const label = isEmptyCol ? "No status" : col;
          return (
            <BoardColumn
              key={col}
              col={col}
              label={label}
              dotColor={dotColor}
              items={items}
              schema={schema}
              displayFields={displayFields}
              onEntityClick={onEntityClick}
            />
          );
        })}
      </div>
    </DndContext>
  );
}

interface BoardColumnProps {
  col: string;
  label: string;
  dotColor: string;
  items: Entity[];
  schema: DatabaseSchema;
  displayFields: FieldDefinition[];
  onEntityClick?: (entity: Entity) => void;
}

function BoardColumn({
  col,
  label,
  dotColor,
  items,
  schema,
  displayFields,
  onEntityClick,
}: BoardColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: `column:${col}`,
    data: { type: "column", column: col },
  });
  const itemIds = items.map((e) => e.id);
  return (
    <div className="flex w-[280px] shrink-0 flex-col">
      <div className="mb-2 flex items-center gap-2 px-1.5">
        <span
          className="size-2 shrink-0 rounded-full"
          style={{ backgroundColor: dotColor }}
          aria-hidden="true"
        />
        <h3 className="text-[12px] font-semibold uppercase tracking-wide text-foreground/80">
          {label}
        </h3>
        <span className="text-[11px] font-medium text-foreground/45 tabular-nums">
          {items.length}
        </span>
      </div>
      <SortableContext items={itemIds} strategy={verticalListSortingStrategy}>
        <div
          ref={setNodeRef}
          className={`flex flex-1 flex-col gap-2 rounded-md p-1 transition-colors ${
            isOver && items.length === 0 ? "bg-accent/40" : ""
          }`}
        >
          {items.map((entity) => (
            <SortableBoardCard
              key={entity.id}
              entity={entity}
              schema={schema}
              displayFields={displayFields}
              onClick={() => onEntityClick?.(entity)}
            />
          ))}
          {items.length === 0 && (
            <div className="flex items-center justify-center py-6 text-[11px] text-foreground/35">
              Drop here
            </div>
          )}
        </div>
      </SortableContext>
    </div>
  );
}

interface CardProps {
  entity: Entity;
  schema: DatabaseSchema;
  displayFields: FieldDefinition[];
  onClick: () => void;
}

function SortableBoardCard({ entity, schema, displayFields, onClick }: CardProps) {
  const { setNodeRef, style, dragProps } = useSortableEntity(entity.id);
  const title = getEntityTitle(schema, entity.fields);
  const meta = displayFields
    .map((field) => ({ field, value: entity.fields[field.slug] }))
    .filter(({ value }) => !isEmptyValue(value));
  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={onClick}
      className="group w-full cursor-pointer rounded-lg border border-border bg-surface-lowest p-3 text-left transition-all duration-150 hover:border-foreground/20 hover:-translate-y-0.5 hover:shadow-sm"
      {...dragProps}
    >
      <p className="text-[13px] font-medium text-foreground leading-snug line-clamp-3">{title}</p>
      {meta.length > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-foreground/70">
          {meta.map(({ field, value }) => (
            <span key={field.id} className="inline-flex min-w-0 max-w-full">
              <FieldRenderer field={field} value={value} />
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
