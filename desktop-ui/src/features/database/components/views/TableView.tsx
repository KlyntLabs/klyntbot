import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
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
  const columns = visibleFields
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
    return rule.direction === "asc" ? " \u2191" : " \u2193";
  };

  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr className="border-b border-border">
            {columns.map((field) => (
              <th
                key={field.id}
                onClick={() => toggleSort(field.slug)}
                className="px-3 py-2 text-left text-xs font-medium text-muted uppercase tracking-wide cursor-pointer hover:bg-surface-hover select-none"
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
              className="border-b border-border/50 cursor-pointer hover:bg-surface-hover transition-colors"
            >
              {columns.map((field) => (
                <td key={field.id} className="px-3 py-2 max-w-[200px]">
                  <FieldRenderer field={field} value={entity.fields[field.slug]} />
                </td>
              ))}
            </tr>
          ))}
          {entities.length === 0 && (
            <tr>
              <td colSpan={columns.length} className="px-3 py-8 text-center text-muted">
                No entities yet
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
