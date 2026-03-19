import type { Objective, Task } from "@shared/types";
import { useMemo } from "react";
import { computeHealthScore, type HealthScoreResult } from "../lib/health-score";

export function useHealthScore(objectives: Objective[], tasks: Task[]): HealthScoreResult {
  return useMemo(() => {
    // OKR progress: weighted avg of all KR progress values
    const allKrs = objectives.flatMap((o) => o.keyResults ?? []);
    const okrProgress =
      allKrs.length > 0
        ? allKrs.reduce((sum, kr) => sum + kr.progress, 0) / allKrs.length / 100
        : 0;

    // Task velocity: completed in last 7 days / total active
    const total = tasks.length || 1;
    const completed = tasks.filter((t) => t.completed).length;
    const taskVelocity = completed / total;

    // Insight freshness and focus quality — placeholder for iteration 1
    // These require additional data sources (dashboard intelligence, insight cache)
    const insightFreshness = 0.5;
    const focusQuality = 0.5;

    return computeHealthScore({ okrProgress, taskVelocity, insightFreshness, focusQuality });
  }, [objectives, tasks]);
}
