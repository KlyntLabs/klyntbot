import { useState, useMemo } from 'react';
import { ChevronLeft, ChevronRight, TrendingUp, TrendingDown, Plus, RefreshCw, AlertCircle, Target, Loader2, AlertTriangle } from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import type {
  FinanceAccount,
  FinanceTransaction,
  BudgetUsage,
  FinanceInvestment,
  FinanceGoal,
  FinanceLiability,
} from '../../lib/types';

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Format integer cents as a currency string (e.g. 123456 → "1,234.56"). */
function formatCents(cents: number, currency = 'USD'): string {
  const abs = Math.abs(cents);
  const symbol = currency === 'USD' ? '$' : currency;
  const formatted = (abs / 100).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return cents < 0 ? `-${symbol}${formatted}` : `${symbol}${formatted}`;
}

/** Format integer cents with explicit +/- sign for display (e.g. +$50.00 / -$50.00). */
function formatCentsSigned(cents: number, currency = 'USD'): string {
  const abs = Math.abs(cents);
  const symbol = currency === 'USD' ? '$' : currency;
  const formatted = (abs / 100).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return cents >= 0 ? `+${symbol}${formatted}` : `-${symbol}${formatted}`;
}

/** Colour for a transaction based on its type. */
function txColor(txType: string): string {
  switch (txType) {
    case 'income': return '#10a37f';
    case 'transfer': return '#3b82f6';
    default: return '#ef4444'; // expense
  }
}

/** Dot colour for transaction list. */
function txDotColor(txType: string): string {
  return txColor(txType);
}

