import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getEntityTitle } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";

interface BoardViewProps {
  schema: DatabaseSchema;
  entities: Entity[];
  groupByField: string;
  cardFields?: string[];
  onEntityClick?: (entity: Entity) => void;
  onEntityMove?: (entityId: string, newValue: string) => void;
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
    return <div className="p-4 text-muted">Select a "select" field to group by.</div>;
  }

  const options: string[] = Array.isArray(groupField.options) ? groupField.options : [];
  const columns = [...options, "\u2014"]; // em dash for entities without a value

  const displayFields =
    cardFields && cardFields.length > 0
      ? schema.fields.filter((f) => cardFields.includes(f.slug) && f.slug !== groupByField)
      : schema.fields.filter((f) => !f.hidden && f.slug !== groupByField).slice(0, 3);

  const groupedEntities = new Map<string, Entity[]>();
  for (const col of columns) {
    groupedEntities.set(col, []);
  }
  for (const entity of entities) {
    const val = String(entity.fields[groupByField] ?? "\u2014");
    const bucket = groupedEntities.get(val) ?? groupedEntities.get("\u2014")!;
    bucket.push(entity);
  }

  return (
    <div className="flex gap-3 overflow-x-auto px-10 pt-3 pb-6">
      {columns.map((col) => {
        const items = groupedEntities.get(col) ?? [];
        return (
          <div key={col} className="flex w-72 shrink-0 flex-col">
            <div className="mb-2 flex items-center gap-2 px-1">
              <h3 className="text-[13px] font-semibold text-foreground">{col}</h3>
              <span className="text-[12px] text-foreground/55 tabular-nums">{items.length}</span>
            </div>
            <div className="flex flex-1 flex-col gap-2 rounded-lg bg-accent/20 p-2">
              {items.map((entity) => {
                const title = getEntityTitle(schema, entity.fields);
                return (
                  <button
                    key={entity.id}
                    type="button"
                    onClick={() => onEntityClick?.(entity)}
                    className="w-full cursor-pointer rounded-md border border-border bg-background p-3 text-left shadow-sm transition-all hover:border-border/80 hover:shadow-md"
                  >
                    <p className="mb-1.5 text-[13px] font-medium text-foreground leading-snug">
                      {title}
                    </p>
                    {displayFields.map((field) => (
                      <div key={field.id} className="mt-1.5 flex items-center gap-1.5 text-[12px]">
                        <span className="shrink-0 text-foreground/55">{field.name}</span>
                        <span className="min-w-0 text-foreground/85">
                          <FieldRenderer field={field} value={entity.fields[field.slug]} />
                        </span>
                      </div>
                    ))}
                  </button>
                );
              })}
              {items.length === 0 && (
                <div className="rounded-md border border-dashed border-border/60 p-4 text-center text-[12px] text-foreground/50">
                  No items
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
