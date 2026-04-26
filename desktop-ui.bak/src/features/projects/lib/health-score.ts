export interface HealthScoreInput {
  okrProgress: number; // 0-1: weighted avg of KR progress
  taskVelocity: number; // 0-1: completed / total tasks in 7 days
  insightFreshness: number; // 0-1: 1.0 if < 7 days, linear decay
  focusQuality: number; // 0-1: avg productivity %
}

export interface HealthScoreBreakdown {
  label: string;
  weight: number;
  value: number;
  contribution: number;
}

export interface HealthScoreResult {
  score: number; // 0-100
  color: "green" | "yellow" | "red";
  breakdown: HealthScoreBreakdown[];
}

const WEIGHTS = [
  { key: "okrProgress" as const, label: "OKR Progress", weight: 0.6 },
  { key: "taskVelocity" as const, label: "Task Velocity", weight: 0.2 },
  { key: "insightFreshness" as const, label: "Insight Freshness", weight: 0.1 },
  { key: "focusQuality" as const, label: "Focus Quality", weight: 0.1 },
];

function clamp01(n: number): number {
  return Math.max(0, Math.min(1, n));
}

export function computeHealthScore(input: HealthScoreInput): HealthScoreResult {
  const breakdown: HealthScoreBreakdown[] = WEIGHTS.map(({ key, label, weight }) => {
    const value = clamp01(input[key]);
    return { label, weight, value, contribution: value * weight };
  });

  const raw = breakdown.reduce((sum, b) => sum + b.contribution, 0);
  const score = Math.round(raw * 100);

  let color: "green" | "yellow" | "red";
  if (score > 70) color = "green";
  else if (score >= 40) color = "yellow";
  else color = "red";

  return { score, color, breakdown };
}
