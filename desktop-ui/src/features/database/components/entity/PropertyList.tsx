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
    <div className="space-y-3">
      {visibleFields.map((field) => (
        <div key={field.id} className="flex flex-col gap-1">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            {field.name}
            {field.required && <span className="text-red-400 ml-0.5">*</span>}
          </span>
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
      ))}
    </div>
  );
}
