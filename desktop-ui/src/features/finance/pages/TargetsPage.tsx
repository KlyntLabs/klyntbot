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
import { COLORS, GOAL_ICONS, LIAB_ICONS, fmtCompact, pct } from "../lib/finance";

type GoalTab = "active" | "achieved" | "abandoned";

export function FinanceTargets() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const { hidden, toggle } = usePrivacyMode();

  const { data: goals, loading: goalsLoading, refetch: rG } = useQuery<FinanceGoal[]>(
    "finance_goals",
    undefined,
    [],
  );
  const { data: liabilities, loading: liabLoading, refetch: rL } = useQuery<FinanceLiability[]>(
    "finance_liabilities",
    undefined,
    [],
  );

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
        <Card className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Active Goals
          </p>
          <p className="text-[24px] font-light text-primary">{activeGoals.length}</p>
        </Card>
        <Card className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Goal Progress
          </p>
          <p className="text-[24px] font-light text-brand tabular-nums">{overallGoalPct}%</p>
        </Card>
        <Card className="p-4">
          <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
            Total Debt
          </p>
          <p className="text-[24px] font-light text-destructive tabular-nums">
            {fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
          </p>
        </Card>
        <Card className="p-4">
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
                      ? "bg-white/[0.12] text-brand"
                      : "text-muted hover:text-secondary hover:bg-white/[0.06]",
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
                            <span
                              className="inline-block text-[9px] font-light px-1.5 py-0.5 rounded-full"
                              style={{
                                backgroundColor: `${COLORS[i % COLORS.length]}18`,
                                color: COLORS[i % COLORS.length],
                              }}
                            >
                              {g.goalType}
                            </span>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-[14px] font-light text-primary tabular-nums">
                            {displayAmount({
                              amount: g.currentAmount,
                              currency: g.currency,
                              baseAmount: g.baseCurrentAmount,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                            })}{" "}
                            <span className="text-dim text-[10px]">
                              /{" "}
                              {displayAmount({
                                amount: g.targetAmount,
                                currency: g.currency,
                                baseAmount: g.baseTargetAmount,
                                baseCurrency,
                                mode,
                                rates,
                              })}
                            </span>
                          </p>
                          <p className="text-[11px] font-light text-brand tabular-nums">{p}%</p>
                        </div>
                      </div>
                      <Progress value={p} />
                      <div className="flex items-center gap-4 mt-2 flex-wrap">
                        {g.deadline && (
                          <span className="text-[10px] text-dim font-light">
                            Deadline: {g.deadline}
                          </span>
                        )}
                        {g.monthlyContribution && (
                          <span className="text-[10px] text-dim font-light">
                            Contributing:{" "}
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
                            })}
                            /mo
                          </span>
                        )}
                        {monthsLeft != null && (
                          <span className="text-[10px] text-muted font-light">
                            ~{monthsLeft} months to go
                          </span>
                        )}
                        <span className="text-[10px] text-dim font-light">
                          {displayAmount({
                            amount: remaining,
                            currency: g.currency,
                            baseAmount:
                              g.baseTargetAmount != null && g.baseCurrentAmount != null
                                ? g.baseTargetAmount - g.baseCurrentAmount
                                : undefined,
                            baseCurrency,
                            mode,
                            rates,
                          })}{" "}
                          remaining
                        </span>
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
              <div className="space-y-3">
                {liabilities.map((l, i) => {
                  const Icon = LIAB_ICONS[l.liabilityType] ?? Wallet;
                  const paid = pct(l.principal - l.remaining, l.principal);
                  const paidAmt = l.principal - l.remaining;
                  const monthsLeft =
                    l.monthlyPayment && l.monthlyPayment > 0
                      ? Math.ceil(l.remaining / l.monthlyPayment)
                      : null;
                  return (
                    <div key={l.id} className="glass-card p-4">
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
                            <p className="text-[13px] font-medium text-secondary">{l.name}</p>
                            <span
                              className="inline-block text-[9px] font-light px-1.5 py-0.5 rounded-full"
                              style={{
                                backgroundColor: `${COLORS[i % COLORS.length]}18`,
                                color: COLORS[i % COLORS.length],
                              }}
                            >
                              {l.liabilityType.replaceAll("_", " ")}
                            </span>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className="text-[14px] font-light text-destructive tabular-nums">
                            {displayAmount({
                              amount: l.remaining,
                              currency: l.currency,
                              baseAmount: l.baseRemaining,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                            })}
                          </p>
                          <p className="text-[10px] text-dim font-light">
                            of{" "}
                            {displayAmount({
                              amount: l.principal,
                              currency: l.currency,
                              baseAmount: l.basePrincipal,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                            })}
                          </p>
                        </div>
                      </div>

                      <Progress value={paid} />

                      <div className="grid grid-cols-4 gap-3 mt-2">
                        <div>
                          <p className="text-[9px] text-dim font-light">Paid</p>
                          <p className="text-[11px] text-success font-light tabular-nums">
                            {displayAmount({
                              amount: paidAmt,
                              currency: l.currency,
                              baseAmount:
                                l.basePrincipal != null && l.baseRemaining != null
                                  ? l.basePrincipal - l.baseRemaining
                                  : undefined,
                              baseCurrency,
                              mode,
                              rates,
                              hidden,
                            })}
                          </p>
                        </div>
                        <div>
                          <p className="text-[9px] text-dim font-light">Progress</p>
                          <p className="text-[11px] text-primary font-light tabular-nums">{paid}%</p>
                        </div>
                        {l.interestRate != null && (
                          <div>
                            <p className="text-[9px] text-dim font-light">APR</p>
                            <p className="text-[11px] text-muted font-light">{l.interestRate}%</p>
                          </div>
                        )}
                        {l.monthlyPayment && (
                          <div>
                            <p className="text-[9px] text-dim font-light">Monthly</p>
                            <p className="text-[11px] text-muted font-light tabular-nums">
                              {displayAmount({
                                amount: l.monthlyPayment,
                                currency: l.currency,
                                baseAmount:
                                  l.basePrincipal != null && l.principal !== 0
                                    ? Math.round(
                                        l.monthlyPayment * (l.basePrincipal / l.principal),
                                      )
                                    : undefined,
                                baseCurrency,
                                mode,
                                rates,
                                hidden,
                              })}
                            </p>
                          </div>
                        )}
                      </div>

                      <div className="flex items-center gap-4 mt-2 pt-2 border-t border-white/[0.04]">
                        {l.dueDate && (
                          <span className="text-[9px] text-dim font-light">Due: {l.dueDate}</span>
                        )}
                        {monthsLeft != null && (
                          <span className="text-[9px] text-muted font-light">
                            ~{monthsLeft} months remaining
                          </span>
                        )}
                      </div>
                    </div>
                  );
                })}

                {/* Total outstanding summary */}
                <div className="glass-card px-4 py-3 flex justify-between items-center">
                  <span className="text-[11px] font-light text-muted">Total Outstanding Debt</span>
                  <span className="text-[14px] font-light text-destructive tabular-nums">
                    {fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
                  </span>
                </div>
              </div>
            )}
          </Card>
        </div>

        {/* ── Right sidebar ────────────────────────────────────────── */}
        <div className="w-72 flex-shrink-0 sticky top-0 self-start space-y-4">
          {/* Goal Progress overview */}
          <Card className="p-4">
            <p className="text-[10px] text-muted uppercase tracking-widest mb-3">Goal Progress</p>
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
                <span className="text-[10px] text-muted font-light">Overall</span>
                <span className="text-[10px] text-brand font-light">{overallGoalPct}%</span>
              </div>
              <Progress value={overallGoalPct} />
            </div>
            {activeGoals.length > 0 && (
              <div className="space-y-2 pt-2 border-t border-white/[0.04]">
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
                          <span className="text-[10px] text-muted font-light truncate max-w-[120px]">
                            {g.name}
                          </span>
                        </div>
                        <span className="text-[10px] text-secondary font-light">{p}%</span>
                      </div>
                      <Progress value={p} />
                    </div>
                  );
                })}
              </div>
            )}
          </Card>

          {/* Debt Breakdown */}
          <Card className="p-4">
            <p className="text-[10px] text-muted uppercase tracking-widest mb-3">Debt Breakdown</p>
            <div className="flex items-center justify-center mb-3">
              <Donut
                segments={liabSegs}
                label="Remaining"
                value={fmtCompact(convertTotal(totalRemaining), displayCur, hidden)}
                size={130}
              />
            </div>
            {liabilities.length > 0 && (
              <div className="space-y-2 pt-2 border-t border-white/[0.04]">
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
                          <span className="text-[10px] text-muted font-light truncate max-w-[120px]">
                            {l.name}
                          </span>
                        </div>
                        <span className="text-[10px] text-secondary font-light">{paid}%</span>
                      </div>
                      <Progress value={paid} />
                    </div>
                  );
                })}
                <div className="pt-2 border-t border-white/[0.04] flex justify-between">
                  <span className="text-[10px] text-muted font-light">Monthly Payments</span>
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
