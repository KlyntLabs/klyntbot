import { Wallet } from "lucide-react";
import { useMemo } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import { COLORS, fmtCompact, fmtMoney, fmtVnd, LIAB_ICONS, pct, toVnd } from "../../lib/finance";
import type { FinanceLiability } from "../../lib/types";
import { Card, SectionLabel } from "../finance/Card";
import { Donut } from "../finance/Donut";
import { FinanceLayout } from "../finance/FinanceLayout";
export function FinanceLiabilities() {
  const { data: liabilities, refetch } = useQuery<FinanceLiability[]>(
    "finance_liabilities",
    undefined,
    [],
  );
  const { data: rates } = useQuery<Record<string, number>>("finance_exchange_rates", undefined, {});
  useEvent<{ entityKind: string }>("entity:updated", refetch);

  const totalRemaining = useMemo(
    () => liabilities.reduce((s, l) => s + toVnd(l.remaining, l.currency, rates), 0),
    [liabilities, rates],
  );
  const totalPrincipal = useMemo(
    () => liabilities.reduce((s, l) => s + toVnd(l.principal, l.currency, rates), 0),
    [liabilities, rates],
  );
  const totalPaid = totalPrincipal - totalRemaining;
  const monthlyTotal = useMemo(
    () => liabilities.reduce((s, l) => s + toVnd(l.monthlyPayment ?? 0, l.currency, rates), 0),
    [liabilities, rates],
  );

  const liabSegs = useMemo(
    () =>
      liabilities.map((l, i) => ({
        name: l.name,
        value: toVnd(l.remaining, l.currency, rates),
        color: COLORS[i % COLORS.length],
      })),
    [liabilities, rates],
  );

  const typeSegs = useMemo(() => {
    const m = new Map<string, number>();
    liabilities.forEach((l) => {
      const t = l.liabilityType.replace("_", " ");
      m.set(t, (m.get(t) ?? 0) + toVnd(l.remaining, l.currency, rates));
    });
    return Array.from(m.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [liabilities, rates]);

  return (
    <FinanceLayout onRefresh={refetch}>
      <div className="grid grid-cols-12 gap-3 auto-rows-min">
        {/* ── Stats row ─────────────────────────────────── */}
        <div className="col-span-12 grid grid-cols-4 gap-3">
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Total Debt
            </p>
            <p className="text-[20px] font-light text-destructive">{fmtCompact(totalRemaining)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Total Paid
            </p>
            <p className="text-[20px] font-light text-success">{fmtCompact(totalPaid)}đ</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Overall Progress
            </p>
            <p className="text-[20px] font-light text-primary">{pct(totalPaid, totalPrincipal)}%</p>
          </Card>
          <Card className="p-4">
            <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-1">
              Monthly Payments
            </p>
            <p className="text-[20px] font-light text-brand">{fmtCompact(monthlyTotal)}đ</p>
          </Card>
        </div>

        {/* ── Liability cards (8col) + Charts (4col) ───── */}
        <div className="col-span-8">
          <SectionLabel>Debts</SectionLabel>
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
                <Card key={l.id} className="p-4">
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
                        <p className="text-[13px] font-light text-secondary">{l.name}</p>
                        <p className="text-[9px] text-dim font-light">
                          {l.liabilityType.replace("_", " ")}
                        </p>
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-[14px] font-light text-destructive">
                        {fmtMoney(l.remaining, l.currency)}
                      </p>
                      <p className="text-[10px] text-dim font-light">
                        of {fmtMoney(l.principal, l.currency)}
                      </p>
                    </div>
                  </div>

                  <div className="h-2 w-full bg-surface-raised rounded-full overflow-hidden mb-2">
                    <div
                      className="h-full bg-success rounded-full transition-all"
                      style={{ width: `${paid}%` }}
                    />
                  </div>

                  <div className="grid grid-cols-4 gap-3">
                    <div>
                      <p className="text-[9px] text-dim font-light">Paid</p>
                      <p className="text-[11px] text-success font-light">
                        {fmtMoney(paidAmt, l.currency)}
                      </p>
                    </div>
                    <div>
                      <p className="text-[9px] text-dim font-light">Progress</p>
                      <p className="text-[11px] text-primary font-light">{paid}%</p>
                    </div>
                    {l.interestRate != null && (
                      <div>
                        <p className="text-[9px] text-dim font-light">Interest Rate</p>
                        <p className="text-[11px] text-muted font-light">{l.interestRate}% APR</p>
                      </div>
                    )}
                    {l.monthlyPayment && (
                      <div>
                        <p className="text-[9px] text-dim font-light">Monthly</p>
                        <p className="text-[11px] text-muted font-light">
                          {fmtMoney(l.monthlyPayment, l.currency)}
                        </p>
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-4 mt-2 pt-2 border-t border-border-subtle">
                    {l.dueDate && (
                      <span className="text-[9px] text-dim font-light">Due: {l.dueDate}</span>
                    )}
                    {monthsLeft != null && (
                      <span className="text-[9px] text-muted font-light">
                        ~{monthsLeft} months remaining
                      </span>
                    )}
                  </div>
                </Card>
              );
            })}
          </div>

          {/* ── Payoff summary ───────────────────────────── */}
          <div className="mt-3">
            <Card className="px-4 py-3 flex justify-between items-center">
              <span className="text-[11px] font-light text-muted">Total Outstanding Debt</span>
              <span className="text-[14px] font-light text-destructive">
                {fmtVnd(totalRemaining)}
              </span>
            </Card>
          </div>
        </div>

        <div className="col-span-4 space-y-3">
          <div>
            <SectionLabel>Debt Breakdown</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut
                segments={liabSegs}
                label="Remaining"
                value={`${fmtCompact(totalRemaining)}đ`}
                size={150}
              />
            </Card>
          </div>
          <div>
            <SectionLabel>By Type</SectionLabel>
            <Card className="p-4 flex items-center justify-center">
              <Donut
                segments={typeSegs}
                label="By type"
                value={`${fmtCompact(totalRemaining)}đ`}
                size={150}
              />
            </Card>
          </div>
          <div>
            <SectionLabel>Payment Schedule</SectionLabel>
            <Card className="p-4 space-y-2">
              {liabilities
                .filter((l) => l.monthlyPayment)
                .map((l) => (
                  <div key={l.id} className="flex justify-between items-center">
                    <span className="text-[10px] text-muted font-light truncate">{l.name}</span>
                    <span className="text-[10px] text-secondary font-light">
                      {fmtMoney(l.monthlyPayment!, l.currency)}/mo
                    </span>
                  </div>
                ))}
              <div className="border-t border-border-subtle pt-2 flex justify-between">
                <span className="text-[10px] text-muted font-light">Total Monthly</span>
                <span className="text-[10px] text-brand font-light">{fmtVnd(monthlyTotal)}/mo</span>
              </div>
            </Card>
          </div>
        </div>
      </div>
    </FinanceLayout>
  );
}
