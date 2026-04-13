import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity, FieldDefinition, SortRule } from "@shared/types";

interface TableViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  visibleFields?: string[];
  sorts?: SortRule[];
  onSortChange?: (sorts: SortRule[]) => void;
  onEntityClick?: (entity: Entity) => void;
}

/** Compute proportional column widths based on field type — title field gets more room */
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
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5" />
      </svg>
    );
  };

  return (
    <div className="w-full overflow-x-auto">
      <table className="w-full table-fixed text-[13px]">
        {/* Column sizing via colgroup */}
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
                  i === 0 ? "pl-10 pr-3" : "px-3"
                } ${i === columns.length - 1 ? "pr-10" : ""}`}
              >
                <span className="inline-flex items-center">
                  {field.name}
                  {sortIndicator(field.slug)}
                </span>
              </th>
            ))}
          </tr>
        </thead>

        <tbody>
          {entities.map((entity) => (
            <tr
              key={entity.id}
              onClick={() => onEntityClick?.(entity)}
              className="group border-b border-border/40 cursor-pointer transition-colors hover:bg-accent/60"
            >
              {columns.map((field, i) => {
                const isTitle = titleField?.slug === field.slug;
                return (
                  <td
                    key={field.id}
                    className={`overflow-hidden text-ellipsis whitespace-nowrap py-2 ${
                      i === 0 ? "pl-10 pr-3" : "px-3"
                    } ${i === columns.length - 1 ? "pr-10" : ""} ${
                      isTitle ? "font-medium text-foreground" : "font-normal text-foreground/85"
                    }`}
                  >
                    <FieldRenderer field={field} value={entity.fields[field.slug]} />
                  </td>
                );
              })}
            </tr>
          ))}

          {entities.length === 0 && (
            <tr>
              <td colSpan={columns.length} className="py-20 text-center">
                <div className="flex flex-col items-center gap-3">
                  <svg
                    className="h-10 w-10 text-foreground/30"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                    strokeWidth={1.25}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z"
                    />
                  </svg>
                  <p className="text-[14px] font-medium text-foreground">No items yet</p>
                  <p className="text-[13px] text-foreground/60">
                    Click <span className="text-brand font-medium">+ New</span> to create your first
                    entry
                  </p>
                </div>
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
