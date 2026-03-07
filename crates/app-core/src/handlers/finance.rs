//! Finance handlers — read-only queries against FinanceStorage repos.

use std::collections::HashMap;

use desktop_shared::commands::{
    CurrencyNetWorth, FinanceNetWorthResponse, FinancePortfolioResponse,
};
use desktop_shared::errors::ApiError;
use futures_util::future::try_join_all;
use storage::rows::finance::FinanceTransactionFilter;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceGoalRow, FinanceInvestmentRow, FinanceLiabilityRow,
    FinanceTransactionRow,
};

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn finance_accounts(&self) -> Result<Vec<FinanceAccountRow>, ApiError> {
        self.repos
            .finance
            .accounts
            .list(false)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_transactions(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<FinanceTransactionRow>, ApiError> {
        let filter = FinanceTransactionFilter {
            limit,
            ..Default::default()
        };
        self.repos
            .finance
            .transactions
            .list(&filter)
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_budget_usage(&self) -> Result<Vec<BudgetUsageRow>, ApiError> {
        self.repos
            .finance
            .budgets
            .all_budget_usage()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_portfolios(&self) -> Result<Vec<FinancePortfolioResponse>, ApiError> {
        let portfolios = self
            .repos
            .finance
            .investments
            .list_portfolios()
            .await
            .map_err(map_storage_err)?;

        let summaries = try_join_all(
            portfolios
                .iter()
                .map(|p| self.repos.finance.investments.portfolio_summary(&p.id)),
        )
        .await
        .map_err(map_storage_err)?;

        Ok(portfolios
            .iter()
            .zip(summaries)
            .map(|(p, summary)| FinancePortfolioResponse {
                id: p.id.clone(),
                name: p.name.clone(),
                description: p.description.clone(),
                currency: p.currency.clone(),
                total_value: summary.total_current_value,
                total_cost_basis: summary.total_cost_basis,
                holding_count: summary.holding_count,
            })
            .collect())
    }

    pub async fn finance_investments(&self) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
        self.repos
            .finance
            .investments
            .list_investments(&Default::default())
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_goals(&self) -> Result<Vec<FinanceGoalRow>, ApiError> {
        self.repos
            .finance
            .goals
            .list_active()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_liabilities(&self) -> Result<Vec<FinanceLiabilityRow>, ApiError> {
        self.repos
            .finance
            .liabilities
            .list_all()
            .await
            .map_err(map_storage_err)
    }

    pub async fn finance_net_worth(&self) -> Result<FinanceNetWorthResponse, ApiError> {
        let (account_totals, investment_totals, liability_totals) = tokio::try_join!(
            self.repos.finance.accounts.total_balance_by_currency(),
            self.repos.finance.investments.total_value_by_currency(),
            self.repos.finance.liabilities.total_remaining_by_currency(),
        )
        .map_err(map_storage_err)?;

        // Merge the three per-currency aggregates into CurrencyNetWorth entries
        let mut by_currency: HashMap<&str, CurrencyNetWorth> = HashMap::new();

        for (currency, total) in &account_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .accounts = *total;
        }
        for (currency, total) in &investment_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .investments = *total;
        }
        for (currency, total) in &liability_totals {
            by_currency
                .entry(currency)
                .or_insert_with(|| CurrencyNetWorth::zero(currency.clone()))
                .liabilities = *total;
        }

        let totals_by_currency: Vec<CurrencyNetWorth> = by_currency
            .into_values()
            .map(|mut c| {
                c.net = c.accounts + c.investments - c.liabilities;
                c
            })
            .collect();

        Ok(FinanceNetWorthResponse { totals_by_currency })
    }

    pub async fn finance_exchange_rates(&self) -> Result<HashMap<String, f64>, ApiError> {
        // Exchange rates are not yet stored in the backend.
        // Return an empty map; the frontend handles the absence gracefully.
        Ok(HashMap::new())
    }
}