/** Format a date string as "Mon DD" (e.g. "Feb 24"). */
function shortDate(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

// ── Shared UI components ─────────────────────────────────────────────────────

function LoadingState({ message = 'Loading...' }: { message?: string }) {
  return (
    <div className="flex items-center justify-center gap-3 p-12">
      <Loader2 className="w-5 h-5 animate-spin" style={{ color: 'var(--codex-accent)' }} strokeWidth={1.5} />
      <span className="text-[14px]" style={{ color: 'var(--codex-fg-subtle)' }}>{message}</span>
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="flex items-center justify-center gap-3 p-12">
      <AlertTriangle className="w-5 h-5" style={{ color: '#ef4444' }} strokeWidth={1.5} />
      <span className="text-[14px]" style={{ color: '#ef4444' }}>{message}</span>
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 p-12">
      <AlertCircle className="w-6 h-6" style={{ color: '#888' }} strokeWidth={1.5} />
      <span className="text-[14px]" style={{ color: '#888' }}>{message}</span>
    </div>
  );
}

// ── Types ────────────────────────────────────────────────────────────────────

type Tab = 'dashboard' | 'transactions' | 'budgets' | 'investments' | 'goals' | 'reports';
type BudgetMode = 'standard' | 'six-jar';
type ReportPeriod = 'weekly' | 'monthly' | 'yearly';

// ── Component ────────────────────────────────────────────────────────────────

export default function Finance() {
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');
  const [budgetMode, setBudgetMode] = useState<BudgetMode>('standard');
  const [reportPeriod, setReportPeriod] = useState<ReportPeriod>('monthly');
  const [expandedGoal, setExpandedGoal] = useState<string | null>(null);

  // ── API calls ──────────────────────────────────────────────────────────────
  const accounts = useApi<FinanceAccount[]>('/api/finance/accounts');
  const transactions = useApi<FinanceTransaction[]>('/api/finance/transactions');
  const budgets = useApi<BudgetUsage[]>('/api/finance/budgets/usage');
  const investments = useApi<FinanceInvestment[]>('/api/finance/investments');
  const goals = useApi<FinanceGoal[]>('/api/finance/goals');
  const liabilities = useApi<FinanceLiability[]>('/api/finance/liabilities');

  // ── Derived dashboard data ─────────────────────────────────────────────────

  const dashboardSummary = useMemo(() => {
    const accts = accounts.data ?? [];
    const txs = transactions.data ?? [];
    const budgs = budgets.data ?? [];
    const invs = investments.data ?? [];
    const gls = goals.data ?? [];
    const liabs = liabilities.data ?? [];

    // Net worth = sum(account balances) + sum(investment current values) - sum(liability remaining)
    const totalAccountBalance = accts.reduce((sum, a) => sum + a.balance, 0);
    const totalInvestmentValue = invs.reduce((sum, inv) => sum + (inv.currentValue ?? 0), 0);
    const totalLiabilityRemaining = liabs.reduce((sum, l) => sum + l.remaining, 0);
    const netWorth = totalAccountBalance + totalInvestmentValue - totalLiabilityRemaining;

    // Total investment cost basis for P&L
    const totalCostBasis = invs.reduce((sum, inv) => sum + inv.costBasis, 0);
    const investmentPL = totalInvestmentValue - totalCostBasis;
    const investmentPLPct = totalCostBasis > 0 ? (investmentPL / totalCostBasis) * 100 : 0;

    // Monthly spending: sum of expense transactions in the current month
    const now = new Date();
    const currentMonth = now.getMonth();
    const currentYear = now.getFullYear();
    const monthlyExpenses = txs.filter((tx) => {
      const d = new Date(tx.txDate);
      return d.getMonth() === currentMonth && d.getFullYear() === currentYear && tx.txType === 'expense';
    });
    const totalMonthlySpending = monthlyExpenses.reduce((sum, tx) => sum + Math.abs(tx.amount), 0);

    // Spending by category for the breakdown bar
    const categorySpending: Record<string, number> = {};
    for (const tx of monthlyExpenses) {
      const cat = tx.category ?? 'Other';
      categorySpending[cat] = (categorySpending[cat] ?? 0) + Math.abs(tx.amount);
    }
    const topCategories = Object.entries(categorySpending)
      .sort(([, a], [, b]) => b - a)
      .slice(0, 3);

    // Budget status
    const activeBudgets = budgs.filter((b) => b.isActive);
    const overBudgetCount = activeBudgets.filter((b) => b.amount > 0 && b.spent > b.amount).length;
    const onTrackCount = activeBudgets.length - overBudgetCount;
    const totalBudgetAmount = activeBudgets.reduce((s, b) => s + b.amount, 0);
    const totalBudgetSpent = activeBudgets.reduce((s, b) => s + b.spent, 0);
    const budgetPct = totalBudgetAmount > 0 ? Math.round((totalBudgetSpent / totalBudgetAmount) * 100) : 0;

    // Recent transactions (latest 5)
    const recentTxs = [...txs]
      .sort((a, b) => new Date(b.txDate).getTime() - new Date(a.txDate).getTime())
      .slice(0, 5);

    // Active goals count + top 3 for preview
    const activeGoals = gls.filter((g) => g.status === 'active');
    const topGoals = activeGoals.slice(0, 3);
    const nextDeadline = activeGoals
      .filter((g) => g.deadline)
      .sort((a, b) => new Date(a.deadline!).getTime() - new Date(b.deadline!).getTime())[0]?.deadline;

    return {
      netWorth,
      totalMonthlySpending,
      topCategories,
      budgetPct,
      activeBudgetCount: activeBudgets.length,
      onTrackCount,
      overBudgetCount,
      totalInvestmentValue,
      investmentPL,
      investmentPLPct,
      recentTxs,
      activeGoals,
      topGoals,
      nextDeadline,
    };
  }, [accounts.data, transactions.data, budgets.data, investments.data, goals.data, liabilities.data]);

  const dashboardLoading = accounts.loading || transactions.loading || budgets.loading || investments.loading || goals.loading || liabilities.loading;
  const dashboardError = accounts.error || transactions.error || budgets.error || investments.error || goals.error || liabilities.error;

  const tabs: { id: Tab; label: string }[] = [
    { id: 'dashboard', label: 'Dashboard' },
    { id: 'transactions', label: 'Transactions' },
    { id: 'budgets', label: 'Budgets' },
    { id: 'investments', label: 'Investments' },
    { id: 'goals', label: 'Goals' },
    { id: 'reports', label: 'Reports' },
  ];

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
      {/* Tab Bar */}
      <div className="border-b px-6 py-3" style={{
        borderColor: 'var(--codex-border-subtle)',
        backgroundColor: 'var(--codex-bg)'
      }}>
        <div className="flex gap-2">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className="px-4 py-1.5 rounded-full text-[13px] transition-all"
              style={{
                backgroundColor: activeTab === tab.id ? 'var(--codex-accent)' : 'transparent',
                color: activeTab === tab.id ? 'white' : 'var(--codex-fg-subtle)',
                border: `1px solid ${activeTab === tab.id ? 'var(--codex-accent)' : 'transparent'}`
              }}
              onMouseEnter={(e) => {
                if (activeTab !== tab.id) {
                  e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)';
                }
              }}
              onMouseLeave={(e) => {
                if (activeTab !== tab.id) {
                  e.currentTarget.style.backgroundColor = 'transparent';
                }
              }}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {/* Dashboard Tab */}
        {activeTab === 'dashboard' && (
          <>
            {dashboardLoading && <LoadingState message="Loading financial data..." />}
            {!dashboardLoading && dashboardError && <ErrorState message={dashboardError.message} />}
            {!dashboardLoading && !dashboardError && (
              <div className="grid grid-cols-2 gap-6">
                {/* Card 1: Net Worth */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('dashboard')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Net Worth</div>
                  <div className="text-[28px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    {formatCents(dashboardSummary.netWorth)}
                  </div>
                  <div className="flex items-center gap-1.5 text-[13px]" style={{ color: dashboardSummary.netWorth >= 0 ? '#10a37f' : '#ef4444' }}>
                    {dashboardSummary.netWorth >= 0
                      ? <TrendingUp className="w-4 h-4" strokeWidth={1.5} />
                      : <TrendingDown className="w-4 h-4" strokeWidth={1.5} />}
                    {(accounts.data?.length ?? 0)} accounts
                  </div>
                </div>

                {/* Card 2: Monthly Spending */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('transactions')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Monthly Spending</div>
                  <div className="text-[28px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    {formatCents(dashboardSummary.totalMonthlySpending)}
                  </div>
                  {dashboardSummary.topCategories.length > 0 && (
                    <>
                      <div className="h-1 rounded-full mb-2 flex overflow-hidden" style={{ backgroundColor: '#0d0d0d' }}>
                        {(() => {
                          const barColors = ['#10a37f', '#f97316', '#8b5cf6', '#666'];
                          const total = dashboardSummary.topCategories.reduce((s, [, v]) => s + v, 0);
                          return dashboardSummary.topCategories.map(([, amount], i) => (
                            <div key={i} style={{ width: `${total > 0 ? (amount / total) * 100 : 0}%`, backgroundColor: barColors[i] ?? '#666' }} />
                          ));
                        })()}
                      </div>
                      <div className="text-[11px]" style={{ color: '#888' }}>
                        {dashboardSummary.topCategories.map(([cat, amount]) => `${cat} ${formatCents(amount)}`).join(' \u00B7 ')}
                      </div>
                    </>
                  )}
                  {dashboardSummary.topCategories.length === 0 && (
                    <div className="text-[11px]" style={{ color: '#888' }}>No expenses this month</div>
                  )}
                </div>

                {/* Card 3: Budget Status */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('budgets')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Budget Status</div>
                  {dashboardSummary.activeBudgetCount === 0 ? (
                    <div className="text-[13px]" style={{ color: '#888' }}>No active budgets</div>
                  ) : (
                    <div className="flex items-center gap-4 mb-3">
                      <div className="relative" style={{ width: '64px', height: '64px' }}>
                        <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                          <circle cx="32" cy="32" r="28" fill="none" stroke="#1a1a1a" strokeWidth="6" />
                          <circle
                            cx="32" cy="32" r="28"
                            fill="none"
                            stroke={dashboardSummary.budgetPct > 100 ? '#ef4444' : '#10a37f'}
                            strokeWidth="6"
                            strokeDasharray={`${2 * Math.PI * 28 * Math.min(dashboardSummary.budgetPct, 100) / 100} ${2 * Math.PI * 28}`}
                            strokeLinecap="round"
                          />
                        </svg>
                        <div className="absolute inset-0 flex items-center justify-center text-[18px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                          {dashboardSummary.budgetPct}%
                        </div>
                      </div>
                      <div>
                        <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                          {dashboardSummary.onTrackCount} of {dashboardSummary.activeBudgetCount} budgets on track
                        </div>
                        {dashboardSummary.overBudgetCount > 0 && (
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: '#ef4444' }}>
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#ef4444' }} />
                            {dashboardSummary.overBudgetCount} over limit
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>

                {/* Card 4: Portfolio Value */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('investments')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Portfolio Value</div>
                  <div className="text-[28px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    {formatCents(dashboardSummary.totalInvestmentValue)}
                  </div>
                  <div className="flex items-center gap-1.5 text-[13px]" style={{ color: dashboardSummary.investmentPL >= 0 ? '#10a37f' : '#ef4444' }}>
                    {dashboardSummary.investmentPL >= 0
                      ? <TrendingUp className="w-4 h-4" strokeWidth={1.5} />
                      : <TrendingDown className="w-4 h-4" strokeWidth={1.5} />}
                    {formatCentsSigned(dashboardSummary.investmentPL)} ({dashboardSummary.investmentPLPct >= 0 ? '+' : ''}{dashboardSummary.investmentPLPct.toFixed(1)}%)
                  </div>
                </div>

                {/* Card 5: Active Goals */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('goals')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Active Goals</div>
                  <div className="flex items-baseline gap-2 mb-3">
                    <span className="text-[28px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{dashboardSummary.activeGoals.length}</span>
                    <span className="text-[14px]" style={{ color: '#888' }}>active</span>
                  </div>
                  {dashboardSummary.topGoals.length > 0 ? (
                    <div className="space-y-2 mb-2">
                      {dashboardSummary.topGoals.map((goal) => {
                        const pct = goal.targetAmount > 0 ? Math.round((goal.currentAmount / goal.targetAmount) * 100) : 0;
                        return (
                          <div key={goal.id}>
                            <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>{goal.name}</div>
                            <div className="h-0.5 rounded-full overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                              <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: '#10a37f' }} />
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ) : (
                    <div className="text-[11px]" style={{ color: '#888' }}>No active goals</div>
                  )}
                  {dashboardSummary.nextDeadline && (
                    <div className="text-[11px]" style={{ color: '#888' }}>
                      Next deadline: {new Date(dashboardSummary.nextDeadline).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}
                    </div>
                  )}
                </div>

                {/* Card 6: Recent Transactions */}
                <div
                  className="p-5 rounded-lg border cursor-pointer transition-all"
                  style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('transactions')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}
                >
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Recent Transactions</div>
                  {dashboardSummary.recentTxs.length > 0 ? (
                    <div className="space-y-2">
                      {dashboardSummary.recentTxs.map((tx) => {
                        const displayAmount = tx.txType === 'income' ? tx.amount : -Math.abs(tx.amount);
                        return (
                          <div key={tx.id} className="flex items-start justify-between">
                            <div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{tx.counterparty ?? tx.category ?? tx.txType}</div>
                              <div className="text-[11px]" style={{ color: '#888' }}>{shortDate(tx.txDate)}</div>
                            </div>
                            <div className="text-[13px]" style={{ color: txColor(tx.txType), fontFamily: 'var(--font-mono)' }}>
                              {formatCentsSigned(displayAmount, tx.currency)}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ) : (
                    <div className="text-[11px]" style={{ color: '#888' }}>No transactions yet</div>
                  )}
                </div>
              </div>
            )}
          </>
        )}

        {/* Transactions Tab */}
        {activeTab === 'transactions' && (
          <div>
            {/* TODO: Wire up add-transaction form — POST /api/finance/transactions */}
            <div className="mb-4 flex items-center gap-3 p-3 rounded-lg border" style={{ backgroundColor: '#1a1a1a', borderColor: '#2a2a2a' }}>
              <span className="text-[14px]" style={{ color: '#888' }}>$</span>
              <input
                type="text"
                placeholder='"$50 groceries" or "income 2000 salary"'
                className="flex-1 bg-transparent outline-none text-[14px]"
                style={{ color: 'var(--codex-fg)' }}
              />
              <button className="px-3 py-1.5 rounded-full text-[12px]" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                + Add
              </button>
            </div>

            {/* TODO: Wire up filter buttons to actually filter the transaction list */}
            <div className="mb-4 flex items-center gap-3">
              <div className="flex gap-2">
                {['All', 'Income', 'Expense', 'Transfer'].map((type) => (
                  <button
                    key={type}
                    className="px-3 py-1.5 rounded-full text-[12px]"
                    style={{
                      backgroundColor: type === 'All' ? 'var(--codex-accent)' : 'transparent',
                      color: type === 'All' ? 'white' : 'var(--codex-fg-subtle)',
                      border: `1px solid ${type === 'All' ? 'var(--codex-accent)' : 'var(--codex-border)'}`
                    }}
                  >
                    {type}
                  </button>
                ))}
              </div>
              {/* TODO: Wire up month navigation to filter by date range */}
              <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)' }}>
                <ChevronLeft className="w-3 h-3" strokeWidth={1.5} />
                {new Date().toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}
                <ChevronRight className="w-3 h-3" strokeWidth={1.5} />
              </div>
              <select className="px-3 py-1.5 rounded text-[12px] outline-none" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}>
                <option>Category</option>
              </select>
            </div>

            {transactions.loading && <LoadingState message="Loading transactions..." />}
            {!transactions.loading && transactions.error && <ErrorState message={transactions.error.message} />}
            {!transactions.loading && !transactions.error && (transactions.data?.length ?? 0) === 0 && (
              <EmptyState message="No transactions recorded yet. Add your first transaction above." />
            )}
            {!transactions.loading && !transactions.error && (transactions.data?.length ?? 0) > 0 && (
              <div className="space-y-1.5">
                {[...(transactions.data ?? [])]
                  .sort((a, b) => new Date(b.txDate).getTime() - new Date(a.txDate).getTime())
                  .map((tx) => {
                    const displayAmount = tx.txType === 'income' ? tx.amount : -Math.abs(tx.amount);
                    const accountName = accounts.data?.find((a) => a.id === tx.accountId)?.name ?? tx.accountId;
                    return (
                      <div key={tx.id} className="flex items-center justify-between p-3 rounded border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                        <div className="flex items-center gap-3">
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: txDotColor(tx.txType) }} />
                          <div>
                            <div className="flex items-center gap-2">
                              <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{tx.counterparty ?? tx.category ?? tx.txType}</span>
                              {tx.isRecurring && <span className="text-[11px]" style={{ color: '#888' }}>{'\u21BB'}</span>}
                            </div>
                            <div className="text-[12px]" style={{ color: '#888' }}>
                              {tx.txType === 'transfer'
                                ? `\u2192 ${tx.notes ?? 'Transfer'}`
                                : tx.category ?? tx.txType}
                            </div>
                          </div>
                        </div>
                        <div className="text-right">
                          <div className="text-[14px]" style={{ color: txColor(tx.txType), fontFamily: 'var(--font-mono)', fontWeight: 500 }}>
                            {formatCentsSigned(displayAmount, tx.currency)}
                          </div>
                          <div className="text-[11px]" style={{ color: '#888' }}>
                            {shortDate(tx.txDate)} &middot; {accountName}
                          </div>
                        </div>
                      </div>
                    );
                  })}
              </div>
            )}
          </div>
        )}

        {/* Budgets Tab */}
        {activeTab === 'budgets' && (
          <div>
            <div className="mb-4 flex items-center justify-between">
              <div className="flex gap-2">
                <button
                  onClick={() => setBudgetMode('standard')}
                  className="px-3 py-1.5 rounded-full text-[12px]"
                  style={{
                    backgroundColor: budgetMode === 'standard' ? 'var(--codex-accent)' : 'transparent',
                    color: budgetMode === 'standard' ? 'white' : 'var(--codex-fg-subtle)',
                    border: `1px solid ${budgetMode === 'standard' ? 'var(--codex-accent)' : 'var(--codex-border)'}`
                  }}
                >
                  Standard
                </button>
                <button
                  onClick={() => setBudgetMode('six-jar')}
                  className="px-3 py-1.5 rounded-full text-[12px]"
                  style={{
                    backgroundColor: budgetMode === 'six-jar' ? 'var(--codex-accent)' : 'transparent',
                    color: budgetMode === 'six-jar' ? 'white' : 'var(--codex-fg-subtle)',
                    border: `1px solid ${budgetMode === 'six-jar' ? 'var(--codex-accent)' : 'var(--codex-border)'}`
                  }}
                >
                  Six-Jar
                </button>
              </div>
              {/* TODO: Wire up create budget — POST /api/finance/budgets */}
              <button className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />
                Create Budget
              </button>
            </div>

            {budgets.loading && <LoadingState message="Loading budgets..." />}
            {!budgets.loading && budgets.error && <ErrorState message={budgets.error.message} />}
            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) === 0 && (
              <EmptyState message="No budgets configured yet. Create your first budget to start tracking spending." />
            )}

            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) > 0 && budgetMode === 'standard' && (
              <div className="space-y-3">
                {(budgets.data ?? [])
                  .filter((b) => b.method !== 'six-jar')
                  .map((budget) => {
                    const pct = budget.amount > 0 ? Math.round((budget.spent / budget.amount) * 100) : 0;
                    const barColor = pct > 100 ? '#ef4444' : pct > 80 ? '#eab308' : '#10a37f';
                    return (
                      <div key={budget.id} className="p-4 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                        <div className="flex items-center justify-between mb-2">
                          <div className="flex items-center gap-2">
                            <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{budget.name}</span>
                            <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#1a1a1a', color: '#888' }}>{budget.period}</span>
                          </div>
                          <div className="text-[13px]" style={{ color: barColor }}>
                            {pct}%{pct > 100 ? ' Over!' : pct > 80 ? ' \u26A0' : ''}
                          </div>
                        </div>
                        <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                          <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: barColor }} />
                        </div>
                        <div className="text-[12px] text-right" style={{ color: 'var(--codex-fg-subtle)' }}>
                          {formatCents(budget.spent)} / {formatCents(budget.amount)}
                        </div>
                      </div>
                    );
                  })}
              </div>
            )}

            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) > 0 && budgetMode === 'six-jar' && (
              <div className="grid grid-cols-3 gap-4">
                {(budgets.data ?? [])
                  .filter((b) => b.method === 'six-jar' && b.jarType)
                  .map((jar) => {
                    const pct = jar.amount > 0 ? Math.round((jar.spent / jar.amount) * 100) : 0;
                    const jarColors: Record<string, string> = {
                      essentials: '#3b82f6',
                      savings: '#10a37f',
                      investment: '#14b8a6',
                      education: '#8b5cf6',
                      entertainment: '#f97316',
                      charity: '#ec4899',
                    };
                    const color = jarColors[jar.jarType ?? ''] ?? '#888';
                    return (
                      <div key={jar.id} className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a', borderLeft: `3px solid ${color}` }}>
                        <div className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{jar.name}</div>
                        <div className="flex justify-center mb-3">
                          <div className="relative" style={{ width: '56px', height: '56px' }}>
                            <svg viewBox="0 0 56 56" style={{ transform: 'rotate(-90deg)' }}>
                              <circle cx="28" cy="28" r="24" fill="none" stroke="#1a1a1a" strokeWidth="5" />
                              <circle
                                cx="28" cy="28" r="24"
                                fill="none"
                                stroke={color}
                                strokeWidth="5"
                                strokeDasharray={`${2 * Math.PI * 24 * (Math.min(pct, 100) / 100)} ${2 * Math.PI * 24}`}
                                strokeLinecap="round"
                              />
                            </svg>
                            <div className="absolute inset-0 flex items-center justify-center text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                              {pct}%
                            </div>
                          </div>
                        </div>
                        <div className="text-center">
                          <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>
                            {formatCents(jar.spent)} / {formatCents(jar.amount)}
                          </div>
                          <div className="text-[11px]" style={{ color: '#888' }}>
                            {jar.category ?? jar.jarType}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                {(budgets.data ?? []).filter((b) => b.method === 'six-jar' && b.jarType).length === 0 && (
                  <div className="col-span-3">
                    <EmptyState message="No six-jar budgets configured. Create budgets with the six-jar method to see them here." />
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* Investments Tab */}
        {activeTab === 'investments' && (
          <div>
            {investments.loading && <LoadingState message="Loading investments..." />}
            {!investments.loading && investments.error && <ErrorState message={investments.error.message} />}
            {!investments.loading && !investments.error && (() => {
              const invs = investments.data ?? [];
              const totalValue = invs.reduce((sum, inv) => sum + (inv.currentValue ?? 0), 0);
              const totalCost = invs.reduce((sum, inv) => sum + inv.costBasis, 0);
              const totalPL = totalValue - totalCost;
              const totalPLPct = totalCost > 0 ? (totalPL / totalCost) * 100 : 0;

              // Asset type allocation for the donut chart
              const byType: Record<string, number> = {};
              for (const inv of invs) {
                byType[inv.assetType] = (byType[inv.assetType] ?? 0) + (inv.currentValue ?? 0);
              }
              const typeEntries = Object.entries(byType).sort(([, a], [, b]) => b - a);
              const typeColors = ['#10a37f', '#f97316', '#3b82f6', '#8b5cf6', '#ec4899', '#666'];

              return (
                <>
                  <div className="mb-4 p-5 rounded-lg border flex items-center justify-between" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                    <div>
                      <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Value</div>
                      <div className="text-[20px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{formatCents(totalValue)}</div>
                    </div>
                    <div>
                      <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Gain/Loss</div>
                      <div className="text-[16px]" style={{ color: totalPL >= 0 ? '#10a37f' : '#ef4444', fontWeight: 500 }}>
                        {formatCentsSigned(totalPL)} ({totalPLPct >= 0 ? '+' : ''}{totalPLPct.toFixed(1)}%)
                      </div>
                    </div>
                    {typeEntries.length > 0 && (
                      <div className="relative" style={{ width: '64px', height: '64px' }}>
                        <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                          {(() => {
                            let offset = 0;
                            return typeEntries.map(([, value], i) => {
                              const frac = totalValue > 0 ? value / totalValue : 0;
                              const el = (
                                <circle
                                  key={i}
                                  cx="32" cy="32" r="28"
                                  fill="none"
                                  stroke={typeColors[i] ?? '#666'}
                                  strokeWidth="8"
                                  strokeDasharray={`${2 * Math.PI * 28 * frac} ${2 * Math.PI * 28}`}
                                  strokeDashoffset={`${-2 * Math.PI * 28 * offset}`}
                                />
                              );
                              offset += frac;
                              return el;
                            });
                          })()}
                        </svg>
                      </div>
                    )}
                  </div>

                  <div className="mb-4 flex items-center justify-between">
                    <select className="px-3 py-1.5 rounded text-[13px] outline-none" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}>
                      <option>All Portfolios</option>
                    </select>
                    <div className="flex gap-2">
                      {/* TODO: Wire up add holding — POST /api/finance/investments */}
                      <button className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                        <Plus className="w-3 h-3" strokeWidth={1.5} />
                        Add Holding
                      </button>
                      {/* TODO: Wire up refresh prices — PATCH /api/finance/investments/:id */}
                      <button className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'transparent', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}>
                        <RefreshCw className="w-3 h-3" strokeWidth={1.5} />
                        Refresh Prices
                      </button>
                    </div>
                  </div>

                  {invs.length === 0 ? (
                    <EmptyState message="No investments tracked yet. Add your first holding to get started." />
                  ) : (
                    <div className="rounded-lg overflow-hidden border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <table className="w-full">
                        <thead>
                          <tr style={{ backgroundColor: '#1a1a1a' }}>
                            <th className="text-left px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Symbol</th>
                            <th className="text-left px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Name</th>
                            <th className="text-left px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Type</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Qty</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Avg Cost</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Price</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>Value</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>P&L</th>
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}>P&L %</th>
                          </tr>
                        </thead>
                        <tbody>
                          {invs.map((inv) => {
                            const avgCost = inv.quantity > 0 ? inv.costBasis / inv.quantity : 0;
                            const value = inv.currentValue ?? 0;
                            const pl = value - inv.costBasis;
                            const plPct = inv.costBasis > 0 ? (pl / inv.costBasis) * 100 : 0;
                            const plColor = pl >= 0 ? '#10a37f' : '#ef4444';
                            return (
                              <tr key={inv.id} style={{ borderTop: '1px solid #1a1a1a' }}>
                                <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{inv.symbol ?? '-'}</td>
                                <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)' }}>{inv.name}</td>
                                <td className="px-4 py-3 text-[12px]" style={{ color: '#888' }}>{inv.assetType}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)' }}>{inv.quantity % 1 === 0 ? inv.quantity : inv.quantity.toFixed(4)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: '#888', fontFamily: 'var(--font-mono)' }}>{formatCents(Math.round(avgCost), inv.currency)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{inv.currentPrice != null ? formatCents(inv.currentPrice, inv.currency) : '-'}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{formatCents(value, inv.currency)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: plColor, fontFamily: 'var(--font-mono)' }}>{formatCentsSigned(pl, inv.currency)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: plColor, fontFamily: 'var(--font-mono)' }}>{plPct >= 0 ? '+' : ''}{plPct.toFixed(1)}%</td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                </>
              );
            })()}
          </div>
        )}

        {/* Goals Tab */}
        {activeTab === 'goals' && (
          <div>
            <div className="mb-4 flex justify-end">
              {/* TODO: Wire up create goal — POST /api/finance/goals */}
              <button className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />
                Create Goal
              </button>
            </div>

            {goals.loading && <LoadingState message="Loading goals..." />}
            {!goals.loading && goals.error && <ErrorState message={goals.error.message} />}
            {!goals.loading && !goals.error && (goals.data?.length ?? 0) === 0 && (
              <EmptyState message="No financial goals yet. Create one to start tracking your progress." />
            )}

            {!goals.loading && !goals.error && (goals.data?.length ?? 0) > 0 && (
              <div className="space-y-4">
                {(goals.data ?? []).map((goal) => {
                  const pct = goal.targetAmount > 0 ? Math.round((goal.currentAmount / goal.targetAmount) * 100) : 0;
                  const isFire = goal.goalType.toLowerCase() === 'fire';
                  const tagColor = isFire ? '#f97316' : goal.goalType === 'savings' ? '#10a37f' : '#3b82f6';
                  const tagLabel = isFire ? 'FIRE' : goal.goalType.charAt(0).toUpperCase() + goal.goalType.slice(1);

                  // Compute deadline display
                  let deadlineDisplay: string | null = null;
                  if (goal.deadline) {
                    const deadlineDate = new Date(goal.deadline);
                    const now = new Date();
                    const monthsLeft = Math.max(0, Math.round((deadlineDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24 * 30)));
                    const dateStr = deadlineDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' });
                    deadlineDisplay = monthsLeft > 0 ? `${dateStr} \u00B7 ${monthsLeft} months left` : dateStr;
                  }

                  const contribDisplay = goal.monthlyContribution != null
                    ? `${formatCents(goal.monthlyContribution, goal.currency)}/mo`
                    : null;

                  return (
                    <div key={goal.id} className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-center justify-between mb-3">
                        <div className="flex items-center gap-2">
                          <span className="text-[16px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{goal.name}</span>
                          <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: tagColor + '20', color: tagColor }}>{tagLabel}</span>
                        </div>
                        <div className="flex items-center gap-1.5 text-[12px]" style={{ color: goal.status === 'active' ? '#10a37f' : '#888' }}>
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: goal.status === 'active' ? '#10a37f' : '#888' }} />
                          {goal.status.charAt(0).toUpperCase() + goal.status.slice(1)}
                        </div>
                      </div>
                      <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: '#10a37f' }} />
                      </div>
                      <div className="flex items-center justify-between mb-3">
                        <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                          {formatCents(goal.currentAmount, goal.currency)} / {formatCents(goal.targetAmount, goal.currency)}
                        </span>
                        <span className="text-[14px]" style={{ color: '#10a37f', fontWeight: 500 }}>{pct}%</span>
                      </div>

                      {/* FIRE-specific details */}
                      {isFire && (goal.expectedReturnRate != null || goal.inflationRate != null) && (
                        <div className="grid grid-cols-2 gap-3 mb-3 p-3 rounded" style={{ backgroundColor: '#1a1a1a' }}>
                          {goal.monthlyContribution != null && (
                            <div>
                              <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Monthly Contribution</div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{formatCents(goal.monthlyContribution, goal.currency)}</div>
                            </div>
                          )}
                          {goal.expectedReturnRate != null && (
                            <div>
                              <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Expected Return</div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{goal.expectedReturnRate}%</div>
                            </div>
                          )}
                          {goal.inflationRate != null && (
                            <div>
                              <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Inflation</div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{goal.inflationRate}%</div>
                            </div>
                          )}
                          {goal.deadline && (
                            <div>
                              <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Target Date</div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>
                                {new Date(goal.deadline).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}
                              </div>
                            </div>
                          )}
                        </div>
                      )}

                      {/* What-if toggle for FIRE goals */}
                      {isFire && (
                        <>
                          <button
                            onClick={() => setExpandedGoal(expandedGoal === goal.id ? null : goal.id)}
                            className="text-[12px]"
                            style={{ color: 'var(--codex-accent)' }}
                          >
                            What-if {expandedGoal === goal.id ? '\u25B4' : '\u25BE'}
                          </button>

                          {expandedGoal === goal.id && (
                            <div className="mt-3 p-3 rounded space-y-3" style={{ backgroundColor: '#1a1a1a' }}>
                              {/* TODO: Wire up what-if calculator with actual projections */}
                              <div>
                                <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Extra monthly savings: $0</label>
                                <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                              </div>
                              <div>
                                <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Expected return: {goal.expectedReturnRate ?? 10}%</label>
                                <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                              </div>
                              <div>
                                <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Inflation: {goal.inflationRate ?? 3}%</label>
                                <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                              </div>
                              <div className="text-[13px] pt-2 border-t" style={{ color: 'var(--codex-accent)', borderColor: '#2a2a2a' }}>
                                What-if projections coming soon
                              </div>
                            </div>
                          )}
                        </>
                      )}

                      {/* Non-FIRE goal footer */}
                      {!isFire && (
                        <div className="flex items-center justify-between text-[12px]" style={{ color: '#888' }}>
                          {deadlineDisplay && <span>Deadline: {deadlineDisplay}</span>}
                          {contribDisplay && <span>{contribDisplay} contribution</span>}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* Reports Tab */}
        {/* TODO: Wire reports to real data — requires server-side aggregation endpoints
            or client-side aggregation from transactions. Keeping mock charts for now. */}
        {activeTab === 'reports' && (
          <div>
            <div className="mb-6 flex items-center gap-4">
              <div className="flex gap-2">
                {(['weekly', 'monthly', 'yearly'] as ReportPeriod[]).map((period) => (
                  <button
                    key={period}
                    onClick={() => setReportPeriod(period)}
                    className="px-3 py-1.5 rounded-full text-[12px] capitalize"
                    style={{
                      backgroundColor: reportPeriod === period ? 'var(--codex-accent)' : 'transparent',
                      color: reportPeriod === period ? 'white' : 'var(--codex-fg-subtle)',
                      border: `1px solid ${reportPeriod === period ? 'var(--codex-accent)' : 'var(--codex-border)'}`
                    }}
                  >
                    {period}
                  </button>
                ))}
              </div>
              <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[13px]" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)' }}>
                <ChevronLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
                February 2026
                <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
              </div>
            </div>

            <div className="space-y-4">
              {/* TODO: Replace with real spending-by-category aggregation from transactions */}
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Spending by Category</h3>
                {transactions.loading && <LoadingState message="Loading..." />}
                {!transactions.loading && transactions.error && <ErrorState message={transactions.error.message} />}
                {!transactions.loading && !transactions.error && (() => {
                  const txs = transactions.data ?? [];
                  const now = new Date();
                  const expenses = txs.filter((tx) => {
                    const d = new Date(tx.txDate);
                    return tx.txType === 'expense' && d.getMonth() === now.getMonth() && d.getFullYear() === now.getFullYear();
                  });
                  const byCat: Record<string, number> = {};
                  for (const tx of expenses) {
                    const cat = tx.category ?? 'Other';
                    byCat[cat] = (byCat[cat] ?? 0) + Math.abs(tx.amount);
                  }
                  const sorted = Object.entries(byCat).sort(([, a], [, b]) => b - a);
                  const maxAmount = sorted[0]?.[1] ?? 1;
                  const catColors = ['#10a37f', '#3b82f6', '#f97316', '#8b5cf6', '#ec4899', '#666'];

                  if (sorted.length === 0) {
                    return <div className="text-[13px]" style={{ color: '#888' }}>No expense data for this period</div>;
                  }

                  return (
                    <div className="space-y-2">
                      {sorted.map(([cat, amount], i) => (
                        <div key={cat} className="flex items-center gap-3">
                          <div className="w-32 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>{cat}</div>
                          <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                            <div style={{ width: `${(amount / maxAmount) * 100}%`, height: '100%', backgroundColor: catColors[i] ?? '#666' }} />
                          </div>
                          <div className="w-20 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                            {formatCents(amount)}
                          </div>
                        </div>
                      ))}
                    </div>
                  );
                })()}
              </div>

              {/* TODO: Replace with real income-by-source aggregation from transactions */}
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Income by Source</h3>
                {transactions.loading && <LoadingState message="Loading..." />}
                {!transactions.loading && transactions.error && <ErrorState message={transactions.error.message} />}
                {!transactions.loading && !transactions.error && (() => {
                  const txs = transactions.data ?? [];
                  const now = new Date();
                  const incomes = txs.filter((tx) => {
                    const d = new Date(tx.txDate);
                    return tx.txType === 'income' && d.getMonth() === now.getMonth() && d.getFullYear() === now.getFullYear();
                  });
                  const bySrc: Record<string, number> = {};
                  for (const tx of incomes) {
                    const src = tx.category ?? tx.counterparty ?? 'Other';
                    bySrc[src] = (bySrc[src] ?? 0) + tx.amount;
                  }
                  const sorted = Object.entries(bySrc).sort(([, a], [, b]) => b - a);
                  const maxAmount = sorted[0]?.[1] ?? 1;
                  const srcColors = ['#10a37f', '#3b82f6', '#14b8a6', '#8b5cf6', '#666'];

                  if (sorted.length === 0) {
                    return <div className="text-[13px]" style={{ color: '#888' }}>No income data for this period</div>;
                  }

                  return (
                    <div className="space-y-2">
                      {sorted.map(([src, amount], i) => (
                        <div key={src} className="flex items-center gap-3">
                          <div className="w-32 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>{src}</div>
                          <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                            <div style={{ width: `${(amount / maxAmount) * 100}%`, height: '100%', backgroundColor: srcColors[i] ?? '#666' }} />
                          </div>
                          <div className="w-20 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                            {formatCents(amount)}
                          </div>
                        </div>
                      ))}
                    </div>
                  );
                })()}
              </div>

              {/* TODO: Replace with real time-series chart from historical transaction data */}
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Spending Trends</h3>
                <div className="h-48 relative" style={{ backgroundColor: '#0d0d0d', borderRadius: '8px', padding: '20px' }}>
                  <svg width="100%" height="100%" viewBox="0 0 500 150" preserveAspectRatio="none">
                    <defs>
                      <linearGradient id="lineGradient" x1="0%" y1="0%" x2="0%" y2="100%">
                        <stop offset="0%" stopColor="#10a37f" stopOpacity="0.3" />
                        <stop offset="100%" stopColor="#10a37f" stopOpacity="0" />
                      </linearGradient>
                    </defs>
                    <path d="M 0 50 L 100 30 L 200 70 L 300 40 L 400 52 L 500 60" fill="url(#lineGradient)" />
                    <path d="M 0 50 L 100 30 L 200 70 L 300 40 L 400 52 L 500 60" fill="none" stroke="#10a37f" strokeWidth="2" />
                    <circle cx="0" cy="50" r="3" fill="#10a37f" />
                    <circle cx="100" cy="30" r="3" fill="#10a37f" />
                    <circle cx="200" cy="70" r="3" fill="#10a37f" />
                    <circle cx="300" cy="40" r="3" fill="#10a37f" />
                    <circle cx="400" cy="52" r="3" fill="#10a37f" />
                    <circle cx="500" cy="60" r="3" fill="#10a37f" />
                  </svg>
                  <div className="absolute bottom-2 left-0 right-0 flex justify-between px-5 text-[11px]" style={{ color: '#666' }}>
                    <span>Oct</span>
                    <span>Nov</span>
                    <span>Dec</span>
                    <span>Jan</span>
                    <span>Feb</span>
                  </div>
                </div>
              </div>

              {/* TODO: Replace with real net worth time-series from account/investment snapshots */}
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Net Worth Over Time</h3>
                <div className="h-48 relative" style={{ backgroundColor: '#0d0d0d', borderRadius: '8px', padding: '20px' }}>
                  <svg width="100%" height="100%" viewBox="0 0 600 150" preserveAspectRatio="none">
                    <defs>
                      <linearGradient id="assetGradient" x1="0%" y1="0%" x2="0%" y2="100%">
                        <stop offset="0%" stopColor="#10a37f" stopOpacity="0.3" />
                        <stop offset="100%" stopColor="#10a37f" stopOpacity="0.1" />
                      </linearGradient>
                    </defs>
                    <path d="M 0 60 L 100 55 L 200 50 L 300 45 L 400 42 L 500 38 L 500 150 L 0 150 Z" fill="url(#assetGradient)" />
                    <path d="M 0 60 L 100 55 L 200 50 L 300 45 L 400 42 L 500 38" fill="none" stroke="white" strokeWidth="2" />
                    <path d="M 0 140 L 100 138 L 200 136 L 300 135 L 400 134 L 500 133 L 500 150 L 0 150 Z" fill="rgba(239, 68, 68, 0.2)" />
                  </svg>
                  <div className="absolute bottom-2 left-0 right-0 flex justify-between px-5 text-[11px]" style={{ color: '#666' }}>
                    <span>Sep</span>
                    <span>Oct</span>
                    <span>Nov</span>
                    <span>Dec</span>
                    <span>Jan</span>
                    <span>Feb</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
