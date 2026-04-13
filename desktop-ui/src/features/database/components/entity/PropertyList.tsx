import { FieldEditor } from "@features/database/components/fields/FieldEditor";
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import type { DatabaseSchema, Entity } from "@shared/types";

interface PropertyListProps {
  schema: DatabaseSchema;
  entity: Entity;
  editing?: boolean;
  onChange?: (slug: string, value: unknown) => void;
}

export function PropertyList({ schema, entity, editing, onChange }: PropertyListProps) {
  const visibleFields = schema.fields.filter((f) => !f.hidden);

  return (
    <div className="divide-y divide-border/30">
      {visibleFields.map((field) => (
        <div key={field.id} className="flex items-start gap-3 py-2">
          {/* Label — fixed width, Notion-style muted left column */}
          <span className="w-36 shrink-0 truncate py-1 text-[13px] text-muted-foreground">
            {field.name}
            {field.required && <span className="text-destructive ml-0.5">*</span>}
          </span>
          {/* Value */}
          <div className="min-w-0 flex-1 py-1 text-[13px]">
            {editing && field.fieldType !== "created_time" && field.fieldType !== "last_edited" ? (
              <FieldEditor
                field={field}
                value={entity.fields[field.slug]}
                onChange={(v) => onChange?.(field.slug, v)}
              />
            ) : (
              <FieldRenderer field={field} value={entity.fields[field.slug]} />
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
