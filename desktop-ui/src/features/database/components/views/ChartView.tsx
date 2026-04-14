import { chartData } from "@features/database/lib/chartData";
import { getChartConfig } from "@features/database/lib/view-defaults";
import type { DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import { useMemo } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface Props {
  schema: DatabaseSchema;
  view: ViewDefinition;
  entities: Entity[];
}

const PIE_COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
];

export function ChartView({ schema, view, entities }: Props) {
  const { config, result } = useMemo(() => {
    const cfg = getChartConfig(view.config);
    return { config: cfg, result: cfg ? chartData(entities, schema, cfg) : null };
  }, [entities, schema, view.config]);

  if (!config) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        Configure this chart in the view settings (chart type, x-axis, aggregation).
      </div>
    );
  }
  if (!result || result.series.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[13px] text-foreground/55">
        No data to chart.
      </div>
    );
  }

  return (
    <div className="h-full w-full p-4">
      <ResponsiveContainer width="100%" height="100%">
        {config.chartType === "pie" ? (
          <PieChart>
            <Pie data={result.series} dataKey="y" nameKey="x" outerRadius="80%" label>
              {result.series.map((p, i) => (
                <Cell key={p.x} fill={PIE_COLORS[i % PIE_COLORS.length]} />
              ))}
            </Pie>
            <Tooltip />
            <Legend />
          </PieChart>
        ) : config.chartType === "line" ? (
          <LineChart data={result.series}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" />
            <XAxis dataKey="x" stroke="var(--muted)" tick={{ fontSize: 11 }} />
            <YAxis stroke="var(--muted)" tick={{ fontSize: 11 }} allowDecimals={false} />
            <Tooltip />
            <Line type="monotone" dataKey="y" stroke="var(--brand)" strokeWidth={2} dot />
          </LineChart>
        ) : (
          <BarChart data={result.series}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" />
            <XAxis dataKey="x" stroke="var(--muted)" tick={{ fontSize: 11 }} />
            <YAxis stroke="var(--muted)" tick={{ fontSize: 11 }} allowDecimals={false} />
            <Tooltip />
            <Bar dataKey="y" fill="var(--brand)" />
          </BarChart>
        )}
      </ResponsiveContainer>
    </div>
  );
}
