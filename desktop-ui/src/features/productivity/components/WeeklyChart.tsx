import { formatDayLabel } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { useMemo } from "react";
import { Bar, BarChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { ChartTooltip, PRODUCTIVITY_LEGEND } from "../lib/constants";

interface WeeklyChartProps {
  summaries: ProductivitySummary[];
}

export function WeeklyChart({ summaries }: WeeklyChartProps) {
  const chartData = useMemo(
    () =>
      summaries.map((s) => ({
        day: formatDayLabel(s.date),
        productive: +(s.productiveSecs / 3600).toFixed(1),
        neutral: +(s.neutralSecs / 3600).toFixed(1),
        distracting: +(s.distractingSecs / 3600).toFixed(1),
      })),
    [summaries],
  );

  return (
    <div className="glass-card p-4 flex flex-col gap-3 col-span-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Weekly Overview</h2>
        <div className="flex items-center gap-3">
          {PRODUCTIVITY_LEGEND.map((item) => (
            <span
              key={item.label}
              className="flex items-center gap-1 text-[10px] font-light text-dim"
            >
              <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: item.color }} />
              {item.label}
            </span>
          ))}
        </div>
      </div>
      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} barCategoryGap="25%" barSize={24}>
            <XAxis
              dataKey="day"
              tick={{ fill: "var(--text-dim)", fontSize: 11, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
            />
            <YAxis
              tick={{ fill: "var(--text-dim)", fontSize: 10, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
              width={28}
              tickFormatter={(v) => `${v}h`}
            />
            <Tooltip
              content={<ChartTooltip />}
              cursor={{ fill: "var(--surface-raised)", radius: 4 }}
            />
            <Bar dataKey="productive" stackId="a" fill="var(--success)" radius={[0, 0, 0, 0]} />
            <Bar dataKey="neutral" stackId="a" fill="var(--text-muted)" />
            <Bar
              dataKey="distracting"
              stackId="a"
              fill="var(--destructive)"
              radius={[3, 3, 0, 0]}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
