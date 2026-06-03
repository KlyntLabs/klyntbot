import Plus from "lucide-react/dist/esm/icons/plus";
import Trash2 from "lucide-react/dist/esm/icons/trash-2";
import { useState } from "react";
import {
  type GoalCreateParams,
  productivityGoalCreate,
  productivityGoalDelete,
  productivityGoalsQuery,
} from "@/api/endpoints/dashboard";
import type { GoalProgressResponse } from "@/bindings";
import { useTauriMutation, useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { cn } from "@/utils/cn";
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
  const [showAdd, setShowAdd] = useState(false);

  const { data: goals } = useTauriQuery<GoalProgressResponse[]>({
    queryKey: qk.productivity.goals(),
    queryFn: () => productivityGoalsQuery(),
    fallback: [],
  });

  const { mutate: createGoal } = useTauriMutation<GoalProgressResponse, GoalCreateParams>({
    mutationFn: productivityGoalCreate,
    invalidates: [qk.productivity.goals()],
  });

  const { mutate: deleteGoal } = useTauriMutation<void, number>({
    mutationFn: productivityGoalDelete,
    invalidates: [qk.productivity.goals()],
  });

  const handleAdd = (params: GoalCreateParams) => {
    void createGoal(params);
  };

  const handleDelete = (id: number) => {
    if (window.confirm("Are you sure you want to delete this goal?")) {
      void deleteGoal(id);
    }
  };

  return (
    <>
      <div className="bg-surface-card-strong border border-ds-border-subtle rounded-lg px-3.5 py-3 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h2 className="text-ui-sm font-medium text-ds-text-subtle m-0">Goals</h2>
          <button
            type="button"
            onClick={() => setShowAdd(true)}
            className="w-6 h-6 rounded-md bg-none border-none text-ds-text-subtle flex items-center justify-center cursor-pointer hover:text-brand hover:bg-surface-control"
            aria-label="Add goal"
          >
            <Plus aria-hidden className="w-3.5 h-3.5" />
          </button>
        </div>

        {goals.length === 0 ? (
          <p className="text-ui-2xs font-light text-ds-text-subtle m-0">No goals set</p>
        ) : (
          <div>
            {goals.map((g) => {
              const pct =
                g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
              return (
                <div key={g.id} className="group flex flex-col gap-1 mb-2">
                  <div className="flex items-center gap-2 text-ui-2xs font-light flex-wrap">
                    <span
                      className={cn(
                        g.met ? "text-success" : "text-brand",
                      )}
                    >
                      {g.met ? "MET" : "IN PROGRESS"}
                    </span>
                    <span>
                      {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                    </span>
                    {g.projectId && (
                      <span className="text-ui-2xs px-1.5 py-0.5 rounded bg-surface-control text-ds-text-subtle">{g.projectId}</span>
                    )}
                    <span>({g.goalType})</span>
                    <span>
                      {formatValue(g.metric, g.currentValue)} /{" "}
                      {formatValue(g.metric, g.targetValue)}
                    </span>
                    <button
                      type="button"
                      onClick={() => handleDelete(g.id)}
                      className="w-5 h-5 rounded bg-none border-none text-transparent cursor-pointer flex items-center justify-center group-hover:text-ds-text-subtle hover:text-destructive"
                      aria-label="Delete goal"
                    >
                      <Trash2 aria-hidden className="w-3 h-3" />
                    </button>
                  </div>
                  <div className="h-1.5 rounded-full bg-surface-control overflow-hidden">
                    <div
                      className={cn("h-full rounded-full transition-[width] duration-300", g.met && "bg-success")}
                      style={{ width: `${pct}%`, backgroundColor: g.met ? undefined : "var(--brand)" }}
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
