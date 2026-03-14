import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import type { FinanceBudgetCreateParams, FinanceBudgetUsage } from "@shared/types";
import { Progress } from "@shared/ui";
import { Plus } from "lucide-react";
import { useMemo, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { COLORS, fmtCompact, fmtMoney, pct, toBase } from "../lib/finance";

export function FinanceBudgets() {
  const {
    data: budgets,
    loading,
    error,
    refetch,
  } = useQuery<FinanceBudgetUsage[]>("finance_budget_usage", undefined, []);
  const { data: rates } = useQuery<Record<string, number>>("finance_exchange_rates", undefined, {});
  const { data: settings } = useQuery<{ defaultCurrency: string }>(
    "finance_settings",
    undefined,
    {},
  );
  const baseCurrency = settings?.defaultCurrency ?? "USD";
  useEvent<{ entityKind: string }>("entity:updated", refetch);

  const activeBudgets = useMemo(() => budgets.filter((b) => b.isActive), [budgets]);

  // Convert to base currency before summing to handle multi-currency
  const totalBudget = useMemo(
    () => activeBudgets.reduce((s, b) => s + toBase(b.amount, b.currency, rates, baseCurrency), 0),
    [activeBudgets, rates],
  );
  const totalSpent = useMemo(
    () => activeBudgets.reduce((s, b) => s + toBase(b.spent, b.currency, rates, baseCurrency), 0),
    [activeBudgets, rates],
  );
  const overBudget = activeBudgets.filter((b) => b.spent > b.amount);

  const spentSegs = useMemo(
    () =>
      activeBudgets.map((b, i) => ({
        name: b.name,
        value: toBase(b.spent, b.currency, rates, baseCurrency),
        color: COLORS[i % COLORS.length],
      })),
    [activeBudgets, rates],
  );
  const budgetSegs = useMemo(
    () =>
      activeBudgets.map((b, i) => ({
        name: b.name,
        value: toBase(b.amount, b.currency, rates, baseCurrency),
        color: COLORS[i % COLORS.length],
      })),
    [activeBudgets, rates],
  );

  // ── Add Budget modal ──
  const [modalOpen, setModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [amount, setAmount] = useState("");
  const [period, setPeriod] = useState("monthly");
  const [currency, setCurrency] = useState("VND");
  const [category, setCategory] = useState("");

  const { mutate: createBudget } = useMutation<FinanceBudgetUsage, FinanceBudgetCreateParams>(
    "finance_budget_create",
    "params",
  );

  const handleCreate = async () => {
    const result = await createBudget({
      name,
      amount: Math.round(Number(amount) * 100),
      period,
      currency,
      category: category || undefined,
    });
    if (!result) return;
    setModalOpen(false);
    setName("");
    setAmount("");
    setCategory("");
    refetch();
  };

  if (loading && budgets.length === 0) {
    return (
      <FinanceLayout onRefresh={refetch}>
        <FinanceSkeleton />
      </FinanceLayout>
    );
  }

  if (error && budgets.length === 0) {
    return (
      <FinanceLayout onRefresh={refetch}>
        <Card className="p-6 text-center">
          <p className="text-[12px] text-destructive mb-2">{error.message}</p>
          <button
            type="button"
            onClick={refetch}
            className="text-[11px] text-brand hover:text-brand-hover transition-colors"
          >
            Retry
          </button>
        </Card>
      </FinanceLayout>
    );
  }

  return (
    <FinanceLayout onRefresh={refetch}>
      <div className="grid grid-cols-12 gap-4 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-4">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Budget
            </p>
            <p className="text-[20px] font-light text-primary tabular-nums">
              {fmtCompact(totalBudget, baseCurrency)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Spent
            </p>
            <p className="text-[20px] font-light text-destructive tabular-nums">
              {fmtCompact(totalSpent, baseCurrency)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Remaining
            </p>
            <p
              className={cn(
                "text-[20px] font-light tabular-nums",
                totalBudget - totalSpent >= 0 ? "text-success" : "text-destructive",
              )}
            >
              {fmtCompact(totalBudget - totalSpent, baseCurrency)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Over Budget
            </p>
            <p
              className={cn(
                "text-[20px] font-light",
                overBudget.length > 0 ? "text-destructive" : "text-success",
              )}
            >
              {overBudget.length}
            </p>
          </Card>
        </div>

        {/* ── Budget cards (8col) + Charts (4col) ─────── */}
        <div className="col-span-8">
          <Card className="p-4">
            <CardHeader
              title="Active Budgets"
              action={
                <button
                  type="button"
                  onClick={() => setModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Budget
                </button>
              }
            />
            <div className="space-y-3">
              {activeBudgets.map((b, i) => {
                const p = pct(b.spent, b.amount);
                const rem = b.amount - b.spent;
                const isOver = rem < 0;
                return (
                  <div key={b.id} className="glass-card p-4">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <div
                          className="w-2.5 h-2.5 rounded-full"
                          style={{ backgroundColor: COLORS[i % COLORS.length] }}
                        />
                        <span className="text-[13px] font-medium text-secondary">{b.name}</span>
                        <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                          {b.period}
                        </span>
                        {b.category && (
                          <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                            {b.category}
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-3">
                        <span className="text-[11px] text-muted font-light tabular-nums">
                          {fmtMoney(b.spent, b.currency)} / {fmtMoney(b.amount, b.currency)}
                        </span>
                        <span
                          className={cn(
                            "text-[11px] font-light tabular-nums",
                            p >= b.alertThreshold
                              ? "text-destructive"
                              : p >= 60
                                ? "text-brand"
                                : "text-success",
                          )}
                        >
                          {p}%
                        </span>
                      </div>
                    </div>
                    <Progress value={Math.min(p, 100)} />
                    <div className="flex items-center justify-between mt-1.5">
                      <span
                        className={cn(
                          "text-[10px] font-light",
                          isOver ? "text-destructive" : "text-success",
                        )}
                      >
                        {isOver
                          ? `${fmtMoney(Math.abs(rem), b.currency)} over budget`
                          : `${fmtMoney(rem, b.currency)} remaining`}
                      </span>
                      {p >= b.alertThreshold && !isOver && (
                        <span className="text-[9px] text-brand font-light">Approaching limit</span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        <div className="col-span-4 space-y-4">
          <Card className="p-4">
            <CardHeader title="Spending Distribution" />
            <div className="flex items-center justify-center">
              <Donut
                segments={spentSegs}
                label="Spent"
                value={fmtCompact(totalSpent, baseCurrency)}
                size={150}
              />
            </div>
          </Card>
          <Card className="p-4">
            <CardHeader title="Budget Allocation" />
            <div className="flex items-center justify-center">
              <Donut
                segments={budgetSegs}
                label="Allocated"
                value={fmtCompact(totalBudget, baseCurrency)}
                size={150}
              />
            </div>
          </Card>
        </div>
      </div>

      {/* ── Add Budget Modal ──────────────────────────── */}
      <FormModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title="Add Budget"
        onSubmit={handleCreate}
        canSubmit={name.trim().length > 0 && Number(amount) > 0}
      >
        <FormField label="Budget Name">
          <input
            className={fieldClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Food & Dining"
            autoFocus
          />
        </FormField>
        <FormField label="Amount">
          <input
            className={fieldClass}
            type="number"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            placeholder="0"
          />
        </FormField>
        <FormField label="Period">
          <select className={fieldClass} value={period} onChange={(e) => setPeriod(e.target.value)}>
            <option value="monthly">Monthly</option>
            <option value="weekly">Weekly</option>
            <option value="yearly">Yearly</option>
          </select>
        </FormField>
        <FormField label="Currency">
          <select
            className={fieldClass}
            value={currency}
            onChange={(e) => setCurrency(e.target.value)}
          >
            <option value="VND">VND</option>
            <option value="USD">USD</option>
            <option value="USDT">USDT</option>
          </select>
        </FormField>
        <FormField label="Category (optional)">
          <input
            className={fieldClass}
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            placeholder="e.g. food, transport"
          />
        </FormField>
      </FormModal>
    </FinanceLayout>
  );
}
