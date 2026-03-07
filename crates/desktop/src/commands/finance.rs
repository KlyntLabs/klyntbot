use std::collections::HashMap;
use std::sync::Arc;

use desktop_shared::commands::FinanceNetWorthResponse;
use desktop_shared::commands::FinancePortfolioResponse;
use desktop_shared::errors::ApiError;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceGoalRow, FinanceInvestmentRow, FinanceLiabilityRow,
    FinanceTransactionRow,
};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn finance_accounts(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<FinanceAccountRow>, ApiError> {
    state.finance_accounts().await
}

#[tauri::command]
pub async fn finance_transactions(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<FinanceTransactionRow>, ApiError> {
    state.finance_transactions(limit).await
}

#[tauri::command]
pub async fn finance_budget_usage(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<BudgetUsageRow>, ApiError> {
    state.finance_budget_usage().await
}

#[tauri::command]
pub async fn finance_portfolios(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<FinancePortfolioResponse>, ApiError> {
    state.finance_portfolios().await
}

#[tauri::command]
pub async fn finance_investments(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
    state.finance_investments().await
}

#[tauri::command]
pub async fn finance_goals(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<FinanceGoalRow>, ApiError> {
    state.finance_goals().await
}

#[tauri::command]
pub async fn finance_liabilities(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<FinanceLiabilityRow>, ApiError> {
    state.finance_liabilities().await
}

#[tauri::command]
pub async fn finance_net_worth(
    state: State<'_, Arc<AppCore>>,
) -> Result<FinanceNetWorthResponse, ApiError> {
    state.finance_net_worth().await
}

#[tauri::command]
pub async fn finance_exchange_rates(
    state: State<'_, Arc<AppCore>>,
) -> Result<HashMap<String, f64>, ApiError> {
    state.finance_exchange_rates().await
}
