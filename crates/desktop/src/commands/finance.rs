use std::collections::HashMap;
use std::sync::Arc;

use desktop_shared::commands::{
    FinanceAccountCreateParams, FinanceAccountUpdateParams, FinanceBudgetCreateParams,
    FinanceBudgetUpdateParams, FinanceCategoryReportResponse, FinanceGoalCreateParams,
    FinanceGoalUpdateParams, FinanceInvestmentCreateParams, FinanceInvestmentUpdateParams,
    FinanceLiabilityCreateParams, FinanceLiabilityUpdateParams, FinanceNetWorthResponse,
    FinancePortfolioCreateParams, FinancePortfolioResponse, FinanceTransactionCreateParams,
    FinanceTransactionFilterParams, FinanceTrendPoint,
};
use desktop_shared::errors::ApiError;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceBudgetRow, FinanceGoalRow, FinanceInvestmentRow,
    FinanceLiabilityRow, FinancePortfolioRow, FinanceTransactionRow,
};
use tauri::State;

use crate::app_core::AppCore;

// ── Read-only queries ───────────────────────────────────────────────────

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
pub async fn finance_transactions_filtered(
    state: State<'_, Arc<AppCore>>,
    params: FinanceTransactionFilterParams,
) -> Result<Vec<FinanceTransactionRow>, ApiError> {
    state.finance_transactions_filtered(params).await
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
pub async fn finance_investments_filtered(
    state: State<'_, Arc<AppCore>>,
    portfolio_id: Option<String>,
) -> Result<Vec<FinanceInvestmentRow>, ApiError> {
    state.finance_investments_filtered(portfolio_id).await
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

// ── Mutations ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn finance_account_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAccountCreateParams,
) -> Result<FinanceAccountRow, ApiError> {
    let (result, updates) = state.finance_account_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_account_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAccountUpdateParams,
) -> Result<FinanceAccountRow, ApiError> {
    let (result, updates) = state.finance_account_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_account_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.finance_account_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_transaction_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceTransactionCreateParams,
) -> Result<FinanceTransactionRow, ApiError> {
    let (result, updates) = state.finance_transaction_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_transaction_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.finance_transaction_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_budget_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceBudgetCreateParams,
) -> Result<FinanceBudgetRow, ApiError> {
    let (result, updates) = state.finance_budget_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_budget_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceBudgetUpdateParams,
) -> Result<FinanceBudgetRow, ApiError> {
    let (result, updates) = state.finance_budget_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_budget_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.finance_budget_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_goal_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceGoalCreateParams,
) -> Result<FinanceGoalRow, ApiError> {
    let (result, updates) = state.finance_goal_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_goal_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceGoalUpdateParams,
) -> Result<FinanceGoalRow, ApiError> {
    let (result, updates) = state.finance_goal_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_goal_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.finance_goal_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_liability_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceLiabilityCreateParams,
) -> Result<FinanceLiabilityRow, ApiError> {
    let (result, updates) = state.finance_liability_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_liability_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceLiabilityUpdateParams,
) -> Result<FinanceLiabilityRow, ApiError> {
    let (result, updates) = state.finance_liability_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_liability_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.finance_liability_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_portfolio_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinancePortfolioCreateParams,
) -> Result<FinancePortfolioRow, ApiError> {
    let (result, updates) = state.finance_portfolio_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_investment_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceInvestmentCreateParams,
) -> Result<FinanceInvestmentRow, ApiError> {
    let (result, updates) = state.finance_investment_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn finance_investment_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceInvestmentUpdateParams,
) -> Result<FinanceInvestmentRow, ApiError> {
    let (result, updates) = state.finance_investment_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Reports ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn finance_report_spending(
    state: State<'_, Arc<AppCore>>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<FinanceCategoryReportResponse, ApiError> {
    state.finance_report_spending(date_from, date_to).await
}

#[tauri::command]
pub async fn finance_report_income(
    state: State<'_, Arc<AppCore>>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> Result<FinanceCategoryReportResponse, ApiError> {
    state.finance_report_income(date_from, date_to).await
}

#[tauri::command]
pub async fn finance_report_trends(
    state: State<'_, Arc<AppCore>>,
    metric: String,
    periods: Option<i64>,
) -> Result<Vec<FinanceTrendPoint>, ApiError> {
    state.finance_report_trends(metric, periods).await
}
