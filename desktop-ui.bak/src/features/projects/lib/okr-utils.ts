import type { Objective } from "@shared/types";

export type ObjectiveStatus = "achieved" | "at_risk" | "on_track";

export function classifyObjective(objective: Objective): ObjectiveStatus {
  if (objective.progress >= 100) return "achieved";
  if (objective.status === "at_risk") return "at_risk";
  const avgKrProgress =
    (objective.keyResults ?? []).length > 0
      ? (objective.keyResults ?? []).reduce((s, kr) => s + kr.progress, 0) /
        (objective.keyResults ?? []).length
      : objective.progress;
  if (avgKrProgress < 30) return "at_risk";
  return "on_track";
}
