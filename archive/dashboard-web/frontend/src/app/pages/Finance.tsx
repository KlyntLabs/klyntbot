import { useState, useMemo, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import {
  ChevronLeft, ChevronRight, TrendingUp, TrendingDown, Plus, RefreshCw,
  AlertCircle, Target, Loader2, AlertTriangle, X, Pencil, Trash2,
  Building2, CreditCard, Wallet, PiggyBank,
} from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type {
  FinanceAccount,
  FinanceTransaction,
  BudgetUsage,
  FinanceInvestment,
  FinanceGoal,
  FinanceLiability,
} from '../../lib/types';

// ── Currency helpers ─────────────────────────────────────────────────────────

/** Zero-decimal currencies — no subunits, display without fractional digits. */
const ZERO_DECIMAL = new Set(['VND', 'JPY', 'KRW', 'IDR', 'CLP', 'ISK', 'HUF', 'TWD']);

/** Prefix symbols for well-known currencies. */
const SYMBOLS: Record<string, string> = {
  USD: '$', EUR: '€', GBP: '£', JPY: '¥', KRW: '₩', VND: '₫',
  CNY: '¥', THB: '฿', BTC: '₿',
};

const CURRENCY_OPTIONS = ['VND', 'USD', 'USDT', 'EUR', 'GBP', 'JPY', 'BTC', 'ETH'];

function formatCents(cents: number, currency = 'USD'): string {
  const abs = Math.abs(cents);
  const decimals = ZERO_DECIMAL.has(currency) ? 0 : 2;
  const value = abs / 100;
  const formatted = value.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  const sym = SYMBOLS[currency];
  if (sym) return cents < 0 ? `-${sym}${formatted}` : `${sym}${formatted}`;
  return cents < 0 ? `-${formatted} ${currency}` : `${formatted} ${currency}`;
}

function formatCentsSigned(cents: number, currency = 'USD'): string {
  const abs = Math.abs(cents);
  const decimals = ZERO_DECIMAL.has(currency) ? 0 : 2;
  const value = abs / 100;
  const formatted = value.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  const sym = SYMBOLS[currency];
  if (sym) return cents >= 0 ? `+${sym}${formatted}` : `-${sym}${formatted}`;
  return cents >= 0 ? `+${formatted} ${currency}` : `-${formatted} ${currency}`;
}

// ── Exchange rate conversion ────────────────────────────────────────────────

/**
 * All rates are "value of 1 unit in USD" — a universal reference.
 * convertCents(cents, from, to) = cents * rates[from] / rates[to]
 * Rates are ALWAYS fetched live — no hardcoded fallbacks for accuracy.
 */

const CRYPTO_IDS: Record<string, string> = { BTC: 'bitcoin', ETH: 'ethereum', USDT: 'tether' };
const RATE_CACHE_KEY = 'klyntbot_exchange_rates';
const RATE_TTL_MS = 3_600_000; // 1 hour

interface CachedRates { rates: Record<string, number>; fetchedAt: number; }

function loadCachedRates(): CachedRates | null {
  try {
    const raw = localStorage.getItem(RATE_CACHE_KEY);
    if (!raw) return null;
    const parsed: CachedRates = JSON.parse(raw);
    if (Date.now() - parsed.fetchedAt < RATE_TTL_MS) return parsed;
    return null;
  } catch { return null; }
}

function saveCachedRates(rates: Record<string, number>) {
  localStorage.setItem(RATE_CACHE_KEY, JSON.stringify({ rates, fetchedAt: Date.now() }));
}

/**
 * Fetch live rates — all expressed as "value of 1 unit in USD".
 * Fiat from open.er-api.com (free, no API key, 150+ currencies incl. VND).
 * Crypto from CoinGecko (free, rate-limited).
 */
async function fetchLiveRates(): Promise<Record<string, number>> {
  const rates: Record<string, number> = { USD: 1 };

  // Fiat: open.er-api.com returns { rates: { VND: 25880, EUR: 0.849, ... } }
  //       meaning 1 USD = 25880 VND, so 1 VND = 1/25880 USD
  try {
    const res = await fetch('https://open.er-api.com/v6/latest/USD');
    if (res.ok) {
      const data = await res.json();
      if (data.result === 'success' && data.rates) {
        for (const [cur, perUSD] of Object.entries(data.rates as Record<string, number>)) {
          if (cur !== 'USD' && perUSD > 0) rates[cur] = 1 / perUSD;
        }
      }
    }
  } catch { /* fall through — crypto still attempted */ }

  // Crypto: price in USD → rates[symbol] = price directly
  const cryptoIds = Object.values(CRYPTO_IDS).join(',');
  try {
    const res = await fetch(
      `https://api.coingecko.com/api/v3/simple/price?ids=${cryptoIds}&vs_currencies=usd`
    );
    if (res.ok) {
      const data = await res.json();
      for (const [symbol, geckoId] of Object.entries(CRYPTO_IDS)) {
        const price = data[geckoId]?.usd;
        if (price) rates[symbol] = price;
      }
    }
  } catch { /* fall through */ }

  return rates;
}

/** Convert cents from one currency to another using the rates table. */
function convertCents(cents: number, from: string, to: string, rates: Record<string, number>): number {
  if (from === to) return cents;
  const fromRate = rates[from] ?? 1;
  const toRate = rates[to] ?? 1;
  return Math.round(cents * fromRate / toRate);
}

function txColor(txType: string): string {
  switch (txType) {
    case 'income': return '#10a37f';
    case 'transfer': return '#3b82f6';
    default: return '#ef4444';
  }
}

function shortDate(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

function addMonths(date: Date, n: number): Date {
  const d = new Date(date);
  d.setMonth(d.getMonth() + n);
  return d;
}

const todayStr = () => new Date().toISOString().split('T')[0];

// ── Shared UI ────────────────────────────────────────────────────────────────

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

// ── Slide-over drawer ────────────────────────────────────────────────────────

interface DrawerProps {
  title: string;
  open: boolean;
  onClose: () => void;
  onSave: () => void;
  saving: boolean;
  error: string | null;
  children: React.ReactNode;
}

function SlideOverDrawer({ title, open, onClose, onSave, saving, error, children }: DrawerProps) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <>
      <div className="fixed inset-0 z-40" style={{ backgroundColor: 'rgba(0,0,0,0.5)' }} onClick={onClose} />
      <div className="fixed right-0 top-0 bottom-0 z-50 flex flex-col w-[440px]" style={{ backgroundColor: '#141414', borderLeft: '1px solid var(--codex-border)' }}>
        <div className="flex items-center justify-between px-6 py-4 border-b" style={{ borderColor: 'var(--codex-border)' }}>
          <span className="text-[15px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{title}</span>
          <button onClick={onClose} className="p-1.5 rounded transition-colors" style={{ color: 'var(--codex-fg-subtle)' }}
            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#2a2a2a'}
            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}>
            <X className="w-4 h-4" strokeWidth={1.5} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          {children}
          {error && (
            <div className="p-3 rounded text-[13px]" style={{ backgroundColor: 'rgba(239,68,68,0.1)', color: '#ef4444', border: '1px solid rgba(239,68,68,0.3)' }}>
              {error}
            </div>
          )}
        </div>
        <div className="px-6 py-4 border-t flex items-center justify-end gap-3" style={{ borderColor: 'var(--codex-border)' }}>
          <button onClick={onClose} className="px-4 py-2 rounded text-[13px] transition-colors"
            style={{ backgroundColor: 'transparent', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}
            onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
            onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}>Cancel</button>
          <button onClick={onSave} disabled={saving}
            className="px-4 py-2 rounded text-[13px] flex items-center gap-2 transition-colors"
            style={{ backgroundColor: 'var(--codex-accent)', color: 'white', opacity: saving ? 0.6 : 1 }}
            onMouseEnter={(e) => { if (!saving) e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'; }}
            onMouseLeave={(e) => { if (!saving) e.currentTarget.style.backgroundColor = 'var(--codex-accent)'; }}>
            {saving && <Loader2 className="w-3.5 h-3.5 animate-spin" strokeWidth={1.5} />}
            Save
          </button>
        </div>
      </div>
    </>,
    document.body,
  );
}

// ── Form helpers ─────────────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="text-[12px] block mb-1.5" style={{ color: '#888' }}>{label}</label>
      {children}
    </div>
  );
}

const inputCss: React.CSSProperties = {
  backgroundColor: '#1a1a1a', border: '1px solid var(--codex-border)',
  color: 'var(--codex-fg)', borderRadius: '6px', padding: '8px 10px',
  fontSize: '13px', width: '100%', outline: 'none',
};

const selectCss: React.CSSProperties = { ...inputCss, cursor: 'pointer' };

/** Renders a currency <select> with common options. */
function CurrencySelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)} style={selectCss}>
      {CURRENCY_OPTIONS.map(c => <option key={c} value={c}>{c}{SYMBOLS[c] ? ` (${SYMBOLS[c]})` : ''}</option>)}
    </select>
  );
}

/** Returns a currency symbol for form labels, e.g. "₫" or "$" or "USDT". */
function currLabel(currency: string): string {
  return SYMBOLS[currency] ?? currency;
}

// ── Types ────────────────────────────────────────────────────────────────────

type Tab = 'dashboard' | 'transactions' | 'budgets' | 'investments' | 'goals' | 'accounts' | 'liabilities' | 'reports';
type BudgetMode = 'standard' | 'six-jar';
type ReportPeriod = 'weekly' | 'monthly' | 'yearly';
type TxFilter = 'all' | 'income' | 'expense' | 'transfer';

type DrawerMode =
  | { kind: 'none' }
  | { kind: 'create-transaction' }
  | { kind: 'edit-transaction'; item: FinanceTransaction }
  | { kind: 'create-budget' }
  | { kind: 'edit-budget'; item: BudgetUsage }
  | { kind: 'create-investment' }
  | { kind: 'edit-investment'; item: FinanceInvestment }
  | { kind: 'create-goal' }
  | { kind: 'edit-goal'; item: FinanceGoal }
  | { kind: 'create-account' }
  | { kind: 'edit-account'; item: FinanceAccount }
  | { kind: 'create-liability' }
  | { kind: 'edit-liability'; item: FinanceLiability };

// Form state types
interface TxForm { accountId: string; txType: string; amount: string; currency: string; txDate: string; category: string; counterparty: string; notes: string; isRecurring: boolean; }
interface BudgetForm { name: string; amount: string; currency: string; period: string; method: string; jarType: string; category: string; startDate: string; alertThreshold: string; }
interface InvestmentForm { portfolioId: string; assetType: string; symbol: string; name: string; quantity: string; costBasis: string; currency: string; currentPrice: string; currentValue: string; purchaseDate: string; notes: string; }
interface GoalForm { name: string; goalType: string; targetAmount: string; currentAmount: string; currency: string; deadline: string; monthlyContribution: string; expectedReturnRate: string; inflationRate: string; notes: string; }
interface AccountForm { name: string; accountType: string; currency: string; balance: string; institution: string; notes: string; }
interface LiabilityForm { name: string; liabilityType: string; principal: string; remaining: string; currency: string; interestRate: string; monthlyPayment: string; dueDate: string; notes: string; }

