import type {
  ChartConfig,
  DatabaseSchema,
  FieldDefinition,
  ViewConfig,
  ViewDefinition,
  ViewType,
} from "@shared/types";

export function getChartConfig(config: ViewConfig): ChartConfig | undefined {
  return (config.layout as { chart?: ChartConfig } | undefined)?.chart;
}

export function withChartConfig(config: ViewConfig, chart: ChartConfig): Partial<ViewConfig> {
  return { layout: { ...(config.layout ?? {}), chart } };
}

function firstFieldOfType(schema: DatabaseSchema, types: FieldDefinition["fieldType"][]) {
  return schema.fields.find((f) => !f.hidden && types.includes(f.fieldType));
}

export function defaultViewConfig(schema: DatabaseSchema, viewType: ViewType): ViewConfig {
  const visible = schema.fields.filter((f) => !f.hidden).map((f) => f.slug);
  const base: ViewConfig = { visibleFields: visible };

  switch (viewType) {
    case "board": {
      const groupField = firstFieldOfType(schema, ["select"]);
      return { ...base, groupBy: groupField?.slug, cardFields: visible.slice(0, 3) };
    }
    case "calendar": {
      const dateField = firstFieldOfType(schema, ["date"]);
      return { ...base, calendarField: dateField?.slug };
    }
    case "timeline": {
      const startField = firstFieldOfType(schema, ["date"]);
      return { ...base, calendarField: startField?.slug };
    }
    case "gallery":
      return { ...base, cardFields: visible.slice(0, 3) };
    case "list":
      return { ...base, cardFields: visible.slice(0, 3) };
    case "chart": {
      const xField =
        firstFieldOfType(schema, ["select", "multi_select"]) ??
        schema.fields.find((f) => !f.hidden);
      return {
        ...base,
        layout: {
          chart: {
            chartType: "bar",
            xAxis: xField?.slug ?? "",
            aggregation: "count",
          },
        },
      };
    }
    case "feed":
      return { ...base, cardFields: visible.slice(0, 3) };
    default:
      return base;
  }
}

export function defaultViewName(views: ViewDefinition[], viewType: ViewType): string {
  const base = viewType.charAt(0).toUpperCase() + viewType.slice(1);
  const sameTypeCount = views.filter((v) => v.viewType === viewType).length;
  return sameTypeCount === 0 ? base : `${base} ${sameTypeCount + 1}`;
}
