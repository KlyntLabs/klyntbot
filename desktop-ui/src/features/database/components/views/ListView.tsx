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
import { useCollapsedGroups } from "@features/database/hooks/useCollapsedGroups";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { groupEntities } from "@features/database/lib/grouping";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { DatabaseSchema, Entity, FieldDefinition, ViewDefinition } from "@shared/types";
import { useMemo } from "react";
import { GroupHeader } from "./GroupHeader";

interface ListViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  view: ViewDefinition;
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

export function ListView({ schema, entities, view, cardFields, onEntityClick }: ListViewProps) {
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

  const groupBy = view.config.groupBy;
  const grouped = Boolean(groupBy);
  const groups = useMemo(
    () => groupEntities(entities, schema, groupBy),
    [entities, schema, groupBy],
  );

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={orderedIds} strategy={verticalListSortingStrategy}>
        <div className="w-full">
          {grouped ? (
            <GroupedList
              groups={groups}
              schema={schema}
              view={view}
              inlineFields={inlineFields}
              onEntityClick={onEntityClick}
            />
          ) : (
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
            </div>
          )}
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

function GroupedList({
  groups,
  schema,
  view,
  inlineFields,
  onEntityClick,
}: {
  groups: ReturnType<typeof groupEntities>;
  schema: DatabaseSchema;
  view: ViewDefinition;
  inlineFields: FieldDefinition[];
  onEntityClick: ((entity: Entity) => void) | undefined;
}) {
  const { collapsed, toggle } = useCollapsedGroups(schema.id, view);
  return (
    <>
      {groups.map((g) => {
        const isCollapsed = collapsed.has(g.key);
        return (
          <section key={g.key}>
            <GroupHeader
              label={g.label}
              count={g.entities.length}
              collapsed={isCollapsed}
              onToggle={() => toggle(g.key)}
            />
            {!isCollapsed && (
              <div className="w-full divide-y divide-border/40">
                {g.entities.map((entity) => (
                  <SortableListRow
                    key={entity.id}
                    entity={entity}
                    title={getEntityTitle(schema, entity.fields)}
                    inlineFields={inlineFields}
                    onClick={() => onEntityClick?.(entity)}
                  />
                ))}
              </div>
            )}
          </section>
        );
      })}
    </>
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
