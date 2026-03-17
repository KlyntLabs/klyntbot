import { formatHumanDuration, formatLongDuration } from "@shared/lib/dates";

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
  const hasData = totalSecs > 0;

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-baseline justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Breakdown</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">
          {formatLongDuration(totalSecs)} total
        </span>
      </div>

      {/* Stacked horizontal bar */}
      <div className="h-3 rounded-full overflow-hidden bg-surface-base flex">
        {hasData ? (
          segments
            .filter((s) => s.value > 0)
            .map((s) => (
              <div
                key={s.name}
                className="h-full first:rounded-l-full last:rounded-r-full transition-all duration-500"
                style={{
                  width: `${(s.value / totalSecs) * 100}%`,
                  backgroundColor: s.color,
                }}
              />
            ))
        ) : (
          <div className="h-full w-full rounded-full bg-surface-low" />
        )}
      </div>

      {/* Legend rows */}
      <div className="flex flex-col gap-1.5">
        {segments.map((s) => {
          const pct = totalSecs > 0 ? Math.round((s.value / totalSecs) * 100) : 0;
          return (
            <div key={s.name} className="flex items-center gap-2">
              <span
                className="w-2 h-2 rounded-[3px] flex-shrink-0"
                style={{ backgroundColor: s.color }}
              />
              <span className="text-[11px] font-light text-secondary flex-1">{s.name}</span>
              <span className="text-[11px] font-medium text-primary tabular-nums">{pct}%</span>
              <span className="text-[10px] font-light text-dim tabular-nums w-12 text-right">
                {formatHumanDuration(s.value)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
