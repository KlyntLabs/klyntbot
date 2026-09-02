import { retentionTextColor } from "@shared/lib/retention";
import { BarChart3, CheckCircle, Flame, Library } from "lucide-react";
import { Area, AreaChart, ResponsiveContainer } from "recharts";
import type { WeeklyStatPoint } from "../hooks/useReviewStats";

interface StatsBarProps {
  totalDue: number;
  streak: number;
  retention: number;
  weekly: WeeklyStatPoint[];
}

function StatCard({
  icon,
  label,
  value,
  valueClass,
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  valueClass?: string;
}) {
  return (
    <div className="glass-card flex items-center gap-2.5 px-3 py-2.5 flex-1 min-w-0">
      <div className="text-fg-secondary shrink-0">{icon}</div>
      <div className="min-w-0">
        <p className="text-ui-xs text-fg-secondary leading-none mb-0.5">{label}</p>
        <p
          className={`text-sm font-semibold tabular-nums leading-none ${valueClass ?? "text-fg"}`}
        >
          {value}
        </p>
      </div>
    </div>
  );
}

export function StatsBar({ totalDue, streak, retention, weekly }: StatsBarProps) {
  const retPct = Math.round(retention * 100);

  return (
    <div className="flex gap-2">
      <StatCard
        icon={<Flame size={16} strokeWidth={1.5} />}
        label="Streak"
        value={streak > 0 ? `${streak}d` : "--"}
      />
      <StatCard icon={<Library size={16} strokeWidth={1.5} />} label="Due" value={totalDue} />
      <StatCard
        icon={<CheckCircle size={16} strokeWidth={1.5} />}
        label="Retention"
        value={retPct > 0 && retPct < 100 ? `${retPct}%` : "--"}
        valueClass={retention < 1.0 ? retentionTextColor(retention) : undefined}
      />
      <div className="glass-card flex items-center gap-2.5 px-3 py-2.5 flex-1 min-w-0">
        <div className="text-fg-secondary shrink-0">
          <BarChart3 size={16} strokeWidth={1.5} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-ui-xs text-fg-secondary leading-none mb-1">This week</p>
          {weekly.length > 0 ? (
            <ResponsiveContainer width="100%" height={24}>
              <AreaChart data={weekly}>
                <Area
                  type="monotone"
                  dataKey="reviews"
                  stroke="var(--ds-accent)"
                  fill="var(--ds-accent)"
                  fillOpacity={0.15}
                  strokeWidth={1.5}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-sm font-semibold text-fg tabular-nums leading-none">--</p>
          )}
        </div>
      </div>
    </div>
  );
}
