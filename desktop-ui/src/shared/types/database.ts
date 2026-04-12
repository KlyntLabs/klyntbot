// ── Field Types ────────────────────────────────────────────

export type FieldType =
  | "text"
  | "number"
  | "select"
  | "multi_select"
  | "date"
  | "checkbox"
  | "url"
  | "email"
  | "phone"
  | "relation"
  | "rollup"
  | "formula"
  | "created_time"
  | "last_edited"
  | "files"
  | "person";

export interface FieldDefinition {
  id: string;
  databaseId: string;
  name: string;
  slug: string;
  fieldType: FieldType;
  options?: unknown;
  position: number;
  required: boolean;
  hidden: boolean;
  aiManaged: boolean;
  aiConfig?: unknown;
  defaultValue?: string;
  createdAt: string;
}

// ── View Types ─────────────────────────────────────────────

export type ViewType = "table" | "board" | "calendar" | "list" | "gallery" | "timeline";

export type FilterOp =
  | "eq"
  | "neq"
  | "gt"
  | "gte"
  | "lt"
  | "lte"
  | "contains"
  | "not_contains"
  | "is_empty"
  | "is_not_empty"
  | "in"
  | "not_in";

export interface FilterRule {
  field: string;
  op: FilterOp;
  value: unknown;
}

export type SortDirection = "asc" | "desc";

export interface SortRule {
  field: string;
  direction: SortDirection;
}

export interface ViewConfig {
  filters?: FilterRule[];
  sorts?: SortRule[];
  visibleFields?: string[];
  groupBy?: string;
  calendarField?: string;
  galleryField?: string;
  cardFields?: string[];
  layout?: Record<string, unknown>;
}

export interface ViewDefinition {
  id: string;
  databaseId: string;
  name: string;
  viewType: ViewType;
  config: ViewConfig;
  position: number;
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
}

// ── Database Schema ────────────────────────────────────────

export interface DatabaseSchema {
  id: string;
  name: string;
  slug: string;
  icon?: string;
  description?: string;
  templateId?: string;
  skillId?: string;
  fields: FieldDefinition[];
  views: ViewDefinition[];
  createdAt: string;
  updatedAt: string;
}

// ── Entity ─────────────────────────────────────────────────

export interface Entity {
  id: string;
  databaseId: string;
  fields: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

// ── Relations ──────────────────────────────────────────────

export interface EntityRelation {
  id: string;
  sourceId: string;
  sourceDbId: string;
  targetId: string;
  targetDbId: string;
  relationType: string;
  inferred: boolean;
  confidence?: number;
  createdAt: string;
}

// ── Dashboard ──────────────────────────────────────────────

export type WidgetType =
  | "count"
  | "list"
  | "chart_bar"
  | "chart_pie"
  | "chart_line"
  | "progress"
  | "heatmap"
  | "table"
  | "metric"
  | "calendar"
  | "custom";

export interface GridPosition {
  row: number;
  col: number;
  width: number;
  height: number;
}

export interface WidgetDefinition {
  id: string;
  widgetType: WidgetType;
  databaseId: string;
  config: Record<string, unknown>;
  position: GridPosition;
}

export interface Dashboard {
  id: string;
  name: string;
  widgets: WidgetDefinition[];
  position: number;
  createdAt: string;
  updatedAt: string;
}

// ── Schema Evolution ───────────────────────────────────────

export interface SchemaEvolution {
  id: string;
  databaseId: string;
  actionType: string;
  actionJson: string;
  confidence: number;
  reasoning: string;
  status: "proposed" | "accepted" | "dismissed";
  source: string;
  createdAt: string;
  resolvedAt?: string;
}

// ── Query Types ────────────────────────────────────────────

export interface QueryParams {
  filters?: FilterRule[];
  sorts?: SortRule[];
  limit?: number;
  offset?: number;
}

export interface QueryResult {
  entities: Entity[];
  total: number;
}

// ── Input Types ────────────────────────────────────────────

export interface CreateDatabaseInput {
  name: string;
  slug?: string;
  icon?: string;
  description?: string;
  templateId?: string;
}

export interface CreateFieldInput {
  name: string;
  slug?: string;
  fieldType: FieldType;
  options?: unknown;
  required?: boolean;
  defaultValue?: string;
  position?: number;
}

export interface CreateEntityInput {
  fields: Record<string, unknown>;
}

export interface UpdateEntityInput {
  fields: Record<string, unknown>;
}
