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
import { getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { DatabaseSchema, Entity, FieldDefinition, SortRule } from "@shared/types";
import { useMemo } from "react";

interface TableViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  visibleFields?: string[];
  sorts?: SortRule[];
  onSortChange?: (sorts: SortRule[]) => void;
  onEntityClick?: (entity: Entity) => void;
}

function getColumnStyle(field: FieldDefinition, isTitleField: boolean): React.CSSProperties {
  if (isTitleField) return { width: "22%", minWidth: 180 };
  switch (field.fieldType) {
    case "text":
      return { width: "14%", minWidth: 120 };
    case "select":
    case "multi_select":
      return { width: "10%", minWidth: 90 };
    case "number":
    case "date":
    case "created_time":
    case "last_edited":
      return { width: "10%", minWidth: 100 };
    case "checkbox":
      return { width: "5%", minWidth: 50 };
    default:
      return { width: "10%", minWidth: 80 };
  }
}

export function TableView({
  schema,
  entities,
  visibleFields,
  sorts,
  onSortChange,
  onEntityClick,
}: TableViewProps) {
  const titleField = getTitleField(schema);
  const reorder = useEntityReorder(schema.id);
  const orderedIds = useMemo(() => entities.map((e) => e.id), [entities]);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const anchors = computeReorderAnchors(orderedIds, String(active.id), String(over.id));
    if (!anchors) return;
    if ((sorts?.length ?? 0) > 0) onSortChange?.([]);
    void reorder.mutate({ entityId: String(active.id), ...anchors });
  };

  const columns =
    visibleFields && visibleFields.length > 0
      ? schema.fields.filter((f) => visibleFields.includes(f.slug) && !f.hidden)
      : schema.fields.filter((f) => !f.hidden);

  const toggleSort = (slug: string) => {
    if (!onSortChange) return;
    const existing = sorts?.find((s) => s.field === slug);
    if (!existing) {
      onSortChange([{ field: slug, direction: "asc" }]);
    } else if (existing.direction === "asc") {
      onSortChange([{ field: slug, direction: "desc" }]);
    } else {
      onSortChange([]);
    }
  };

  const sortIndicator = (slug: string) => {
    const rule = sorts?.find((s) => s.field === slug);
    if (!rule) return null;
    return (
      <svg
        className={`ml-1 inline-block h-3 w-3 text-foreground transition-transform ${
          rule.direction === "desc" ? "rotate-180" : ""
        }`}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        strokeWidth={2}
        role="img"
        aria-label="sort direction"
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5" />
      </svg>
    );
  };

  return (
    <div className="w-full overflow-x-auto">
      <table className="w-full table-fixed text-[13px]">
        <colgroup>
          {columns.map((field) => (
            <col key={field.id} style={getColumnStyle(field, titleField?.slug === field.slug)} />
          ))}
        </colgroup>

        <thead className="sticky top-0 z-10 bg-background">
          <tr className="border-y border-border">
            {columns.map((field, i) => (
              <th
                key={field.id}
                onClick={() => toggleSort(field.slug)}
                className={`whitespace-nowrap py-2 text-left text-[12px] font-medium text-foreground/70 cursor-pointer select-none transition-colors hover:bg-accent hover:text-foreground ${
                  i === 0 ? "pl-4 pr-3" : "px-3"
                } ${i === columns.length - 1 ? "pr-4" : ""}`}
              >
                <span className="inline-flex items-center">
                  {field.name}
                  {sortIndicator(field.slug)}
                </span>
              </th>
            ))}
          </tr>
        </thead>

        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext items={orderedIds} strategy={verticalListSortingStrategy}>
            <tbody>
              {entities.map((entity) => (
                <SortableRow
                  key={entity.id}
                  entity={entity}
                  columns={columns}
                  titleSlug={titleField?.slug}
                  onClick={() => onEntityClick?.(entity)}
                />
              ))}
              {entities.length === 0 && (
                <tr>
                  <td colSpan={columns.length} className="py-20 text-center">
                    <div className="flex flex-col items-center gap-3">
                      <p className="text-[14px] font-medium text-foreground">No items yet</p>
                      <p className="text-[13px] text-foreground/60">
                        Click <span className="text-brand font-medium">+ New</span> to create your
                        first entry
                      </p>
                    </div>
                  </td>
                </tr>
              )}
            </tbody>
          </SortableContext>
        </DndContext>
      </table>
    </div>
  );
}

interface SortableRowProps {
  entity: Entity;
  columns: FieldDefinition[];
  titleSlug: string | undefined;
  onClick: () => void;
}

function SortableRow({ entity, columns, titleSlug, onClick }: SortableRowProps) {
  const { setNodeRef, style, dragProps } = useSortableEntity(entity.id);
  return (
    <tr
      ref={setNodeRef}
      style={style}
      data-entity-id={entity.id}
      onClick={onClick}
      className="group border-b border-border/40 cursor-pointer transition-colors hover:bg-accent/60"
      {...dragProps}
    >
      {columns.map((field, i) => {
        const isTitle = titleSlug === field.slug;
        return (
          <td
            key={field.id}
            className={`overflow-hidden text-ellipsis whitespace-nowrap py-2 ${
              i === 0 ? "pl-4 pr-3" : "px-3"
            } ${i === columns.length - 1 ? "pr-4" : ""} ${
              isTitle ? "font-medium text-foreground" : "font-normal text-foreground/85"
            }`}
          >
            <FieldRenderer field={field} value={entity.fields[field.slug]} />
          </td>
        );
      })}
    </tr>
  );
}
