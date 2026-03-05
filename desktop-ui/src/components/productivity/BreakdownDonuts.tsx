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

function MiniDonut({
  name,
  value,
  total,
  color,
}: {
  name: string;
  value: number;
  total: number;
  color: string;
}) {
  const pct = total > 0 ? Math.round((value / total) * 100) : 0;
  const data = useMemo(
    () => [{ value: value }, { value: Math.max(total - value, 0) }],
    [value, total],
  );

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative">
        <PieChart width={56} height={56}>
          <Pie
            data={data}
            cx={27}
            cy={27}
            innerRadius={18}
            outerRadius={25}
            startAngle={90}
            endAngle={-270}
            dataKey="value"
            stroke="none"
          >
            <Cell fill={color} />
            <Cell fill="var(--surface-raised)" />
          </Pie>
        </PieChart>
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-[11px] font-light text-primary tabular-nums">{pct}%</span>
        </div>
      </div>
      <span className="text-[11px] font-medium text-secondary">{name}</span>
      <span className="text-[10px] font-light text-dim">{formatLongDuration(value)}</span>
    </div>
  );
}

export function BreakdownDonuts({ segments, totalSecs }: BreakdownDonutsProps) {
  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Breakdown</h2>
      <div className="flex items-start justify-around">
        {segments.map((s) => (
          <MiniDonut key={s.name} name={s.name} value={s.value} total={totalSecs} color={s.color} />
        ))}
      </div>
    </div>
  );
}
