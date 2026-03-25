import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

interface RetentionChartProps {
  data: { date: string; avgRetention: number; reviewCount: number }[];
  height?: number;
}

export function RetentionChart({ data, height = 200 }: RetentionChartProps) {
  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-[200px] text-[11px] text-muted-foreground">
        No retention data yet. Start reviewing flashcards to see trends.
      </div>
    );
  }

  const formatted = data.map((d) => ({
    ...d,
    retention: Math.round(d.avgRetention * 100),
    label: new Date(`${d.date}T00:00:00`).toLocaleDateString(undefined, {
      month: "2-digit",
      day: "2-digit",
    }),
  }));

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={formatted}>
        <XAxis dataKey="label" tick={{ fontSize: 10 }} stroke="var(--muted)" />
        <YAxis domain={[0, 100]} tick={{ fontSize: 10 }} stroke="var(--muted)" />
        <Tooltip
          contentStyle={{
            background: "var(--surface-raised)",
            border: "1px solid var(--border)",
            borderRadius: 8,
            fontSize: 11,
          }}
          formatter={(value: number) => [`${value}%`, "Retention"]}
        />
        <Area
          type="monotone"
          dataKey="retention"
          stroke="var(--brand)"
          fill="var(--brand)"
          fillOpacity={0.1}
          strokeWidth={2}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
