import type {
  FinanceAccount,
  FinanceTransaction,
  FinanceBudgetUsage,
  FinancePortfolio,
  FinanceInvestment,
  FinanceGoal,
  FinanceLiability,
  FinanceNetWorth,
} from '../lib/types';

// Exchange rates to VND (approximate)
export const mockExchangeRates: Record<string, number> = {
  VND: 1,
  USD: 25_850,
  USDT: 25_850,
  EUR: 27_200,
  BTC: 2_250_000_000,
  ETH: 67_000_000,
};

export const mockAccounts: FinanceAccount[] = [
  { id: 'acc-1', name: 'Vietcombank', accountType: 'bank', currency: 'VND', balance: 45_000_000_00, institution: 'Vietcombank', notes: null, isArchived: false },
  { id: 'acc-2', name: 'Momo Wallet', accountType: 'ewallet', currency: 'VND', balance: 3_200_000_00, institution: 'Momo', notes: null, isArchived: false },
  { id: 'acc-3', name: 'Binance Spot', accountType: 'crypto_wallet', currency: 'USDT', balance: 4_800_00, institution: 'Binance', notes: null, isArchived: false },
  { id: 'acc-4', name: 'Cash', accountType: 'cash', currency: 'VND', balance: 2_500_000_00, institution: null, notes: 'Physical cash', isArchived: false },
  { id: 'acc-5', name: 'VCBS Securities', accountType: 'brokerage', currency: 'VND', balance: 15_000_000_00, institution: 'VCBS', notes: null, isArchived: false },
];

export const mockTransactions: FinanceTransaction[] = [
  { id: 'tx-1', accountId: 'acc-1', txType: 'expense', amount: 350_000_00, currency: 'VND', category: 'Food', subcategory: 'Dining', counterparty: 'Highlands Coffee', notes: null, txDate: '2026-03-02', transferId: null },
  { id: 'tx-2', accountId: 'acc-1', txType: 'income', amount: 25_000_000_00, currency: 'VND', category: 'Salary', subcategory: null, counterparty: 'Employer', notes: 'March salary', txDate: '2026-03-01', transferId: null },
  { id: 'tx-3', accountId: 'acc-2', txType: 'expense', amount: 150_000_00, currency: 'VND', category: 'Transport', subcategory: 'Grab', counterparty: 'Grab', notes: null, txDate: '2026-03-01', transferId: null },
  { id: 'tx-4', accountId: 'acc-3', txType: 'expense', amount: 542_00, currency: 'USDT', category: 'Investment', subcategory: 'Crypto', counterparty: 'Binance', notes: 'Buy ETH', txDate: '2026-02-28', transferId: null },
  { id: 'tx-5', accountId: 'acc-1', txType: 'expense', amount: 8_500_000_00, currency: 'VND', category: 'Housing', subcategory: 'Rent', counterparty: 'Landlord', notes: 'March rent', txDate: '2026-02-28', transferId: null },
  { id: 'tx-6', accountId: 'acc-1', txType: 'transfer', amount: 5_000_000_00, currency: 'VND', category: null, subcategory: null, counterparty: null, notes: 'Top up Momo', txDate: '2026-02-27', transferId: 'tfr-1' },
  { id: 'tx-7', accountId: 'acc-1', txType: 'expense', amount: 2_000_000_00, currency: 'VND', category: 'Shopping', subcategory: 'Electronics', counterparty: 'Shopee', notes: 'USB-C hub', txDate: '2026-02-26', transferId: null },
  { id: 'tx-8', accountId: 'acc-2', txType: 'expense', amount: 89_000_00, currency: 'VND', category: 'Food', subcategory: 'Groceries', counterparty: 'Bach Hoa Xanh', notes: null, txDate: '2026-02-26', transferId: null },
  { id: 'tx-9', accountId: 'acc-1', txType: 'expense', amount: 500_000_00, currency: 'VND', category: 'Entertainment', subcategory: null, counterparty: 'Netflix + Spotify', notes: null, txDate: '2026-02-25', transferId: null },
  { id: 'tx-10', accountId: 'acc-3', txType: 'income', amount: 120_00, currency: 'USDT', category: 'Investment', subcategory: 'Staking', counterparty: 'Binance Earn', notes: 'USDT staking reward', txDate: '2026-02-24', transferId: null },
];

export const mockBudgets: FinanceBudgetUsage[] = [
  { id: 'bgt-1', name: 'Food & Dining', amount: 5_000_000_00, currency: 'VND', period: 'monthly', category: 'Food', method: 'standard', jarType: null, isActive: true, alertThreshold: 80, spent: 3_890_000_00 },
  { id: 'bgt-2', name: 'Housing', amount: 10_000_000_00, currency: 'VND', period: 'monthly', category: 'Housing', method: 'standard', jarType: null, isActive: true, alertThreshold: 80, spent: 8_500_000_00 },
  { id: 'bgt-3', name: 'Transport', amount: 2_000_000_00, currency: 'VND', period: 'monthly', category: 'Transport', method: 'standard', jarType: null, isActive: true, alertThreshold: 80, spent: 950_000_00 },
  { id: 'bgt-4', name: 'Entertainment', amount: 3_000_000_00, currency: 'VND', period: 'monthly', category: 'Entertainment', method: 'standard', jarType: null, isActive: true, alertThreshold: 80, spent: 500_000_00 },
  { id: 'bgt-5', name: 'Shopping', amount: 4_000_000_00, currency: 'VND', period: 'monthly', category: 'Shopping', method: 'standard', jarType: null, isActive: true, alertThreshold: 80, spent: 2_000_000_00 },
];

