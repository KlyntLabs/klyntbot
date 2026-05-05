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
      <div className="dashboard__goals">
        <div className="dashboard__goals-header">
          <h2>Goals</h2>
          <button
            type="button"
            onClick={() => setShowAdd(true)}
            className="dashboard__goals-add-btn"
            aria-label="Add goal"
          >
            <Plus aria-hidden />
          </button>
        </div>

        {goals.length === 0 ? (
          <p className="dashboard__goals-empty">No goals set</p>
        ) : (
          <div>
            {goals.map((g) => {
              const pct =
                g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
              return (
                <div key={g.id} className="dashboard__goal-row">
                  <div className="dashboard__goal-meta">
                    <span
                      className={
                        g.met
                          ? "dashboard__goal-status dashboard__goal-status--met"
                          : "dashboard__goal-status dashboard__goal-status--in-progress"
                      }
                    >
                      {g.met ? "MET" : "IN PROGRESS"}
                    </span>
                    <span>
                      {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                    </span>
                    {g.projectId && (
                      <span className="dashboard__goal-project-tag">{g.projectId}</span>
                    )}
                    <span>({g.goalType})</span>
                    <span>
                      {formatValue(g.metric, g.currentValue)} /{" "}
                      {formatValue(g.metric, g.targetValue)}
                    </span>
                    <button
                      type="button"
                      onClick={() => handleDelete(g.id)}
                      className="dashboard__goal-delete-btn"
                      aria-label="Delete goal"
                    >
                      <Trash2 aria-hidden />
                    </button>
                  </div>
                  <div className="dashboard__goal-bar-track">
                    <div
                      className={
                        g.met
                          ? "dashboard__goal-bar-fill dashboard__goal-bar-fill--met"
                          : "dashboard__goal-bar-fill"
                      }
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