function makeEmptyForms(cur: string) {
  return {
    tx: { accountId: '', txType: 'expense', amount: '', currency: cur, txDate: todayStr(), category: '', counterparty: '', notes: '', isRecurring: false } as TxForm,
    budget: { name: '', amount: '', currency: cur, period: 'monthly', method: 'envelope', jarType: '', category: '', startDate: todayStr(), alertThreshold: '80' } as BudgetForm,
    investment: { portfolioId: 'main', assetType: 'stock', symbol: '', name: '', quantity: '', costBasis: '', currency: cur, currentPrice: '', currentValue: '', purchaseDate: '', notes: '' } as InvestmentForm,
    goal: { name: '', goalType: 'savings', targetAmount: '', currentAmount: '0', currency: cur, deadline: '', monthlyContribution: '', expectedReturnRate: '', inflationRate: '', notes: '' } as GoalForm,
    account: { name: '', accountType: 'checking', currency: cur, balance: '0', institution: '', notes: '' } as AccountForm,
    liability: { name: '', liabilityType: 'loan', principal: '', remaining: '', currency: cur, interestRate: '', monthlyPayment: '', dueDate: '', notes: '' } as LiabilityForm,
  };
}

// ── Component ────────────────────────────────────────────────────────────────

export default function Finance() {
  const [activeTab, setActiveTab] = useState<Tab>('dashboard');
  const [budgetMode, setBudgetMode] = useState<BudgetMode>('standard');
  const [reportPeriod, setReportPeriod] = useState<ReportPeriod>('monthly');
  const [expandedGoal, setExpandedGoal] = useState<string | null>(null);

  // Load default currency from config
  const { data: financeSettings } = useApi<{ defaultCurrency?: string }>('/api/settings/finance');
  const defaultCurrency = financeSettings?.defaultCurrency || 'VND';
  const emptyForms = useMemo(() => makeEmptyForms(defaultCurrency), [defaultCurrency]);

  // Drawer state
  const [drawer, setDrawer] = useState<DrawerMode>({ kind: 'none' });
  const [mutating, setMutating] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);

  // Transaction filter state
  const [txTypeFilter, setTxTypeFilter] = useState<TxFilter>('all');
  const [txMonth, setTxMonth] = useState<Date>(new Date());
  const [txCategory, setTxCategory] = useState<string>('all');

  // Delete confirmation
  const [deletingId, setDeletingId] = useState<string | null>(null);

  // Form state
  const [txForm, setTxForm] = useState<TxForm>(emptyForms.tx);
  const [budgetForm, setBudgetForm] = useState<BudgetForm>(emptyForms.budget);
  const [investmentForm, setInvestmentForm] = useState<InvestmentForm>(emptyForms.investment);
  const [goalForm, setGoalForm] = useState<GoalForm>(emptyForms.goal);
  const [accountForm, setAccountForm] = useState<AccountForm>(emptyForms.account);
  const [liabilityForm, setLiabilityForm] = useState<LiabilityForm>(emptyForms.liability);

  // ── API calls ──────────────────────────────────────────────────────────────
  const accounts = useApi<FinanceAccount[]>('/api/finance/accounts');
  const transactions = useApi<FinanceTransaction[]>('/api/finance/transactions');
  const budgets = useApi<BudgetUsage[]>('/api/finance/budgets/usage');
  const investments = useApi<FinanceInvestment[]>('/api/finance/investments');
  const goals = useApi<FinanceGoal[]>('/api/finance/goals');
  const liabilities = useApi<FinanceLiability[]>('/api/finance/liabilities');

  // ── Live exchange rates (no fallbacks — always real rates) ──────────────
  const [rates, setRates] = useState<Record<string, number> | null>(() => loadCachedRates()?.rates ?? null);
  const [ratesLoading, setRatesLoading] = useState(!rates);
  const [ratesError, setRatesError] = useState<string | null>(null);
  const [ratesUpdatedAt, setRatesUpdatedAt] = useState<Date | null>(() => {
    const cached = loadCachedRates();
    return cached ? new Date(cached.fetchedAt) : null;
  });

  useEffect(() => {
    let cancelled = false;
    const cached = loadCachedRates();
    if (cached) {
      setRates(cached.rates);
      setRatesUpdatedAt(new Date(cached.fetchedAt));
      setRatesLoading(false);
      return;
    }
    setRatesLoading(true);
    setRatesError(null);
    fetchLiveRates().then((live) => {
      if (cancelled) return;
      setRates(live);
      setRatesUpdatedAt(new Date());
      setRatesLoading(false);
      saveCachedRates(live);
    }).catch((err) => {
      if (cancelled) return;
      setRatesError(err?.message ?? 'Failed to fetch exchange rates');
      setRatesLoading(false);
    });
    return () => { cancelled = true; };
  }, [defaultCurrency]);

  /** Convert cents from a source currency to the display currency.
   *  When rates aren't loaded yet, returns cents unchanged (no conversion). */
  const cvt = useCallback((cents: number, fromCurrency?: string) => {
    const from = fromCurrency ?? defaultCurrency;
    if (!rates || from === defaultCurrency) return cents;
    return convertCents(cents, from, defaultCurrency, rates);
  }, [defaultCurrency, rates]);

  /** Format with rate conversion: converts to display currency then formats.
   *  When rates aren't loaded, formats in the original currency. */
  const fmt = useCallback((cents: number, currency?: string) => {
    const from = currency ?? defaultCurrency;
    if (!rates || from === defaultCurrency) return formatCents(cents, from);
    return formatCents(convertCents(cents, from, defaultCurrency, rates), defaultCurrency);
  }, [defaultCurrency, rates]);

  const fmtSigned = useCallback((cents: number, currency?: string) => {
    const from = currency ?? defaultCurrency;
    if (!rates || from === defaultCurrency) return formatCentsSigned(cents, from);
    return formatCentsSigned(convertCents(cents, from, defaultCurrency, rates), defaultCurrency);
  }, [defaultCurrency, rates]);

  // ── Populate form on drawer open ───────────────────────────────────────────
  useEffect(() => {
    setMutationError(null);
    switch (drawer.kind) {
      case 'create-transaction': setTxForm(emptyForms.tx); break;
      case 'edit-transaction': {
        const t = drawer.item;
        setTxForm({ accountId: t.accountId, txType: t.txType, amount: String(t.amount / 100), currency: t.currency, txDate: t.txDate, category: t.category ?? '', counterparty: t.counterparty ?? '', notes: t.notes ?? '', isRecurring: t.isRecurring });
        break;
      }
      case 'create-budget': setBudgetForm(emptyForms.budget); break;
      case 'edit-budget': {
        const b = drawer.item;
        setBudgetForm({ name: b.name, amount: String(b.amount / 100), currency: b.currency, period: b.period, method: b.method, jarType: b.jarType ?? '', category: b.category ?? '', startDate: b.startDate, alertThreshold: String(b.alertThreshold) });
        break;
      }
      case 'create-investment': setInvestmentForm(emptyForms.investment); break;
      case 'edit-investment': {
        const inv = drawer.item;
        setInvestmentForm({ portfolioId: inv.portfolioId, assetType: inv.assetType, symbol: inv.symbol ?? '', name: inv.name, quantity: String(inv.quantity), costBasis: String(inv.costBasis / 100), currency: inv.currency, currentPrice: inv.currentPrice != null ? String(inv.currentPrice / 100) : '', currentValue: inv.currentValue != null ? String(inv.currentValue / 100) : '', purchaseDate: inv.purchaseDate ?? '', notes: inv.notes ?? '' });
        break;
      }
      case 'create-goal': setGoalForm(emptyForms.goal); break;
      case 'edit-goal': {
        const g = drawer.item;
        setGoalForm({ name: g.name, goalType: g.goalType, targetAmount: String(g.targetAmount / 100), currentAmount: String(g.currentAmount / 100), currency: g.currency, deadline: g.deadline ?? '', monthlyContribution: g.monthlyContribution != null ? String(g.monthlyContribution / 100) : '', expectedReturnRate: g.expectedReturnRate != null ? String(g.expectedReturnRate) : '', inflationRate: g.inflationRate != null ? String(g.inflationRate) : '', notes: g.notes ?? '' });
        break;
      }
      case 'create-account': setAccountForm(emptyForms.account); break;
      case 'edit-account': {
        const a = drawer.item;
        setAccountForm({ name: a.name, accountType: a.accountType, currency: a.currency, balance: String(a.balance / 100), institution: a.institution ?? '', notes: a.notes ?? '' });
        break;
      }
      case 'create-liability': setLiabilityForm(emptyForms.liability); break;
      case 'edit-liability': {
        const l = drawer.item;
        setLiabilityForm({ name: l.name, liabilityType: l.liabilityType, principal: String(l.principal / 100), remaining: String(l.remaining / 100), currency: l.currency, interestRate: l.interestRate != null ? String(l.interestRate) : '', monthlyPayment: l.monthlyPayment != null ? String(l.monthlyPayment / 100) : '', dueDate: l.dueDate ?? '', notes: l.notes ?? '' });
        break;
      }
    }
  }, [drawer, emptyForms]);

  // ── Derived data ───────────────────────────────────────────────────────────

  const dashboardSummary = useMemo(() => {
    const accts = accounts.data ?? [];
    const txs = transactions.data ?? [];
    const budgs = budgets.data ?? [];
    const invs = investments.data ?? [];
    const gls = goals.data ?? [];
    const liabs = liabilities.data ?? [];

    // All aggregations convert to the display currency first
    const totalAccountBalance = accts.reduce((sum, a) => sum + cvt(a.balance, a.currency), 0);
    const totalInvestmentValue = invs.reduce((sum, inv) => sum + cvt(inv.currentValue ?? 0, inv.currency), 0);
    const totalLiabilityRemaining = liabs.reduce((sum, l) => sum + cvt(l.remaining, l.currency), 0);
    const netWorth = totalAccountBalance + totalInvestmentValue - totalLiabilityRemaining;

    const totalCostBasis = invs.reduce((sum, inv) => sum + cvt(inv.costBasis, inv.currency), 0);
    const investmentPL = totalInvestmentValue - totalCostBasis;
    const investmentPLPct = totalCostBasis > 0 ? (investmentPL / totalCostBasis) * 100 : 0;

    const now = new Date();
    const currentMonth = now.getMonth();
    const currentYear = now.getFullYear();
    const monthlyExpenses = txs.filter((tx) => {
      const d = new Date(tx.txDate);
      return d.getMonth() === currentMonth && d.getFullYear() === currentYear && tx.txType === 'expense';
    });
    const totalMonthlySpending = monthlyExpenses.reduce((sum, tx) => sum + cvt(Math.abs(tx.amount), tx.currency), 0);

    const categorySpending: Record<string, number> = {};
    for (const tx of monthlyExpenses) {
      const cat = tx.category ?? 'Other';
      categorySpending[cat] = (categorySpending[cat] ?? 0) + cvt(Math.abs(tx.amount), tx.currency);
    }
    const topCategories = Object.entries(categorySpending).sort(([, a], [, b]) => b - a).slice(0, 3);

    const activeBudgets = budgs.filter((b) => b.isActive);
    const overBudgetCount = activeBudgets.filter((b) => b.amount > 0 && b.spent > b.amount).length;
    const onTrackCount = activeBudgets.length - overBudgetCount;
    const totalBudgetAmount = activeBudgets.reduce((s, b) => s + b.amount, 0);
    const totalBudgetSpent = activeBudgets.reduce((s, b) => s + b.spent, 0);
    const budgetPct = totalBudgetAmount > 0 ? Math.round((totalBudgetSpent / totalBudgetAmount) * 100) : 0;

    const recentTxs = [...txs].sort((a, b) => new Date(b.txDate).getTime() - new Date(a.txDate).getTime()).slice(0, 5);

    const activeGoals = gls.filter((g) => g.status === 'active');
    const topGoals = activeGoals.slice(0, 3);
    const nextDeadline = activeGoals.filter((g) => g.deadline).sort((a, b) => new Date(a.deadline!).getTime() - new Date(b.deadline!).getTime())[0]?.deadline;

    return { netWorth, totalMonthlySpending, topCategories, budgetPct, activeBudgetCount: activeBudgets.length, onTrackCount, overBudgetCount, totalInvestmentValue, investmentPL, investmentPLPct, recentTxs, activeGoals, topGoals, nextDeadline };
  }, [accounts.data, transactions.data, budgets.data, investments.data, goals.data, liabilities.data, cvt]);

  const dashboardLoading = accounts.loading || transactions.loading || budgets.loading || investments.loading || goals.loading || liabilities.loading;
  const dashboardError = accounts.error || transactions.error || budgets.error || investments.error || goals.error || liabilities.error;

  // Unique categories for filter dropdown
  const txCategories = useMemo(() => {
    const cats = new Set<string>();
    for (const tx of transactions.data ?? []) { if (tx.category) cats.add(tx.category); }
    return Array.from(cats).sort();
  }, [transactions.data]);

  // Filtered + sorted transactions
  const filteredTransactions = useMemo(() => {
    let txs = transactions.data ?? [];
    if (txTypeFilter !== 'all') txs = txs.filter(t => t.txType === txTypeFilter);
    txs = txs.filter(t => {
      const d = new Date(t.txDate);
      return d.getFullYear() === txMonth.getFullYear() && d.getMonth() === txMonth.getMonth();
    });
    if (txCategory !== 'all') txs = txs.filter(t => t.category === txCategory);
    return [...txs].sort((a, b) => new Date(b.txDate).getTime() - new Date(a.txDate).getTime());
  }, [transactions.data, txTypeFilter, txMonth, txCategory]);

  // ── Mutation helpers ───────────────────────────────────────────────────────

  const closeDrawer = useCallback(() => { setDrawer({ kind: 'none' }); setMutationError(null); }, []);

  async function runMutation(fn: () => Promise<void>, refetchFn: () => void) {
    setMutating(true);
    setMutationError(null);
    try {
      await fn();
      refetchFn();
      closeDrawer();
    } catch (err) {
      setMutationError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setMutating(false);
    }
  }

  async function handleDelete(id: string, path: string, refetchFn: () => void) {
    if (deletingId !== id) { setDeletingId(id); return; }
    try {
      await apiFetch(`${path}/${id}`, { method: 'DELETE' });
      refetchFn();
    } catch {
      // silently handle
    } finally {
      setDeletingId(null);
    }
  }

  function dollars(s: string): number {
    const v = Math.round(parseFloat(s) * 100);
    if (isNaN(v)) throw new Error('Invalid amount');
    return v;
  }

  function optDollars(s: string): number | undefined {
    if (!s) return undefined;
    return dollars(s);
  }

  function optFloat(s: string): number | undefined {
    if (!s) return undefined;
    const v = parseFloat(s);
    if (isNaN(v)) return undefined;
    return v;
  }

  function optStr(s: string): string | undefined { return s || undefined; }

  // ── Save handlers ──────────────────────────────────────────────────────────

  function handleSaveTransaction() {
    if (!txForm.accountId) { setMutationError('Account is required'); return; }
    if (!txForm.amount) { setMutationError('Amount is required'); return; }
    if (!txForm.txDate) { setMutationError('Date is required'); return; }
    runMutation(async () => {
      const body = { accountId: txForm.accountId, txType: txForm.txType, amount: dollars(txForm.amount), currency: txForm.currency || defaultCurrency, txDate: txForm.txDate, category: optStr(txForm.category), counterparty: optStr(txForm.counterparty), notes: optStr(txForm.notes), isRecurring: txForm.isRecurring };
      if (drawer.kind === 'edit-transaction') {
        await apiFetch(`/api/finance/transactions/${drawer.item.id}`, { method: 'PATCH', body });
      } else {
        await apiFetch<FinanceTransaction>('/api/finance/transactions', { body });
      }
    }, () => { transactions.refetch(); accounts.refetch(); });
  }

  function handleSaveBudget() {
    if (!budgetForm.name) { setMutationError('Name is required'); return; }
    if (!budgetForm.amount) { setMutationError('Amount is required'); return; }
    runMutation(async () => {
      const body = { name: budgetForm.name, amount: dollars(budgetForm.amount), currency: budgetForm.currency || defaultCurrency, period: budgetForm.period, method: budgetForm.method, jarType: budgetForm.method === 'six-jar' ? optStr(budgetForm.jarType) : undefined, category: optStr(budgetForm.category), startDate: budgetForm.startDate, alertThreshold: parseInt(budgetForm.alertThreshold) || 80 };
      if (drawer.kind === 'edit-budget') {
        await apiFetch(`/api/finance/budgets/${drawer.item.id}`, { method: 'PATCH', body: { name: body.name, amount: body.amount, category: body.category } });
      } else {
        await apiFetch('/api/finance/budgets', { body });
      }
    }, budgets.refetch);
  }

  function handleSaveInvestment() {
    if (!investmentForm.name) { setMutationError('Name is required'); return; }
    if (!investmentForm.quantity) { setMutationError('Quantity is required'); return; }
    if (!investmentForm.costBasis) { setMutationError('Cost basis is required'); return; }
    runMutation(async () => {
      if (drawer.kind === 'edit-investment') {
        await apiFetch(`/api/finance/investments/${drawer.item.id}`, { method: 'PATCH', body: { currentPrice: optDollars(investmentForm.currentPrice), currentValue: optDollars(investmentForm.currentValue), quantity: optFloat(investmentForm.quantity), costBasis: optDollars(investmentForm.costBasis), notes: optStr(investmentForm.notes) } });
      } else {
        await apiFetch('/api/finance/investments', { body: { portfolioId: investmentForm.portfolioId || 'main', assetType: investmentForm.assetType, symbol: optStr(investmentForm.symbol), name: investmentForm.name, quantity: parseFloat(investmentForm.quantity), costBasis: dollars(investmentForm.costBasis), currency: investmentForm.currency || defaultCurrency, currentPrice: optDollars(investmentForm.currentPrice), currentValue: optDollars(investmentForm.currentValue), purchaseDate: optStr(investmentForm.purchaseDate), notes: optStr(investmentForm.notes) } });
      }
    }, investments.refetch);
  }

  function handleSaveGoal() {
    if (!goalForm.name) { setMutationError('Name is required'); return; }
    if (!goalForm.targetAmount) { setMutationError('Target amount is required'); return; }
    runMutation(async () => {
      if (drawer.kind === 'edit-goal') {
        await apiFetch(`/api/finance/goals/${drawer.item.id}`, { method: 'PATCH', body: { name: goalForm.name, currentAmount: dollars(goalForm.currentAmount), targetAmount: dollars(goalForm.targetAmount), monthlyContribution: optDollars(goalForm.monthlyContribution), expectedReturnRate: optFloat(goalForm.expectedReturnRate), inflationRate: optFloat(goalForm.inflationRate), deadline: optStr(goalForm.deadline) } });
      } else {
        await apiFetch('/api/finance/goals', { body: { name: goalForm.name, goalType: goalForm.goalType, targetAmount: dollars(goalForm.targetAmount), currentAmount: optDollars(goalForm.currentAmount), currency: goalForm.currency || defaultCurrency, deadline: optStr(goalForm.deadline), monthlyContribution: optDollars(goalForm.monthlyContribution), expectedReturnRate: optFloat(goalForm.expectedReturnRate), inflationRate: optFloat(goalForm.inflationRate), notes: optStr(goalForm.notes) } });
      }
    }, goals.refetch);
  }

  function handleSaveAccount() {
    if (!accountForm.name) { setMutationError('Name is required'); return; }
    runMutation(async () => {
      if (drawer.kind === 'edit-account') {
        await apiFetch(`/api/finance/accounts/${drawer.item.id}`, { method: 'PATCH', body: { name: accountForm.name, balance: dollars(accountForm.balance), institution: optStr(accountForm.institution), notes: optStr(accountForm.notes) } });
      } else {
        await apiFetch('/api/finance/accounts', { body: { name: accountForm.name, accountType: accountForm.accountType, currency: accountForm.currency || defaultCurrency, balance: optDollars(accountForm.balance), institution: optStr(accountForm.institution), notes: optStr(accountForm.notes) } });
      }
    }, accounts.refetch);
  }

  function handleSaveLiability() {
    if (!liabilityForm.name) { setMutationError('Name is required'); return; }
    if (!liabilityForm.principal) { setMutationError('Principal is required'); return; }
    runMutation(async () => {
      if (drawer.kind === 'edit-liability') {
        await apiFetch(`/api/finance/liabilities/${drawer.item.id}`, { method: 'PATCH', body: { remaining: optDollars(liabilityForm.remaining), monthlyPayment: optDollars(liabilityForm.monthlyPayment), interestRate: optFloat(liabilityForm.interestRate), notes: optStr(liabilityForm.notes) } });
      } else {
        await apiFetch('/api/finance/liabilities', { body: { name: liabilityForm.name, liabilityType: liabilityForm.liabilityType, principal: dollars(liabilityForm.principal), remaining: optDollars(liabilityForm.remaining), currency: liabilityForm.currency || defaultCurrency, interestRate: optFloat(liabilityForm.interestRate), monthlyPayment: optDollars(liabilityForm.monthlyPayment), dueDate: optStr(liabilityForm.dueDate), notes: optStr(liabilityForm.notes) } });
      }
    }, liabilities.refetch);
  }

  // ── Tabs ────────────────────────────────────────────────────────────────────

  const tabs: { id: Tab; label: string }[] = [
    { id: 'dashboard', label: 'Dashboard' },
    { id: 'transactions', label: 'Transactions' },
    { id: 'budgets', label: 'Budgets' },
    { id: 'investments', label: 'Investments' },
    { id: 'goals', label: 'Goals' },
    { id: 'accounts', label: 'Accounts' },
    { id: 'liabilities', label: 'Liabilities' },
    { id: 'reports', label: 'Reports' },
  ];

  // ── Render helpers (inline action buttons) ─────────────────────────────────

  const actionBtn = (onClick: () => void, icon: React.ReactNode, danger = false) => (
    <button onClick={(e) => { e.stopPropagation(); onClick(); }}
      className="p-1.5 rounded transition-colors"
      style={{ color: danger ? '#ef4444' : 'var(--codex-fg-subtle)' }}
      onMouseEnter={(e) => e.currentTarget.style.backgroundColor = '#2a2a2a'}
      onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}>
      {icon}
    </button>
  );

  const confirmDeleteBtn = (id: string, path: string, refetchFn: () => void) => (
    deletingId === id ? (
      <button onClick={(e) => { e.stopPropagation(); handleDelete(id, path, refetchFn); }}
        className="px-2 py-1 rounded text-[11px] transition-colors"
        style={{ backgroundColor: 'rgba(239,68,68,0.15)', color: '#ef4444', border: '1px solid rgba(239,68,68,0.3)' }}>
        Confirm?
      </button>
    ) : actionBtn(() => handleDelete(id, path, refetchFn), <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />, true)
  );

  // ── Main render ────────────────────────────────────────────────────────────

  // Determine which save handler to call
  const drawerTitle = (() => {
    switch (drawer.kind) {
      case 'create-transaction': return 'Add Transaction';
      case 'edit-transaction': return 'Edit Transaction';
      case 'create-budget': return 'Create Budget';
      case 'edit-budget': return 'Edit Budget';
      case 'create-investment': return 'Add Holding';
      case 'edit-investment': return 'Edit Holding';
      case 'create-goal': return 'Create Goal';
      case 'edit-goal': return 'Edit Goal';
      case 'create-account': return 'Add Account';
      case 'edit-account': return 'Edit Account';
      case 'create-liability': return 'Add Liability';
      case 'edit-liability': return 'Edit Liability';
      default: return '';
    }
  })();

  const drawerSave = (() => {
    switch (drawer.kind) {
      case 'create-transaction': case 'edit-transaction': return handleSaveTransaction;
      case 'create-budget': case 'edit-budget': return handleSaveBudget;
      case 'create-investment': case 'edit-investment': return handleSaveInvestment;
      case 'create-goal': case 'edit-goal': return handleSaveGoal;
      case 'create-account': case 'edit-account': return handleSaveAccount;
      case 'create-liability': case 'edit-liability': return handleSaveLiability;
      default: return () => {};
    }
  })();

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
      {/* Tab Bar */}
      <div className="border-b px-6 py-3" style={{ borderColor: 'var(--codex-border-subtle)', backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex gap-2">
          {tabs.map((tab) => (
            <button key={tab.id} onClick={() => setActiveTab(tab.id)}
              className="px-4 py-1.5 rounded-full text-[13px] transition-all"
              style={{ backgroundColor: activeTab === tab.id ? 'var(--codex-accent)' : 'transparent', color: activeTab === tab.id ? 'white' : 'var(--codex-fg-subtle)', border: `1px solid ${activeTab === tab.id ? 'var(--codex-accent)' : 'transparent'}` }}
              onMouseEnter={(e) => { if (activeTab !== tab.id) e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
              onMouseLeave={(e) => { if (activeTab !== tab.id) e.currentTarget.style.backgroundColor = 'transparent'; }}>
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {/* ════ Dashboard Tab ════ */}
        {activeTab === 'dashboard' && (
          <>
            {dashboardLoading && <LoadingState message="Loading financial data..." />}
            {!dashboardLoading && dashboardError && <ErrorState message={dashboardError.message} />}
            {!dashboardLoading && !dashboardError && (
              <div className="grid grid-cols-2 gap-6">
                {/* Card 1: Net Worth */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('accounts')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Net Worth</div>
                  <div className="text-[28px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{fmt(dashboardSummary.netWorth)}</div>
                  <div className="flex items-center gap-1.5 text-[13px]" style={{ color: dashboardSummary.netWorth >= 0 ? '#10a37f' : '#ef4444' }}>
                    {dashboardSummary.netWorth >= 0 ? <TrendingUp className="w-4 h-4" strokeWidth={1.5} /> : <TrendingDown className="w-4 h-4" strokeWidth={1.5} />}
                    {(accounts.data?.length ?? 0)} accounts
                  </div>
                </div>

                {/* Card 2: Monthly Spending */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('transactions')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Monthly Spending</div>
                  <div className="text-[28px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{fmt(dashboardSummary.totalMonthlySpending)}</div>
                  {dashboardSummary.topCategories.length > 0 ? (
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
                        {dashboardSummary.topCategories.map(([cat, amount]) => `${cat} ${fmt(amount)}`).join(' \u00B7 ')}
                      </div>
                    </>
                  ) : (
                    <div className="text-[11px]" style={{ color: '#888' }}>No expenses this month</div>
                  )}
                </div>

                {/* Card 3: Budget Status */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('budgets')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Budget Status</div>
                  {dashboardSummary.activeBudgetCount === 0 ? (
                    <div className="text-[13px]" style={{ color: '#888' }}>No active budgets</div>
                  ) : (
                    <div className="flex items-center gap-4 mb-3">
                      <div className="relative" style={{ width: '64px', height: '64px' }}>
                        <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                          <circle cx="32" cy="32" r="28" fill="none" stroke="#1a1a1a" strokeWidth="6" />
                          <circle cx="32" cy="32" r="28" fill="none"
                            stroke={dashboardSummary.budgetPct > 100 ? '#ef4444' : '#10a37f'}
                            strokeWidth="6"
                            strokeDasharray={`${2 * Math.PI * 28 * Math.min(dashboardSummary.budgetPct, 100) / 100} ${2 * Math.PI * 28}`}
                            strokeLinecap="round" />
                        </svg>
                        <div className="absolute inset-0 flex items-center justify-center text-[18px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{dashboardSummary.budgetPct}%</div>
                      </div>
                      <div>
                        <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>{dashboardSummary.onTrackCount} of {dashboardSummary.activeBudgetCount} budgets on track</div>
                        {dashboardSummary.overBudgetCount > 0 && (
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: '#ef4444' }}>
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: '#ef4444' }} />{dashboardSummary.overBudgetCount} over limit
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>

                {/* Card 4: Portfolio Value */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('investments')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
                  <div className="text-[12px] mb-3" style={{ color: '#888' }}>Portfolio Value</div>
                  <div className="text-[28px] mb-2" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{fmt(dashboardSummary.totalInvestmentValue)}</div>
                  <div className="flex items-center gap-1.5 text-[13px]" style={{ color: dashboardSummary.investmentPL >= 0 ? '#10a37f' : '#ef4444' }}>
                    {dashboardSummary.investmentPL >= 0 ? <TrendingUp className="w-4 h-4" strokeWidth={1.5} /> : <TrendingDown className="w-4 h-4" strokeWidth={1.5} />}
                    {fmtSigned(dashboardSummary.investmentPL)} ({dashboardSummary.investmentPLPct >= 0 ? '+' : ''}{dashboardSummary.investmentPLPct.toFixed(1)}%)
                  </div>
                </div>

                {/* Card 5: Active Goals */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('goals')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
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
                    <div className="text-[11px]" style={{ color: '#888' }}>Next deadline: {new Date(dashboardSummary.nextDeadline).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}</div>
                  )}
                </div>

                {/* Card 6: Recent Transactions */}
                <div className="p-5 rounded-lg border cursor-pointer transition-all" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}
                  onClick={() => setActiveTab('transactions')}
                  onMouseEnter={(e) => e.currentTarget.style.filter = 'brightness(1.1)'}
                  onMouseLeave={(e) => e.currentTarget.style.filter = 'brightness(1)'}>
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
                            <div className="text-[13px]" style={{ color: txColor(tx.txType), fontFamily: 'var(--font-mono)' }}>{fmtSigned(displayAmount, tx.currency)}</div>
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

        {/* ════ Transactions Tab ════ */}
        {activeTab === 'transactions' && (
          <div>
            <div className="mb-4 flex items-center gap-3">
              <button onClick={() => setDrawer({ kind: 'create-transaction' })}
                className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5"
                style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />Add Transaction
              </button>
            </div>

            <div className="mb-4 flex items-center gap-3">
              <div className="flex gap-2">
                {(['all', 'income', 'expense', 'transfer'] as TxFilter[]).map((type) => (
                  <button key={type} onClick={() => setTxTypeFilter(type)}
                    className="px-3 py-1.5 rounded-full text-[12px] capitalize"
                    style={{ backgroundColor: txTypeFilter === type ? 'var(--codex-accent)' : 'transparent', color: txTypeFilter === type ? 'white' : 'var(--codex-fg-subtle)', border: `1px solid ${txTypeFilter === type ? 'var(--codex-accent)' : 'var(--codex-border)'}` }}>
                    {type === 'all' ? 'All' : type}
                  </button>
                ))}
              </div>
              <div className="flex items-center gap-1 px-3 py-1.5 rounded text-[12px]" style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)' }}>
                <button onClick={() => setTxMonth(addMonths(txMonth, -1))} className="p-0.5 rounded hover:bg-white/10"><ChevronLeft className="w-3 h-3" strokeWidth={1.5} /></button>
                <span className="min-w-[80px] text-center">{txMonth.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}</span>
                <button onClick={() => setTxMonth(addMonths(txMonth, 1))} className="p-0.5 rounded hover:bg-white/10"><ChevronRight className="w-3 h-3" strokeWidth={1.5} /></button>
              </div>
              <select value={txCategory} onChange={(e) => setTxCategory(e.target.value)}
                className="px-3 py-1.5 rounded text-[12px] outline-none"
                style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg)', border: '1px solid var(--codex-border)' }}>
                <option value="all">All Categories</option>
                {txCategories.map(c => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>

            {transactions.loading && <LoadingState message="Loading transactions..." />}
            {!transactions.loading && transactions.error && <ErrorState message={transactions.error.message} />}
            {!transactions.loading && !transactions.error && filteredTransactions.length === 0 && (
              <EmptyState message="No transactions for this period." />
            )}
            {!transactions.loading && !transactions.error && filteredTransactions.length > 0 && (
              <div className="space-y-1.5">
                {filteredTransactions.map((tx) => {
                  const displayAmount = tx.txType === 'income' ? tx.amount : -Math.abs(tx.amount);
                  const accountName = accounts.data?.find((a) => a.id === tx.accountId)?.name ?? tx.accountId.slice(0, 8);
                  return (
                    <div key={tx.id} className="group flex items-center justify-between p-3 rounded border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-center gap-3">
                        <div className="w-2 h-2 rounded-full" style={{ backgroundColor: txColor(tx.txType) }} />
                        <div>
                          <div className="flex items-center gap-2">
                            <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{tx.counterparty ?? tx.category ?? tx.txType}</span>
                            {tx.isRecurring && <span className="text-[11px]" style={{ color: '#888' }}>{'\u21BB'}</span>}
                          </div>
                          <div className="text-[12px]" style={{ color: '#888' }}>
                            {tx.txType === 'transfer' ? `\u2192 ${tx.notes ?? 'Transfer'}` : tx.category ?? tx.txType}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-3">
                        <div className="text-right">
                          <div className="text-[14px]" style={{ color: txColor(tx.txType), fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{fmtSigned(displayAmount, tx.currency)}</div>
                          <div className="text-[11px]" style={{ color: '#888' }}>{shortDate(tx.txDate)} &middot; {accountName}{tx.currency !== defaultCurrency ? ` (${formatCents(displayAmount, tx.currency)})` : ''}</div>
                        </div>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                          {actionBtn(() => setDrawer({ kind: 'edit-transaction', item: tx }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                          {confirmDeleteBtn(tx.id, '/api/finance/transactions', () => { transactions.refetch(); accounts.refetch(); })}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* ════ Budgets Tab ════ */}
        {activeTab === 'budgets' && (
          <div>
            <div className="mb-4 flex items-center justify-between">
              <div className="flex gap-2">
                <button onClick={() => setBudgetMode('standard')} className="px-3 py-1.5 rounded-full text-[12px]"
                  style={{ backgroundColor: budgetMode === 'standard' ? 'var(--codex-accent)' : 'transparent', color: budgetMode === 'standard' ? 'white' : 'var(--codex-fg-subtle)', border: `1px solid ${budgetMode === 'standard' ? 'var(--codex-accent)' : 'var(--codex-border)'}` }}>Standard</button>
                <button onClick={() => setBudgetMode('six-jar')} className="px-3 py-1.5 rounded-full text-[12px]"
                  style={{ backgroundColor: budgetMode === 'six-jar' ? 'var(--codex-accent)' : 'transparent', color: budgetMode === 'six-jar' ? 'white' : 'var(--codex-fg-subtle)', border: `1px solid ${budgetMode === 'six-jar' ? 'var(--codex-accent)' : 'var(--codex-border)'}` }}>Six-Jar</button>
              </div>
              <button onClick={() => setDrawer({ kind: 'create-budget' })}
                className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5"
                style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />Create Budget
              </button>
            </div>

            {budgets.loading && <LoadingState message="Loading budgets..." />}
            {!budgets.loading && budgets.error && <ErrorState message={budgets.error.message} />}
            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) === 0 && (
              <EmptyState message="No budgets configured yet. Create your first budget to start tracking spending." />
            )}

            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) > 0 && budgetMode === 'standard' && (
              <div className="space-y-3">
                {(budgets.data ?? []).filter((b) => b.method !== 'six-jar').map((budget) => {
                  const pct = budget.amount > 0 ? Math.round((budget.spent / budget.amount) * 100) : 0;
                  const barColor = pct > 100 ? '#ef4444' : pct > 80 ? '#eab308' : '#10a37f';
                  return (
                    <div key={budget.id} className="group p-4 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2">
                          <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{budget.name}</span>
                          <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#1a1a1a', color: '#888' }}>{budget.period}</span>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className="text-[13px]" style={{ color: barColor }}>{pct}%{pct > 100 ? ' Over!' : pct > 80 ? ' \u26A0' : ''}</span>
                          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                            {actionBtn(() => setDrawer({ kind: 'edit-budget', item: budget }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                            {confirmDeleteBtn(budget.id, '/api/finance/budgets', budgets.refetch)}
                          </div>
                        </div>
                      </div>
                      <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: barColor }} />
                      </div>
                      <div className="text-[12px] text-right" style={{ color: 'var(--codex-fg-subtle)' }}>{fmt(budget.spent, budget.currency)} / {fmt(budget.amount, budget.currency)}</div>
                    </div>
                  );
                })}
              </div>
            )}

            {!budgets.loading && !budgets.error && (budgets.data?.length ?? 0) > 0 && budgetMode === 'six-jar' && (
              <div className="grid grid-cols-3 gap-4">
                {(budgets.data ?? []).filter((b) => b.method === 'six-jar' && b.jarType).map((jar) => {
                  const pct = jar.amount > 0 ? Math.round((jar.spent / jar.amount) * 100) : 0;
                  const jarColors: Record<string, string> = { essentials: '#3b82f6', savings: '#10a37f', investment: '#14b8a6', education: '#8b5cf6', entertainment: '#f97316', charity: '#ec4899' };
                  const color = jarColors[jar.jarType ?? ''] ?? '#888';
                  return (
                    <div key={jar.id} className="group p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a', borderLeft: `3px solid ${color}` }}>
                      <div className="flex items-center justify-between mb-3">
                        <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{jar.name}</span>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                          {actionBtn(() => setDrawer({ kind: 'edit-budget', item: jar }), <Pencil className="w-3 h-3" strokeWidth={1.5} />)}
                        </div>
                      </div>
                      <div className="flex justify-center mb-3">
                        <div className="relative" style={{ width: '56px', height: '56px' }}>
                          <svg viewBox="0 0 56 56" style={{ transform: 'rotate(-90deg)' }}>
                            <circle cx="28" cy="28" r="24" fill="none" stroke="#1a1a1a" strokeWidth="5" />
                            <circle cx="28" cy="28" r="24" fill="none" stroke={color} strokeWidth="5"
                              strokeDasharray={`${2 * Math.PI * 24 * (Math.min(pct, 100) / 100)} ${2 * Math.PI * 24}`} strokeLinecap="round" />
                          </svg>
                          <div className="absolute inset-0 flex items-center justify-center text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{pct}%</div>
                        </div>
                      </div>
                      <div className="text-center">
                        <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)' }}>{fmt(jar.spent, jar.currency)} / {fmt(jar.amount, jar.currency)}</div>
                        <div className="text-[11px]" style={{ color: '#888' }}>{jar.category ?? jar.jarType}</div>
                      </div>
                    </div>
                  );
                })}
                {(budgets.data ?? []).filter((b) => b.method === 'six-jar' && b.jarType).length === 0 && (
                  <div className="col-span-3"><EmptyState message="No six-jar budgets configured." /></div>
                )}
              </div>
            )}
          </div>
        )}

        {/* ════ Investments Tab ════ */}
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

              const byType: Record<string, number> = {};
              for (const inv of invs) { byType[inv.assetType] = (byType[inv.assetType] ?? 0) + (inv.currentValue ?? 0); }
              const typeEntries = Object.entries(byType).sort(([, a], [, b]) => b - a);
              const typeColors = ['#10a37f', '#f97316', '#3b82f6', '#8b5cf6', '#ec4899', '#666'];

              return (
                <>
                  <div className="mb-4 p-5 rounded-lg border flex items-center justify-between" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                    <div>
                      <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Value</div>
                      <div className="text-[20px]" style={{ color: 'var(--codex-fg)', fontWeight: 600 }}>{fmt(totalValue)}</div>
                    </div>
                    <div>
                      <div className="text-[11px] mb-1" style={{ color: '#888' }}>Total Gain/Loss</div>
                      <div className="text-[16px]" style={{ color: totalPL >= 0 ? '#10a37f' : '#ef4444', fontWeight: 500 }}>
                        {fmtSigned(totalPL)} ({totalPLPct >= 0 ? '+' : ''}{totalPLPct.toFixed(1)}%)
                      </div>
                    </div>
                    {typeEntries.length > 0 && (
                      <div className="relative" style={{ width: '64px', height: '64px' }}>
                        <svg viewBox="0 0 64 64" style={{ transform: 'rotate(-90deg)' }}>
                          {(() => {
                            let offset = 0;
                            return typeEntries.map(([, value], i) => {
                              const frac = totalValue > 0 ? value / totalValue : 0;
                              const el = <circle key={i} cx="32" cy="32" r="28" fill="none" stroke={typeColors[i] ?? '#666'} strokeWidth="8"
                                strokeDasharray={`${2 * Math.PI * 28 * frac} ${2 * Math.PI * 28}`}
                                strokeDashoffset={`${-2 * Math.PI * 28 * offset}`} />;
                              offset += frac;
                              return el;
                            });
                          })()}
                        </svg>
                      </div>
                    )}
                  </div>

                  <div className="mb-4 flex items-center justify-end gap-2">
                    <button onClick={() => setDrawer({ kind: 'create-investment' })}
                      className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5"
                      style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                      onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                      onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                      <Plus className="w-3 h-3" strokeWidth={1.5} />Add Holding
                    </button>
                    <button onClick={async () => {
                      const invsList = investments.data ?? [];
                      for (const inv of invsList) {
                        if (inv.currentPrice != null && inv.quantity > 0) {
                          const val = Math.round(inv.currentPrice * inv.quantity);
                          await apiFetch(`/api/finance/investments/${inv.id}`, { method: 'PATCH', body: { currentValue: val } });
                        }
                      }
                      investments.refetch();
                    }} className="px-3 py-1.5 rounded text-[12px] flex items-center gap-1.5"
                      style={{ backgroundColor: 'transparent', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}>
                      <RefreshCw className="w-3 h-3" strokeWidth={1.5} />Refresh Values
                    </button>
                  </div>

                  {invs.length === 0 ? (
                    <EmptyState message="No investments tracked yet." />
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
                            <th className="text-right px-4 py-3 text-[11px] font-normal" style={{ color: '#888' }}></th>
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
                              <tr key={inv.id} className="group" style={{ borderTop: '1px solid #1a1a1a' }}>
                                <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{inv.symbol ?? '-'}</td>
                                <td className="px-4 py-3 text-[13px]" style={{ color: 'var(--codex-fg)' }}>{inv.name}</td>
                                <td className="px-4 py-3 text-[12px]" style={{ color: '#888' }}>{inv.assetType}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)' }}>{inv.quantity % 1 === 0 ? inv.quantity : inv.quantity.toFixed(4)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: '#888', fontFamily: 'var(--font-mono)' }}>{fmt(Math.round(avgCost), inv.currency)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{inv.currentPrice != null ? fmt(inv.currentPrice, inv.currency) : '-'}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{fmt(value, inv.currency)}</td>
                                <td className="px-4 py-3 text-[13px] text-right" style={{ color: plColor, fontFamily: 'var(--font-mono)' }}>{fmtSigned(pl, inv.currency)} ({plPct >= 0 ? '+' : ''}{plPct.toFixed(1)}%)</td>
                                <td className="px-4 py-3">
                                  <div className="flex items-center gap-1 justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                                    {actionBtn(() => setDrawer({ kind: 'edit-investment', item: inv }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                                    {confirmDeleteBtn(inv.id, '/api/finance/investments', investments.refetch)}
                                  </div>
                                </td>
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

        {/* ════ Goals Tab ════ */}
        {activeTab === 'goals' && (
          <div>
            <div className="mb-4 flex justify-end">
              <button onClick={() => setDrawer({ kind: 'create-goal' })}
                className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5"
                style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />Create Goal
              </button>
            </div>

            {goals.loading && <LoadingState message="Loading goals..." />}
            {!goals.loading && goals.error && <ErrorState message={goals.error.message} />}
            {!goals.loading && !goals.error && (goals.data?.length ?? 0) === 0 && (
              <EmptyState message="No financial goals yet." />
            )}

            {!goals.loading && !goals.error && (goals.data?.length ?? 0) > 0 && (
              <div className="space-y-4">
                {(goals.data ?? []).map((goal) => {
                  const pct = goal.targetAmount > 0 ? Math.round((goal.currentAmount / goal.targetAmount) * 100) : 0;
                  const isFire = goal.goalType.toLowerCase() === 'fire';
                  const tagColor = isFire ? '#f97316' : goal.goalType === 'savings' ? '#10a37f' : '#3b82f6';
                  const tagLabel = isFire ? 'FIRE' : goal.goalType.charAt(0).toUpperCase() + goal.goalType.slice(1);

                  let deadlineDisplay: string | null = null;
                  if (goal.deadline) {
                    const deadlineDate = new Date(goal.deadline);
                    const now = new Date();
                    const monthsLeft = Math.max(0, Math.round((deadlineDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24 * 30)));
                    const dateStr = deadlineDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' });
                    deadlineDisplay = monthsLeft > 0 ? `${dateStr} \u00B7 ${monthsLeft} months left` : dateStr;
                  }

                  const contribDisplay = goal.monthlyContribution != null ? `${fmt(goal.monthlyContribution, goal.currency)}/mo` : null;

                  return (
                    <div key={goal.id} className="group p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-center justify-between mb-3">
                        <div className="flex items-center gap-2">
                          <span className="text-[16px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{goal.name}</span>
                          <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: tagColor + '20', color: tagColor }}>{tagLabel}</span>
                        </div>
                        <div className="flex items-center gap-2">
                          <div className="flex items-center gap-1.5 text-[12px]" style={{ color: goal.status === 'active' ? '#10a37f' : '#888' }}>
                            <div className="w-2 h-2 rounded-full" style={{ backgroundColor: goal.status === 'active' ? '#10a37f' : '#888' }} />
                            {goal.status.charAt(0).toUpperCase() + goal.status.slice(1)}
                          </div>
                          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                            {actionBtn(() => setDrawer({ kind: 'edit-goal', item: goal }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                            {confirmDeleteBtn(goal.id, '/api/finance/goals', goals.refetch)}
                          </div>
                        </div>
                      </div>
                      <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: '#10a37f' }} />
                      </div>
                      <div className="flex items-center justify-between mb-3">
                        <span className="text-[14px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{fmt(goal.currentAmount, goal.currency)} / {fmt(goal.targetAmount, goal.currency)}</span>
                        <span className="text-[14px]" style={{ color: '#10a37f', fontWeight: 500 }}>{pct}%</span>
                      </div>

                      {isFire && (goal.expectedReturnRate != null || goal.inflationRate != null) && (
                        <div className="grid grid-cols-2 gap-3 mb-3 p-3 rounded" style={{ backgroundColor: '#1a1a1a' }}>
                          {goal.monthlyContribution != null && (
                            <div>
                              <div className="text-[11px] mb-0.5" style={{ color: '#888' }}>Monthly Contribution</div>
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{fmt(goal.monthlyContribution, goal.currency)}</div>
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
                              <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{new Date(goal.deadline).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}</div>
                            </div>
                          )}
                        </div>
                      )}

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

        {/* ════ Accounts Tab ════ */}
        {activeTab === 'accounts' && (
          <div>
            <div className="mb-4 flex justify-end">
              <button onClick={() => setDrawer({ kind: 'create-account' })}
                className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5"
                style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />Add Account
              </button>
            </div>

            {accounts.loading && <LoadingState message="Loading accounts..." />}
            {!accounts.loading && accounts.error && <ErrorState message={accounts.error.message} />}
            {!accounts.loading && !accounts.error && (accounts.data?.length ?? 0) === 0 && (
              <EmptyState message="No accounts yet. Add your first account." />
            )}

            {!accounts.loading && !accounts.error && (accounts.data?.length ?? 0) > 0 && (
              <div className="grid grid-cols-2 gap-4">
                {(accounts.data ?? []).map((acct) => {
                  const typeIcons: Record<string, React.ReactNode> = {
                    checking: <Wallet className="w-5 h-5" strokeWidth={1.5} />,
                    savings: <PiggyBank className="w-5 h-5" strokeWidth={1.5} />,
                    credit: <CreditCard className="w-5 h-5" strokeWidth={1.5} />,
                    investment: <TrendingUp className="w-5 h-5" strokeWidth={1.5} />,
                  };
                  const icon = typeIcons[acct.accountType] ?? <Building2 className="w-5 h-5" strokeWidth={1.5} />;
                  const balColor = acct.balance >= 0 ? 'var(--codex-fg)' : '#ef4444';

                  return (
                    <div key={acct.id} className="group p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-start justify-between mb-3">
                        <div className="flex items-center gap-3">
                          <div style={{ color: '#888' }}>{icon}</div>
                          <div>
                            <div className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{acct.name}</div>
                            <div className="text-[12px]" style={{ color: '#888' }}>{acct.accountType} {acct.institution ? `\u00B7 ${acct.institution}` : ''}</div>
                          </div>
                        </div>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                          {actionBtn(() => setDrawer({ kind: 'edit-account', item: acct }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                          {confirmDeleteBtn(acct.id, '/api/finance/accounts', accounts.refetch)}
                        </div>
                      </div>
                      <div className="text-[24px]" style={{ color: balColor, fontFamily: 'var(--font-mono)', fontWeight: 600 }}>{fmt(acct.balance, acct.currency)}</div>
                      {acct.notes && <div className="text-[11px] mt-2" style={{ color: '#888' }}>{acct.notes}</div>}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* ════ Liabilities Tab ════ */}
        {activeTab === 'liabilities' && (
          <div>
            <div className="mb-4 flex items-center justify-between">
              <div className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                Total: {fmt((liabilities.data ?? []).reduce((s, l) => s + l.remaining, 0))} remaining
              </div>
              <button onClick={() => setDrawer({ kind: 'create-liability' })}
                className="px-3 py-1.5 rounded-full text-[12px] flex items-center gap-1.5"
                style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
                onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}>
                <Plus className="w-3 h-3" strokeWidth={1.5} />Add Liability
              </button>
            </div>

            {liabilities.loading && <LoadingState message="Loading liabilities..." />}
            {!liabilities.loading && liabilities.error && <ErrorState message={liabilities.error.message} />}
            {!liabilities.loading && !liabilities.error && (liabilities.data?.length ?? 0) === 0 && (
              <EmptyState message="No liabilities tracked." />
            )}

            {!liabilities.loading && !liabilities.error && (liabilities.data?.length ?? 0) > 0 && (
              <div className="space-y-4">
                {[...(liabilities.data ?? [])].sort((a, b) => b.remaining - a.remaining).map((liability) => {
                  const paidOff = liability.principal - liability.remaining;
                  const pct = liability.principal > 0 ? Math.round((paidOff / liability.principal) * 100) : 0;
                  return (
                    <div key={liability.id} className="group p-5 rounded-lg border" style={{ backgroundColor: '#141414', borderColor: '#1a1a1a' }}>
                      <div className="flex items-center justify-between mb-3">
                        <div className="flex items-center gap-2">
                          <span className="text-[16px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>{liability.name}</span>
                          <span className="px-2 py-0.5 rounded text-[10px]" style={{ backgroundColor: '#1a1a1a', color: '#888' }}>{liability.liabilityType}</span>
                          {liability.interestRate != null && (
                            <span className="text-[12px]" style={{ color: '#ef4444' }}>{liability.interestRate}% APR</span>
                          )}
                        </div>
                        <div className="flex items-center gap-2">
                          {liability.monthlyPayment != null && (
                            <span className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>{fmt(liability.monthlyPayment, liability.currency)}/mo</span>
                          )}
                          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                            {actionBtn(() => setDrawer({ kind: 'edit-liability', item: liability }), <Pencil className="w-3.5 h-3.5" strokeWidth={1.5} />)}
                            {confirmDeleteBtn(liability.id, '/api/finance/liabilities', liabilities.refetch)}
                          </div>
                        </div>
                      </div>
                      <div className="h-2 rounded-full overflow-hidden mb-2" style={{ backgroundColor: '#1a1a1a' }}>
                        <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: '#10a37f' }} />
                      </div>
                      <div className="flex items-center justify-between text-[13px]">
                        <span style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                          {fmt(liability.remaining, liability.currency)} remaining
                        </span>
                        <span style={{ color: '#888' }}>
                          {pct}% paid ({fmt(paidOff, liability.currency)} of {fmt(liability.principal, liability.currency)})
                        </span>
                      </div>
                      {liability.dueDate && (
                        <div className="text-[11px] mt-2" style={{ color: '#888' }}>
                          Due: {new Date(liability.dueDate).toLocaleDateString('en-US', { month: 'short', year: 'numeric' })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* ════ Reports Tab ════ */}
        {activeTab === 'reports' && (() => {
          const txs = transactions.data ?? [];
          const budgetList = budgets.data ?? [];
          const acctList = accounts.data ?? [];
          const liabList = liabilities.data ?? [];
          const now = new Date();

          // Period filter logic
          const inPeriod = (dateStr: string): boolean => {
            const d = new Date(dateStr);
            if (reportPeriod === 'weekly') {
              const weekAgo = new Date(now);
              weekAgo.setDate(weekAgo.getDate() - 7);
              return d >= weekAgo && d <= now;
            }
            if (reportPeriod === 'yearly') return d.getFullYear() === now.getFullYear();
            return d.getMonth() === now.getMonth() && d.getFullYear() === now.getFullYear();
          };

          const periodLabel = reportPeriod === 'weekly' ? 'This Week' : reportPeriod === 'yearly' ? String(now.getFullYear()) : now.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
          const periodTxs = txs.filter(tx => inPeriod(tx.txDate));
          const totalIncome = periodTxs.filter(tx => tx.txType === 'income').reduce((s, tx) => s + cvt(tx.amount, tx.currency), 0);
          const totalExpenses = periodTxs.filter(tx => tx.txType === 'expense').reduce((s, tx) => s + cvt(Math.abs(tx.amount), tx.currency), 0);
          const netCashFlow = totalIncome - totalExpenses;
          const savingsRate = totalIncome > 0 ? Math.round((netCashFlow / totalIncome) * 100) : 0;

          // Spending by category (converted to display currency)
          const expenseTxs = periodTxs.filter(tx => tx.txType === 'expense');
          const byCat: Record<string, number> = {};
          for (const tx of expenseTxs) { const cat = tx.category ?? 'Other'; byCat[cat] = (byCat[cat] ?? 0) + cvt(Math.abs(tx.amount), tx.currency); }
          const catSorted = Object.entries(byCat).sort(([, a], [, b]) => b - a);
          const catMax = catSorted[0]?.[1] ?? 1;
          const catColors = ['#10a37f', '#3b82f6', '#f97316', '#8b5cf6', '#ec4899', '#14b8a6', '#eab308', '#666'];

          // Income by source (converted to display currency)
          const incomeTxs = periodTxs.filter(tx => tx.txType === 'income');
          const bySrc: Record<string, number> = {};
          for (const tx of incomeTxs) { const src = tx.category ?? tx.counterparty ?? 'Other'; bySrc[src] = (bySrc[src] ?? 0) + cvt(tx.amount, tx.currency); }
          const srcSorted = Object.entries(bySrc).sort(([, a], [, b]) => b - a);
          const srcMax = srcSorted[0]?.[1] ?? 1;
          const srcColors = ['#10a37f', '#3b82f6', '#14b8a6', '#8b5cf6', '#666'];

          // Top expenses (sort by converted amount)
          const topExpenses = [...expenseTxs].sort((a, b) => cvt(Math.abs(b.amount), b.currency) - cvt(Math.abs(a.amount), a.currency)).slice(0, 5);
          const acctMap = new Map(acctList.map(a => [a.id, a.name]));

          // Net worth (converted)
          const totalAssets = acctList.filter(a => !a.isArchived && a.balance >= 0).reduce((s, a) => s + cvt(a.balance, a.currency), 0);
          const totalLiab = liabList.reduce((s, l) => s + cvt(l.remaining, l.currency), 0);
          const netWorth = totalAssets - totalLiab;

          const cardStyle: React.CSSProperties = { backgroundColor: '#141414', borderColor: '#1a1a1a' };

          return (
            <div>
              {/* Period toggle + label */}
              <div className="mb-6 flex items-center justify-between">
                <div className="flex gap-2">
                  {(['weekly', 'monthly', 'yearly'] as ReportPeriod[]).map((period) => (
                    <button key={period} onClick={() => setReportPeriod(period)}
                      className="px-3 py-1.5 rounded-full text-[12px] capitalize"
                      style={{ backgroundColor: reportPeriod === period ? 'var(--codex-accent)' : 'transparent', color: reportPeriod === period ? 'white' : 'var(--codex-fg-subtle)', border: `1px solid ${reportPeriod === period ? 'var(--codex-accent)' : 'var(--codex-border)'}` }}>
                      {period}
                    </button>
                  ))}
                </div>
                <span className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>{periodLabel}</span>
              </div>

              {transactions.loading && <LoadingState message="Loading report data..." />}
              {!transactions.loading && transactions.error && <ErrorState message={transactions.error.message} />}
              {!transactions.loading && !transactions.error && (
                <div className="space-y-4">
                  {/* ── Summary cards ── */}
                  <div className="grid grid-cols-4 gap-3">
                    <div className="p-4 rounded-lg border" style={cardStyle}>
                      <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: '#888' }}>Total Income</div>
                      <div className="text-[18px]" style={{ color: '#10a37f', fontFamily: 'var(--font-mono)' }}>{fmt(totalIncome)}</div>
                    </div>
                    <div className="p-4 rounded-lg border" style={cardStyle}>
                      <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: '#888' }}>Total Expenses</div>
                      <div className="text-[18px]" style={{ color: '#ef4444', fontFamily: 'var(--font-mono)' }}>{fmt(totalExpenses)}</div>
                    </div>
                    <div className="p-4 rounded-lg border" style={cardStyle}>
                      <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: '#888' }}>Net Cash Flow</div>
                      <div className="text-[18px]" style={{ color: netCashFlow >= 0 ? '#10a37f' : '#ef4444', fontFamily: 'var(--font-mono)' }}>{fmtSigned(netCashFlow)}</div>
                    </div>
                    <div className="p-4 rounded-lg border" style={cardStyle}>
                      <div className="text-[11px] uppercase tracking-wider mb-1" style={{ color: '#888' }}>Savings Rate</div>
                      <div className="text-[18px]" style={{ color: savingsRate >= 20 ? '#10a37f' : savingsRate >= 0 ? '#eab308' : '#ef4444', fontFamily: 'var(--font-mono)' }}>{savingsRate}%</div>
                    </div>
                  </div>

                  {/* ── Spending by Category ── */}
                  <div className="p-5 rounded-lg border" style={cardStyle}>
                    <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Spending by Category</h3>
                    {catSorted.length === 0
                      ? <div className="text-[13px]" style={{ color: '#888' }}>No expense data for this period.</div>
                      : <div className="space-y-2">
                          {catSorted.map(([cat, amount], i) => (
                            <div key={cat} className="flex items-center gap-3">
                              <div className="w-32 text-[13px] truncate" style={{ color: 'var(--codex-fg-subtle)' }}>{cat}</div>
                              <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                                <div style={{ width: `${(amount / catMax) * 100}%`, height: '100%', backgroundColor: catColors[i % catColors.length], borderRadius: '4px', transition: 'width 0.3s ease' }} />
                              </div>
                              <div className="w-24 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{fmt(amount)}</div>
                            </div>
                          ))}
                          <div className="flex justify-end pt-2 border-t" style={{ borderColor: '#222' }}>
                            <span className="text-[13px]" style={{ color: 'var(--codex-fg-muted)', fontFamily: 'var(--font-mono)' }}>Total: {fmt(totalExpenses)}</span>
                          </div>
                        </div>
                    }
                  </div>

                  {/* ── Income by Source ── */}
                  <div className="p-5 rounded-lg border" style={cardStyle}>
                    <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Income by Source</h3>
                    {srcSorted.length === 0
                      ? <div className="text-[13px]" style={{ color: '#888' }}>No income data for this period.</div>
                      : <div className="space-y-2">
                          {srcSorted.map(([src, amount], i) => (
                            <div key={src} className="flex items-center gap-3">
                              <div className="w-32 text-[13px] truncate" style={{ color: 'var(--codex-fg-subtle)' }}>{src}</div>
                              <div className="flex-1 h-6 rounded overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                                <div style={{ width: `${(amount / srcMax) * 100}%`, height: '100%', backgroundColor: srcColors[i % srcColors.length], borderRadius: '4px', transition: 'width 0.3s ease' }} />
                              </div>
                              <div className="w-24 text-right text-[13px]" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>{fmt(amount)}</div>
                            </div>
                          ))}
                          <div className="flex justify-end pt-2 border-t" style={{ borderColor: '#222' }}>
                            <span className="text-[13px]" style={{ color: 'var(--codex-fg-muted)', fontFamily: 'var(--font-mono)' }}>Total: {fmt(totalIncome)}</span>
                          </div>
                        </div>
                    }
                  </div>

                  <div className="grid grid-cols-2 gap-4">
                    {/* ── Budget vs Actual ── */}
                    <div className="p-5 rounded-lg border" style={cardStyle}>
                      <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Budget vs Actual</h3>
                      {budgetList.length === 0
                        ? <div className="text-[13px]" style={{ color: '#888' }}>No budgets configured.</div>
                        : <div className="space-y-3">
                            {budgetList.filter(b => b.method !== 'six-jar').map(b => {
                              const pct = b.amount > 0 ? Math.round((b.spent / b.amount) * 100) : 0;
                              const over = pct > 100;
                              return (
                                <div key={b.id}>
                                  <div className="flex justify-between text-[12px] mb-1">
                                    <span style={{ color: 'var(--codex-fg-subtle)' }}>{b.name}</span>
                                    <span style={{ color: over ? '#ef4444' : 'var(--codex-fg-muted)', fontFamily: 'var(--font-mono)' }}>{fmt(b.spent, b.currency)} / {fmt(b.amount, b.currency)}</span>
                                  </div>
                                  <div className="h-2 rounded-full overflow-hidden" style={{ backgroundColor: '#1a1a1a' }}>
                                    <div style={{ width: `${Math.min(pct, 100)}%`, height: '100%', backgroundColor: over ? '#ef4444' : pct > 80 ? '#eab308' : '#10a37f', borderRadius: '4px', transition: 'width 0.3s ease' }} />
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                      }
                    </div>

                    {/* ── Top Expenses ── */}
                    <div className="p-5 rounded-lg border" style={cardStyle}>
                      <h3 className="text-[15px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Top Expenses</h3>
                      {topExpenses.length === 0
                        ? <div className="text-[13px]" style={{ color: '#888' }}>No expenses this period.</div>
                        : <div className="space-y-2.5">
                            {topExpenses.map((tx, i) => (
                              <div key={tx.id} className="flex items-center gap-3">
                                <span className="w-5 text-[11px] text-center" style={{ color: '#666' }}>{i + 1}</span>
                                <div className="flex-1 min-w-0">
                                  <div className="text-[13px] truncate" style={{ color: 'var(--codex-fg)' }}>{tx.counterparty ?? tx.category ?? 'Unknown'}</div>
                                  <div className="text-[11px]" style={{ color: '#666' }}>{tx.category ?? ''} · {shortDate(tx.txDate)} · {acctMap.get(tx.accountId) ?? ''}</div>
                                </div>
                                <span className="text-[13px]" style={{ color: '#ef4444', fontFamily: 'var(--font-mono)' }}>{fmt(Math.abs(tx.amount), tx.currency)}</span>
                              </div>
                            ))}
                          </div>
                      }
                    </div>
                  </div>

                  {/* ── Account Balances & Net Worth ── */}
                  <div className="p-5 rounded-lg border" style={cardStyle}>
                    <div className="flex items-center justify-between mb-4">
                      <h3 className="text-[15px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Account Summary</h3>
                      <div className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                        Net Worth: <span style={{ color: netWorth >= 0 ? '#10a37f' : '#ef4444', fontFamily: 'var(--font-mono)', fontWeight: 500 }}>{fmt(netWorth)}</span>
                      </div>
                    </div>
                    <div className="grid grid-cols-2 gap-x-6 gap-y-2">
                      {acctList.filter(a => !a.isArchived).map(a => (
                        <div key={a.id} className="flex items-center justify-between py-1.5 border-b" style={{ borderColor: '#1a1a1a' }}>
                          <div>
                            <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{a.name}</div>
                            <div className="text-[11px]" style={{ color: '#666' }}>{a.accountType}{a.institution ? ` · ${a.institution}` : ''}</div>
                          </div>
                          <span className="text-[13px]" style={{ color: a.balance >= 0 ? 'var(--codex-fg)' : '#ef4444', fontFamily: 'var(--font-mono)' }}>{fmt(a.balance, a.currency)}</span>
                        </div>
                      ))}
                      {liabList.length > 0 && (
                        <>
                          <div className="col-span-2 mt-2 mb-1 text-[11px] uppercase tracking-wider" style={{ color: '#666' }}>Liabilities</div>
                          {liabList.map(l => (
                            <div key={l.id} className="flex items-center justify-between py-1.5 border-b" style={{ borderColor: '#1a1a1a' }}>
                              <div>
                                <div className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>{l.name}</div>
                                <div className="text-[11px]" style={{ color: '#666' }}>{l.liabilityType}{l.interestRate ? ` · ${l.interestRate}% APR` : ''}</div>
                              </div>
                              <span className="text-[13px]" style={{ color: '#ef4444', fontFamily: 'var(--font-mono)' }}>-{fmt(l.remaining, l.currency)}</span>
                            </div>
                          ))}
                        </>
                      )}
                    </div>
                  </div>
                </div>
              )}
            </div>
          );
        })()}
      </div>

      {/* ════ Slide-over drawer forms ════ */}
      <SlideOverDrawer title={drawerTitle} open={drawer.kind !== 'none'} onClose={closeDrawer} onSave={drawerSave} saving={mutating} error={mutationError}>
        {/* Transaction form */}
        {(drawer.kind === 'create-transaction' || drawer.kind === 'edit-transaction') && (
          <>
            <Field label="Account">
              <select value={txForm.accountId} onChange={(e) => setTxForm({ ...txForm, accountId: e.target.value })} style={selectCss}>
                <option value="">Select account...</option>
                {(accounts.data ?? []).map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
              </select>
            </Field>
            <Field label="Type">
              <select value={txForm.txType} onChange={(e) => setTxForm({ ...txForm, txType: e.target.value })} style={selectCss}>
                <option value="expense">Expense</option>
                <option value="income">Income</option>
                <option value="transfer">Transfer</option>
              </select>
            </Field>
            <Field label={`Amount (${currLabel(txForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(txForm.currency) ? '1' : '0.01'} value={txForm.amount} onChange={(e) => setTxForm({ ...txForm, amount: e.target.value })} placeholder={ZERO_DECIMAL.has(txForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Currency">
              <CurrencySelect value={txForm.currency} onChange={(v) => setTxForm({ ...txForm, currency: v })} />
            </Field>
            <Field label="Date">
              <input type="date" value={txForm.txDate} onChange={(e) => setTxForm({ ...txForm, txDate: e.target.value })} style={inputCss} />
            </Field>
            <Field label="Category">
              <input type="text" value={txForm.category} onChange={(e) => setTxForm({ ...txForm, category: e.target.value })} placeholder="e.g. Groceries, Dining, Salary" style={inputCss} list="categories" />
              <datalist id="categories">{txCategories.map(c => <option key={c} value={c} />)}</datalist>
            </Field>
            <Field label="Counterparty">
              <input type="text" value={txForm.counterparty} onChange={(e) => setTxForm({ ...txForm, counterparty: e.target.value })} placeholder="e.g. Whole Foods, Acme Corp" style={inputCss} />
            </Field>
            <Field label="Notes">
              <textarea value={txForm.notes} onChange={(e) => setTxForm({ ...txForm, notes: e.target.value })} rows={2} style={{ ...inputCss, resize: 'vertical' }} />
            </Field>
            <label className="flex items-center gap-2 text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>
              <input type="checkbox" checked={txForm.isRecurring} onChange={(e) => setTxForm({ ...txForm, isRecurring: e.target.checked })} style={{ accentColor: 'var(--codex-accent)' }} />
              Recurring transaction
            </label>
          </>
        )}

        {/* Budget form */}
        {(drawer.kind === 'create-budget' || drawer.kind === 'edit-budget') && (
          <>
            <Field label="Name">
              <input type="text" value={budgetForm.name} onChange={(e) => setBudgetForm({ ...budgetForm, name: e.target.value })} placeholder="e.g. Groceries" style={inputCss} />
            </Field>
            <Field label={`Amount (${currLabel(budgetForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(budgetForm.currency) ? '1' : '0.01'} value={budgetForm.amount} onChange={(e) => setBudgetForm({ ...budgetForm, amount: e.target.value })} placeholder={ZERO_DECIMAL.has(budgetForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Currency">
              <CurrencySelect value={budgetForm.currency} onChange={(v) => setBudgetForm({ ...budgetForm, currency: v })} />
            </Field>
            <Field label="Period">
              <select value={budgetForm.period} onChange={(e) => setBudgetForm({ ...budgetForm, period: e.target.value })} style={selectCss}>
                <option value="monthly">Monthly</option>
                <option value="weekly">Weekly</option>
                <option value="yearly">Yearly</option>
              </select>
            </Field>
            {drawer.kind === 'create-budget' && (
              <>
                <Field label="Method">
                  <select value={budgetForm.method} onChange={(e) => setBudgetForm({ ...budgetForm, method: e.target.value })} style={selectCss}>
                    <option value="envelope">Envelope</option>
                    <option value="six-jar">Six-Jar</option>
                  </select>
                </Field>
                {budgetForm.method === 'six-jar' && (
                  <Field label="Jar Type">
                    <select value={budgetForm.jarType} onChange={(e) => setBudgetForm({ ...budgetForm, jarType: e.target.value })} style={selectCss}>
                      <option value="">Select jar...</option>
                      <option value="essentials">Essentials (55%)</option>
                      <option value="savings">Long-term Savings (10%)</option>
                      <option value="investment">Financial Freedom (10%)</option>
                      <option value="education">Education (10%)</option>
                      <option value="entertainment">Play (10%)</option>
                      <option value="charity">Give (5%)</option>
                    </select>
                  </Field>
                )}
              </>
            )}
            <Field label="Category">
              <input type="text" value={budgetForm.category} onChange={(e) => setBudgetForm({ ...budgetForm, category: e.target.value })} placeholder="e.g. Food, Transport" style={inputCss} />
            </Field>
            {drawer.kind === 'create-budget' && (
              <Field label="Start Date">
                <input type="date" value={budgetForm.startDate} onChange={(e) => setBudgetForm({ ...budgetForm, startDate: e.target.value })} style={inputCss} />
              </Field>
            )}
            <Field label="Alert Threshold (%)">
              <input type="number" min="0" max="100" value={budgetForm.alertThreshold} onChange={(e) => setBudgetForm({ ...budgetForm, alertThreshold: e.target.value })} style={inputCss} />
            </Field>
          </>
        )}

        {/* Investment form */}
        {(drawer.kind === 'create-investment' || drawer.kind === 'edit-investment') && (
          <>
            {drawer.kind === 'create-investment' && (
              <>
                <Field label="Name">
                  <input type="text" value={investmentForm.name} onChange={(e) => setInvestmentForm({ ...investmentForm, name: e.target.value })} placeholder="e.g. Apple Inc." style={inputCss} />
                </Field>
                <Field label="Symbol">
                  <input type="text" value={investmentForm.symbol} onChange={(e) => setInvestmentForm({ ...investmentForm, symbol: e.target.value.toUpperCase() })} placeholder="e.g. AAPL" style={inputCss} />
                </Field>
                <Field label="Asset Type">
                  <select value={investmentForm.assetType} onChange={(e) => setInvestmentForm({ ...investmentForm, assetType: e.target.value })} style={selectCss}>
                    <option value="stock">Stock</option>
                    <option value="etf">ETF</option>
                    <option value="crypto">Crypto</option>
                    <option value="bond">Bond</option>
                    <option value="real-estate">Real Estate</option>
                    <option value="cash">Cash</option>
                    <option value="other">Other</option>
                  </select>
                </Field>
                <Field label="Portfolio">
                  <input type="text" value={investmentForm.portfolioId} onChange={(e) => setInvestmentForm({ ...investmentForm, portfolioId: e.target.value })} style={inputCss} />
                </Field>
              </>
            )}
            <Field label="Quantity">
              <input type="number" step="0.0001" value={investmentForm.quantity} onChange={(e) => setInvestmentForm({ ...investmentForm, quantity: e.target.value })} placeholder="0" style={inputCss} />
            </Field>
            <Field label={`Cost Basis (${currLabel(investmentForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(investmentForm.currency) ? '1' : '0.01'} value={investmentForm.costBasis} onChange={(e) => setInvestmentForm({ ...investmentForm, costBasis: e.target.value })} placeholder={ZERO_DECIMAL.has(investmentForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Currency">
              <CurrencySelect value={investmentForm.currency} onChange={(v) => setInvestmentForm({ ...investmentForm, currency: v })} />
            </Field>
            <Field label={`Current Price (${currLabel(investmentForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(investmentForm.currency) ? '1' : '0.01'} value={investmentForm.currentPrice} onChange={(e) => setInvestmentForm({ ...investmentForm, currentPrice: e.target.value })} placeholder={ZERO_DECIMAL.has(investmentForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label={`Current Value (${currLabel(investmentForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(investmentForm.currency) ? '1' : '0.01'} value={investmentForm.currentValue} onChange={(e) => setInvestmentForm({ ...investmentForm, currentValue: e.target.value })} placeholder="Auto-calculated or manual" style={inputCss} />
            </Field>
            {drawer.kind === 'create-investment' && (
              <Field label="Purchase Date">
                <input type="date" value={investmentForm.purchaseDate} onChange={(e) => setInvestmentForm({ ...investmentForm, purchaseDate: e.target.value })} style={inputCss} />
              </Field>
            )}
            <Field label="Notes">
              <textarea value={investmentForm.notes} onChange={(e) => setInvestmentForm({ ...investmentForm, notes: e.target.value })} rows={2} style={{ ...inputCss, resize: 'vertical' }} />
            </Field>
          </>
        )}

        {/* Goal form */}
        {(drawer.kind === 'create-goal' || drawer.kind === 'edit-goal') && (
          <>
            <Field label="Name">
              <input type="text" value={goalForm.name} onChange={(e) => setGoalForm({ ...goalForm, name: e.target.value })} placeholder="e.g. Emergency Fund" style={inputCss} />
            </Field>
            {drawer.kind === 'create-goal' && (
              <Field label="Goal Type">
                <select value={goalForm.goalType} onChange={(e) => setGoalForm({ ...goalForm, goalType: e.target.value })} style={selectCss}>
                  <option value="savings">Savings</option>
                  <option value="investment">Investment</option>
                  <option value="fire">FIRE</option>
                  <option value="debt-payoff">Debt Payoff</option>
                  <option value="emergency-fund">Emergency Fund</option>
                  <option value="other">Other</option>
                </select>
              </Field>
            )}
            <Field label={`Target Amount (${currLabel(goalForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(goalForm.currency) ? '1' : '0.01'} value={goalForm.targetAmount} onChange={(e) => setGoalForm({ ...goalForm, targetAmount: e.target.value })} placeholder={ZERO_DECIMAL.has(goalForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label={`Current Amount (${currLabel(goalForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(goalForm.currency) ? '1' : '0.01'} value={goalForm.currentAmount} onChange={(e) => setGoalForm({ ...goalForm, currentAmount: e.target.value })} placeholder={ZERO_DECIMAL.has(goalForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Currency">
              <CurrencySelect value={goalForm.currency} onChange={(v) => setGoalForm({ ...goalForm, currency: v })} />
            </Field>
            <Field label="Deadline">
              <input type="date" value={goalForm.deadline} onChange={(e) => setGoalForm({ ...goalForm, deadline: e.target.value })} style={inputCss} />
            </Field>
            <Field label={`Monthly Contribution (${currLabel(goalForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(goalForm.currency) ? '1' : '0.01'} value={goalForm.monthlyContribution} onChange={(e) => setGoalForm({ ...goalForm, monthlyContribution: e.target.value })} placeholder="Optional" style={inputCss} />
            </Field>
            {(goalForm.goalType === 'fire' || goalForm.goalType === 'investment') && (
              <Field label="Expected Return Rate (%)">
                <input type="number" step="0.1" value={goalForm.expectedReturnRate} onChange={(e) => setGoalForm({ ...goalForm, expectedReturnRate: e.target.value })} placeholder="e.g. 7.0" style={inputCss} />
              </Field>
            )}
            {goalForm.goalType === 'fire' && (
              <Field label="Inflation Rate (%)">
                <input type="number" step="0.1" value={goalForm.inflationRate} onChange={(e) => setGoalForm({ ...goalForm, inflationRate: e.target.value })} placeholder="e.g. 3.0" style={inputCss} />
              </Field>
            )}
            <Field label="Notes">
              <textarea value={goalForm.notes} onChange={(e) => setGoalForm({ ...goalForm, notes: e.target.value })} rows={2} style={{ ...inputCss, resize: 'vertical' }} />
            </Field>
          </>
        )}

        {/* Account form */}
        {(drawer.kind === 'create-account' || drawer.kind === 'edit-account') && (
          <>
            <Field label="Name">
              <input type="text" value={accountForm.name} onChange={(e) => setAccountForm({ ...accountForm, name: e.target.value })} placeholder="e.g. Chase Checking" style={inputCss} />
            </Field>
            {drawer.kind === 'create-account' && (
              <Field label="Account Type">
                <select value={accountForm.accountType} onChange={(e) => setAccountForm({ ...accountForm, accountType: e.target.value })} style={selectCss}>
                  <option value="checking">Checking</option>
                  <option value="savings">Savings</option>
                  <option value="credit">Credit Card</option>
                  <option value="investment">Investment</option>
                  <option value="cash">Cash</option>
                  <option value="other">Other</option>
                </select>
              </Field>
            )}
            <Field label={`Balance (${currLabel(accountForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(accountForm.currency) ? '1' : '0.01'} value={accountForm.balance} onChange={(e) => setAccountForm({ ...accountForm, balance: e.target.value })} placeholder={ZERO_DECIMAL.has(accountForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Currency">
              <CurrencySelect value={accountForm.currency} onChange={(v) => setAccountForm({ ...accountForm, currency: v })} />
            </Field>
            <Field label="Institution">
              <input type="text" value={accountForm.institution} onChange={(e) => setAccountForm({ ...accountForm, institution: e.target.value })} placeholder="e.g. JPMorgan Chase" style={inputCss} />
            </Field>
            <Field label="Notes">
              <textarea value={accountForm.notes} onChange={(e) => setAccountForm({ ...accountForm, notes: e.target.value })} rows={2} style={{ ...inputCss, resize: 'vertical' }} />
            </Field>
          </>
        )}

        {/* Liability form */}
        {(drawer.kind === 'create-liability' || drawer.kind === 'edit-liability') && (
          <>
            {drawer.kind === 'create-liability' && (
              <>
                <Field label="Name">
                  <input type="text" value={liabilityForm.name} onChange={(e) => setLiabilityForm({ ...liabilityForm, name: e.target.value })} placeholder="e.g. Student Loan" style={inputCss} />
                </Field>
                <Field label="Type">
                  <select value={liabilityForm.liabilityType} onChange={(e) => setLiabilityForm({ ...liabilityForm, liabilityType: e.target.value })} style={selectCss}>
                    <option value="loan">Loan</option>
                    <option value="mortgage">Mortgage</option>
                    <option value="credit_card">Credit Card</option>
                    <option value="personal">Personal</option>
                    <option value="other">Other</option>
                  </select>
                </Field>
                <Field label={`Principal (${currLabel(liabilityForm.currency)})`}>
                  <input type="number" step={ZERO_DECIMAL.has(liabilityForm.currency) ? '1' : '0.01'} value={liabilityForm.principal} onChange={(e) => setLiabilityForm({ ...liabilityForm, principal: e.target.value })} placeholder={ZERO_DECIMAL.has(liabilityForm.currency) ? '0' : '0.00'} style={inputCss} />
                </Field>
                <Field label="Currency">
                  <CurrencySelect value={liabilityForm.currency} onChange={(v) => setLiabilityForm({ ...liabilityForm, currency: v })} />
                </Field>
              </>
            )}
            <Field label={`Remaining (${currLabel(liabilityForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(liabilityForm.currency) ? '1' : '0.01'} value={liabilityForm.remaining} onChange={(e) => setLiabilityForm({ ...liabilityForm, remaining: e.target.value })} placeholder={ZERO_DECIMAL.has(liabilityForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            <Field label="Interest Rate (%)">
              <input type="number" step="0.01" value={liabilityForm.interestRate} onChange={(e) => setLiabilityForm({ ...liabilityForm, interestRate: e.target.value })} placeholder="e.g. 4.5" style={inputCss} />
            </Field>
            <Field label={`Monthly Payment (${currLabel(liabilityForm.currency)})`}>
              <input type="number" step={ZERO_DECIMAL.has(liabilityForm.currency) ? '1' : '0.01'} value={liabilityForm.monthlyPayment} onChange={(e) => setLiabilityForm({ ...liabilityForm, monthlyPayment: e.target.value })} placeholder={ZERO_DECIMAL.has(liabilityForm.currency) ? '0' : '0.00'} style={inputCss} />
            </Field>
            {drawer.kind === 'create-liability' && (
              <Field label="Due Date">
                <input type="date" value={liabilityForm.dueDate} onChange={(e) => setLiabilityForm({ ...liabilityForm, dueDate: e.target.value })} style={inputCss} />
              </Field>
            )}
            <Field label="Notes">
              <textarea value={liabilityForm.notes} onChange={(e) => setLiabilityForm({ ...liabilityForm, notes: e.target.value })} rows={2} style={{ ...inputCss, resize: 'vertical' }} />
            </Field>
          </>
        )}
      </SlideOverDrawer>
    </div>
  );
}
