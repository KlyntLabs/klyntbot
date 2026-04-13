import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity, SortRule } from "@shared/types";

interface TableViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  visibleFields?: string[];
  sorts?: SortRule[];
  onSortChange?: (sorts: SortRule[]) => void;
  onEntityClick?: (entity: Entity) => void;
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
    return <span className="ml-1 text-brand">{rule.direction === "asc" ? "↑" : "↓"}</span>;
  };

  return (
    <table className="w-full table-fixed text-sm">
      <thead>
        <tr className="border-b border-border">
          {columns.map((field) => (
            <th
              key={field.id}
              onClick={() => toggleSort(field.slug)}
              className="overflow-hidden text-ellipsis whitespace-nowrap px-4 py-1.5 text-left text-xs font-light text-muted-foreground cursor-pointer hover:bg-accent select-none transition-colors"
            >
              {field.name}
              {sortIndicator(field.slug)}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {entities.map((entity) => (
          <tr
            key={entity.id}
            onClick={() => onEntityClick?.(entity)}
            className="group border-b border-border/30 cursor-pointer hover:bg-accent transition-colors"
          >
            {columns.map((field) => (
              <td
                key={field.id}
                className={`overflow-hidden text-ellipsis whitespace-nowrap px-4 py-1.5 text-foreground ${
                  titleField?.slug === field.slug ? "font-medium" : "font-light"
                }`}
              >
                <FieldRenderer field={field} value={entity.fields[field.slug]} />
              </td>
            ))}
          </tr>
        ))}
        {entities.length === 0 && (
          <tr>
            <td colSpan={columns.length} className="px-3 py-12 text-center text-muted-foreground">
              No entities yet. Click <span className="text-brand">+ New</span> to create one.
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
