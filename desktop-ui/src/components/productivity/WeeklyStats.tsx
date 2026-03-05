import { formatHumanDuration } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";

interface WeeklyStatsProps {
  summaries: ProductivitySummary[];
}

export function WeeklyStats({ summaries }: WeeklyStatsProps) {
  const days = summaries.length || 1;
  const totalActive = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
  const totalProductive = summaries.reduce((s, d) => s + d.productiveSecs, 0);
  const totalFocusSessions = summaries.reduce((s, d) => s + d.focusSessionsCount, 0);
  const scores = summaries.map((s) => s.productivityScore).filter((s): s is number => s != null);
  const avgScore =
    scores.length > 0 ? Math.round(scores.reduce((a, b) => a + b, 0) / scores.length) : 0;
  const qualities = summaries.map((s) => s.avgSessionQuality).filter((q): q is number => q != null);
  const avgQuality =
    qualities.length > 0
      ? Math.round((qualities.reduce((a, b) => a + b, 0) / qualities.length) * 100)
      : 0;

  const stats = [
    { label: "Avg Score", value: `${avgScore}/100` },
    { label: "Total Active", value: formatHumanDuration(totalActive) },
    { label: "Avg Daily", value: formatHumanDuration(Math.round(totalActive / days)) },
    { label: "Productive", value: formatHumanDuration(totalProductive) },
    { label: "Focus Sessions", value: `${totalFocusSessions}` },
    { label: "Avg Quality", value: `${avgQuality}%` },
  ];

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Stats</h2>
      <div className="grid grid-cols-2 gap-3">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col gap-0.5">
            <span className="text-[10px] font-light text-dim">{s.label}</span>
            <span className="text-[16px] font-light text-primary tabular-nums">{s.value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
