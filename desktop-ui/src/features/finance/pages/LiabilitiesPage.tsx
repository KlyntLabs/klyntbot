import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { FinanceLiability, FinanceLiabilityCreateParams } from "@shared/types";
import { Progress } from "@shared/ui";
import { Plus, Wallet } from "lucide-react";
import { useMemo, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { FormField, FormModal, fieldClass } from "../components/FormModal";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { displayAmount } from "../lib/displayAmount";
import { COLORS, fmtCompact, LIAB_ICONS, pct } from "../lib/finance";

export function FinanceLiabilities() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const {
    data: liabilities,
    loading,
    error,
    refetch,
  } = useQuery<FinanceLiability[]>("finance_liabilities", undefined, []);

  useEvent<{ entityKind: string }>("entity:updated", refetch);

  const totalRemaining = useMemo(
    () => liabilities.reduce((s, l) => s + (l.baseRemaining ?? l.remaining), 0),
    [liabilities],
  );
  const totalPrincipal = useMemo(
    () => liabilities.reduce((s, l) => s + (l.basePrincipal ?? l.principal), 0),
    [liabilities],
  );
  const totalPaid = totalPrincipal - totalRemaining;
  const monthlyTotal = useMemo(
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

  const liabSegs = useMemo(
    () =>
      liabilities.map((l, i) => ({
        name: l.name,
        value: l.baseRemaining ?? l.remaining,
        color: COLORS[i % COLORS.length],
      })),
    [liabilities],
  );

  const typeSegs = useMemo(() => {
    const m = new Map<string, number>();
    for (const l of liabilities) {
      const t = l.liabilityType.replaceAll("_", " ");
      m.set(t, (m.get(t) ?? 0) + (l.baseRemaining ?? l.remaining));
    }
    return Array.from(m.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [liabilities]);

  // ── Add Liability modal ──
  const [modalOpen, setModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [liabilityType, setLiabilityType] = useState("personal_loan");
  const [principal, setPrincipal] = useState("");
  const [currency, setCurrency] = useState("VND");
  const [interestRate, setInterestRate] = useState("");
  const [monthlyPayment, setMonthlyPayment] = useState("");
  const [dueDate, setDueDate] = useState("");

  const { mutate: createLiability } = useMutation<FinanceLiability, FinanceLiabilityCreateParams>(
    "finance_liability_create",
    "params",
  );

  const handleCreate = async () => {
    const result = await createLiability({
      name,
      liabilityType,
      principal: Math.round(Number(principal) * 100),
      currency,
      interestRate: interestRate ? Number(interestRate) : undefined,
      monthlyPayment: monthlyPayment ? Math.round(Number(monthlyPayment) * 100) : undefined,
      dueDate: dueDate || undefined,
    });
    if (!result) return;
    setModalOpen(false);
    setName("");
    setPrincipal("");
    setInterestRate("");
    setMonthlyPayment("");
    setDueDate("");
    refetch();
  };

  if (loading && liabilities.length === 0) {
    return (
      <FinanceLayout
        onRefresh={refetch}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <FinanceSkeleton />
      </FinanceLayout>
    );
  }

  if (error && liabilities.length === 0) {
    return (
      <FinanceLayout
        onRefresh={refetch}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
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
    <FinanceLayout
      onRefresh={refetch}
      currencyMode={mode}
      currencies={currencies}
      onSelectCurrency={setMode}
    >
      <div className="grid grid-cols-12 gap-4 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-4">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Debt
            </p>
            <p className="text-[20px] font-light text-destructive tabular-nums">
              {fmtCompact(convertTotal(totalRemaining), displayCur)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Total Paid
            </p>
            <p className="text-[20px] font-light text-success tabular-nums">
              {fmtCompact(convertTotal(totalPaid), displayCur)}
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Overall Progress
            </p>
            <p className="text-[20px] font-light text-primary tabular-nums">
              {pct(totalPaid, totalPrincipal)}%
            </p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-medium uppercase tracking-wider mb-1">
              Monthly Payments
            </p>
            <p className="text-[20px] font-light text-brand tabular-nums">
              {fmtCompact(convertTotal(monthlyTotal), displayCur)}
            </p>
          </Card>
        </div>

        {/* ── Liability cards (8col) + Charts (4col) ───── */}
        <div className="col-span-8">
          <Card className="p-4">
            <CardHeader
              title="Debts"
              action={
                <button
                  type="button"
                  onClick={() => setModalOpen(true)}
                  className="flex items-center gap-1 text-[10px] text-brand font-light hover:text-brand-hover transition-colors"
                >
                  <Plus className="w-3 h-3" strokeWidth={1.5} /> Add Liability
                </button>
              }
            />
            {liabilities.length === 0 ? (
              <div className="py-8 text-center text-[11px] text-dim font-light">
                No liabilities tracked
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
                            <p className="text-[9px] text-dim font-light">
                              {l.liabilityType.replaceAll("_", " ")}
                            </p>
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
                            })}
                          </p>
                        </div>
                        <div>
                          <p className="text-[9px] text-dim font-light">Progress</p>
                          <p className="text-[11px] text-primary font-light tabular-nums">
                            {paid}%
                          </p>
                        </div>
                        {l.interestRate != null && (
                          <div>
                            <p className="text-[9px] text-dim font-light">Interest Rate</p>
                            <p className="text-[11px] text-muted font-light">
                              {l.interestRate}% APR
                            </p>
                          </div>
                        )}
                        {l.monthlyPayment && (
                          <div>
                            <p className="text-[9px] text-dim font-light">Monthly</p>
                            <p className="text-[11px] text-muted font-light">
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
              </div>
            )}

            {/* ── Payoff summary ───────────────────────────── */}
            {liabilities.length > 0 && (
              <div className="mt-3 glass-card px-4 py-3 flex justify-between items-center">
                <span className="text-[11px] font-light text-muted">Total Outstanding Debt</span>
                <span className="text-[14px] font-light text-destructive">
                  {fmtCompact(convertTotal(totalRemaining), displayCur)}
                </span>
              </div>
            )}
          </Card>
        </div>

        <div className="col-span-4 space-y-4">
          <Card className="p-4">
            <CardHeader title="Debt Breakdown" />
            <div className="flex items-center justify-center">
              <Donut
                segments={liabSegs}
                label="Remaining"
                value={fmtCompact(convertTotal(totalRemaining), displayCur)}
                size={150}
              />
            </div>
          </Card>
          <Card className="p-4">
            <CardHeader title="By Type" />
            <div className="flex items-center justify-center">
              <Donut
                segments={typeSegs}
                label="By type"
                value={fmtCompact(convertTotal(totalRemaining), displayCur)}
                size={150}
              />
            </div>
          </Card>
          <Card className="p-4">
            <CardHeader title="Payment Schedule" />
            <div className="space-y-2">
              {liabilities
                .filter((l) => l.monthlyPayment)
                .map((l) => (
                  <div key={l.id} className="flex justify-between items-center">
                    <span className="text-[10px] text-muted font-light truncate">{l.name}</span>
                    <span className="text-[10px] text-secondary font-light">
                      {displayAmount({
                        amount: l.monthlyPayment ?? 0,
                        currency: l.currency,
                        baseAmount:
                          l.basePrincipal != null && l.principal !== 0
                            ? Math.round((l.monthlyPayment ?? 0) * (l.basePrincipal / l.principal))
                            : undefined,
                        baseCurrency,
                        mode,
                        rates,
                      })}
                      /mo
                    </span>
                  </div>
                ))}
              <div className="border-t border-white/[0.04] pt-2 flex justify-between">
                <span className="text-[10px] text-muted font-light">Total Monthly</span>
                <span className="text-[10px] text-brand font-light">
                  {fmtCompact(convertTotal(monthlyTotal), displayCur)}/mo
                </span>
              </div>
            </div>
          </Card>
        </div>
      </div>

      {/* ── Add Liability Modal ──────────────────────── */}
      <FormModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title="Add Liability"
        onSubmit={handleCreate}
        canSubmit={name.trim().length > 0 && Number(principal) > 0}
      >
        <FormField label="Liability Name">
          <input
            className={fieldClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Home Mortgage"
            autoFocus
          />
        </FormField>
        <FormField label="Liability Type">
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
            value={currency}
            onChange={(e) => setCurrency(e.target.value)}
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
