// ── Finance Account ────────────────────────────────────────

export interface FinanceAccount {
  id: string;
  name: string;
  accountType: string;
  currency: string;
  balance: number;
  institution: string | null;
  notes: string | null;
  isArchived: boolean;
  baseBalance?: number;
  baseCurrency?: string;
  exchangeRate?: number;
}

// ── Finance Transaction ────────────────────────────────────

export interface FinanceTransaction {
  id: string;
  accountId: string;
  txType: "income" | "expense" | "transfer";
  amount: number;
  currency: string;
  category: string | null;
  subcategory: string | null;
  counterparty: string | null;
  notes: string | null;
  txDate: string;
  transferId: string | null;
  baseAmount?: number;
  baseCurrency?: string;
  exchangeRate?: number;
}

// ── Finance Budget ─────────────────────────────────────────

export interface FinanceBudgetUsage {
  id: string;
  name: string;
  amount: number;
  currency: string;
  period: string;
  category: string | null;
  method: string;
  jarType: string | null;
  isActive: boolean;
  alertThreshold: number;
  spent: number;
  baseAmount?: number;
  baseCurrency?: string;
}

// ── Finance Portfolio & Investments ────────────────────────

export interface FinancePortfolio {
  id: string;
  name: string;
  description: string | null;
  currency: string;
  totalValue: number;
  totalCostBasis: number;
  holdingCount: number;
}

export interface FinanceInvestment {
  id: string;
  portfolioId: string;
  assetType: string;
  symbol: string | null;
  name: string;
  quantity: number;
  costBasis: number;
  currency: string;
  currentPrice: number | null;
  currentValue: number | null;
  marketCurrency?: string;
  baseCostBasis?: number;
  baseCurrentValue?: number;
  baseCurrency?: string;
  purchaseRate?: number;
  marketRate?: number;
}

// ── Finance Goals & Liabilities ────────────────────────────

export interface FinanceGoal {
  id: string;
  name: string;
  goalType: string;
  targetAmount: number;
  currentAmount: number;
  currency: string;
  status: string;
  deadline: string | null;
  monthlyContribution: number | null;
  baseTargetAmount?: number;
  baseCurrentAmount?: number;
  baseCurrency?: string;
}

export interface FinanceLiability {
  id: string;
  name: string;
  liabilityType: string;
  principal: number;
  remaining: number;
  currency: string;
  interestRate: number | null;
  monthlyPayment: number | null;
  dueDate: string | null;
  basePrincipal?: number;
  baseRemaining?: number;
  baseCurrency?: string;
}

// ── Finance Net Worth ───────────────────────────────────────

export interface FinanceNetWorth {
  totalsByCurrency: {
    currency: string;
    accounts: number;
    investments: number;
    liabilities: number;
    net: number;
  }[];
}

// ── Finance Reports ────────────────────────────────────────

export interface FinanceMonthlySummary {
  currentIncome: number;
  currentSpending: number;
  previousIncome: number;
  previousSpending: number;
}

export interface FinanceCategoryReport {
  total: number;
  breakdown: { category: string; amount: number; pct: number }[];
}

export interface FinanceTrendPoint {
  period: string;
  value: number;
  changePct: number | null;
}

// ── Finance Daily Spending ────────────────────────────────

export interface DailySpending {
  date: string;
  totalSpending: number;
  txCount: number;
}

export interface FinanceDailySpendingResponse {
  days: DailySpending[];
}

export interface FinancePeriodSummary {
  income: number;
  spending: number;
}

// ── Finance Mutation Parameters ────────────────────────────

export interface FinanceAccountCreateParams {
  name: string;
  accountType: string;
  currency?: string;
  balance?: number;
  institution?: string;
  notes?: string;
}

export interface FinanceTransactionCreateParams {
  accountId: string;
  txType: "income" | "expense" | "transfer";
  amount: number;
  currency?: string;
  category?: string;
  subcategory?: string;
  counterparty?: string;
  txDate?: string;
  notes?: string;
}

export interface FinanceBudgetCreateParams {
  name: string;
  amount: number;
  period: string;
  currency?: string;
  category?: string;
  alertThreshold?: number;
}

export interface FinanceGoalCreateParams {
  name: string;
  goalType: string;
  targetAmount: number;
  currency?: string;
  currentAmount?: number;
  deadline?: string;
  monthlyContribution?: number;
  notes?: string;
}

export interface FinanceLiabilityCreateParams {
  name: string;
  liabilityType: string;
  principal: number;
  currency?: string;
  remaining?: number;
  interestRate?: number;
  monthlyPayment?: number;
  dueDate?: string;
  notes?: string;
}

export interface FinancePortfolioCreateParams {
  name: string;
  description?: string;
  currency?: string;
}

export interface FinanceInvestmentCreateParams {
  portfolioId: string;
  assetType: string;
  costBasis: number;
  quantity: number;
  symbol?: string;
  name?: string;
  currency?: string;
  marketCurrency?: string;
}
