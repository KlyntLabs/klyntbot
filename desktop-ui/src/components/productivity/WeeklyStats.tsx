import { useMemo } from "react";
import { formatHumanDuration } from "../../lib/dates";
import type { ProductivitySummary } from "../../lib/types";
import { scoreColor } from "./shared";

interface WeeklyStatsProps {
  summaries: ProductivitySummary[];
}

interface StatItem {
  label: string;
  value: string;
  color?: string;
  sub?: string;
}

export function WeeklyStats({ summaries }: WeeklyStatsProps) {
  const stats: StatItem[] = useMemo(() => {
    const days = summaries.length || 1;
    const totalActive = summaries.reduce((s, d) => s + d.totalActiveSecs, 0);
    const totalProductive = summaries.reduce((s, d) => s + d.productiveSecs, 0);
    const totalDistracting = summaries.reduce((s, d) => s + d.distractingSecs, 0);
    const totalFocusSessions = summaries.reduce((s, d) => s + d.focusSessionsCount, 0);
    const scores = summaries.map((s) => s.productivityScore).filter((s): s is number => s != null);
    const avgScore =
      scores.length > 0 ? Math.round(scores.reduce((a, b) => a + b, 0) / scores.length) : 0;
    const productiveRatio = totalActive > 0 ? Math.round((totalProductive / totalActive) * 100) : 0;

    return [
      {
        label: "Avg Score",
        value: `${avgScore}`,
        color: scoreColor(avgScore),
        sub: "/100",
      },
      { label: "Total Active", value: formatHumanDuration(totalActive) },
      { label: "Avg Daily", value: formatHumanDuration(Math.round(totalActive / days)) },
      {
        label: "Productive",
        value: `${productiveRatio}%`,
        color: "var(--success)",
        sub: formatHumanDuration(totalProductive),
      },
      { label: "Focus Sessions", value: `${totalFocusSessions}` },
      {
        label: "Distracting",
        value: formatHumanDuration(totalDistracting),
        color: totalDistracting > 0 ? "var(--destructive)" : undefined,
      },
    ];
  }, [summaries]);

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Weekly Stats</h2>
      <div className="grid grid-cols-2 gap-x-4 gap-y-3">
        {stats.map((s) => (
          <div key={s.label} className="flex flex-col gap-0.5">
            <span className="text-[10px] font-light text-dim uppercase tracking-wider">
              {s.label}
            </span>
            <div className="flex items-baseline gap-1">
              <span
                className="text-[17px] font-light tabular-nums leading-tight"
                style={{ color: s.color ?? "var(--text-primary)" }}
              >
                {s.value}
              </span>
              {s.sub && <span className="text-[10px] font-light text-dim">{s.sub}</span>}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
