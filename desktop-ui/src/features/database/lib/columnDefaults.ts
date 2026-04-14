import type { FieldDefinition } from "@shared/types";

export const MIN_COLUMN_WIDTH = 60;
export const MAX_COLUMN_WIDTH = 800;

/** Default pixel widths per field type — used when the view has no explicit width. */
export function defaultColumnWidth(field: FieldDefinition, isTitle: boolean): number {
  if (isTitle) return 280;
  switch (field.fieldType) {
    case "text":
      return 180;
    case "select":
    case "multi_select":
      return 140;
    case "number":
    case "date":
    case "created_time":
    case "last_edited":
      return 140;
    case "checkbox":
      return MIN_COLUMN_WIDTH;
    default:
      return 140;
  }
}
