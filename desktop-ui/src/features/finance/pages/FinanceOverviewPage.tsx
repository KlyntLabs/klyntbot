import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/utils";
import type {
  FinanceAccount,
  FinanceBudgetUsage,
  FinanceCategoryReport,
  FinanceGoal,
  FinanceInvestment,
  FinanceLiability,
  FinanceNetWorth,
  FinancePortfolio,
  FinanceTransaction,
} from "@shared/types";
import { Progress } from "@shared/ui";
import {
  ArrowDownRight,
  ArrowLeftRight,
  ArrowUpRight,
  Target,
  TrendingDown,
  TrendingUp,
  Wallet,
} from "lucide-react";
import { useCallback, useMemo } from "react";
import { useNavigate } from "react-router";
import { Card, CardHeader } from "../components/Card";
import { Donut } from "../components/Donut";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import {
  ACCT_ICONS,
  COLORS,
  fmtCompact,
  fmtMoney,
  GOAL_ICONS,
  LIAB_ICONS,
  pct,
  retPct,
} from "../lib/finance";

export function Finance() {
  const navigate = useNavigate();

  const {
    data: accounts,
    loading,
    error,
    refetch: rA,
  } = useQuery<FinanceAccount[]>("finance_accounts", undefined, []);
  const { data: transactions, refetch: rT } = useQuery<FinanceTransaction[]>(
    "finance_transactions",
    { limit: 8 },
    [],
  );
  const { data: budgets, refetch: rB } = useQuery<FinanceBudgetUsage[]>(
    "finance_budget_usage",
    undefined,
    [],
  );
  const { data: portfolios, refetch: rP } = useQuery<FinancePortfolio[]>(
    "finance_portfolios",
    undefined,
    [],
  );
  const { data: investments, refetch: rI } = useQuery<FinanceInvestment[]>(
    "finance_investments",
    undefined,
    [],
  );
  const { data: goals, refetch: rG } = useQuery<FinanceGoal[]>("finance_goals", undefined, []);
  const { data: liabilities, refetch: rL } = useQuery<FinanceLiability[]>(
    "finance_liabilities",
    undefined,
    [],
  );
  const { data: netWorth, refetch: rN } = useQuery<FinanceNetWorth>(
    "finance_net_worth",
    undefined,
    { totalsByCurrency: [] },
  );
  const { data: settings } = useQuery<{ defaultCurrency: string }>(
    "finance_settings",
    undefined,
    {},
  );
  const baseCurrency = settings?.defaultCurrency ?? "USD";
  const { data: spendingReport } = useQuery<FinanceCategoryReport>(
    "finance_report_spending",
    undefined,
    { total: 0, breakdown: [] },
  );

  const refetchAll = useCallback(() => {
    rA();
    rT();
    rB();
    rP();
    rI();
    rG();
    rL();
    rN();
  }, [rA, rT, rB, rP, rI, rG, rL, rN]);
  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  const totalNet = useMemo(
    () =>
      accounts.reduce((s, a) => s + (a.baseBalance ?? 0), 0) +
      investments.reduce((s, i) => s + (i.baseCurrentValue ?? 0), 0) -
      liabilities.reduce((s, l) => s + (l.baseRemaining ?? 0), 0),
    [accounts, investments, liabilities],
  );
  const totalAssets = useMemo(
    () =>
      accounts.reduce((s, a) => s + (a.baseBalance ?? 0), 0) +
      investments.reduce((s, i) => s + (i.baseCurrentValue ?? 0), 0),
    [accounts, investments],
  );
  const totalDebt = useMemo(
    () => liabilities.reduce((s, l) => s + (l.baseRemaining ?? 0), 0),
    [liabilities],
  );

  // Compute spending from transactions using pre-computed base amounts
  const totalSpend = useMemo(
    () =>
      transactions
        .filter((t) => t.txType === "expense")
        .reduce((s, t) => s + (t.baseAmount ?? 0), 0),
    [transactions],
  );

  const spendingSegs = useMemo(() => {
    const m = new Map<string, number>();
    for (const t of transactions.filter((t) => t.txType === "expense")) {
      const base = t.baseAmount ?? 0;
      if (base > 0) m.set(t.category ?? "other", (m.get(t.category ?? "other") ?? 0) + base);
    }
    return [...m.entries()].map(([name, value], i) => ({
      name,
      value,
      color: COLORS[i % COLORS.length],
    }));
  }, [transactions]);

  const totalIncome = useMemo(
    () =>
      transactions
        .filter((t) => t.txType === "income")
        .reduce((s, t) => s + (t.baseAmount ?? 0), 0),
    [transactions],
  );

  const investSegs = useMemo(() => {
    const m = new Map<string, number>();
    for (const i of investments) {
      m.set(i.assetType, (m.get(i.assetType) ?? 0) + (i.baseCurrentValue ?? 0));
    }
    return Array.from(m.entries())
      .sort((a, b) => b[1] - a[1])
      .map(([name, value], i) => ({ name, value, color: COLORS[i % COLORS.length] }));
  }, [investments]);
  const totalInvest = useMemo(() => investSegs.reduce((s, a) => s + a.value, 0), [investSegs]);

  const accountMap = useMemo(() => new Map(accounts.map((a) => [a.id, a])), [accounts]);
  const active = useMemo(() => accounts.filter((a) => !a.isArchived), [accounts]);

  if (loading && accounts.length === 0) {
    return (
      <FinanceLayout onRefresh={refetchAll}>
        <FinanceSkeleton rows={6} />
      </FinanceLayout>
    );
  }

  if (error && accounts.length === 0) {
    return (
      <FinanceLayout onRefresh={refetchAll}>
        <Card className="p-6 text-center">
          <p className="text-[12px] text-destructive mb-2">{error.message}</p>
          <button
            type="button"
            onClick={refetchAll}
            className="text-[11px] text-brand hover:text-brand-hover transition-colors"
          >
            Retry
          </button>
        </Card>
      </FinanceLayout>
    );
  }

  return (
    <FinanceLayout onRefresh={refetchAll}>
      <div className="grid grid-cols-12 gap-4 auto-rows-min">
        {/* ── ROW 1: Net Worth (3col) + Accounts (9col) ────── */}
        <div className="col-span-3">
          <Card className="p-4">
            <CardHeader title="Net Worth" />
            <p className="text-[28px] font-light text-primary tracking-tight leading-none mb-3 tabular-nums">
              {fmtCompact(totalNet, baseCurrency)}
            </p>
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <TrendingUp className="w-3 h-3 text-success" strokeWidth={1.5} />
                  <span className="text-[10px] text-muted font-light">Assets</span>
                </div>
                <span className="text-[10px] text-success font-light tabular-nums">
                  {fmtCompact(totalAssets, baseCurrency)}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <TrendingDown className="w-3 h-3 text-destructive" strokeWidth={1.5} />
                  <span className="text-[10px] text-muted font-light">Debt</span>
                </div>
                <span className="text-[10px] text-destructive font-light tabular-nums">
                  {fmtCompact(totalDebt, baseCurrency)}
                </span>
              </div>
            </div>
            <div className="flex gap-3 mt-3 pt-2.5 border-t border-white/[0.04]">
              {netWorth.totalsByCurrency.map((c) => (
                <span key={c.currency} className="text-[9px] font-light">
                  <span className="text-dim">{c.currency}</span>{" "}
                  <span className="text-secondary">{fmtMoney(c.net, c.currency)}</span>
                </span>
              ))}
            </div>
          </Card>
        </div>

        <div className="col-span-9">
          <Card className="p-4">
            <CardHeader title="Accounts" />
            <div className="grid grid-cols-5 gap-3">
              {active.slice(0, 5).map((acct) => {
                const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
                return (
                  <div
                    key={acct.id}
                    className="glass-card p-3.5 cursor-pointer hover:bg-white/[0.06] transition-colors"
                    onClick={() => navigate(`/finance/accounts?id=${acct.id}`)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") navigate(`/finance/accounts?id=${acct.id}`);
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="flex items-center gap-2 mb-2.5">
                      <div className="w-7 h-7 rounded-lg bg-white/[0.08] flex items-center justify-center flex-shrink-0">
                        <Icon className="w-3.5 h-3.5 text-muted" strokeWidth={1.5} />
                      </div>
                      <div className="min-w-0">
                        <p className="text-[11px] font-medium text-secondary truncate">
                          {acct.name}
                        </p>
                        <p className="text-[9px] text-dim font-light">
                          {acct.accountType.replaceAll("_", " ")}
                        </p>
                      </div>
                    </div>
                    <p className="text-[14px] font-light text-primary tabular-nums">
                      {fmtMoney(acct.balance, acct.currency)}
                    </p>
                    {acct.currency !== baseCurrency && acct.baseBalance != null && (
                      <p className="text-[9px] text-dim font-light mt-0.5">
                        ≈ {fmtCompact(acct.baseBalance, baseCurrency)}
                      </p>
                    )}
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        {/* ── ROW 2: Spending table (9col) + Donut (3col) ──── */}
        <div className="col-span-9 flex flex-col">
          <Card className="overflow-hidden flex-1">
            <div className="px-4 pt-4">
              <CardHeader title="Spending by Category" />
            </div>
            <div className="grid grid-cols-[1fr_100px_100px_100px_50px] gap-3 border-b border-white/[0.08] text-[10px] text-dim font-light px-4 py-2">
              <div>Category</div>
              <div className="text-right">Budget</div>
              <div className="text-right">Spent</div>
              <div className="text-right">Remaining</div>
              <div className="text-right">%</div>
            </div>
            {budgets
              .filter((b) => b.isActive)
              .map((b, i) => {
                const p = pct(b.spent, b.amount);
                const rem = b.amount - b.spent;
                return (
                  <div
                    key={b.id}
                    className="grid grid-cols-[1fr_100px_100px_100px_50px] gap-3 px-4 py-2.5 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                  >
                    <div className="flex items-center gap-2">
                      <div
                        className="w-2 h-2 rounded-full flex-shrink-0"
                        style={{ backgroundColor: COLORS[i % COLORS.length] }}
                      />
                      <span className="text-[12px] font-medium text-secondary">{b.name}</span>
                    </div>
                    <div className="text-right text-[12px] font-light text-muted tabular-nums">
                      {fmtMoney(b.amount, b.currency)}
                    </div>
                    <div className="text-right text-[12px] font-light text-primary tabular-nums">
                      {fmtMoney(b.spent, b.currency)}
                    </div>
                    <div
                      className={cn(
                        "text-right text-[12px] font-light tabular-nums",
                        rem < 0 ? "text-destructive" : "text-success",
                      )}
                    >
                      {rem < 0 ? "-" : ""}
                      {fmtMoney(Math.abs(rem), b.currency)}
                    </div>
                    <div
                      className={cn(
                        "text-right text-[11px] font-light tabular-nums",
                        p >= b.alertThreshold
                          ? "text-destructive"
                          : p >= 60
                            ? "text-brand"
                            : "text-success",
                      )}
                    >
                      {p}%
                    </div>
                  </div>
                );
              })}
          </Card>
        </div>

        <div className="col-span-3">
          <Card className="p-4 flex items-center justify-center h-full">
            <Donut
              segments={spendingSegs}
              label="Total spending"
              value={fmtCompact(totalSpend, baseCurrency)}
            />
          </Card>
        </div>

        {/* ── ROW 3: Transactions (9col) + Summary+Invest (3col) */}
        <div className="col-span-9 flex flex-col">
          <Card className="overflow-hidden flex-1">
            <div className="px-4 pt-4">
              <CardHeader title="Transactions" />
            </div>
            {transactions.map((tx) => {
              const acct = accountMap.get(tx.accountId);
              const TxI =
                tx.txType === "income"
                  ? ArrowDownRight
                  : tx.txType === "expense"
                    ? ArrowUpRight
                    : ArrowLeftRight;
              const col =
                tx.txType === "income"
                  ? "text-success"
                  : tx.txType === "expense"
                    ? "text-destructive"
                    : "text-info";
              const pre = tx.txType === "income" ? "+" : tx.txType === "expense" ? "-" : "";
              return (
                <div
                  key={tx.id}
                  className="flex items-center gap-3 px-4 py-2 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                >
                  <span className="text-[10px] text-dim font-light w-10 flex-shrink-0 tabular-nums">
                    {tx.txDate.slice(5)}
                  </span>
                  <TxI className={cn("w-3 h-3 flex-shrink-0", col)} strokeWidth={1.5} />
                  <span className="text-[12px] font-light text-secondary truncate flex-1">
                    {tx.counterparty ?? tx.notes ?? tx.txType}
                  </span>
                  {tx.category && (
                    <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                      {tx.category}
                    </span>
                  )}
                  {acct && <span className="text-[10px] text-dim font-light">{acct.name}</span>}
                  <span
                    className={cn(
                      "text-[12px] font-light w-24 text-right flex-shrink-0 tabular-nums",
                      col,
                    )}
                  >
                    {pre}
                    {fmtMoney(tx.amount, tx.currency)}
                  </span>
                </div>
              );
            })}
          </Card>
        </div>

        <div className="col-span-3 space-y-4">
          <Card className="p-4">
            <CardHeader title="Summary" />
            <div className="space-y-2">
              <div className="flex justify-between">
                <span className="text-[10px] text-muted font-light">Transactions</span>
                <span className="text-[10px] text-primary font-light">{transactions.length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-[10px] text-muted font-light">Income</span>
                <span className="text-[10px] text-success font-light">
                  {fmtCompact(totalIncome, baseCurrency)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-[10px] text-muted font-light">Spending</span>
                <span className="text-[10px] text-destructive font-light">
                  {fmtCompact(totalSpend, baseCurrency)}
                </span>
              </div>
              <div className="border-t border-white/[0.04] pt-2 flex justify-between">
                <span className="text-[10px] text-muted font-light">Savings rate</span>
                <span className="text-[10px] text-brand font-light tabular-nums">
                  {totalIncome > 0 ? pct(totalIncome - totalSpend, totalIncome) : 0}%
                </span>
              </div>
            </div>
          </Card>

          <Card className="p-4">
            <CardHeader title="Investments" />
            <Donut
              segments={investSegs}
              label="Portfolio"
              value={fmtCompact(totalInvest, baseCurrency)}
              size={130}
            />
            <div className="mt-3 pt-2.5 border-t border-white/[0.04] space-y-1.5">
              {portfolios.map((p) => {
                const r = retPct(p.totalValue, p.totalCostBasis);
                return (
                  <div key={p.id} className="flex justify-between">
                    <span className="text-[10px] text-muted font-light truncate">{p.name}</span>
                    <span
                      className={cn(
                        "text-[10px] font-light",
                        r >= 0 ? "text-success" : "text-destructive",
                      )}
                    >
                      {r >= 0 ? "+" : ""}
                      {r}%
                    </span>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        {/* ── ROW 4: Goals (6col) + Liabilities (6col) ─────── */}
        <div className="col-span-6">
          <Card className="overflow-hidden">
            <div className="px-4 pt-4">
              <CardHeader title="Goals" />
            </div>
            <div className="divide-y divide-border-subtle">
              {goals.map((g) => {
                const p = pct(g.currentAmount, g.targetAmount);
                const Icon = GOAL_ICONS[g.goalType] ?? Target;
                return (
                  <div key={g.id} className="px-4 py-3 hover:bg-white/[0.06] transition-colors">
                    <div className="flex items-center justify-between mb-1.5">
                      <div className="flex items-center gap-2">
                        <Icon
                          className={cn(
                            "w-3.5 h-3.5",
                            g.goalType === "fire" ? "text-brand" : "text-muted",
                          )}
                          strokeWidth={1.5}
                        />
                        <span className="text-[12px] font-medium text-secondary">{g.name}</span>
                        <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                          {g.goalType}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-[10px] text-dim font-light">
                          {fmtCompact(g.currentAmount, g.currency)} /{" "}
                          {fmtCompact(g.targetAmount, g.currency)}
                        </span>
                        <span className="text-[10px] text-brand font-light">{p}%</span>
                      </div>
                    </div>
                    <Progress value={p} />
                    <div className="flex gap-3 mt-1">
                      {g.deadline && (
                        <span className="text-[9px] text-dim font-light">Due {g.deadline}</span>
                      )}
                      {g.monthlyContribution && (
                        <span className="text-[9px] text-dim font-light">
                          {fmtCompact(g.monthlyContribution, g.currency)}/mo
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        <div className="col-span-6">
          <Card className="overflow-hidden">
            <div className="px-4 pt-4">
              <CardHeader title="Liabilities" />
            </div>
            {liabilities.map((l) => {
              const Icon = LIAB_ICONS[l.liabilityType] ?? Wallet;
              const paid = pct(l.principal - l.remaining, l.principal);
              return (
                <div
                  key={l.id}
                  className="px-4 py-3 hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0"
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <div className="flex items-center gap-2">
                      <Icon className="w-3.5 h-3.5 text-destructive/60" strokeWidth={1.5} />
                      <span className="text-[12px] font-medium text-secondary">{l.name}</span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-white/[0.06] text-dim">
                        {l.liabilityType.replaceAll("_", " ")}
                      </span>
                    </div>
                    <span className="text-[12px] font-light text-destructive tabular-nums">
                      {fmtMoney(l.remaining, l.currency)}
                    </span>
                  </div>
                  <div className="h-1.5 w-full bg-white/[0.08] rounded-full overflow-hidden">
                    <div className="h-full bg-success rounded-full" style={{ width: `${paid}%` }} />
                  </div>
                  <div className="flex gap-3 mt-1">
                    <span className="text-[9px] text-dim font-light">{paid}% paid</span>
                    {l.interestRate != null && (
                      <span className="text-[9px] text-dim font-light">{l.interestRate}% APR</span>
                    )}
                    {l.monthlyPayment && (
                      <span className="text-[9px] text-dim font-light">
                        {fmtMoney(l.monthlyPayment, l.currency)}/mo
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
            <div className="px-4 py-2.5 border-t border-white/[0.08] bg-white/[0.02] flex justify-between">
              <span className="text-[10px] font-light text-muted">Total Debt</span>
              <span className="text-[10px] font-light text-destructive">
                {fmtCompact(totalDebt, baseCurrency)}
              </span>
            </div>
          </Card>
        </div>
      </div>
      <div className="h-3" />
    </FinanceLayout>
  );
}
