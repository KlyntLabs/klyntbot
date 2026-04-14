import type { ChartConfig, DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

export interface ChartPoint {
  x: string;
  y: number;
}

export interface ChartResult {
  series: ChartPoint[];
  xLabel: string;
  yLabel: string;
}

const TOP_N = 12;

function bucketKeys(entity: Entity, field: FieldDefinition): string[] {
  const v = entity.fields[field.slug];
  if (v === null || v === undefined || v === "") return [];
  if (Array.isArray(v)) return v.length === 0 ? [] : v.map(String);
  if (
    field.fieldType === "date" ||
    field.fieldType === "created_time" ||
    field.fieldType === "last_edited"
  ) {
    return [String(v).slice(0, 10)];
  }
  return [String(v)];
}

export function chartData(
  entities: Entity[],
  schema: DatabaseSchema,
  config: ChartConfig,
): ChartResult {
  const field = schema.fields.find((f) => f.slug === config.xAxis);
  const xLabel = field?.name ?? config.xAxis;
  const yLabel = config.aggregation === "count" ? "Count" : (config.yField ?? "Value");
  if (!field) return { series: [], xLabel, yLabel };

  const counts = new Map<string, number>();
  for (const e of entities) {
    for (const key of bucketKeys(e, field)) {
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }

  const sorted = [...counts.entries()].map(([x, y]) => ({ x, y })).sort((a, b) => b.y - a.y);

  if (sorted.length <= TOP_N) return { series: sorted, xLabel, yLabel };
  const head = sorted.slice(0, TOP_N);
  const otherY = sorted.slice(TOP_N).reduce((acc, p) => acc + p.y, 0);
  return { series: [...head, { x: "Other", y: otherY }], xLabel, yLabel };
}
