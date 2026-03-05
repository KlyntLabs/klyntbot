import { useMemo } from "react";
import { Cell, Pie, PieChart } from "recharts";
import { formatLongDuration } from "../../lib/dates";

interface BreakdownSegment {
  name: string;
  value: number;
  color: string;
}

interface BreakdownDonutsProps {
  segments: BreakdownSegment[];
  totalSecs: number;
}

export function BreakdownDonuts({ segments, totalSecs }: BreakdownDonutsProps) {
  // Single unified donut with all segments
  const pieData = useMemo(() => {
    const filled = segments.filter((s) => s.value > 0);
    if (filled.length === 0) return [{ name: "Empty", value: 1, color: "var(--surface-raised)" }];
    return filled;
  }, [segments]);

  const hasData = pieData[0]?.name !== "Empty";

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Breakdown</h2>

      <div className="flex items-center gap-4">
        {/* Central donut */}
        <div className="relative flex-shrink-0">
          <PieChart width={80} height={80}>
            <Pie
              data={pieData}
              cx={39}
              cy={39}
              innerRadius={26}
              outerRadius={36}
              startAngle={90}
              endAngle={-270}
              dataKey="value"
              stroke="none"
              paddingAngle={hasData ? 3 : 0}
            >
              {pieData.map((entry) => (
                <Cell key={entry.name} fill={entry.color} />
              ))}
            </Pie>
          </PieChart>
          {/* Center label */}
          <div className="absolute inset-0 flex items-center justify-center">
            <span className="text-[11px] font-light text-dim tabular-nums">
              {formatLongDuration(totalSecs)}
            </span>
          </div>
        </div>

        {/* Segment details */}
        <div className="flex-1 flex flex-col gap-2">
          {segments.map((s) => {
            const pct = totalSecs > 0 ? Math.round((s.value / totalSecs) * 100) : 0;
            return (
              <div key={s.name} className="flex items-center gap-2">
                <span
                  className="w-2 h-2 rounded-full flex-shrink-0"
                  style={{ backgroundColor: s.color }}
                />
                <span className="text-[11px] font-light text-secondary flex-1">{s.name}</span>
                <span className="text-[11px] font-medium text-primary tabular-nums">{pct}%</span>
                <span className="text-[10px] font-light text-dim tabular-nums w-14 text-right">
                  {formatLongDuration(s.value)}
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
