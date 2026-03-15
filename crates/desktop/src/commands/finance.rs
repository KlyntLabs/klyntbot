use std::collections::HashMap;
use std::sync::Arc;

use desktop_shared::commands::{
    FinanceAccountCreateParams, FinanceAccountUpdateParams, FinanceBudgetCreateParams,
    FinanceBudgetUpdateParams, FinanceCategoryReportResponse, FinanceDailySpendingResponse,
    FinanceGoalCreateParams, FinanceGoalUpdateParams, FinanceInvestmentCreateParams,
    FinanceInvestmentUpdateParams, FinanceLiabilityCreateParams, FinanceLiabilityUpdateParams,
    FinanceMonthlySummaryResponse, FinanceNetWorthResponse, FinancePeriodSummaryResponse,
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

#[tauri::command]
pub async fn finance_monthly_summary(
    state: State<'_, Arc<AppCore>>,
) -> Result<FinanceMonthlySummaryResponse, ApiError> {
    state.finance_monthly_summary().await
}

#[tauri::command]
pub async fn finance_daily_spending(
    state: State<'_, Arc<AppCore>>,
    date_from: String,
    date_to: String,
) -> Result<FinanceDailySpendingResponse, ApiError> {
    state.finance_daily_spending(date_from, date_to).await
}

#[tauri::command]
pub async fn finance_period_summary(
    state: State<'_, Arc<AppCore>>,
    date_from: String,
    date_to: String,
) -> Result<FinancePeriodSummaryResponse, ApiError> {
    state.finance_period_summary(date_from, date_to).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "finance_accounts",
    "finance_transactions",
    "finance_transactions_filtered",
    "finance_budget_usage",
    "finance_portfolios",
    "finance_investments",
    "finance_investments_filtered",
    "finance_goals",
    "finance_liabilities",
    "finance_net_worth",
    "finance_exchange_rates",
    "finance_account_create",
    "finance_account_update",
    "finance_account_delete",
    "finance_transaction_create",
    "finance_transaction_delete",
    "finance_budget_create",
    "finance_budget_update",
    "finance_budget_delete",
    "finance_goal_create",
    "finance_goal_update",
    "finance_goal_delete",
    "finance_liability_create",
    "finance_liability_update",
    "finance_liability_delete",
    "finance_portfolio_create",
    "finance_investment_create",
    "finance_investment_update",
    "finance_report_spending",
    "finance_report_income",
    "finance_report_trends",
    "finance_monthly_summary",
    "finance_daily_spending",
    "finance_period_summary",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        // Queries
        "finance_accounts" => dev::val(core.finance_accounts().await),
        "finance_transactions" => {
            dev::val(core.finance_transactions(dev::get(body, "limit")).await)
        }
        "finance_transactions_filtered" => dev::val(
            core.finance_transactions_filtered(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_budget_usage" => dev::val(core.finance_budget_usage().await),
        "finance_portfolios" => dev::val(core.finance_portfolios().await),
        "finance_investments" => dev::val(core.finance_investments().await),
        "finance_investments_filtered" => dev::val(
            core.finance_investments_filtered(dev::get(body, "portfolio_id"))
                .await,
        ),
        "finance_goals" => dev::val(core.finance_goals().await),
        "finance_liabilities" => dev::val(core.finance_liabilities().await),
        "finance_net_worth" => dev::val(core.finance_net_worth().await),
        "finance_exchange_rates" => dev::val(core.finance_exchange_rates().await),
        // Mutations
        "finance_account_create" => dev::val_rh(
            core.finance_account_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_account_update" => dev::val_rh(
            core.finance_account_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_account_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.finance_account_delete(id).await)
        }
        "finance_transaction_create" => dev::val_rh(
            core.finance_transaction_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_transaction_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.finance_transaction_delete(id).await)
        }
        "finance_budget_create" => dev::val_rh(
            core.finance_budget_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_budget_update" => dev::val_rh(
            core.finance_budget_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_budget_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.finance_budget_delete(id).await)
        }
        "finance_goal_create" => dev::val_rh(
            core.finance_goal_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_goal_update" => dev::val_rh(
            core.finance_goal_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_goal_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.finance_goal_delete(id).await)
        }
        "finance_liability_create" => dev::val_rh(
            core.finance_liability_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_liability_update" => dev::val_rh(
            core.finance_liability_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_liability_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.finance_liability_delete(id).await)
        }
        "finance_portfolio_create" => dev::val_rh(
            core.finance_portfolio_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_investment_create" => dev::val_rh(
            core.finance_investment_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_investment_update" => dev::val_rh(
            core.finance_investment_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        // Reports
        "finance_report_spending" => dev::val(
            core.finance_report_spending(dev::get(body, "date_from"), dev::get(body, "date_to"))
                .await,
        ),
        "finance_report_income" => dev::val(
            core.finance_report_income(dev::get(body, "date_from"), dev::get(body, "date_to"))
                .await,
        ),
        "finance_report_trends" => {
            let metric = try_field!(dev::get_str(body, "metric"));
            dev::val(
                core.finance_report_trends(metric, dev::get(body, "periods"))
                    .await,
            )
        }
        "finance_monthly_summary" => dev::val(core.finance_monthly_summary().await),
        "finance_daily_spending" => {
            let date_from = try_field!(dev::get_str(body, "date_from"));
            let date_to = try_field!(dev::get_str(body, "date_to"));
            dev::val(core.finance_daily_spending(date_from, date_to).await)
        }
        "finance_period_summary" => {
            let date_from = try_field!(dev::get_str(body, "date_from"));
            let date_to = try_field!(dev::get_str(body, "date_to"));
            dev::val(core.finance_period_summary(date_from, date_to).await)
        }
        _ => return None,
    })
}
