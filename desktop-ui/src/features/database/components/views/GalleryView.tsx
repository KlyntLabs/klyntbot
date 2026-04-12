import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";

interface GalleryViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
}

export function GalleryView({ schema, entities, cardFields, onEntityClick }: GalleryViewProps) {
  const titleField = getTitleField(schema);

  const displayFields = cardFields
    ? schema.fields.filter((f) => cardFields.includes(f.slug))
    : schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 4);

  return (
    <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-3 lg:grid-cols-4">
      {entities.map((entity) => {
        const title = getEntityTitle(schema, entity.fields);
        return (
          <button
            key={entity.id}
            type="button"
            onClick={() => onEntityClick?.(entity)}
            className="cursor-pointer rounded-lg border border-border bg-surface-base p-4 text-left shadow-sm transition-colors hover:bg-surface-hover"
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
      })}
      {entities.length === 0 && (
        <div className="col-span-full py-8 text-center text-muted">No entities yet</div>
      )}
    </div>
  );
}
