import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import type { GoalProgress } from "../../lib/types";

function metricLabel(metric: string): string {
  switch (metric) {
    case "productive_hours":
      return "productive hours";
    case "focus_sessions":
      return "focus sessions";
    case "productivity_score":
      return "score";
    case "max_distracting_mins":
      return "distracting mins";
    default:
      return metric;
  }
}

function formatValue(metric: string, value: number): string {
  if (metric === "productive_hours") return `${value.toFixed(1)}h`;
  if (metric === "max_distracting_mins") return `${Math.round(value)}m`;
  return `${Math.round(value)}`;
}

export function GoalsProgress() {
  const { data: goals, refetch } = useQuery<GoalProgress[]>("productivity_goals", undefined, []);

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

  if (goals.length === 0) {
    return (
      <div className="bg-surface-base rounded-xl p-4">
        <h2 className="text-[13px] font-medium text-secondary mb-3">Goals</h2>
        <p className="text-[12px] font-light text-dim">No goals set</p>
      </div>
    );
  }

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary">Goals</h2>
      <div className="flex flex-col gap-2">
        {goals.map((g) => {
          const pct = g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
          return (
            <div key={g.id} className="flex flex-col gap-1">
              <div className="flex items-center justify-between text-[11px] font-light">
                <div className="flex items-center gap-2">
                  <span className={g.met ? "text-success" : "text-brand"}>
                    {g.met ? "MET" : "IN PROGRESS"}
                  </span>
                  <span className="text-primary">
                    {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                  </span>
                  <span className="text-dim">({g.goalType})</span>
                </div>
                <span className="text-muted tabular-nums">
                  {formatValue(g.metric, g.currentValue)} / {formatValue(g.metric, g.targetValue)}
                </span>
              </div>
              <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${g.met ? "bg-success" : "bg-brand"}`}
                  style={{ width: `${pct}%` }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
