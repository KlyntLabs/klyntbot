import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type {
  FinanceAccount,
  FinanceBudgetUsage,
  FinanceCategoryReport,
  FinanceDailySpendingResponse,
  FinancePeriodSummary,
  FinanceTransaction,
  FinanceTransactionCreateParams,
} from "@shared/types";
import { ArrowDownRight, ArrowLeftRight, ArrowUpRight, Plus, Wallet } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Card, CardHeader } from "../components/Card";
import { CashFlowStats } from "../components/CashFlowStats";
import { DaySummary } from "../components/DaySummary";
import { FinanceLayout } from "../components/FinanceLayout";
import { FormField, fieldClass } from "../components/FormModal";
import { PeriodSelector } from "../components/PeriodSelector";
import { SlidePanel } from "../components/SlidePanel";
import { SpendingHeatmap } from "../components/SpendingHeatmap";
import { useFinanceCurrency } from "../hooks/useFinanceCurrency";
import { usePeriodState } from "../hooks/usePeriodState";
import { usePrivacyMode } from "../hooks/usePrivacyMode";
import { displayAmount } from "../lib/displayAmount";
import { ACCT_ICONS, COLORS, fmtCompact, pct } from "../lib/finance";
import { computeHeatmapLevels } from "../lib/heatmapColors";

