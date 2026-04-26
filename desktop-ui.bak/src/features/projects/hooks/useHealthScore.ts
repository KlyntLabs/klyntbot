import { useQuery } from "@shared/hooks/useQuery";
import type { Objective, Task } from "@shared/types";
import { useMemo } from "react";
import { computeHealthScore, type HealthScoreResult } from "../lib/health-score";

interface ProjectHealthMetrics {
  focusQuality: number | null;
  insightFreshness: number | null;
}

export function useHealthScore(
  objectives: Objective[],
  tasks: Task[],
  projectId?: string,
): HealthScoreResult {
  const { data: metrics } = useQuery<ProjectHealthMetrics>(
    "project_health_metrics",
    projectId ? { projectId } : undefined,
    { enabled: !!projectId, focusQuality: null, insightFreshness: null },
  );

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

    // Use real data from backend, fall back to 0.5 when no data available
    const insightFreshness = metrics.insightFreshness ?? 0.5;
    const focusQuality = metrics.focusQuality ?? 0.5;

    return computeHealthScore({ okrProgress, taskVelocity, insightFreshness, focusQuality });
  }, [objectives, tasks, metrics]);
}
