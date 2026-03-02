//! Finance IPC commands — read-only queries against FinanceStorage repos.

use std::collections::HashMap;

use desktop_shared::commands::{
    CurrencyNetWorth, FinanceNetWorthResponse, FinancePortfolioResponse,
};
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceGoalRow, FinanceInvestmentRow, FinanceLiabilityRow,
    FinanceTransactionRow,
};
use storage::rows::finance::{FinanceInvestmentFilter, FinanceTransactionFilter};
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

    let mut result = Vec::with_capacity(portfolios.len());
    for p in &portfolios {
        let summary = state
            .repos
            .finance
            .investments
            .portfolio_summary(&p.id)
            .await
            .map_err(|e| e.to_string())?;

        result.push(FinancePortfolioResponse {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            currency: p.currency.clone(),
            total_value: summary.total_current_value,
            total_cost_basis: summary.total_cost_basis,
            holding_count: summary.holding_count,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn finance_investments(
    state: State<'_, AppCore>,
) -> Result<Vec<FinanceInvestmentRow>, String> {
    let filter = FinanceInvestmentFilter::default();
    state
        .repos
        .finance
        .investments
        .list_investments(&filter)
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
    let inv_filter = FinanceInvestmentFilter::default();
    let (accounts, investments, liabilities) = tokio::join!(
        state.repos.finance.accounts.list(false),
        state
            .repos
            .finance
            .investments
            .list_investments(&inv_filter),
        state.repos.finance.liabilities.list_all(),
    );
    let accounts = accounts.map_err(|e| e.to_string())?;
    let investments = investments.map_err(|e| e.to_string())?;
    let liabilities = liabilities.map_err(|e| e.to_string())?;

    // Aggregate by currency
    let mut by_currency: HashMap<&str, CurrencyNetWorth> = HashMap::new();

    for a in &accounts {
        let entry = by_currency
            .entry(&a.currency)
            .or_insert_with(|| CurrencyNetWorth {
                currency: a.currency.clone(),
                accounts: 0,
                investments: 0,
                liabilities: 0,
                net: 0,
            });
        entry.accounts += a.balance;
    }

    for i in &investments {
        let value = i.current_value.unwrap_or(i.cost_basis);
        let entry = by_currency
            .entry(&i.currency)
            .or_insert_with(|| CurrencyNetWorth {
                currency: i.currency.clone(),
                accounts: 0,
                investments: 0,
                liabilities: 0,
                net: 0,
            });
        entry.investments += value;
    }

    for l in &liabilities {
        let entry = by_currency
            .entry(&l.currency)
            .or_insert_with(|| CurrencyNetWorth {
                currency: l.currency.clone(),
                accounts: 0,
                investments: 0,
                liabilities: 0,
                net: 0,
            });
        entry.liabilities += l.remaining;
    }

    // Compute net for each currency
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