export function CashFlowPage() {
  const period = usePeriodState();
  const { hidden, toggle } = usePrivacyMode();
  const { mode, setMode, baseCurrency, rates, currencies, displayCur, convertTotal } =
    useFinanceCurrency();

  // ── Period-scoped queries ──────────────────────────────────────
  const { data: periodSummary, refetch: rPS } = useQuery<FinancePeriodSummary>(
    "finance_period_summary",
    { dateFrom: period.dateFrom, dateTo: period.dateTo },
    { income: 0, spending: 0 },
  );

  const { data: dailySpendingResp, refetch: rDS } = useQuery<FinanceDailySpendingResponse>(
    "finance_daily_spending",
    { dateFrom: period.dateFrom, dateTo: period.dateTo },
    { days: [] },
  );

  const { data: spendingReport, refetch: rSR } = useQuery<FinanceCategoryReport>(
    "finance_report_spending",
    { dateFrom: period.dateFrom, dateTo: period.dateTo },
    { total: 0, breakdown: [] },
  );

  // ── Transaction filter state ───────────────────────────────────
  const [txFilter, setTxFilter] = useState<string>("all");
  const [accountFilter, setAccountFilter] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");

  useEffect(() => {
    const t = setTimeout(() => setDebouncedQuery(searchQuery), 300);
    return () => clearTimeout(t);
  }, [searchQuery]);

  const txDateFrom = period.selectedDay ?? period.dateFrom;
  const txDateTo = period.selectedDay ?? period.dateTo;

  const { data: transactions, refetch: rTX } = useQuery<FinanceTransaction[]>(
    "finance_transactions_filtered",
    {
      params: {
        dateFrom: txDateFrom,
        dateTo: txDateTo,
        txType: txFilter === "all" ? undefined : txFilter,
        accountId: accountFilter,
        query: debouncedQuery || undefined,
        limit: 50,
      },
    },
    [],
  );

  // ── Non-period-scoped queries ─────────────────────────────────
  const { data: accounts, refetch: rA } = useQuery<FinanceAccount[]>(
    "finance_accounts",
    undefined,
    [],
  );
  const { data: budgets, refetch: rB } = useQuery<FinanceBudgetUsage[]>(
    "finance_budget_usage",
    undefined,
    [],
  );

  const refetchAll = useCallback(() => {
    rPS();
    rDS();
    rSR();
    rTX();
    rA();
    rB();
  }, [rPS, rDS, rSR, rTX, rA, rB]);

  useEvent<{ entityKind: string }>("entity:updated", refetchAll);

  // ── Computed heatmap data ─────────────────────────────────────
  const { heatmapLevels, dailyCounts } = useMemo(() => {
    const values = new Map<string, number>();
    const counts = new Map<string, number>();
    for (const d of dailySpendingResp.days) {
      values.set(d.date, d.totalSpending);
      counts.set(d.date, d.txCount);
    }
    return { heatmapLevels: computeHeatmapLevels(values), dailyCounts: counts };
  }, [dailySpendingResp]);

  const [heatmapYear, heatmapMonth] = useMemo(() => {
    const d = new Date(period.dateFrom);
    return [d.getFullYear(), d.getMonth() + 1];
  }, [period.dateFrom]);

  const todayStr = new Date().toISOString().slice(0, 10);

  // ── Day summary data ─────────────────────────────────────────
  const selectedDayData = useMemo(() => {
    if (!period.selectedDay) return null;
    const day = dailySpendingResp.days.find((d) => d.date === period.selectedDay);
    const dayTxs = transactions.filter((t) => t.txDate === period.selectedDay);
    const categories = [
      ...new Set(dayTxs.filter((t) => t.category).map((t) => t.category as string)),
    ];
    return {
      txCount: day?.txCount ?? dayTxs.length,
      totalSpending: day?.totalSpending ?? 0,
      categories,
    };
  }, [period.selectedDay, dailySpendingResp, transactions]);

  // ── Merged spending + budget data ────────────────────────────
  const spendingWithBudgets = useMemo(() => {
    const activeBudgets = budgets.filter((b) => b.isActive);
    const budgetByCategory = new Map(
      activeBudgets.filter((b) => b.category).map((b) => [b.category as string, b]),
    );

    // Start with spending categories
    const merged: {
      category: string;
      spent: number;
      budget: FinanceBudgetUsage | null;
      pct: number;
    }[] = spendingReport.breakdown.map((item) => {
      const bud = budgetByCategory.get(item.category);
      if (bud) budgetByCategory.delete(item.category);
      return {
        category: item.category,
        spent: item.amount,
        budget: bud ?? null,
        pct: item.pct,
      };
    });

    // Add budgets with no spending this period
    for (const [, bud] of budgetByCategory) {
      merged.push({
        category: bud.category ?? bud.name,
        spent: 0,
        budget: bud,
        pct: 0,
      });
    }

    return merged;
  }, [spendingReport, budgets]);

  // ── Add Transaction panel ─────────────────────────────────────
  const [panelOpen, setPanelOpen] = useState(false);
  const [txAccountId, setTxAccountId] = useState("");
  const [txType, setTxType] = useState<"income" | "expense" | "transfer">("expense");
  const [txAmount, setTxAmount] = useState("");
  const [txCategory, setTxCategory] = useState("");
  const [txCounterparty, setTxCounterparty] = useState("");
  const [txDate, setTxDate] = useState(todayISO);
  const [txNotes, setTxNotes] = useState("");

  const { mutate: createTx } = useMutation<FinanceTransaction, FinanceTransactionCreateParams>(
    "finance_transaction_create",
    "params",
  );

  const activeAccounts = useMemo(() => accounts.filter((a) => !a.isArchived), [accounts]);

  const handleOpenPanel = () => {
    if (activeAccounts.length > 0 && !txAccountId) setTxAccountId(activeAccounts[0].id);
    setPanelOpen(true);
  };

  const handleCreateTx = async () => {
    if (!txAccountId || !txAmount) return;
    const result = await createTx({
      accountId: txAccountId,
      txType,
      amount: Math.round(Number(txAmount) * 100),
      category: txCategory || undefined,
      counterparty: txCounterparty || undefined,
      txDate: txDate || undefined,
      notes: txNotes || undefined,
    });
    if (!result) return;
    setPanelOpen(false);
    setTxAmount("");
    setTxCategory("");
    setTxCounterparty("");
    setTxNotes("");
    refetchAll();
  };

  return (
    <FinanceLayout
      hidden={hidden}
      onTogglePrivacy={toggle}
      currencyMode={mode}
      currencies={currencies}
      onSelectCurrency={setMode}
    >
      <PeriodSelector
        label={period.label}
        mode={period.mode}
        onPrev={period.prev}
        onNext={period.next}
        onSetMode={period.setMode}
      />

      <CashFlowStats
        income={periodSummary.income}
        spending={periodSummary.spending}
        displayCur={displayCur}
        convertTotal={convertTotal}
        hidden={hidden}
      />

      <div className="flex gap-4 mt-4">
        {/* Left column — scrollable main content */}
        <div className="flex-1 min-w-0 space-y-4">
          {/* Day summary (shown when a day is selected) */}
          {period.selectedDay && selectedDayData && (
            <DaySummary
              date={period.selectedDay}
              txCount={selectedDayData.txCount}
              totalSpending={selectedDayData.totalSpending}
              categories={selectedDayData.categories}
              onClose={() => period.selectDay(null)}
              displayCur={displayCur}
              convertTotal={convertTotal}
              hidden={hidden}
            />
          )}

          {/* Transaction filter bar */}
          <div className="flex items-center gap-2 mb-3">
            <div className="flex-1 relative">
              <input
                type="text"
                placeholder="Search transactions…"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="glass-input w-full px-3 py-2 text-xs font-light"
                spellCheck={false}
              />
            </div>
            <div className="flex gap-1 bg-card rounded-xl p-1" role="tablist">
              {["all", "income", "expense", "transfer"].map((t) => (
                <button
                  key={t}
                  type="button"
                  role="tab"
                  aria-selected={t === txFilter}
                  onClick={() => setTxFilter(t)}
                  className={`px-2.5 py-1 rounded-lg text-2xs font-light capitalize transition-colors ${
                    t === txFilter
                      ? "glass-button-active text-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          {/* Transaction list */}
          <Card className="overflow-hidden">
            <div className="px-4 pt-4 flex items-center justify-between mb-3">
              <CardHeader
                title={`Transactions (${transactions.length})`}
                action={
                  <button
                    type="button"
                    onClick={handleOpenPanel}
                    className="flex items-center gap-1 text-2xs text-brand font-light hover:text-brand-hover transition-colors"
                  >
                    <Plus className="size-3" strokeWidth={1.5} /> Add
                  </button>
                }
              />
            </div>
            {transactions.length === 0 ? (
              <div className="px-4 pb-4 text-center text-[11px] text-dim font-light">
                No transactions match your filters
              </div>
            ) : (
              transactions.map((tx) => {
                const TxIcon =
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
                    className="flex items-center gap-3 px-4 py-2 hover:bg-accent transition-colors border-b border-border-subtle last:border-b-0"
                  >
                    <span className="text-2xs text-dim font-light w-10 flex-shrink-0 tabular-nums">
                      {tx.txDate.slice(5)}
                    </span>
                    <TxIcon className={cn("size-3 flex-shrink-0", col)} strokeWidth={1.5} />
                    <span className="text-xs font-light text-muted-foreground truncate flex-1">
                      {tx.counterparty ?? tx.notes ?? tx.txType}
                    </span>
                    {tx.category && (
                      <span className="px-1.5 py-0.5 text-[9px] font-light rounded bg-accent text-dim">
                        {tx.category}
                      </span>
                    )}
                    <span
                      className={cn(
                        "text-xs font-light w-24 text-right flex-shrink-0 tabular-nums",
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
              })
            )}
          </Card>
        </div>

        {/* Right sidebar — sticky */}
        <div className="w-72 flex-shrink-0 sticky top-0 self-start space-y-4">
          {/* Spending heatmap */}
          <Card compact className="p-3">
            <SpendingHeatmap
              year={heatmapYear}
              month={heatmapMonth}
              levels={heatmapLevels}
              dailyCounts={dailyCounts}
              selectedDay={period.selectedDay}
              onSelectDay={period.selectDay}
              today={todayStr}
            />
          </Card>

          {/* Accounts sidebar */}
          <Card compact className="p-3">
            <p className="text-2xs text-muted-foreground uppercase tracking-widest mb-2">
              Accounts
            </p>
            {activeAccounts.map((acct) => {
              const Icon = ACCT_ICONS[acct.accountType] ?? Wallet;
              const isSelected = accountFilter === acct.id;
              return (
                <button
                  type="button"
                  key={acct.id}
                  onClick={() => setAccountFilter(isSelected ? null : acct.id)}
                  className={cn(
                    "flex items-center gap-2 px-2 py-1.5 rounded-lg cursor-pointer transition-colors w-full text-left",
                    isSelected ? "bg-muted" : "hover:bg-accent",
                  )}
                >
                  <Icon className="size-3.5 text-muted-foreground" strokeWidth={1.5} />
                  <span className="text-[11px] text-muted-foreground flex-1 truncate">
                    {acct.name}
                  </span>
                  <span className="text-2xs text-muted-foreground tabular-nums">
                    {displayAmount({
                      amount: acct.balance,
                      currency: acct.currency,
                      baseAmount: acct.baseBalance,
                      baseCurrency,
                      mode,
                      rates,
                      hidden,
                    })}
                  </span>
                </button>
              );
            })}
          </Card>

          {/* Spending & Budgets (merged) */}
          <Card compact className="overflow-hidden">
            <div className="px-3 pt-3 pb-1">
              <p className="text-2xs text-muted-foreground uppercase tracking-widest">
                Spending & Budgets
              </p>
            </div>
            {spendingWithBudgets.map((item, i) => {
              // Use baseAmount for budget comparison (same currency as spending report)
              const budgetBase = item.budget?.baseAmount ?? 0;
              const budgetPct = budgetBase > 0 ? pct(item.spent, budgetBase) : 0;
              const hasBudget = budgetBase > 0;
              const barColor = hasBudget
                ? budgetPct >= 80
                  ? "#f43f5e"
                  : budgetPct >= 50
                    ? "#f97316"
                    : "#34d399"
                : COLORS[i % COLORS.length];
              const barWidth = hasBudget ? Math.min(budgetPct, 100) : item.pct;

              return (
                <div
                  key={item.category}
                  className="px-3 py-2 border-b border-border-subtle last:border-b-0"
                >
                  <div className="flex items-center justify-between mb-1">
                    <div className="flex items-center gap-1.5">
                      <div
                        className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                        style={{ backgroundColor: COLORS[i % COLORS.length] }}
                      />
                      <span className="text-2xs text-muted-foreground capitalize">
                        {item.category}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-2xs text-foreground tabular-nums">
                        {fmtCompact(convertTotal(item.spent), displayCur, hidden)}
                      </span>
                      {hasBudget && (
                        <span className="text-[9px] text-dim tabular-nums">
                          / {fmtCompact(convertTotal(budgetBase), displayCur, hidden)}
                        </span>
                      )}
                      <span
                        className="text-[9px] font-light tabular-nums min-w-[28px] text-right"
                        style={{ color: barColor }}
                      >
                        {hasBudget ? `${budgetPct}%` : `${Math.round(item.pct)}%`}
                      </span>
                    </div>
                  </div>
                  <div className="h-1 bg-accent rounded-full">
                    <div
                      className="h-full rounded-full"
                      style={{ width: `${barWidth}%`, background: barColor }}
                    />
                  </div>
                  {hasBudget && budgetPct >= 80 && (
                    <p className="text-[8px] text-destructive mt-0.5">
                      ⚠ {budgetPct >= 100 ? "Over budget" : `${100 - budgetPct}% left`}
                    </p>
                  )}
                </div>
              );
            })}
            {spendingWithBudgets.length === 0 && (
              <p className="px-3 py-4 text-2xs text-dim text-center">No spending this period</p>
            )}
          </Card>
        </div>
      </div>

      {/* Add Transaction SlidePanel */}
      <SlidePanel open={panelOpen} onClose={() => setPanelOpen(false)} title="Add Transaction">
        <div className="space-y-3">
          <FormField label="Account">
            <select
              className={fieldClass}
              value={txAccountId}
              onChange={(e) => setTxAccountId(e.target.value)}
            >
              <option value="" disabled>
                Select account
              </option>
              {activeAccounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </FormField>
          <FormField label="Type">
            <div className="flex gap-2">
              {(["expense", "income", "transfer"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setTxType(t)}
                  className={cn(
                    "flex-1 py-1.5 text-xs rounded-md border transition-colors capitalize",
                    txType === t
                      ? "border-brand/50 text-brand bg-brand/5"
                      : "border-border text-muted-foreground bg-accent hover:bg-muted",
                  )}
                >
                  {t}
                </button>
              ))}
            </div>
          </FormField>
          <FormField label="Amount">
            <input
              className={fieldClass}
              type="number"
              value={txAmount}
              onChange={(e) => setTxAmount(e.target.value)}
              placeholder="0"
            />
          </FormField>
          <FormField label="Category">
            <input
              className={fieldClass}
              value={txCategory}
              onChange={(e) => setTxCategory(e.target.value)}
              placeholder="e.g. Food, Transport"
            />
          </FormField>
          <FormField label="Counterparty">
            <input
              className={fieldClass}
              value={txCounterparty}
              onChange={(e) => setTxCounterparty(e.target.value)}
              placeholder="e.g. Grab, Shopee"
            />
          </FormField>
          <FormField label="Date">
            <input
              className={fieldClass}
              type="date"
              value={txDate}
              onChange={(e) => setTxDate(e.target.value)}
            />
          </FormField>
          <FormField label="Notes">
            <input
              className={fieldClass}
              value={txNotes}
              onChange={(e) => setTxNotes(e.target.value)}
              placeholder="Optional notes"
            />
          </FormField>
          <div className="pt-2">
            <button
              type="button"
              onClick={handleCreateTx}
              disabled={!txAccountId || !txAmount}
              className="w-full py-2 text-xs rounded-lg bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Add Transaction
            </button>
          </div>
        </div>
      </SlidePanel>
    </FinanceLayout>
  );
}