export const mockPortfolios: FinancePortfolio[] = [
  { id: 'pf-1', name: 'Crypto Portfolio', description: 'Long-term crypto holdings', currency: 'USDT', totalValue: 8_250_00, totalCostBasis: 6_500_00, holdingCount: 3 },
  { id: 'pf-2', name: 'VN Stocks', description: 'Vietnamese equities', currency: 'VND', totalValue: 28_000_000_00, totalCostBasis: 22_000_000_00, holdingCount: 4 },
];

export const mockInvestments: FinanceInvestment[] = [
  { id: 'inv-1', portfolioId: 'pf-1', assetType: 'crypto', symbol: 'BTC', name: 'Bitcoin', quantity: 0.025, costBasis: 2_100_00, currency: 'USDT', currentPrice: 87_000_00, currentValue: 2_175_00 },
  { id: 'inv-2', portfolioId: 'pf-1', assetType: 'crypto', symbol: 'ETH', name: 'Ethereum', quantity: 1.5, costBasis: 2_800_00, currency: 'USDT', currentPrice: 2_600_00, currentValue: 3_900_00 },
  { id: 'inv-3', portfolioId: 'pf-1', assetType: 'crypto', symbol: 'SOL', name: 'Solana', quantity: 15, costBasis: 1_600_00, currency: 'USDT', currentPrice: 145_00, currentValue: 2_175_00 },
  { id: 'inv-4', portfolioId: 'pf-2', assetType: 'stock', symbol: 'VNM', name: 'Vinamilk', quantity: 500, costBasis: 6_000_000_00, currency: 'VND', currentPrice: 14_500_00, currentValue: 7_250_000_00 },
  { id: 'inv-5', portfolioId: 'pf-2', assetType: 'stock', symbol: 'FPT', name: 'FPT Corp', quantity: 200, costBasis: 8_000_000_00, currency: 'VND', currentPrice: 52_000_00, currentValue: 10_400_000_00 },
  { id: 'inv-6', portfolioId: 'pf-2', assetType: 'stock', symbol: 'VHM', name: 'Vinhomes', quantity: 300, costBasis: 4_500_000_00, currency: 'VND', currentPrice: 14_000_00, currentValue: 4_200_000_00 },
  { id: 'inv-7', portfolioId: 'pf-2', assetType: 'stock', symbol: 'MWG', name: 'Mobile World', quantity: 400, costBasis: 3_500_000_00, currency: 'VND', currentPrice: 15_400_00, currentValue: 6_160_000_00 },
];

export const mockGoals: FinanceGoal[] = [
  { id: 'goal-1', name: 'Emergency Fund', goalType: 'savings', targetAmount: 100_000_000_00, currentAmount: 45_000_000_00, currency: 'VND', status: 'active', deadline: '2026-12-31', monthlyContribution: 5_000_000_00 },
  { id: 'goal-2', name: 'New Laptop', goalType: 'purchase', targetAmount: 35_000_000_00, currentAmount: 12_000_000_00, currency: 'VND', status: 'active', deadline: '2026-06-30', monthlyContribution: 3_000_000_00 },
  { id: 'goal-3', name: 'FIRE Target', goalType: 'fire', targetAmount: 5_000_000_000_00, currentAmount: 320_000_000_00, currency: 'VND', status: 'active', deadline: null, monthlyContribution: 10_000_000_00 },
];

export const mockLiabilities: FinanceLiability[] = [
  { id: 'liab-1', name: 'Student Loan', liabilityType: 'student_loan', principal: 50_000_000_00, remaining: 32_000_000_00, currency: 'VND', interestRate: 6.5, monthlyPayment: 2_000_000_00, dueDate: '2028-06-01' },
  { id: 'liab-2', name: 'Credit Card', liabilityType: 'credit_card', principal: 5_000_000_00, remaining: 3_200_000_00, currency: 'VND', interestRate: 18.0, monthlyPayment: 1_000_000_00, dueDate: null },
];

export const mockNetWorth: FinanceNetWorth = {
  totalsByCurrency: [
    { currency: 'VND', accounts: 65_700_000_00, investments: 28_010_000_00, liabilities: 35_200_000_00, net: 58_510_000_00 },
    { currency: 'USDT', accounts: 4_800_00, investments: 8_250_00, liabilities: 0, net: 13_050_00 },
  ],
};
