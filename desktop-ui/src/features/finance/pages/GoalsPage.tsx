import { Plus, Target } from "lucide-react";
import { useMemo, useState } from "react";
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { COLORS, fmtCompact, fmtMoney, GOAL_ICONS, pct, toVnd } from "../lib/finance";
import type { FinanceGoal, FinanceGoalCreateParams } from "@shared/types";
import { cn } from "@shared/lib/utils";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { Progress } from "@shared/ui";

type GoalTab = "active" | "achieved" | "abandoned";

export function FinanceGoals() {
  const {
    data: goals,
    loading,
    error,
    refetch,
  } = useQuery<FinanceGoal[]>("finance_goals", undefined, []);
  const { data: rates } = useQuery<Record<string, number>>("finance_exchange_rates", undefined, {});
  useEvent<{ entityKind: string }>("entity:updated", refetch);

  const [tab, setTab] = useState<GoalTab>("active");

  const filteredGoals = useMemo(() => goals.filter((g) => g.status === tab), [goals, tab]);

  // Convert to VND for cross-currency summing
  const activeGoals = useMemo(() => goals.filter((g) => g.status === "active"), [goals]);
  const totalTarget = useMemo(
    () => activeGoals.reduce((s, g) => s + toVnd(g.targetAmount, g.currency, rates), 0),
    [activeGoals, rates],
  );
  const totalSaved = useMemo(
    () => activeGoals.reduce((s, g) => s + toVnd(g.currentAmount, g.currency, rates), 0),
    [activeGoals, rates],
  );
  const monthlyTotal = useMemo(
    () => activeGoals.reduce((s, g) => s + toVnd(g.monthlyContribution ?? 0, g.currency, rates), 0),
    [activeGoals, rates],
  );

  const goalSegs = useMemo(
    () =>
      activeGoals.map((g, i) => ({
        name: g.name,
        value: toVnd(g.currentAmount, g.currency, rates),
        color: COLORS[i % COLORS.length],
      })),
    [activeGoals, rates],
  );

  // ── Add Goal modal ──
  const [modalOpen, setModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [goalType, setGoalType] = useState("savings");
  const [targetAmount, setTargetAmount] = useState("");
  const [currency, setCurrency] = useState("VND");
  const [deadline, setDeadline] = useState("");
  const [monthlyContribution, setMonthlyContribution] = useState("");

  const { mutate: createGoal } = useMutation<FinanceGoal, FinanceGoalCreateParams>(
    "finance_goal_create",
    "params",
  );

  const handleCreate = async () => {
    const result = await createGoal({
      name,
      goalType,
      targetAmount: Math.round(Number(targetAmount) * 100),
      currency,
      deadline: deadline || undefined,
      monthlyContribution: monthlyContribution
        ? Math.round(Number(monthlyContribution) * 100)
        : undefined,
    });
    if (!result) return;
    setModalOpen(false);
    setName("");
    setTargetAmount("");
    setDeadline("");
    setMonthlyContribution("");
    refetch();
  };

  const tabs: { key: GoalTab; label: string }[] = [
    { key: "active", label: "Active" },
    { key: "achieved", label: "Achieved" },
    { key: "abandoned", label: "Abandoned" },
  ];

  if (loading && goals.length === 0) {
    return (
      <FinanceLayout onRefresh={refetch}>
        <FinanceSkeleton />
      </FinanceLayout>
    );
  }

  if (error && goals.length === 0) {
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
              Active Goals
            </p>
            <p className="text-[20px] font-light text-primary">{activeGoals.length}</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Saved
            </p>
            <p className="text-[20px] font-light text-success tabular-nums">
              {fmtCompact(totalSaved)}đ
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Target
            </p>
            <p className="text-[20px] font-light text-primary tabular-nums">
              {fmtCompact(totalTarget)}đ
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Monthly Contributions
            </p>
            <p className="text-[20px] font-light text-brand tabular-nums">
              {fmtCompact(monthlyTotal)}đ
            </p>
          </Card>
        </div>

        {/* ── Goal cards (8col) + Chart (4col) ─────────── */}
        <div className="col-span-8">
          <Card className="p-4">
            <CardHeader
              title="Goals"
              action={
                <button
                  type="button"
                  onClick={() => setModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Goal
                </button>
              }
            />
            {/* Status tabs */}
            <div className="flex items-center gap-0.5 mb-4">
              {tabs.map((t) => (
                <button
                  key={t.key}
                  type="button"
                  onClick={() => setTab(t.key)}
                  className={cn(
                    "px-2.5 py-1 rounded-md text-[11px] font-light transition-colors",
                    tab === t.key
                      ? "bg-white/[0.12] text-brand"
                      : "text-muted hover:text-secondary hover:bg-white/[0.06]",
                  )}
                >
                  {t.label}
                </button>
              ))}
            </div>

            {filteredGoals.length === 0 ? (
              <div className="py-8 text-center text-[11px] text-dim font-light">No {tab} goals</div>
            ) : (
              <div className="space-y-3">
                {filteredGoals.map((g, i) => {
                  const p = pct(g.currentAmount, g.targetAmount);
                  const Icon = GOAL_ICONS[g.goalType] ?? Target;
                  const remaining = g.targetAmount - g.currentAmount;
                  const monthsLeft =
                    g.monthlyContribution && g.monthlyContribution > 0
                      ? Math.ceil(remaining / g.monthlyContribution)
                      : null;
                  return (
                    <div key={g.id} className="glass-card p-4">
                      <div className="flex items-center justify-between mb-3">
                        <div className="flex items-center gap-2">
                          <div
                            className="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0"
                            style={{ backgroundColor: `${COLORS[i % COLORS.length]}20` }}
                          >
                            <Icon
                              className="w-4 h-4"
                              style={{ color: COLORS[i % COLORS.length] }}
                              strokeWidth={1.5}
                            />
                          </div>
                          <div>
                            <p className="text-[13px] font-medium text-secondary">{g.name}</p>
                            <p className="text-[9px] text-dim font-light">{g.goalType}</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-[14px] font-light text-primary tabular-nums">
                            {fmtMoney(g.currentAmount, g.currency)}{" "}
                            <span className="text-dim text-[10px]">
                              / {fmtMoney(g.targetAmount, g.currency)}
                            </span>
                          </p>
                          <p className="text-[11px] font-light text-brand tabular-nums">{p}%</p>
                        </div>
                      </div>
                      <Progress value={p} />
                      <div className="flex items-center gap-4 mt-2">
                        {g.deadline && (
                          <span className="text-[10px] text-dim font-light">
                            Deadline: {g.deadline}
                          </span>
                        )}
                        {g.monthlyContribution && (
                          <span className="text-[10px] text-dim font-light">
                            Contributing: {fmtMoney(g.monthlyContribution, g.currency)}/mo
                          </span>
                        )}
                        {monthsLeft != null && (
                          <span className="text-[10px] text-muted font-light">
                            ~{monthsLeft} months to go
                          </span>
                        )}
                        <span className="text-[10px] text-dim font-light">
                          {fmtMoney(remaining, g.currency)} remaining
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </Card>
        </div>

        <div className="col-span-4 space-y-4">
          <Card className="p-4">
            <CardHeader title="Progress Distribution" />
            <div className="flex items-center justify-center">
              <Donut
                segments={goalSegs}
                label="Saved"
                value={`${fmtCompact(totalSaved)}đ`}
                size={150}
              />
            </div>
          </Card>
          <Card className="p-4">
            <CardHeader title="Overall Progress" />
            <div className="mb-3">
              <div className="flex justify-between mb-1">
                <span className="text-[10px] text-muted font-light">Total Progress</span>
                <span className="text-[10px] text-brand font-light">
                  {pct(totalSaved, totalTarget)}%
                </span>
              </div>
              <Progress value={pct(totalSaved, totalTarget)} />
            </div>
            <div className="space-y-2 pt-2 border-t border-white/[0.04]">
              {activeGoals.map((g, i) => {
                const p = pct(g.currentAmount, g.targetAmount);
                return (
                  <div key={g.id}>
                    <div className="flex justify-between mb-0.5">
                      <div className="flex items-center gap-1.5">
                        <div
                          className="w-1.5 h-1.5 rounded-full"
                          style={{ backgroundColor: COLORS[i % COLORS.length] }}
                        />
                        <span className="text-[10px] text-muted font-light">{g.name}</span>
                      </div>
                      <span className="text-[10px] text-secondary font-light">{p}%</span>
                    </div>
                    <Progress value={p} />
                  </div>
                );
              })}
            </div>
          </Card>
        </div>
      </div>

      {/* ── Add Goal Modal ────────────────────────────── */}
      <FormModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title="Add Goal"
        onSubmit={handleCreate}
        canSubmit={name.trim().length > 0 && Number(targetAmount) > 0}
      >
        <FormField label="Goal Name">
          <input
            className={fieldClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Emergency Fund"
            autoFocus
          />
        </FormField>
        <FormField label="Goal Type">
          <select
            className={fieldClass}
            value={goalType}
            onChange={(e) => setGoalType(e.target.value)}
          >
            <option value="savings">Savings</option>
            <option value="purchase">Purchase</option>
            <option value="fire">FIRE</option>
            <option value="custom">Custom</option>
          </select>
        </FormField>
        <FormField label="Target Amount">
          <input
            className={fieldClass}
            type="number"
            value={targetAmount}
            onChange={(e) => setTargetAmount(e.target.value)}
            placeholder="0"
          />
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
        <FormField label="Deadline (optional)">
          <input
            className={fieldClass}
            type="date"
            value={deadline}
            onChange={(e) => setDeadline(e.target.value)}
          />
        </FormField>
        <FormField label="Monthly Contribution (optional)">
          <input
            className={fieldClass}
            type="number"
            value={monthlyContribution}
            onChange={(e) => setMonthlyContribution(e.target.value)}
            placeholder="0"
          />
        </FormField>
      </FormModal>
    </FinanceLayout>
  );
}
