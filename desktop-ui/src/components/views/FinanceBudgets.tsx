import { useMemo } from 'react';
import { Plus } from 'lucide-react';
import { Progress } from '../ui/Progress';
import { useQuery } from '../../hooks/useQuery';
import { useEvent } from '../../hooks/useEvent';
import { cn } from '../../lib/utils';
import { fmtMoney, fmtCompact, pct, COLORS } from '../../lib/finance';
import { FinanceLayout } from '../finance/FinanceLayout';
import { Card, SectionLabel } from '../finance/Card';
import { Donut } from '../finance/Donut';
import type { FinanceBudgetUsage } from '../../lib/types';
export function FinanceBudgets() {
  const { data: budgets, refetch } = useQuery<FinanceBudgetUsage[]>('finance_budget_usage', undefined, []);
  useEvent<{ entityKind: string }>('entity:updated', refetch);

  const activeBudgets = budgets.filter(b => b.isActive);
  const totalBudget = useMemo(() => activeBudgets.reduce((s, b) => s + b.amount, 0), [activeBudgets]);
  const totalSpent = useMemo(() => activeBudgets.reduce((s, b) => s + b.spent, 0), [activeBudgets]);
  const overBudget = activeBudgets.filter(b => b.spent > b.amount);

  const spentSegs = useMemo(() =>
    activeBudgets.map((b, i) => ({ name: b.name, value: b.spent, color: COLORS[i % COLORS.length] })),
    [activeBudgets],
  );
  const budgetSegs = useMemo(() =>
    activeBudgets.map((b, i) => ({ name: b.name, value: b.amount, color: COLORS[i % COLORS.length] })),
    [activeBudgets],
  );

  return (
    <FinanceLayout onRefresh={refetch}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">

        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Total Budget</p>
            <p className="text-[20px] font-light text-primary">{fmtCompact(totalBudget)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Total Spent</p>
            <p className="text-[20px] font-light text-destructive">{fmtCompact(totalSpent)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Remaining</p>
            <p className={cn('text-[20px] font-light', totalBudget - totalSpent >= 0 ? 'text-success' : 'text-destructive')}>
              {fmtCompact(totalBudget - totalSpent)}đ
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">Over Budget</p>
            <p className={cn('text-[20px] font-light', overBudget.length > 0 ? 'text-destructive' : 'text-success')}>
              {overBudget.length}
            </p>
          </Card>
        </div>

        {/* ── Budget cards (8col) + Charts (4col) ─────── */}
        <div className="col-span-8">
          <div className="flex items-center justify-between mb-2">
            <SectionLabel>Active Budgets</SectionLabel>
            <button className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors">
              <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Budget
            </button>
          </div>
          <div className="space-y-3">
            {activeBudgets.map((b, i) => {
              const p = pct(b.spent, b.amount);
              const rem = b.amount - b.spent;
              const isOver = rem < 0;
              return (
                <Card key={b.id} className="p-4">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: COLORS[i % COLORS.length] }} />
                      <span className="text-[13px] font-light text-secondary">{b.name}</span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-surface-base text-dim">{b.period}</span>
                      {b.category && <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-surface-base text-dim">{b.category}</span>}
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-[11px] text-muted font-light">{fmtMoney(b.spent, b.currency)} / {fmtMoney(b.amount, b.currency)}</span>
                      <span className={cn('text-[11px] font-light', p >= b.alertThreshold ? 'text-destructive' : p >= 60 ? 'text-brand' : 'text-success')}>{p}%</span>
                    </div>
                  </div>
                  <Progress value={Math.min(p, 100)} />
                  <div className="flex items-center justify-between mt-1.5">
                    <span className={cn('text-[10px] font-light', isOver ? 'text-destructive' : 'text-success')}>
                      {isOver ? `${fmtMoney(Math.abs(rem), b.currency)} over budget` : `${fmtMoney(rem, b.currency)} remaining`}
                    </span>
                    {p >= b.alertThreshold && !isOver && (
                      <span className="text-[9px] text-brand font-light">Approaching limit</span>
                    )}
                  </div>
                </Card>
              );
            })}
          </div>
        </div>

        <div className="col-span-4 space-y-3">
          <div>
            <SectionLabel>Spending Distribution</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut segments={spentSegs} label="Spent" value={fmtCompact(totalSpent) + 'đ'} size={150} />
            </Card>
          </div>
          <div>
            <SectionLabel>Budget Allocation</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut segments={budgetSegs} label="Allocated" value={fmtCompact(totalBudget) + 'đ'} size={150} />
            </Card>
          </div>
        </div>

      </div>
    </FinanceLayout>
  );
}
