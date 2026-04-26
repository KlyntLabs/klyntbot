const clamp = (min: number, max: number, v: number) => Math.max(min, Math.min(max, v));

export interface HealthFactor {
  name: string;
  value: number; // 0–100
  color: string;
}

export interface HealthScore {
  score: number; // 0–100
  factors: HealthFactor[];
  status: string;
  statusColor: string;
}

export function computeHealthScore(params: {
  totalIncome: number;
  totalSpending: number;
  totalAssets: number;
  totalDebt: number;
  budgets: { spent: number; amount: number }[];
  goals: { currentAmount: number; targetAmount: number }[];
}): HealthScore {
  const { totalIncome, totalSpending, totalAssets, totalDebt, budgets, goals } = params;

  const savingsRate =
    totalIncome > 0 ? clamp(0, 100, ((totalIncome - totalSpending) / totalIncome) * 200) : 0;

  const debtRatio =
    totalAssets > 0 ? clamp(0, 100, (1 - totalDebt / totalAssets) * 100) : totalDebt > 0 ? 0 : 75;

  const budgetAdherence =
    budgets.length > 0
      ? budgets.reduce((sum, b) => sum + clamp(0, 100, (1 - b.spent / b.amount) * 100), 0) /
        budgets.length
      : 75;

  const goalProgress =
    goals.length > 0
      ? goals.reduce((sum, g) => sum + clamp(0, 100, (g.currentAmount / g.targetAmount) * 100), 0) /
        goals.length
      : 50;

  const score = Math.round((savingsRate + debtRatio + budgetAdherence + goalProgress) / 4);

  const factors: HealthFactor[] = [
    { name: "Savings Rate", value: Math.round(savingsRate), color: "#34d399" },
    { name: "Debt Ratio", value: Math.round(debtRatio), color: "#60a5fa" },
    { name: "Budget Adherence", value: Math.round(budgetAdherence), color: "#f97316" },
    { name: "Goal Progress", value: Math.round(goalProgress), color: "#a78bfa" },
  ];

  const status =
    score >= 70
      ? "Good — improving ↑"
      : score >= 40
        ? "Fair — watch spending"
        : "Needs attention ↓";
  const statusColor = score >= 70 ? "#34d399" : score >= 40 ? "#f97316" : "#f43f5e";

  return { score, factors, status, statusColor };
}

export function scoreColor(score: number): string {
  if (score >= 70) return "#34d399";
  if (score >= 40) return "#f97316";
  return "#f43f5e";
}
