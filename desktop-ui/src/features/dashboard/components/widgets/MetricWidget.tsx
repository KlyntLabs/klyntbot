import { useEntities } from "@features/database/hooks/useEntities";

interface MetricWidgetProps {
  databaseId: string;
  title: string;
  valueField: string;
  aggregation: "sum" | "avg" | "min" | "max" | "count";
}

export function MetricWidget({ databaseId, title, valueField, aggregation }: MetricWidgetProps) {
  const { data, loading } = useEntities(databaseId, { limit: 1000 });

  const compute = (): string => {
    const entities = data?.entities ?? [];
    const values = entities
      .map((e) => Number(e.fields[valueField]))
      .filter((v) => !Number.isNaN(v));

    if (values.length === 0) return "\u2014";

    switch (aggregation) {
      case "count":
        return values.length.toLocaleString();
      case "sum":
        return values.reduce((a, b) => a + b, 0).toLocaleString();
      case "avg":
        return (values.reduce((a, b) => a + b, 0) / values.length).toLocaleString(undefined, {
          maximumFractionDigits: 1,
        });
      case "min":
        return Math.min(...values).toLocaleString();
      case "max":
        return Math.max(...values).toLocaleString();
    }
  };

  return (
    <div className="flex flex-col items-center justify-center h-full p-4">
      <span className="text-3xl font-bold tabular-nums">{loading ? "..." : compute()}</span>
      <span className="text-sm text-muted mt-1">{title}</span>
      <span className="text-xs text-muted">{aggregation}</span>
    </div>
  );
}
