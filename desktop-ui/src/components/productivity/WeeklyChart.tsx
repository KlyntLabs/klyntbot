import { Bar, BarChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { formatDayLabel } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";

interface WeeklyChartProps {
  summaries: ProductivitySummary[];
}

export function WeeklyChart({ summaries }: WeeklyChartProps) {
  const chartData = summaries.map((s) => ({
    day: formatDayLabel(s.date),
    productive: +(s.productiveSecs / 3600).toFixed(1),
    neutral: +(s.neutralSecs / 3600).toFixed(1),
    distracting: +(s.distractingSecs / 3600).toFixed(1),
  }));

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Overview</h2>
      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} barCategoryGap="20%">
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
              width={30}
              label={{
                value: "Hours",
                angle: -90,
                position: "insideLeft",
                style: { fill: "var(--text-dim)", fontSize: 10, fontWeight: 300 },
              }}
            />
            <Tooltip
              contentStyle={{
                background: "var(--surface-floating)",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius)",
                fontSize: 11,
                fontWeight: 300,
              }}
              labelStyle={{ color: "var(--text-primary)" }}
            />
            <Bar dataKey="productive" stackId="a" fill="var(--success)" radius={[0, 0, 0, 0]} />
            <Bar dataKey="neutral" stackId="a" fill="var(--text-muted)" />
            <Bar
              dataKey="distracting"
              stackId="a"
              fill="var(--destructive)"
              radius={[2, 2, 0, 0]}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
