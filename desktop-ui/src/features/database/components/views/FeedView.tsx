import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle, getTitleField } from "@features/database/lib/schema-utils";
import { formatRelativeTime } from "@shared/lib/dates";
import type { DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import { useMemo } from "react";

interface Props {
  schema: DatabaseSchema;
  view: ViewDefinition;
  entities: Entity[];
  onEntityClick?: (entity: Entity) => void;
}

const PAGE_SIZE = 100;

export function FeedView({ schema, view, entities, onEntityClick }: Props) {
  const titleField = getTitleField(schema);
  const cardFieldSlugs = view.config.cardFields;
  const inlineFields = useMemo(() => {
    if (cardFieldSlugs && cardFieldSlugs.length > 0) {
      return schema.fields.filter((f) => cardFieldSlugs.includes(f.slug));
    }
    return schema.fields.filter((f) => !f.hidden && f !== titleField).slice(0, 3);
  }, [schema, titleField, cardFieldSlugs]);

  const sorted = useMemo(() => {
    return [...entities].sort((a, b) => {
      const ta = Date.parse(a.updatedAt ?? a.createdAt ?? "");
      const tb = Date.parse(b.updatedAt ?? b.createdAt ?? "");
      return tb - ta;
    });
  }, [entities]);

  if (sorted.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        No items yet
      </div>
    );
  }

  const visible = sorted.slice(0, PAGE_SIZE);

  return (
    <div className="mx-auto w-full max-w-3xl px-4 py-4">
      <ul className="space-y-2">
        {visible.map((entity) => (
          <li key={entity.id}>
            <button
              type="button"
              onClick={() => onEntityClick?.(entity)}
              className="w-full rounded-lg border border-border bg-surface-base p-3 text-left transition-colors hover:bg-surface-hover"
            >
              <div className="mb-1 flex items-baseline justify-between gap-3">
                <span className="truncate text-[14px] font-semibold text-foreground">
                  {getEntityTitle(schema, entity.fields)}
                </span>
                <span className="shrink-0 text-[11px] text-foreground/55">
                  {formatRelativeTime(entity.updatedAt ?? entity.createdAt ?? "")}
                </span>
              </div>
              <div className="flex flex-wrap gap-3 text-[12px] text-foreground/70">
                {inlineFields.map((field) => (
                  <span key={field.id} className="flex items-center gap-1">
                    <span className="text-foreground/45">{field.name}:</span>
                    <FieldRenderer field={field} value={entity.fields[field.slug]} />
                  </span>
                ))}
              </div>
            </button>
          </li>
        ))}
      </ul>
      {sorted.length > PAGE_SIZE && (
        <p className="mt-4 text-center text-[12px] text-foreground/45">
          Showing {PAGE_SIZE} of {sorted.length} — refine filters to narrow down.
        </p>
      )}
    </div>
  );
}
