import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { rectSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";
import { useMemo } from "react";

interface GalleryViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

export function GalleryView({ schema, entities, cardFields, onEntityClick }: GalleryViewProps) {
  const titleField = getTitleField(schema);
  const reorder = useEntityReorder(schema.id);
  const orderedIds = useMemo(() => entities.map((e) => e.id), [entities]);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const displayFields =
    cardFields && cardFields.length > 0
      ? schema.fields.filter((f) => cardFields.includes(f.slug))
      : schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 4);

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const anchors = computeReorderAnchors(orderedIds, String(active.id), String(over.id));
    if (!anchors) return;
    void reorder.mutate({ entityId: String(active.id), ...anchors });
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={orderedIds} strategy={rectSortingStrategy}>
        <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {entities.map((entity) => (
            <SortableCard
              key={entity.id}
              entity={entity}
              title={getEntityTitle(schema, entity.fields)}
              displayFields={displayFields}
              onClick={() => onEntityClick?.(entity)}
            />
          ))}
          {entities.length === 0 && (
            <div className="col-span-full py-8 text-center text-muted">No entities yet</div>
          )}
        </div>
      </SortableContext>
    </DndContext>
  );
}

interface CardProps {
  entity: Entity;
  title: string;
  displayFields: FieldDefinition[];
  onClick: () => void;
}

function SortableCard({ entity, title, displayFields, onClick }: CardProps) {
  const { setNodeRef, style, dragProps } = useSortableEntity(entity.id);
  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={onClick}
      className="cursor-pointer rounded-lg border border-border bg-surface-base p-4 text-left shadow-sm transition-colors hover:bg-surface-hover"
      {...dragProps}
    >
      <p className="mb-2 truncate text-sm font-semibold">{title}</p>
      <div className="space-y-1">
        {displayFields.map((field) => (
          <div key={field.id} className="flex items-start gap-1 text-xs">
            <span className="shrink-0 text-muted">{field.name}:</span>
            <FieldRenderer field={field} value={entity.fields[field.slug]} />
          </div>
        ))}
      </div>
    </button>
  );
}
