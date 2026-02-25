import { useState } from 'react';
import { ChevronLeft, ChevronRight, TrendingUp, TrendingDown, Plus, RefreshCw, AlertCircle, Target } from 'lucide-react';

// TODO: Replace all mock data in this file with real API calls:
//   - Dashboard summary: compute from /api/finance/accounts, /api/finance/transactions, /api/finance/budgets/usage, /api/finance/investments, /api/finance/goals
//   - Transactions tab: useApi<FinanceTransaction[]>('/api/finance/transactions')
//   - Budgets tab: useApi<BudgetUsage[]>('/api/finance/budgets/usage')
//   - Investments tab: useApi<FinanceInvestment[]>('/api/finance/investments')
//   - Goals tab: useApi<FinanceGoal[]>('/api/finance/goals')
//   - Reports tab: aggregate from transaction data
//   - All write operations: POST/PATCH/DELETE for each resource

type Tab = 'dashboard' | 'transactions' | 'budgets' | 'investments' | 'goals' | 'reports';
type BudgetMode = 'standard' | 'six-jar';
type ReportPeriod = 'weekly' | 'monthly' | 'yearly';

export default function Finance() {
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');
  const [budgetMode, setBudgetMode] = useState<BudgetMode>('standard');
  const [reportPeriod, setReportPeriod] = useState<ReportPeriod>('monthly');
  const [expandedGoal, setExpandedGoal] = useState<string | null>(null);

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
                $24,350.00
              </div>
              <div className="flex items-center gap-1.5 text-[13px]" style={{ color: '#10a37f' }}>
                <TrendingUp className="w-4 h-4" strokeWidth={1.5} />
                +$1,200 this month
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
                $3,420.00
              </div>
              <div className="h-1 rounded-full mb-2 flex overflow-hidden" style={{ backgroundColor: '#0d0d0d' }}>
                <div style={{ width: '35%', backgroundColor: '#10a37f' }} />
                <div style={{ width: '26%', backgroundColor: '#f97316' }} />
                <div style={{ width: '23%', backgroundColor: '#8b5cf6' }} />
                <div style={{ width: '16%', backgroundColor: '#666' }} />
              </div>
              <div className="text-[11px]" style={{ color: '#888' }}>
                Food $1,200 &middot; Transport $800 &middot; Rent $900
              </div>
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
              <div className="flex items-center gap-4 mb-3">
                <div className="relative" style={{ width: '64px', height: '64px' }}>
                  <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                    <circle cx="32" cy="32" r="28" fill="none" stroke="#1a1a1a" strokeWidth="6" />
                    <circle
                      cx="32" cy="32" r="28"
                      fill="none"
                      stroke="#10a37f"
                      strokeWidth="6"
                      strokeDasharray={`${2 * Math.PI * 28 * 0.72} ${2 * Math.PI * 28}`}
                      strokeLinecap="round"
                    />
                  </svg>
                  <div className="absolute inset-0 flex items-center justify-center text-[18px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                    72%
                  </div>
                </div>
                <div>
                  <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                    3 of 4 budgets on track
                  </div>
                  <div className="flex items-center gap-1.5 text-[12px]" style={{ color: '#ef4444' }}>
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#ef4444' }} />
                    1 over limit
                  </div>
                </div>
              </div>
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
                $12,450.00
              </div>
              <div className="flex items-center gap-1.5 text-[13px]" style={{ color: '#10a37f' }}>
                <TrendingUp className="w-4 h-4" strokeWidth={1.5} />
                +$1,230 (+10.9%)
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
                <span className="text-[28px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>3</span>
                <span className="text-[14px]" style={{ color: '#888' }}>active</span>
              </div>
              <div className="space-y-2 mb-2">
                <div>
                  <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Emergency Fund</div>
                  <div className="h-0.5 rounded-full overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                    <div style={{ width: '65%', height: '100%', backgroundColor: '#10a37f' }} />
                  </div>
                </div>
                <div>
                  <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>New Laptop</div>
                  <div className="h-0.5 rounded-full overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                    <div style={{ width: '40%', height: '100%', backgroundColor: '#10a37f' }} />
                  </div>
                </div>
                <div>
                  <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>FIRE</div>
                  <div className="h-0.5 rounded-full overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                    <div style={{ width: '12%', height: '100%', backgroundColor: '#10a37f' }} />
                  </div>
                </div>
              </div>
              <div className="text-[11px]" style={{ color: '#888' }}>
                Next deadline: Jun 2027
              </div>
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
              <div className="space-y-2">
                {[
                  { cat: 'Groceries', date: 'Feb 24', amount: '-$50.00', color: '#ef4444' },
                  { cat: 'Monthly Salary', date: 'Feb 20', amount: '+$2,000.00', color: '#10a37f' },
                  { cat: 'Transfer to Savings', date: 'Feb 18', amount: '-$500.00', color: '#3b82f6' },
                  { cat: 'Electric Bill', date: 'Feb 15', amount: '-$120.00', color: '#ef4444' },
                  { cat: 'Coffee Shop', date: 'Feb 14', amount: '-$8.50', color: '#ef4444' },
                ].map((tx, i) => (
                  <div key={i} className="flex items-start justify-between">
                    <div>
                      <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{tx.cat}</div>
                      <div className="text-[11px]" style={{ color: '#888' }}>{tx.date}</div>
                    </div>
                    <div className="text-[13px]" style={{ color: tx.color, fontFamily: 'var(--font-mono)' }}>
                      {tx.amount}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Transactions Tab */}
        {activeTab === 'transactions' && (
          <div>
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
              <div className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)' }}>
                <ChevronLeft className="w-3 h-3" strokeWidth={1.5} />
                Feb 2026
                <ChevronRight className="w-3 h-3" strokeWidth={1.5} />
              </div>
              <select className="px-3 py-1.5 rounded text-[12px] outline-none" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}>
                <option>Category</option>
              </select>
            </div>

            <div className="space-y-1.5">
              {[
                { dot: '#ef4444', title: 'Groceries', sub: 'Food & Drink', amount: '-$50.00', color: '#ef4444', date: 'Feb 24', account: 'Main Bank' },
                { dot: '#10a37f', title: 'Monthly Salary', sub: 'Income', amount: '+$2,000.00', color: '#10a37f', date: 'Feb 20', account: 'Main Bank' },
                { dot: '#3b82f6', title: 'Transfer to Savings', sub: '\u2192 Savings Account', amount: '-$500.00', color: '#3b82f6', date: 'Feb 18', account: 'Main Bank' },
                { dot: '#ef4444', title: 'Electric Bill', sub: 'Utilities', amount: '-$120.00', color: '#ef4444', date: 'Feb 15', account: 'Main Bank' },
                { dot: '#ef4444', title: 'Coffee Shop', sub: 'Food & Drink', amount: '-$8.50', color: '#ef4444', date: 'Feb 14', account: 'Main Bank', recurring: true },
                { dot: '#10a37f', title: 'Freelance Payment', sub: 'Income', amount: '+$500.00', color: '#10a37f', date: 'Feb 10', account: 'Main Bank' },
              ].map((tx, i) => (
                <div key={i} className="flex items-center justify-between p-3 rounded border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                  <div className="flex items-center gap-3">
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: tx.dot }} />
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{tx.title}</span>
                        {tx.recurring && <span className="text-[11px]" style={{ color: '#888' }}>\u21BB</span>}
                      </div>
                      <div className="text-[12px]" style={{ color: '#888' }}>{tx.sub}</div>
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="text-[14px]" style={{ color: tx.color, fontFamily: 'var(--font-mono)', fontWeight: 500 }}>
                      {tx.amount}
                    </div>
                    <div className="text-[11px]" style={{ color: '#888' }}>
                      {tx.date} &middot; {tx.account}
                    </div>
                  </div>
                </div>
              ))}
            </div>
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
              <button className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />
                Create Budget
              </button>
            </div>

            {budgetMode === 'standard' && (
              <div className="space-y-3">
                {[
                  { name: 'Food & Drink', spent: 1360, total: 2000, pct: 68, color: '#10a37f' },
                  { name: 'Transport', spent: 425, total: 500, pct: 85, color: '#eab308' },
                  { name: 'Entertainment', spent: 330, total: 300, pct: 110, color: '#ef4444' },
                  { name: 'Utilities', spent: 135, total: 300, pct: 45, color: '#10a37f' },
                ].map((budget, i) => (
                  <div key={i} className="p-4 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{budget.name}</span>
                        <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#1a1a1a', color: '#888' }}>Monthly</span>
                      </div>
                      <div className="text-[13px]" style={{ color: budget.color }}>
                        {budget.pct}%{budget.pct > 100 ? ' Over!' : budget.pct > 80 ? ' \u26A0' : ''}
                      </div>
                    </div>
                    <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                      <div style={{ width: `${Math.min(budget.pct, 100)}%`, height: '100%', backgroundColor: budget.color }} />
                    </div>
                    <div className="text-[12px] text-right" style={{ color: 'var(--codex-fg-subtle)' }}>
                      ${budget.spent} / ${budget.total}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {budgetMode === 'six-jar' && (
              <div className="grid grid-cols-3 gap-4">
                {[
                  { name: 'Essentials', color: '#3b82f6', pct: 72, spent: 1100, total: 1540, income: '55% of income' },
                  { name: 'Savings', color: '#10a37f', pct: 35, spent: 175, total: 500, income: '10% of income' },
                  { name: 'Investment', color: '#14b8a6', pct: 60, spent: 300, total: 500, income: '10% of income' },
                  { name: 'Education', color: '#8b5cf6', pct: 20, spent: 100, total: 500, income: '10% of income' },
                  { name: 'Entertainment', color: '#f97316', pct: 90, spent: 450, total: 500, income: '10% of income' },
                  { name: 'Charity', color: '#ec4899', pct: 40, spent: 100, total: 250, income: '5% of income' },
                ].map((jar, i) => (
                  <div key={i} className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a', borderLeft: `3px solid ${jar.color}` }}>
                    <div className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{jar.name}</div>
                    <div className="flex justify-center mb-3">
                      <div className="relative" style={{ width: '56px', height: '56px' }}>
                        <svg viewBox="0 0 56 56" style={{ transform: 'rotate(-90deg)' }}>
                          <circle cx="28" cy="28" r="24" fill="none" stroke="#1a1a1a" strokeWidth="5" />
                          <circle
                            cx="28" cy="28" r="24"
                            fill="none"
                            stroke={jar.color}
                            strokeWidth="5"
                            strokeDasharray={`${2 * Math.PI * 24 * (jar.pct / 100)} ${2 * Math.PI * 24}`}
                            strokeLinecap="round"
                          />
                        </svg>
                        <div className="absolute inset-0 flex items-center justify-center text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>
                          {jar.pct}%
                        </div>
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>
                        ${jar.spent} / ${jar.total}
                      </div>
                      <div className="text-[11px]" style={{ color: '#888' }}>
                        {jar.income}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Investments Tab */}
        {activeTab === 'investments' && (
          <div>
            <div className="mb-4 p-5 rounded-lg border flex items-center justify-between" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
              <div>
                <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Value</div>
                <div className="text-[20px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>$12,450.00</div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Gain/Loss</div>
                <div className="text-[16px]" style={{ color: '#10a37f', fontWeight: 500 }}>+$1,230 (+10.9%)</div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: '#888' }}>Day Change</div>
                <div className="text-[16px]" style={{ color: '#10a37f', fontWeight: 500 }}>+$45.20 (+0.36%)</div>
              </div>
              <div className="relative" style={{ width: '64px', height: '64px' }}>
                <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                  <circle cx="32" cy="32" r="28" fill="none" stroke="#10a37f" strokeWidth="8" strokeDasharray={`${2 * Math.PI * 28 * 0.45} ${2 * Math.PI * 28}`} />
                  <circle cx="32" cy="32" r="28" fill="none" stroke="#f97316" strokeWidth="8" strokeDasharray={`${2 * Math.PI * 28 * 0.30} ${2 * Math.PI * 28}`} strokeDashoffset={`${-2 * Math.PI * 28 * 0.45}`} />
                  <circle cx="32" cy="32" r="28" fill="none" stroke="#3b82f6" strokeWidth="8" strokeDasharray={`${2 * Math.PI * 28 * 0.20} ${2 * Math.PI * 28}`} strokeDashoffset={`${-2 * Math.PI * 28 * 0.75}`} />
                  <circle cx="32" cy="32" r="28" fill="none" stroke="#666" strokeWidth="8" strokeDasharray={`${2 * Math.PI * 28 * 0.05} ${2 * Math.PI * 28}`} strokeDashoffset={`${-2 * Math.PI * 28 * 0.95}`} />
                </svg>
              </div>
            </div>

            <div className="mb-4 flex items-center justify-between">
              <select className="px-3 py-1.5 rounded text-[13px] outline-none" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}>
                <option>All Portfolios</option>
              </select>
              <div className="flex gap-2">
                <button className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                  <Plus className="w-3 h-3" strokeWidth={1.5} />
                  Add Holding
                </button>
                <button className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'transparent', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}>
                  <RefreshCw className="w-3 h-3" strokeWidth={1.5} />
                  Refresh Prices
                </button>
              </div>
            </div>

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
                  {[
                    { symbol: 'AAPL', name: 'Apple Inc.', type: 'Stock', qty: '10', avgCost: '$150.00', price: '$178.50', value: '$1,785.00', pl: '+$285.00', plPct: '+19.0%', plColor: '#10a37f' },
                    { symbol: 'BTC', name: 'Bitcoin', type: 'Crypto', qty: '0.5', avgCost: '$30,000', price: '$51,200', value: '$25,600.00', pl: '+$10,600', plPct: '+70.7%', plColor: '#10a37f' },
                    { symbol: 'VTI', name: 'Vanguard Total', type: 'ETF', qty: '25', avgCost: '$200.00', price: '$225.40', value: '$5,635.00', pl: '+$635.00', plPct: '+12.7%', plColor: '#10a37f' },
                    { symbol: 'VBTLX', name: 'Vanguard Bond', type: 'Bond', qty: '50', avgCost: '$10.00', price: '$9.80', value: '$490.00', pl: '-$10.00', plPct: '-2.0%', plColor: '#ef4444' },
                  ].map((holding, i) => (
                    <tr key={i} style={{ borderTop: '1px solid #1a1a1a' }}>
                      <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{holding.symbol}</td>
                      <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)' }}>{holding.name}</td>
                      <td className="px-4 py-3 text-[12px]" style={{ color: '#888' }}>{holding.type}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)' }}>{holding.qty}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: '#888', fontFamily: 'var(--font-mono)' }}>{holding.avgCost}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{holding.price}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{holding.value}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: holding.plColor, fontFamily: 'var(--font-mono)' }}>{holding.pl}</td>
                      <td className="px-4 py-3 text-[13px] text-right" style={{ color: holding.plColor, fontFamily: 'var(--font-mono)' }}>{holding.plPct}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* Goals Tab */}
        {activeTab === 'goals' && (
          <div>
            <div className="mb-4 flex justify-end">
              <button className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5" style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />
                Create Goal
              </button>
            </div>

            <div className="space-y-4">
              {[
                { name: 'Emergency Fund', tag: 'Savings', tagColor: '#10a37f', pct: 65, current: 6500, target: 10000, deadline: 'Dec 2026 \u00B7 10 months left', contrib: '$500/mo contribution' },
                { name: 'New Laptop', tag: 'Purchase', tagColor: '#3b82f6', pct: 40, current: 800, target: 2000, deadline: 'Aug 2026 \u00B7 6 months left', contrib: '$200/mo' },
              ].map((goal, i) => (
                <div key={i} className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <span className="text-[16px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{goal.name}</span>
                      <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: goal.tagColor + '20', color: goal.tagColor }}>{goal.tag}</span>
                    </div>
                    <div className="flex items-center gap-1.5 text-[12px]" style={{ color: '#10a37f' }}>
                      <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#10a37f' }} />
                      Active
                    </div>
                  </div>
                  <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                    <div style={{ width: `${goal.pct}%`, height: '100%', backgroundColor: '#10a37f' }} />
                  </div>
                  <div className="flex items-center justify-between mb-3">
                    <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>${goal.current.toLocaleString()} / ${goal.target.toLocaleString()}</span>
                    <span className="text-[14px]" style={{ color: '#10a37f', fontWeight: 500 }}>{goal.pct}%</span>
                  </div>
                  <div className="flex items-center justify-between text-[12px]" style={{ color: '#888' }}>
                    <span>Deadline: {goal.deadline}</span>
                    <span>{goal.contrib}</span>
                  </div>
                </div>
              ))}

              {/* FIRE Goal with What-if */}
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <span className="text-[16px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Financial Independence</span>
                    <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#f9731620', color: '#f97316' }}>FIRE</span>
                  </div>
                  <div className="flex items-center gap-1.5 text-[12px]" style={{ color: '#10a37f' }}>
                    <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#10a37f' }} />
                    Active
                  </div>
                </div>
                <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                  <div style={{ width: '12%', height: '100%', backgroundColor: '#10a37f' }} />
                </div>
                <div className="flex items-center justify-between mb-4">
                  <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>$120,000 / $1,000,000</span>
                  <span className="text-[14px]" style={{ color: '#10a37f', fontWeight: 500 }}>12%</span>
                </div>

                <div className="grid grid-cols-2 gap-3 mb-3 p-3 rounded" style={{ backgroundColor: '#1a1a1a' }}>
                  <div>
                    <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Withdrawal Rate</div>
                    <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>4%</div>
                  </div>
                  <div>
                    <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Expected Return</div>
                    <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>10%</div>
                  </div>
                  <div>
                    <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Inflation</div>
                    <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>3.3%</div>
                  </div>
                  <div>
                    <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Projected FIRE</div>
                    <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>2042</div>
                  </div>
                </div>

                <button
                  onClick={() => setExpandedGoal(expandedGoal === 'fire' ? null : 'fire')}
                  className="text-[12px]"
                  style={{ color: 'var(--codex-accent)' }}
                >
                  What-if {expandedGoal === 'fire' ? '\u25B4' : '\u25BE'}
                </button>

                {expandedGoal === 'fire' && (
                  <div className="mt-3 p-3 rounded space-y-3" style={{ backgroundColor: '#1a1a1a' }}>
                    <div>
                      <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Extra monthly savings: $0</label>
                      <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                    </div>
                    <div>
                      <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Expected return: 10%</label>
                      <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                    </div>
                    <div>
                      <label className="text-[12px] mb-1 block" style={{ color: '#888' }}>Inflation: 3.3%</label>
                      <input type="range" className="w-full" style={{ accentColor: 'var(--codex-accent)' }} />
                    </div>
                    <div className="text-[13px] pt-2 border-t" style={{ color: 'var(--codex-accent)', borderColor: '#2a2a2a' }}>
                      You could reach FIRE by March 2040 (2 years earlier)
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Reports Tab */}
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
              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Spending by Category</h3>
                <div className="space-y-2">
                  {[
                    { name: 'Food & Drink', amount: 1200, color: '#10a37f', pct: 100 },
                    { name: 'Rent', amount: 900, color: '#3b82f6', pct: 75 },
                    { name: 'Transport', amount: 800, color: '#f97316', pct: 66 },
                    { name: 'Utilities', amount: 420, color: '#8b5cf6', pct: 35 },
                    { name: 'Entertainment', amount: 330, color: '#ec4899', pct: 27 },
                    { name: 'Other', amount: 170, color: '#666', pct: 14 },
                  ].map((cat, i) => (
                    <div key={i} className="flex items-center gap-3">
                      <div className="w-32 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>{cat.name}</div>
                      <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${cat.pct}%`, height: '100%', backgroundColor: cat.color }} />
                      </div>
                      <div className="w-20 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                        ${cat.amount}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Income by Source</h3>
                <div className="space-y-2">
                  {[
                    { name: 'Salary', amount: 4000, color: '#10a37f', pct: 100 },
                    { name: 'Freelance', amount: 500, color: '#3b82f6', pct: 12.5 },
                    { name: 'Dividends', amount: 120, color: '#14b8a6', pct: 3 },
                  ].map((src, i) => (
                    <div key={i} className="flex items-center gap-3">
                      <div className="w-32 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>{src.name}</div>
                      <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${src.pct}%`, height: '100%', backgroundColor: src.color }} />
                      </div>
                      <div className="w-20 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                        ${src.amount}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

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
