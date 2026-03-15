export interface PulseRow {
  label: string;
  direction: "up" | "down" | "flat";
  hint: string;
  barWidth: number;
  color: string;
}

export function computeMonthlyPulse(params: {
  currentIncome: number;
  currentSpending: number;
  previousIncome: number;
  previousSpending: number;
}): PulseRow[] {
  const { currentIncome, currentSpending, previousIncome, previousSpending } = params;

  const incPct = previousIncome > 0 ? Math.round(((currentIncome - previousIncome) / previousIncome) * 100) : 0;
  const spendPct = previousSpending > 0 ? Math.round(((currentSpending - previousSpending) / previousSpending) * 100) : 0;

  const curSavingsRate = currentIncome > 0 ? Math.round(((currentIncome - currentSpending) / currentIncome) * 100) : 0;
  const prevSavingsRate = previousIncome > 0 ? Math.round(((previousIncome - previousSpending) / previousIncome) * 100) : 0;

  const incDir: PulseRow["direction"] = incPct > 2 ? "up" : incPct < -2 ? "down" : "flat";
  const spendDir: PulseRow["direction"] = spendPct > 2 ? "up" : spendPct < -2 ? "down" : "flat";
  const savDir: PulseRow["direction"] = curSavingsRate > prevSavingsRate + 2 ? "up" : curSavingsRate < prevSavingsRate - 2 ? "down" : "flat";

  return [
    {
      label: "Income vs Last Month",
      direction: incDir,
      hint: incDir === "flat" ? "Stable · on track" : `${incPct > 0 ? "+" : ""}${incPct}% ${incPct > 0 ? "higher" : "lower"} · ${incPct > 0 ? "on track" : "watch this"}`,
      barWidth: Math.min(100, Math.max(20, 50 + incPct)),
      color: "#34d399",
    },
    {
      label: "Spending vs Last Month",
      direction: spendDir,
      hint: spendDir === "flat" ? "Stable" : `${spendPct > 0 ? "+" : ""}${spendPct}% ${spendPct > 0 ? "higher" : "lower"} · ${spendPct < 0 ? "improving" : "watch this"}`,
      barWidth: Math.min(100, Math.max(20, 50 + spendPct)),
      color: "#f43f5e",
    },
    {
      label: "Savings Rate",
      direction: savDir,
      hint: `${curSavingsRate}%${prevSavingsRate > 0 ? ` · ${savDir === "up" ? "up" : savDir === "down" ? "down" : "same as"} from ${prevSavingsRate}%` : ""}`,
      barWidth: Math.min(100, Math.max(10, curSavingsRate)),
      color: "#60a5fa",
    },
  ];
}

export const DIRECTION_ICONS: Record<PulseRow["direction"], string> = {
  up: "↑",
  down: "↓",
  flat: "≈",
};
