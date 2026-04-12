import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";

interface ListViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

export function ListView({ schema, entities, cardFields, onEntityClick }: ListViewProps) {
  const titleField = getTitleField(schema);

  const inlineFields = cardFields
    ? schema.fields.filter((f) => cardFields.includes(f.slug))
    : schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 3);

  return (
    <div className="divide-y divide-border">
      {entities.map((entity) => {
        const title = getEntityTitle(schema, entity.fields);
        return (
          <button
            key={entity.id}
            type="button"
            onClick={() => onEntityClick?.(entity)}
            className="flex w-full cursor-pointer items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-surface-hover"
          >
            <span className="flex-1 truncate text-sm font-medium">{title}</span>
            <div className="flex shrink-0 items-center gap-3 text-xs text-muted">
              {inlineFields.map((field) => (
                <span key={field.id} className="max-w-[120px] truncate">
                  <FieldRenderer field={field} value={entity.fields[field.slug]} />
                </span>
              ))}
            </div>
          </button>
        );
      })}
      {entities.length === 0 && (
        <div className="px-4 py-8 text-center text-muted">No entities yet</div>
      )}
    </div>
  );
}
