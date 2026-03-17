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
  FinanceMonthlySummary,
  FinanceNetWorth,
  FinancePortfolio,
  FinanceTransaction,
} from "@shared/types";
import { Progress } from "@shared/ui";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Target, Wallet } from "lucide-react";
import { useCallback, useMemo } from "react";
import { Link } from "react-router";
import { BudgetStrip } from "../components/BudgetStrip";
import { Card } from "../components/Card";
import { FinanceLayout } from "../components/FinanceLayout";
import { FinanceSkeleton } from "../components/FinanceSkeleton";
import { HealthScoreRing } from "../components/HealthScoreRing";
import { MonthlyPulse } from "../components/MonthlyPulse";
import { NetWorthCard } from "../components/NetWorthCard";
import { SensitiveDivider } from "../components/SensitiveDivider";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { usePrivacyMode } from "../hooks/usePrivacyMode";
import { displayAmount, displayHint } from "../lib/displayAmount";
import {
  ACCT_ICONS,
  COLORS,
  fmtCompact,
  GOAL_ICONS,
  LIAB_ICONS,
  pct,
  retPct,
} from "../lib/finance";
import { computeHealthScore } from "../lib/healthScore";
import { computeMonthlyPulse } from "../lib/monthlyPulse";

export function Finance() {
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();
  const { hidden, toggle } = usePrivacyMode();

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
  const { data: _spendingReport } = useQuery<FinanceCategoryReport>(
    "finance_report_spending",
    undefined,
    { total: 0, breakdown: [] },
  );
  const { data: monthlySummary } = useQuery<FinanceMonthlySummary>(
    "finance_monthly_summary",
    undefined,
    { currentIncome: 0, currentSpending: 0, previousIncome: 0, previousSpending: 0 },
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

  const totalInvest = useMemo(
    () => investments.reduce((s, i) => s + (i.baseCurrentValue ?? 0), 0),
    [investments],
  );

  const accountMap = useMemo(() => new Map(accounts.map((a) => [a.id, a])), [accounts]);
  const active = useMemo(() => accounts.filter((a) => !a.isArchived), [accounts]);

  const healthScore = useMemo(
    () =>
      computeHealthScore({
        totalIncome: monthlySummary.currentIncome,
        totalSpending: monthlySummary.currentSpending,
        totalAssets,
        totalDebt,
        budgets: budgets
          .filter((b) => b.isActive)
          .map((b) => ({ spent: b.spent, amount: b.amount })),
        goals: goals.map((g) => ({ currentAmount: g.currentAmount, targetAmount: g.targetAmount })),
      }),
    [monthlySummary, accounts, investments, liabilities, budgets, goals],
  );

  const pulseRows = useMemo(() => computeMonthlyPulse(monthlySummary), [monthlySummary]);

  if (loading && accounts.length === 0) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
        <FinanceSkeleton rows={6} />
      </FinanceLayout>
    );
  }

  if (error && accounts.length === 0) {
    return (
      <FinanceLayout
        hidden={hidden}
        onTogglePrivacy={toggle}
        currencyMode={mode}
        currencies={currencies}
        onSelectCurrency={setMode}
      >
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
    <FinanceLayout
      hidden={hidden}
      onTogglePrivacy={toggle}
      currencyMode={mode}
      currencies={currencies}
      onSelectCurrency={setMode}
    >
      {/* ── SAFE ZONE ─────────────────────────────────────── */}

      {/* Hero row: Health Score + Monthly Pulse */}
      <div className="grid grid-cols-2 gap-4 mb-4">
        <Card className="p-5">
          <HealthScoreRing health={healthScore} />
        </Card>
        <Card className="p-5">
          <MonthlyPulse rows={pulseRows} />
        </Card>
      </div>

      {/* Budget Strip */}
      <div className="mb-2">
        <BudgetStrip budgets={budgets} />
      </div>

      {/* ── SENSITIVE DIVIDER ─────────────────────────────── */}
      <SensitiveDivider />

      {/* ── SENSITIVE ZONE ────────────────────────────────── */}

      {/* Net Worth — full width */}
      <div className="mb-4">
        <NetWorthCard
          totalNet={totalNet}
          totalAssets={totalAssets}
          totalInvest={totalInvest}
          totalDebt={totalDebt}
          displayCur={displayCur}
          convertTotal={convertTotal}
          hidden={hidden}
        />
        {mode === "multi" && netWorth.totalsByCurrency.length > 0 && (
          <div className="flex gap-3 mt-2 px-1">
            {netWorth.totalsByCurrency.map((c) => (
              <span key={c.currency} className="text-[9px] font-light">
                <span className="text-dim">{c.currency}</span>{" "}
                <span className="text-muted-foreground">
                  {hidden ? "•••••" : fmtCompact(c.net, c.currency)}
                </span>
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Two-zone grid */}
      <div className="grid grid-cols-2 gap-4 mb-4">
        {/* Left: Accounts + Goals */}
        <div className="space-y-4">
          {/* Accounts */}
          <Card className="overflow-hidden">
            <div className="px-4 pt-4 pb-1">
              <div className="flex items-center justify-between mb-2.5 px-1">
                <span className="text-[10px] text-muted-foreground uppercase tracking-widest">Accounts</span>
                <Link
                  to="/finance/cashflow"
                  className="text-[10px] text-brand normal-case tracking-normal"
                >
                  View all →
                </Link>
              </div>
            </div>
            <div className="divide-y divide-border-subtle">
              {active.slice(0, 6).map((acct) => {
                const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
                const hint = displayHint({
                  amount: acct.balance,
                  currency: acct.currency,
                  baseAmount: acct.baseBalance,
                  baseCurrency,
                  mode,
                  rates,
                  hidden,
                });
                return (
                  <div
                    key={acct.id}
                    className="flex items-center gap-3 px-4 py-2.5 hover:bg-accent transition-colors"
                  >
                    <div className="w-7 h-7 rounded-lg bg-muted flex items-center justify-center flex-shrink-0">
                      <Icon className="w-3.5 h-3.5 text-muted-foreground" strokeWidth={1.5} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-[11px] font-medium text-muted-foreground truncate">{acct.name}</p>
                      <p className="text-[9px] text-dim font-light">
                        {acct.accountType.replaceAll("_", " ")}
                      </p>
                    </div>
                    <div className="text-right flex-shrink-0">
                      <p className="text-[12px] font-light text-foreground tabular-nums">
                        {displayAmount({
                          amount: acct.balance,
                          currency: acct.currency,
                          baseAmount: acct.baseBalance,
                          baseCurrency,
                          mode,
                          rates,
                          hidden,
                        })}
                      </p>
                      {hint && <p className="text-[9px] text-dim font-light mt-0.5">≈ {hint}</p>}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>

          {/* Goals */}
          <Card className="overflow-hidden">
            <div className="px-4 pt-4 pb-1">
              <div className="flex items-center justify-between mb-2.5 px-1">
                <span className="text-[10px] text-muted-foreground uppercase tracking-widest">Goals</span>
                <Link
                  to="/finance/targets"
                  className="text-[10px] text-brand normal-case tracking-normal"
                >
                  View all →
                </Link>
              </div>
            </div>
            <div className="divide-y divide-border-subtle">
              {goals.map((g) => {
                const p = pct(g.currentAmount, g.targetAmount);
                const Icon = GOAL_ICONS[g.goalType] ?? Target;
                return (
                  <div key={g.id} className="px-4 py-3 hover:bg-accent transition-colors">
                    <div className="flex items-center gap-2 mb-1.5">
                      <Icon
                        className={cn(
                          "w-3.5 h-3.5 flex-shrink-0",
                          g.goalType === "fire" ? "text-brand" : "text-muted-foreground",
                        )}
                        strokeWidth={1.5}
                      />
                      <span className="text-[11px] font-medium text-muted-foreground truncate flex-1">
                        {g.name}
                      </span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-accent text-dim flex-shrink-0">
                        {g.goalType}
                      </span>
                      <span className="text-[10px] text-brand font-light flex-shrink-0">{p}%</span>
                    </div>
                    <Progress value={p} />
                    <div className="flex gap-2 mt-1">
                      <span className="text-[9px] text-dim font-light">
                        {displayAmount({
                          amount: g.currentAmount,
                          currency: g.currency,
                          baseAmount: g.baseCurrentAmount,
                          baseCurrency,
                          mode,
                          rates,
                          compact: true,
                          hidden,
                        })}{" "}
                        /{" "}
                        {displayAmount({
                          amount: g.targetAmount,
                          currency: g.currency,
                          baseAmount: g.baseTargetAmount,
                          baseCurrency,
                          mode,
                          rates,
                          compact: true,
                          hidden,
                        })}
                      </span>
                      {g.deadline && (
                        <span className="text-[9px] text-dim font-light">· Due {g.deadline}</span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        {/* Right: Investments + Liabilities */}
        <div className="space-y-4">
          {/* Investments */}
          <Card className="overflow-hidden">
            <div className="px-4 pt-4 pb-1">
              <div className="flex items-center justify-between mb-2.5 px-1">
                <span className="text-[10px] text-muted-foreground uppercase tracking-widest">
                  Investments
                </span>
                <Link
                  to="/finance/investments"
                  className="text-[10px] text-brand normal-case tracking-normal"
                >
                  View all →
                </Link>
              </div>
            </div>
            <div className="px-4 pb-3">
              <p className="text-[28px] font-light text-foreground tracking-tight leading-none tabular-nums mb-0.5">
                {fmtCompact(convertTotal(totalInvest), displayCur, hidden)}
              </p>
              <p className="text-[9px] text-dim font-light">Total portfolio value</p>
            </div>
            {portfolios.length > 0 && (
              <div className="divide-y divide-border-subtle">
                {portfolios.map((p) => {
                  const r = retPct(p.totalValue, p.totalCostBasis);
                  return (
                    <div
                      key={p.id}
                      className="flex items-center justify-between px-4 py-2.5 hover:bg-accent transition-colors"
                    >
                      <span className="text-[11px] text-muted-foreground truncate">{p.name}</span>
                      <span
                        className={cn(
                          "text-[11px] font-light flex-shrink-0 ml-2",
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
            )}
          </Card>

          {/* Liabilities */}
          <Card className="overflow-hidden">
            <div className="px-4 pt-4 pb-1">
              <div className="flex items-center justify-between mb-2.5 px-1">
                <span className="text-[10px] text-muted-foreground uppercase tracking-widest">
                  Liabilities
                </span>
                <Link
                  to="/finance/targets"
                  className="text-[10px] text-brand normal-case tracking-normal"
                >
                  View all →
                </Link>
              </div>
            </div>
            <div className="divide-y divide-border-subtle">
              {liabilities.map((l) => {
                const Icon = LIAB_ICONS[l.liabilityType] ?? Wallet;
                const paid = pct(l.principal - l.remaining, l.principal);
                return (
                  <div key={l.id} className="px-4 py-3 hover:bg-accent transition-colors">
                    <div className="flex items-center gap-2 mb-1.5">
                      <Icon
                        className="w-3.5 h-3.5 text-destructive/60 flex-shrink-0"
                        strokeWidth={1.5}
                      />
                      <span className="text-[11px] font-medium text-muted-foreground truncate flex-1">
                        {l.name}
                      </span>
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-accent text-dim flex-shrink-0">
                        {l.liabilityType.replaceAll("_", " ")}
                      </span>
                      {l.interestRate != null && (
                        <span className="text-[9px] text-dim font-light flex-shrink-0">
                          {l.interestRate}% APR
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 mb-1.5">
                      <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
                        <div
                          className="h-full bg-success rounded-full"
                          style={{ width: `${paid}%` }}
                        />
                      </div>
                      <span className="text-[9px] text-dim font-light flex-shrink-0">{paid}%</span>
                    </div>
                    <span className="text-[12px] font-light text-destructive tabular-nums">
                      {displayAmount({
                        amount: l.remaining,
                        currency: l.currency,
                        baseAmount: l.baseRemaining,
                        baseCurrency,
                        mode,
                        rates,
                        hidden,
                      })}
                    </span>
                  </div>
                );
              })}
            </div>
            {liabilities.length > 0 && (
              <div className="px-4 py-2.5 border-t border-border bg-card flex justify-between">
                <span className="text-[10px] font-light text-muted-foreground">Total Debt</span>
                <span className="text-[10px] font-light text-destructive">
                  {fmtCompact(convertTotal(totalDebt), displayCur, hidden)}
                </span>
              </div>
            )}
          </Card>
        </div>
      </div>

      {/* Transactions — full width */}
      <Card className="overflow-hidden mb-3">
        <div className="px-4 pt-4 pb-1">
          <div className="flex items-center justify-between mb-2.5 px-1">
            <span className="text-[10px] text-muted-foreground uppercase tracking-widest">
              Recent Transactions
            </span>
            <Link
              to="/finance/cashflow"
              className="text-[10px] text-brand normal-case tracking-normal"
            >
              View all →
            </Link>
          </div>
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
          const bgCol =
            tx.txType === "income"
              ? "bg-success/10"
              : tx.txType === "expense"
                ? "bg-destructive/10"
                : "bg-info/10";
          const pre = tx.txType === "income" ? "+" : tx.txType === "expense" ? "-" : "";
          return (
            <div
              key={tx.id}
              className="flex items-center gap-3 px-4 py-2.5 hover:bg-accent transition-colors border-b border-border-subtle last:border-b-0"
            >
              <span className="text-[10px] text-dim font-light w-10 flex-shrink-0 tabular-nums">
                {tx.txDate.slice(5)}
              </span>
              <div
                className={cn(
                  "w-7 h-7 rounded-lg flex items-center justify-center flex-shrink-0",
                  bgCol,
                )}
              >
                <TxI className={cn("w-3.5 h-3.5", col)} strokeWidth={1.5} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-[12px] font-light text-muted-foreground truncate">
                  {tx.counterparty ?? tx.notes ?? tx.txType}
                </p>
                <div className="flex items-center gap-1.5 mt-0.5">
                  {tx.category && (
                    <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-accent text-dim">
                      {tx.category}
                    </span>
                  )}
                  {acct && <span className="text-[9px] text-dim font-light">{acct.name}</span>}
                </div>
              </div>
              <span
                className={cn(
                  "text-[12px] font-light w-24 text-right flex-shrink-0 tabular-nums",
                  col,
                )}
              >
                {pre}
                {displayAmount({
                  amount: tx.amount,
                  currency: tx.currency,
                  baseAmount: tx.baseAmount,
                  baseCurrency,
                  mode,
                  rates,
                  hidden,
                })}
              </span>
            </div>
          );
        })}
      </Card>

      <div className="h-3" />
    </FinanceLayout>
  );
}
