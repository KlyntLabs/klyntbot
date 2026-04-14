import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { DatabaseSchema, Entity } from "@shared/types";
import type { Row } from "@tanstack/react-table";

interface Props {
  row: Row<Entity>;
  schema: DatabaseSchema;
  top: number;
  onClick: () => void;
}

export function TableBodyRow({ row, schema, top, onClick }: Props) {
  const entity = row.original;
  const { setNodeRef, style: dragStyle, dragProps } = useSortableEntity(entity.id);
  const titleSlug = getTitleField(schema)?.slug;
  const visibleCells = row.getVisibleCells();

  return (
    <div
      ref={setNodeRef}
      data-entity-id={entity.id}
      onClick={onClick}
      className="absolute left-0 top-0 flex cursor-pointer border-b border-border/40 transition-colors hover:bg-accent/60"
      style={{
        ...dragStyle,
        transform: `translateY(${top}px) ${dragStyle.transform ?? ""}`,
        minWidth: visibleCells.reduce((w, c) => w + c.column.getSize(), 0),
        width: "100%",
      }}
      {...dragProps}
    >
      {visibleCells.map((cell, i) => {
        const field = cell.column.columnDef.meta?.field;
        if (!field) return null;
        const isTitle = field.slug === titleSlug;
        const isLast = i === visibleCells.length - 1;
        const size = cell.column.getSize();
        return (
          <div
            key={cell.id}
            className={`overflow-hidden text-ellipsis whitespace-nowrap px-3 py-2 text-[13px] ${
              isLast ? "" : "border-r border-border/40"
            } ${isTitle ? "font-medium text-foreground" : "font-normal text-foreground/85"}`}
            style={isLast ? { flex: 1, minWidth: size } : { width: size }}
          >
            <FieldRenderer field={field} value={entity.fields[field.slug]} />
          </div>
        );
      })}
    </div>
  );
}
