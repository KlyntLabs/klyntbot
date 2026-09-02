import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { GoalProgress } from "@shared/types";
import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { AddGoalDialog } from "./AddGoalDialog";

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
    case "project_hours":
      return "project hours";
    default:
      return metric;
  }
}

function formatValue(metric: string, value: number): string {
  if (metric === "productive_hours" || metric === "project_hours") return `${value.toFixed(1)}h`;
  if (metric === "max_distracting_mins") return `${Math.round(value)}m`;
  return `${Math.round(value)}`;
}

export function GoalsProgress() {
  const { data: goals, refetch } = useQuery<GoalProgress[]>("productivity_goals", undefined, [], {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "productivity",
  });
  const [showAdd, setShowAdd] = useState(false);
  const { mutate: createGoal } = useMutation<
    void,
    { goal_type: string; metric: string; target_value: number }
  >("productivity_goal_create");
  const { mutate: deleteGoal } = useMutation<void, { id: number }>("productivity_goal_delete");

  const handleAdd = async (params: { goal_type: string; metric: string; target_value: number }) => {
    await createGoal(params);
    refetch();
  };

  const handleDelete = async (id: number) => {
    await deleteGoal({ id });
    refetch();
  };

  return (
    <>
      <div className="glass-card p-4 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h2 className="text-ui font-medium text-fg-secondary">Goals</h2>
          <button
            type="button"
            onClick={() => setShowAdd(true)}
            className="size-6 rounded-md flex items-center justify-center text-fg-secondary hover:text-brand hover:bg-control-hover transition-colors"
          >
            <Plus className="size-3.5" />
          </button>
        </div>

        {goals.length === 0 ? (
          <p className="text-ui-sm font-light text-fg-dim">No goals set</p>
        ) : (
          <div className="flex flex-col gap-2">
            {goals.map((g) => {
              const pct =
                g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
              return (
                <div key={g.id} className="group flex flex-col gap-1">
                  <div className="flex items-center justify-between text-ui-xs font-light">
                    <div className="flex items-center gap-2">
                      <span className={g.met ? "text-status-success" : "text-brand"}>
                        {g.met ? "MET" : "IN PROGRESS"}
                      </span>
                      <span className="text-fg">
                        {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                      </span>
                      {g.projectId && (
                        <span className="text-[9px] px-1.5 py-0.5 rounded bg-control-hover text-fg-secondary">
                          {g.projectId}
                        </span>
                      )}
                      <span className="text-fg-dim">({g.goalType})</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-fg-secondary tabular-nums">
                        {formatValue(g.metric, g.currentValue)} /{" "}
                        {formatValue(g.metric, g.targetValue)}
                      </span>
                      <button
                        type="button"
                        onClick={() => handleDelete(g.id)}
                        className="size-5 rounded flex items-center justify-center text-transparent group-hover:text-fg-secondary hover:!text-status-danger transition-colors"
                      >
                        <Trash2 className="size-3" />
                      </button>
                    </div>
                  </div>
                  <div className="h-1.5 rounded-full bg-control-hover overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-[width] ${g.met ? "bg-status-success" : "bg-brand"}`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <AddGoalDialog open={showAdd} onClose={() => setShowAdd(false)} onAdd={handleAdd} />
    </>
  );
}
