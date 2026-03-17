import type { FinanceBudgetUsage } from "@shared/types";
import { pct } from "../lib/finance";
import { Card } from "./Card";

function budgetStatusText(p: number): string {
  if (p >= 90) return `⚠ Near limit — ${100 - p}% remaining`;
  if (p >= 70) return "On pace for this period";
  return "Well under budget";
}

function budgetColor(p: number): string {
  if (p >= 80) return "#f43f5e";
  if (p >= 50) return "#f97316";
  return "#34d399";
}

export function BudgetStrip({ budgets }: { budgets: FinanceBudgetUsage[] }) {
  const active = budgets.filter((b) => b.isActive);
  if (active.length === 0) return null;

  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-2">
      {active.map((b) => {
        const p = pct(b.spent, b.amount);
        const color = budgetColor(p);
        return (
          <Card key={b.id} className="p-3.5">
            <div className="flex justify-between items-center mb-2">
              <span className="text-[12px] text-secondary">{b.name}</span>
              <span className="text-[13px] font-light tabular-nums" style={{ color }}>
                {p}%
              </span>
            </div>
            <div className="h-1 bg-surface-base rounded-full">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${Math.min(p, 100)}%`,
                  background: color,
                  transition: "width 0.6s ease",
                }}
              />
            </div>
            <p className="text-[9px] text-dim mt-1.5">{budgetStatusText(p)}</p>
          </Card>
        );
      })}
    </div>
  );
}
