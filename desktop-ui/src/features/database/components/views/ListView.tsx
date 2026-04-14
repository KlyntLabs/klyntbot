import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";
import { useMemo } from "react";

interface ListViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

export function ListView({ schema, entities, cardFields, onEntityClick }: ListViewProps) {
  const titleField = getTitleField(schema);
  const reorder = useEntityReorder(schema.id);
  const orderedIds = useMemo(() => entities.map((e) => e.id), [entities]);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const inlineFields =
    cardFields && cardFields.length > 0
      ? schema.fields.filter((f) => cardFields.includes(f.slug))
      : schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 3);

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const anchors = computeReorderAnchors(orderedIds, String(active.id), String(over.id));
    if (!anchors) return;
    void reorder.mutate({ entityId: String(active.id), ...anchors });
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={orderedIds} strategy={verticalListSortingStrategy}>
        <div className="w-full divide-y divide-border/40">
          {entities.map((entity) => (
            <SortableListRow
              key={entity.id}
              entity={entity}
              title={getEntityTitle(schema, entity.fields)}
              inlineFields={inlineFields}
              onClick={() => onEntityClick?.(entity)}
            />
          ))}
          {entities.length === 0 && (
            <div className="px-4 py-16 text-center text-[13px] text-foreground/60">
              No items yet
            </div>
          )}
        </div>
      </SortableContext>
    </DndContext>
  );
}

interface RowProps {
  entity: Entity;
  title: string;
  inlineFields: FieldDefinition[];
  onClick: () => void;
}

function SortableListRow({ entity, title, inlineFields, onClick }: RowProps) {
  const { setNodeRef, style, dragProps } = useSortableEntity(entity.id);
  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={onClick}
      className="flex w-full cursor-pointer items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-accent/60"
      {...dragProps}
    >
      <span className="flex-1 truncate text-[13px] font-medium text-foreground">{title}</span>
      <div className="flex shrink-0 items-center gap-3 text-[12px] text-foreground/70">
        {inlineFields.map((field) => (
          <span key={field.id} className="max-w-[140px] truncate">
            <FieldRenderer field={field} value={entity.fields[field.slug]} />
          </span>
        ))}
      </div>
    </button>
  );
}
