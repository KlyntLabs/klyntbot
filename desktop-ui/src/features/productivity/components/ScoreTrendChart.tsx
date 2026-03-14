import { formatDate } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types/productivity";
import { useMemo } from "react";
import { Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

interface Props {
  summaries: ProductivitySummary[];
}

export function ScoreTrendChart({ summaries }: Props) {
  const data = useMemo(
    () =>
      summaries
        .filter((s) => s.productivityScore != null)
        .map((s) => ({
          date: formatDate(s.date),
          score: Math.round(s.productivityScore ?? 0),
          baseline:
            s.scoreTrend != null && s.productivityScore != null
              ? Math.round(s.productivityScore - s.scoreTrend)
              : null,
        })),
    [summaries],
  );

  if (data.length < 2) return null;

  return (
    <div className="mt-2">
      <div className="text-xs font-medium text-muted mb-1 px-1">Score Trend</div>
      <ResponsiveContainer width="100%" height={120}>
        <LineChart data={data} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
          <XAxis dataKey="date" tick={{ fontSize: 10 }} />
          <YAxis domain={[0, 100]} tick={{ fontSize: 10 }} />
          <Tooltip
            contentStyle={{ fontSize: 11 }}
            formatter={(value: number, name: string) => [
              value,
              name === "score" ? "Score" : "Baseline",
            ]}
          />
          <Line
            type="monotone"
            dataKey="score"
            stroke="var(--accent)"
            strokeWidth={2}
            dot={{ r: 3 }}
          />
          <Line
            type="monotone"
            dataKey="baseline"
            stroke="var(--text-muted)"
            strokeWidth={1}
            strokeDasharray="4 4"
            dot={false}
            connectNulls
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
