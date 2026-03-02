//! Finance IPC commands — read-only queries against FinanceStorage repos.

use std::collections::HashMap;

use desktop_shared::commands::{
    CurrencyNetWorth, FinanceNetWorthResponse, FinancePortfolioResponse,
};
use futures_util::future::try_join_all;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceGoalRow, FinanceInvestmentRow, FinanceLiabilityRow,
    FinanceTransactionRow,
};
use storage::rows::finance::FinanceTransactionFilter;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn finance_accounts(state: State<'_, AppCore>) -> Result<Vec<FinanceAccountRow>, String> {
    state
        .repos
        .finance
        .accounts
        .list(false)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_transactions(
    state: State<'_, AppCore>,
    limit: Option<i64>,
) -> Result<Vec<FinanceTransactionRow>, String> {
    let filter = FinanceTransactionFilter {
        limit,
        ..Default::default()
    };
    state
        .repos
        .finance
        .transactions
        .list(&filter)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_budget_usage(
    state: State<'_, AppCore>,
) -> Result<Vec<BudgetUsageRow>, String> {
    state
        .repos
        .finance
        .budgets
        .all_budget_usage()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_portfolios(
    state: State<'_, AppCore>,
) -> Result<Vec<FinancePortfolioResponse>, String> {
    let portfolios = state
        .repos
        .finance
        .investments
        .list_portfolios()
        .await
        .map_err(|e| e.to_string())?;

    let summaries = try_join_all(
        portfolios
            .iter()
            .map(|p| state.repos.finance.investments.portfolio_summary(&p.id)),
    )
    .await
    .map_err(|e| e.to_string())?;

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

#[tauri::command]
pub async fn finance_investments(
    state: State<'_, AppCore>,
) -> Result<Vec<FinanceInvestmentRow>, String> {
    state
        .repos
        .finance
        .investments
        .list_investments(&Default::default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_goals(state: State<'_, AppCore>) -> Result<Vec<FinanceGoalRow>, String> {
    state
        .repos
        .finance
        .goals
        .list_active()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_liabilities(
    state: State<'_, AppCore>,
) -> Result<Vec<FinanceLiabilityRow>, String> {
    state
        .repos
        .finance
        .liabilities
        .list_all()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn finance_net_worth(
    state: State<'_, AppCore>,
) -> Result<FinanceNetWorthResponse, String> {
    let (account_totals, investment_totals, liability_totals) = tokio::try_join!(
        state.repos.finance.accounts.total_balance_by_currency(),
        state.repos.finance.investments.total_value_by_currency(),
        state.repos.finance.liabilities.total_remaining_by_currency(),
    )
    .map_err(|e| e.to_string())?;

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

#[tauri::command]
pub async fn finance_exchange_rates() -> Result<HashMap<String, f64>, String> {
    // Exchange rates are not yet stored in the backend.
    // Return an empty map; the frontend handles the absence gracefully.
    Ok(HashMap::new())
}
