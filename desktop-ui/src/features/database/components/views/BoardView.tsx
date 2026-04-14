import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle } from "@features/database/lib/schema-utils";
import { tagColor } from "@shared/lib/tagColor";
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

interface BoardViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  groupByField: string;
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
  onEntityMove?: (entityId: string, newValue: string) => void;
}

const EMPTY_GROUP = "\u2014";

function isEmptyValue(v: unknown): boolean {
  return v == null || v === "" || (Array.isArray(v) && v.length === 0);
}

export function BoardView({
  schema,
  entities,
  groupByField,
  cardFields,
  onEntityClick,
}: BoardViewProps) {
  const groupField = schema.fields.find((f) => f.slug === groupByField);
  if (!groupField || groupField.fieldType !== "select") {
    return (
      <div className="p-6 text-[13px] text-foreground/55">
        Select a "select" field to group this board by.
      </div>
    );
  }

  const options: string[] = Array.isArray(groupField.options) ? groupField.options : [];
  const columns = [...options, EMPTY_GROUP];

  const displayFields: FieldDefinition[] =
    cardFields && cardFields.length > 0
      ? schema.fields.filter((f) => cardFields.includes(f.slug) && f.slug !== groupByField)
      : schema.fields.filter((f) => !f.hidden && f.slug !== groupByField).slice(0, 3);

  const groupedEntities = new Map<string, Entity[]>();
  for (const col of columns) groupedEntities.set(col, []);
  for (const entity of entities) {
    const val = String(entity.fields[groupByField] ?? EMPTY_GROUP);
    const bucket = groupedEntities.get(val) ?? groupedEntities.get(EMPTY_GROUP)!;
    bucket.push(entity);
  }

  return (
    <div className="flex h-full gap-4 overflow-x-auto px-4 pt-3 pb-6">
      {columns.map((col) => {
        const items = groupedEntities.get(col) ?? [];
        const isEmptyCol = col === EMPTY_GROUP;
        const dotColor = isEmptyCol ? "var(--color-dim)" : tagColor(col);
        const label = isEmptyCol ? "No status" : col;

        return (
          <div key={col} className="flex w-[280px] shrink-0 flex-col">
            <div className="mb-2 flex items-center gap-2 px-1.5">
              <span
                className="size-2 shrink-0 rounded-full"
                style={{ backgroundColor: dotColor }}
                aria-hidden="true"
              />
              <h3 className="text-[12px] font-semibold uppercase tracking-wide text-foreground/80">
                {label}
              </h3>
              <span className="text-[11px] font-medium text-foreground/45 tabular-nums">
                {items.length}
              </span>
            </div>
            <div className="flex flex-1 flex-col gap-2">
              {items.map((entity) => {
                const title = getEntityTitle(schema, entity.fields);
                const meta = displayFields
                  .map((field) => ({ field, value: entity.fields[field.slug] }))
                  .filter(({ value }) => !isEmptyValue(value));

                return (
                  <button
                    key={entity.id}
                    type="button"
                    onClick={() => onEntityClick?.(entity)}
                    className="group w-full cursor-pointer rounded-lg border border-border bg-surface-lowest p-3 text-left transition-all duration-150 hover:border-foreground/20 hover:-translate-y-0.5 hover:shadow-sm"
                  >
                    <p className="text-[13px] font-medium text-foreground leading-snug line-clamp-3">
                      {title}
                    </p>
                    {meta.length > 0 && (
                      <div className="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-foreground/70">
                        {meta.map(({ field, value }) => (
                          <span key={field.id} className="inline-flex min-w-0 max-w-full">
                            <FieldRenderer field={field} value={value} />
                          </span>
                        ))}
                      </div>
                    )}
                  </button>
                );
              })}
              {items.length === 0 && (
                <div className="flex items-center justify-center py-6 text-[11px] text-foreground/35">
                  Drop here
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
