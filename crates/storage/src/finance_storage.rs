//! Aggregate of all finance repository handles.

use sqlx::SqlitePool;

use crate::repos::{
    FinanceAccountRepo, FinanceAllocationRepo, FinanceBudgetRepo, FinanceExchangeRateRepo,
    FinanceGoalRepo, FinanceInvestmentRepo, FinanceLiabilityRepo, FinanceSnapshotRepo,
    FinanceTransactionRepo,
};

/// Aggregate of all finance repositories, constructed from a single pool.
#[derive(Debug, Clone)]
pub struct FinanceStorage {
    pool: SqlitePool,
    pub accounts: FinanceAccountRepo,
    pub transactions: FinanceTransactionRepo,
    pub budgets: FinanceBudgetRepo,
    pub investments: FinanceInvestmentRepo,
    pub goals: FinanceGoalRepo,
    pub liabilities: FinanceLiabilityRepo,
    pub allocations: FinanceAllocationRepo,
    pub snapshots: FinanceSnapshotRepo,
    pub exchange_rates: FinanceExchangeRateRepo,
}

impl FinanceStorage {
    pub fn from_pool(pool: &SqlitePool) -> Self {
        Self {
            pool: pool.clone(),
            accounts: FinanceAccountRepo::new(pool.clone()),
            transactions: FinanceTransactionRepo::new(pool.clone()),
            budgets: FinanceBudgetRepo::new(pool.clone()),
            investments: FinanceInvestmentRepo::new(pool.clone()),
            goals: FinanceGoalRepo::new(pool.clone()),
            liabilities: FinanceLiabilityRepo::new(pool.clone()),
            allocations: FinanceAllocationRepo::new(pool.clone()),
            snapshots: FinanceSnapshotRepo::new(pool.clone()),
            exchange_rates: FinanceExchangeRateRepo::new(pool.clone()),
        }
    }

    /// Access the underlying SQLite pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
