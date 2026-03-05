import { Bar, BarChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { ProductivitySummary } from "../../lib/types";

interface MonthlyChartProps {
  summaries: ProductivitySummary[];
}

export function MonthlyChart({ summaries }: MonthlyChartProps) {
  const chartData = summaries.map((s) => ({
    day: parseInt(s.date.slice(8), 10),
    productive: +(s.productiveSecs / 3600).toFixed(1),
    neutral: +(s.neutralSecs / 3600).toFixed(1),
    distracting: +(s.distractingSecs / 3600).toFixed(1),
  }));

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3 col-span-3">
      <h2 className="text-[13px] font-medium text-secondary">Monthly Overview</h2>
      <div className="h-48">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} barCategoryGap="10%">
            <XAxis
              dataKey="day"
              tick={{ fill: "var(--text-dim)", fontSize: 9, fontWeight: 300 }}
              axisLine={false}
              tickLine={false}
              interval={1}
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
              labelFormatter={(day) => `Day ${day}`}
            />
            <Bar dataKey="productive" stackId="a" fill="var(--success)" />
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
