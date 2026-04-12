import type { DatabaseSchema, FieldDefinition } from "@shared/types";

/** Event name for database mutation cache invalidation. */
export const DATABASE_UPDATED_EVENT = "database:updated";

/** Emit database:updated event for cache invalidation. */
export function emitDatabaseUpdated() {
  window.dispatchEvent(new CustomEvent(DATABASE_UPDATED_EVENT));
}

/** Find the best "title" field from a schema — checks for common slugs, then required text fields. */
export function getTitleField(schema: DatabaseSchema): FieldDefinition | undefined {
  return (
    schema.fields.find((f) => f.slug === "title" || f.slug === "name") ??
    schema.fields.find((f) => f.required && f.fieldType === "text") ??
    schema.fields[0]
  );
}

/** Get the display title for an entity using the schema's title field. */
export function getEntityTitle(schema: DatabaseSchema, fields: Record<string, unknown>): string {
  const titleField = getTitleField(schema);
  if (!titleField) return "Untitled";
  const val = fields[titleField.slug];
  return val != null ? String(val) : "Untitled";
}
