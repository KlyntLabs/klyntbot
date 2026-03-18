import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import type {
  FinanceGoal,
  FinanceGoalCreateParams,
  FinanceLiability,
  FinanceLiabilityCreateParams,
} from "@shared/types";
import { Progress } from "@shared/ui";
import { Plus, Target, Wallet } from "lucide-react";
import { useMemo, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { usePrivacyMode } from "../hooks/usePrivacyMode";
import { displayAmount } from "../lib/displayAmount";
import { COLORS, fmtCompact, GOAL_ICONS, LIAB_ICONS, pct } from "../lib/finance";

type GoalTab = "active" | "achieved" | "abandoned";

export function FinanceTargets() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const { hidden, toggle } = usePrivacyMode();

  const {
    data: goals,
    loading: goalsLoading,
    refetch: rG,
  } = useQuery<FinanceGoal[]>("finance_goals", undefined, []);
  const {
    data: liabilities,
    loading: liabLoading,
    refetch: rL,
  } = useQuery<FinanceLiability[]>("finance_liabilities", undefined, []);

  useEvent<{ entityKind: string }>("entity:updated", () => {
    rG();
    rL();
  });

  // ── Goal tab state ───────────────────────────────────────────────
  const [goalTab, setGoalTab] = useState<GoalTab>("active");

  const activeGoals = useMemo(() => goals.filter((g) => g.status === "active"), [goals]);
  const filteredGoals = useMemo(() => goals.filter((g) => g.status === goalTab), [goals, goalTab]);

  // ── Goal aggregates ──────────────────────────────────────────────
  const totalGoalTarget = useMemo(
    () => activeGoals.reduce((s, g) => s + (g.baseTargetAmount ?? g.targetAmount), 0),
    [activeGoals],
  );
  const totalGoalSaved = useMemo(
    () => activeGoals.reduce((s, g) => s + (g.baseCurrentAmount ?? g.currentAmount), 0),
    [activeGoals],
  );
  const overallGoalPct = pct(totalGoalSaved, totalGoalTarget);

  // ── Liability aggregates ─────────────────────────────────────────
  const totalRemaining = useMemo(
    () => liabilities.reduce((s, l) => s + (l.baseRemaining ?? l.remaining), 0),
    [liabilities],
  );
  const totalPrincipal = useMemo(
    () => liabilities.reduce((s, l) => s + (l.basePrincipal ?? l.principal), 0),
    [liabilities],
  );
  const totalPaid = totalPrincipal - totalRemaining;
  const debtPaidPct = pct(totalPaid, totalPrincipal);

  const liabMonthlyTotal = useMemo(
    () =>
      liabilities.reduce((s, l) => {
        if (l.monthlyPayment == null) return s;
        if (l.basePrincipal != null && l.principal !== 0) {
          return s + Math.round(l.monthlyPayment * (l.basePrincipal / l.principal));
        }
        return s + l.monthlyPayment;
      }, 0),
    [liabilities],
  );

  // ── Donut segments ───────────────────────────────────────────────
  const goalSegs = useMemo(
    () =>
      activeGoals.map((g, i) => ({
        name: g.name,
        value: g.baseCurrentAmount ?? g.currentAmount,
        color: COLORS[i % COLORS.length],
      })),
    [activeGoals],
  );

  const liabSegs = useMemo(
    () =>
      liabilities.map((l, i) => ({
        name: l.name,
        value: l.baseRemaining ?? l.remaining,
        color: COLORS[i % COLORS.length],
      })),
    [liabilities],
  );

  // ── Add Goal modal state ─────────────────────────────────────────
  const [goalModalOpen, setGoalModalOpen] = useState(false);
  const [gName, setGName] = useState("");
  const [goalType, setGoalType] = useState("savings");
  const [targetAmount, setTargetAmount] = useState("");
  const [gCurrency, setGCurrency] = useState("VND");
  const [deadline, setDeadline] = useState("");
  const [monthlyContribution, setMonthlyContribution] = useState("");

  const { mutate: createGoal } = useMutation<FinanceGoal, FinanceGoalCreateParams>(
    "finance_goal_create",
    "params",
  );

  const handleCreateGoal = async () => {
    const result = await createGoal({
      name: gName,
      goalType,
      targetAmount: Math.round(Number(targetAmount) * 100),
      currency: gCurrency,
      deadline: deadline || undefined,
      monthlyContribution: monthlyContribution
        ? Math.round(Number(monthlyContribution) * 100)
        : undefined,
    });
    if (!result) return;
    setGoalModalOpen(false);
    setGName("");
    setTargetAmount("");
    setDeadline("");
    setMonthlyContribution("");
    rG();
  };

  // ── Add Liability modal state ────────────────────────────────────
  const [liabModalOpen, setLiabModalOpen] = useState(false);
  const [lName, setLName] = useState("");
  const [liabilityType, setLiabilityType] = useState("personal_loan");
  const [principal, setPrincipal] = useState("");
  const [lCurrency, setLCurrency] = useState("VND");
  const [interestRate, setInterestRate] = useState("");
  const [monthlyPayment, setMonthlyPayment] = useState("");
  const [dueDate, setDueDate] = useState("");

  const { mutate: createLiability } = useMutation<FinanceLiability, FinanceLiabilityCreateParams>(
    "finance_liability_create",
    "params",
  );

  const handleCreateLiability = async () => {
    const result = await createLiability({
      name: lName,
      liabilityType,
      principal: Math.round(Number(principal) * 100),
      currency: lCurrency,
      interestRate: interestRate ? Number(interestRate) : undefined,
      monthlyPayment: monthlyPayment ? Math.round(Number(monthlyPayment) * 100) : undefined,
      dueDate: dueDate || undefined,
    });
    if (!result) return;
    setLiabModalOpen(false);
    setLName("");
    setPrincipal("");
    setInterestRate("");
    setMonthlyPayment("");
    setDueDate("");
    rL();
  };

  const loading = goalsLoading && goals.length === 0 && liabLoading && liabilities.length === 0;

  if (loading) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <FinanceSkeleton />
      </FinanceLayout>
    );
  }

  const goalTabs: { key: GoalTab; label: string }[] = [
    { key: "active", label: "Active" },
    { key: "achieved", label: "Achieved" },
    { key: "abandoned", label: "Abandoned" },
  ];

  return (
    <FinanceLayout
      hidden={hidden}
      onTogglePrivacy={toggle}
      currencyMode={mode}
      currencies={currencies}
      onSelectCurrency={setMode}
    >
      {/* ── Stats row ─────────────────────────────────────────────── */}
      <div className="grid grid-cols-4 gap-3 mb-4">
        <Card compact className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Active Goals
          </p>
          <p className="text-[24px] font-light text-foreground">{activeGoals.length}</p>
        </Card>
        <Card compact className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Goal Progress
          </p>
          <p className="text-[24px] font-light text-brand tabular-nums">{overallGoalPct}%</p>
        </Card>
        <Card compact className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Total Debt
          </p>
          <p className="text-[24px] font-light text-destructive tabular-nums">
            {fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
          </p>
        </Card>
        <Card compact className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Debt Paid
          </p>
          <p className="text-[24px] font-light text-success tabular-nums">{debtPaidPct}%</p>
        </Card>
      </div>

      <div className="flex gap-4">
        {/* ── Left: main content ──────────────────────────────────── */}
        <div className="flex-1 min-w-0 space-y-4">
          {/* GOALS section */}
          <Card className="p-4">
            <CardHeader
              title="Goals"
              action={
                <button
                  type="button"
                  onClick={() => setGoalModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Goal
                </button>
              }
            />
            {/* Status tabs */}
            <div className="flex items-center gap-0.5 mb-4">
              {goalTabs.map((t) => (
                <button
                  key={t.key}
                  type="button"
                  onClick={() => setGoalTab(t.key)}
                  className={cn(
                    "px-2.5 py-1 rounded-md text-[11px] font-light transition-colors",
                    goalTab === t.key
                      ? "bg-muted text-brand"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent",
                  )}
                >
                  {t.label}
                </button>
              ))}
            </div>

            {filteredGoals.length === 0 ? (
              <div className="py-8 text-center text-[11px] text-dim font-light">
                No {goalTab} goals
              </div>
            ) : (
              <div className="divide-y divide-border-subtle">
                {filteredGoals.map((g, i) => {
                  const p = pct(g.currentAmount, g.targetAmount);
                  const Icon = GOAL_ICONS[g.goalType] ?? Target;
                  return (
                    <div key={g.id} className="py-2.5 first:pt-0 last:pb-0">
                      <div className="flex items-center justify-between mb-1.5">
                        <div className="flex items-center gap-2 min-w-0">
                          <Icon
                            className="w-3.5 h-3.5 flex-shrink-0"
                            style={{ color: COLORS[i % COLORS.length] }}
                            strokeWidth={1.5}
                          />
                          <span className="text-[12px] font-medium text-muted-foreground truncate">
                            {g.name}
                          </span>
                          <span
                            className="text-[8px] font-light px-1.5 py-0.5 rounded-full flex-shrink-0"
                            style={{
                              backgroundColor: `${COLORS[i % COLORS.length]}18`,
                              color: COLORS[i % COLORS.length],
                            }}
                          >
                            {g.goalType}
                          </span>
                        </div>
                        <div className="flex items-center gap-2 flex-shrink-0">
                          <span className="text-[11px] text-dim tabular-nums">
                            {displayAmount({
                              amount: g.currentAmount,
                              currency: g.currency,
                              baseAmount: g.baseCurrentAmount,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                              compact: true,
                            })}
                            {" / "}
                            {displayAmount({
                              amount: g.targetAmount,
                              currency: g.currency,
                              baseAmount: g.baseTargetAmount,
                              baseCurrency,
                              mode,
                              rates,
                              compact: true,
                            })}
                          </span>
                          <span className="text-[10px] text-brand tabular-nums w-8 text-right">
                            {p}%
                          </span>
                        </div>
                      </div>
                      <div className="h-1 bg-accent rounded-full">
                        <div
                          className="h-full rounded-full"
                          style={{
                            width: `${p}%`,
                            background: COLORS[i % COLORS.length],
                            transition: "width 0.5s ease",
                          }}
                        />
                      </div>
                      <div className="flex items-center gap-3 mt-1">
                        {g.deadline && (
                          <span className="text-[9px] text-dim">Due {g.deadline}</span>
                        )}
                        {g.monthlyContribution != null && g.monthlyContribution > 0 && (
                          <span className="text-[9px] text-dim">
                            {displayAmount({
                              amount: g.monthlyContribution,
                              currency: g.currency,
                              baseAmount:
                                g.baseTargetAmount != null && g.targetAmount !== 0
                                  ? Math.round(
                                      g.monthlyContribution * (g.baseTargetAmount / g.targetAmount),
                                    )
                                  : undefined,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                              compact: true,
                            })}
                            /mo
                          </span>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </Card>

          {/* DEBTS section */}
          <Card className="p-4">
            <CardHeader
              title="Debts"
              action={
                <button
                  type="button"
                  onClick={() => setLiabModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Debt
                </button>
              }
            />
            {liabilities.length === 0 ? (
              <div className="py-8 text-center text-[11px] text-dim font-light">
                No debts tracked
              </div>
            ) : (
              <>
                <div className="divide-y divide-border-subtle">
                  {liabilities.map((l, i) => {
                    const Icon = LIAB_ICONS[l.liabilityType] ?? Wallet;
                    const paid = pct(l.principal - l.remaining, l.principal);
                    return (
                      <div key={l.id} className="py-2.5 first:pt-0 last:pb-0">
                        <div className="flex items-center justify-between mb-1.5">
                          <div className="flex items-center gap-2 min-w-0">
                            <Icon
                              className="w-3.5 h-3.5 flex-shrink-0 text-destructive/60"
                              strokeWidth={1.5}
                            />
                            <span className="text-[12px] font-medium text-muted-foreground truncate">
                              {l.name}
                            </span>
                            <span className="text-[8px] font-light px-1.5 py-0.5 rounded-full bg-accent text-dim flex-shrink-0">
                              {l.liabilityType.replaceAll("_", " ")}
                            </span>
                          </div>
                          <div className="flex items-center gap-2 flex-shrink-0">
                            <span className="text-[11px] text-destructive tabular-nums">
                              {displayAmount({
                                amount: l.remaining,
                                currency: l.currency,
                                baseAmount: l.baseRemaining,
                                baseCurrency,
                                mode,
                                rates,
                                hidden,
                                compact: true,
                              })}
                            </span>
                            <span className="text-[10px] text-success tabular-nums w-8 text-right">
                              {paid}%
                            </span>
                          </div>
                        </div>
                        <div className="h-1 bg-accent rounded-full">
                          <div
                            className="h-full bg-success rounded-full"
                            style={{ width: `${paid}%`, transition: "width 0.5s ease" }}
                          />
                        </div>
                        <div className="flex items-center gap-3 mt-1">
                          {l.interestRate != null && (
                            <span className="text-[9px] text-dim">{l.interestRate}% APR</span>
                          )}
                          {l.monthlyPayment != null && l.monthlyPayment > 0 && (
                            <span className="text-[9px] text-dim">
                              {displayAmount({
                                amount: l.monthlyPayment,
                                currency: l.currency,
                                baseAmount:
                                  l.basePrincipal != null && l.principal !== 0
                                    ? Math.round(l.monthlyPayment * (l.basePrincipal / l.principal))
                                    : undefined,
                                baseCurrency,
                                mode,
                                rates,
                                hidden,
                                compact: true,
                              })}
                              /mo
                            </span>
                          )}
                          {l.dueDate && (
                            <span className="text-[9px] text-dim">Due {l.dueDate}</span>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
                {liabilities.length > 0 && (
                  <div className="mt-3 pt-2.5 border-t border-border flex justify-between">
                    <span className="text-[10px] text-muted-foreground">Total Debt</span>
                    <span className="text-[11px] text-destructive tabular-nums">
                      {fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
                    </span>
                  </div>
                )}
              </>
            )}
          </Card>
        </div>

        {/* ── Right sidebar ────────────────────────────────────────── */}
        <div className="w-72 flex-shrink-0 sticky top-0 self-start space-y-4">
          {/* Goal Progress overview */}
          <Card compact className="p-4">
            <p className="text-[10px] text-muted-foreground uppercase tracking-widest mb-3">
              Goal Progress
            </p>
            <div className="flex items-center justify-center mb-3">
              <Donut
                segments={goalSegs}
                label="Saved"
                value={fmtCompact(convertTotal(totalGoalSaved), displayCur, hidden)}
                size={130}
              />
            </div>
            <div className="mb-2">
              <div className="flex justify-between mb-1">
                <span className="text-[10px] text-muted-foreground font-light">Overall</span>
                <span className="text-[10px] text-brand font-light">{overallGoalPct}%</span>
              </div>
              <Progress value={overallGoalPct} />
            </div>
            {activeGoals.length > 0 && (
              <div className="space-y-2 pt-2 border-t border-border-subtle">
                {activeGoals.map((g, i) => {
                  const p = pct(g.currentAmount, g.targetAmount);
                  return (
                    <div key={g.id}>
                      <div className="flex justify-between mb-0.5">
                        <div className="flex items-center gap-1.5">
                          <div
                            className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                            style={{ backgroundColor: COLORS[i % COLORS.length] }}
                          />
                          <span className="text-[10px] text-muted-foreground font-light truncate max-w-[120px]">
                            {g.name}
                          </span>
                        </div>
                        <span className="text-[10px] text-muted-foreground font-light">{p}%</span>
                      </div>
                      <Progress value={p} />
                    </div>
                  );
                })}
              </div>
            )}
          </Card>

          {/* Debt Breakdown */}
          <Card compact className="p-4">
            <p className="text-[10px] text-muted-foreground uppercase tracking-widest mb-3">
              Debt Breakdown
            </p>
            <div className="flex items-center justify-center mb-3">
              <Donut
                segments={liabSegs}
                label="Remaining"
                value={fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
                size={130}
              />
            </div>
            {liabilities.length > 0 && (
              <div className="space-y-2 pt-2 border-t border-border-subtle">
                {liabilities.map((l, i) => {
                  const paid = pct(l.principal - l.remaining, l.principal);
                  return (
                    <div key={l.id}>
                      <div className="flex justify-between mb-0.5">
                        <div className="flex items-center gap-1.5">
                          <div
                            className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                            style={{ backgroundColor: COLORS[i % COLORS.length] }}
                          />
                          <span className="text-[10px] text-muted-foreground font-light truncate max-w-[120px]">
                            {l.name}
                          </span>
                        </div>
                        <span className="text-[10px] text-muted-foreground font-light">
                          {paid}%
                        </span>
                      </div>
                      <Progress value={paid} />
                    </div>
                  );
                })}
                <div className="pt-2 border-t border-border-subtle flex justify-between">
                  <span className="text-[10px] text-muted-foreground font-light">
                    Monthly Payments
                  </span>
                  <span className="text-[10px] text-brand font-light tabular-nums">
                    {fmtCompact(convertTotal(liabMonthlyTotal), displayCur, hidden)}/mo
                  </span>
                </div>
              </div>
            )}
          </Card>
        </div>
      </div>

      {/* ── Add Goal Modal ──────────────────────────────────────────── */}
      <FormModal
        open={goalModalOpen}
        onClose={() => setGoalModalOpen(false)}
        title="Add Goal"
        onSubmit={handleCreateGoal}
        canSubmit={gName.trim().length > 0 && Number(targetAmount) > 0}
      >
        <FormField label="Goal Name">
          <input
            className={fieldClass}
            value={gName}
            onChange={(e) => setGName(e.target.value)}
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
            value={gCurrency}
            onChange={(e) => setGCurrency(e.target.value)}
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

      {/* ── Add Liability Modal ─────────────────────────────────────── */}
      <FormModal
        open={liabModalOpen}
        onClose={() => setLiabModalOpen(false)}
        title="Add Debt"
        onSubmit={handleCreateLiability}
        canSubmit={lName.trim().length > 0 && Number(principal) > 0}
      >
        <FormField label="Debt Name">
          <input
            className={fieldClass}
            value={lName}
            onChange={(e) => setLName(e.target.value)}
            placeholder="e.g. Home Mortgage"
            autoFocus
          />
        </FormField>
        <FormField label="Debt Type">
          <select
            className={fieldClass}
            value={liabilityType}
            onChange={(e) => setLiabilityType(e.target.value)}
          >
            <option value="personal_loan">Personal Loan</option>
            <option value="mortgage">Mortgage</option>
            <option value="credit_card">Credit Card</option>
            <option value="student_loan">Student Loan</option>
            <option value="other">Other</option>
          </select>
        </FormField>
        <FormField label="Principal Amount">
          <input
            className={fieldClass}
            type="number"
            value={principal}
            onChange={(e) => setPrincipal(e.target.value)}
            placeholder="0"
          />
        </FormField>
        <FormField label="Currency">
          <select
            className={fieldClass}
            value={lCurrency}
            onChange={(e) => setLCurrency(e.target.value)}
          >
            <option value="VND">VND</option>
            <option value="USD">USD</option>
            <option value="USDT">USDT</option>
          </select>
        </FormField>
        <FormField label="Interest Rate % (optional)">
          <input
            className={fieldClass}
            type="number"
            step="0.1"
            value={interestRate}
            onChange={(e) => setInterestRate(e.target.value)}
            placeholder="0.0"
          />
        </FormField>
        <FormField label="Monthly Payment (optional)">
          <input
            className={fieldClass}
            type="number"
            value={monthlyPayment}
            onChange={(e) => setMonthlyPayment(e.target.value)}
            placeholder="0"
          />
        </FormField>
        <FormField label="Due Date (optional)">
          <input
            className={fieldClass}
            type="date"
            value={dueDate}
            onChange={(e) => setDueDate(e.target.value)}
          />
        </FormField>
      </FormModal>
    </FinanceLayout>
  );
}
