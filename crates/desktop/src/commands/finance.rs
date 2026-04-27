use std::collections::HashMap;
use std::sync::Arc;

use desktop_shared::commands::{
    FinanceAccountCreateParams, FinanceAccountUpdateParams, FinanceAllocationTargetUpsertParams,
    FinanceBudgetCreateParams, FinanceBudgetUpdateParams, FinanceCategoryReportResponse,
    FinanceDailySpendingResponse, FinanceGoalCreateParams, FinanceGoalUpdateParams,
    FinanceInvestmentCreateParams, FinanceInvestmentTxCreateParams, FinanceInvestmentUpdateParams,
    FinanceLiabilityCreateParams, FinanceLiabilityUpdateParams, FinanceMonthlySummaryResponse,
    FinanceNetWorthResponse, FinancePeriodSummaryResponse, FinancePortfolioCreateParams,
    FinancePortfolioResponse, FinanceTransactionCreateParams, FinanceTransactionFilterParams,
    FinanceTrendPoint,
};
use desktop_shared::CommandResult;
use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountRow, FinanceAllocationTargetRow, FinanceBudgetRow,
    FinanceGoalRow, FinanceInvestmentRow, FinanceInvestmentTxRow, FinanceLiabilityRow,
    FinancePortfolioRow, FinanceTransactionRow,
};
use tauri::State;

use crate::app_core::AppCore;

// ── Read-only queries ───────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn finance_accounts(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<FinanceAccountRow>> {
    state.finance_accounts().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_transactions(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> CommandResult<Vec<FinanceTransactionRow>> {
    state.finance_transactions(limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_transactions_filtered(
    state: State<'_, Arc<AppCore>>,
    params: FinanceTransactionFilterParams,
) -> CommandResult<Vec<FinanceTransactionRow>> {
    state.finance_transactions_filtered(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_budget_usage(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<BudgetUsageRow>> {
    state.finance_budget_usage().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_portfolios(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<FinancePortfolioResponse>> {
    state.finance_portfolios().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_investments(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<FinanceInvestmentRow>> {
    state.finance_investments().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_investments_filtered(
    state: State<'_, Arc<AppCore>>,
    portfolio_id: Option<String>,
) -> CommandResult<Vec<FinanceInvestmentRow>> {
    state.finance_investments_filtered(portfolio_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_goals(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<FinanceGoalRow>> {
    state.finance_goals().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_liabilities(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<FinanceLiabilityRow>> {
    state.finance_liabilities().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_net_worth(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<FinanceNetWorthResponse> {
    state.finance_net_worth().await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_exchange_rates(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<HashMap<String, f64>> {
    state.finance_exchange_rates().await
}

// ── Mutations ───────────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn finance_account_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAccountCreateParams,
) -> CommandResult<FinanceAccountRow> {
    let (result, updates) = state.finance_account_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_account_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAccountUpdateParams,
) -> CommandResult<FinanceAccountRow> {
    let (result, updates) = state.finance_account_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_account_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.finance_account_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_transaction_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceTransactionCreateParams,
) -> CommandResult<FinanceTransactionRow> {
    let (result, updates) = state.finance_transaction_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_transaction_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.finance_transaction_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_budget_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceBudgetCreateParams,
) -> CommandResult<FinanceBudgetRow> {
    let (result, updates) = state.finance_budget_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_budget_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceBudgetUpdateParams,
) -> CommandResult<FinanceBudgetRow> {
    let (result, updates) = state.finance_budget_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_budget_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.finance_budget_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_goal_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceGoalCreateParams,
) -> CommandResult<FinanceGoalRow> {
    let (result, updates) = state.finance_goal_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_goal_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceGoalUpdateParams,
) -> CommandResult<FinanceGoalRow> {
    let (result, updates) = state.finance_goal_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_goal_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.finance_goal_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_liability_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceLiabilityCreateParams,
) -> CommandResult<FinanceLiabilityRow> {
    let (result, updates) = state.finance_liability_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_liability_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceLiabilityUpdateParams,
) -> CommandResult<FinanceLiabilityRow> {
    let (result, updates) = state.finance_liability_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_liability_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.finance_liability_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_portfolio_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinancePortfolioCreateParams,
) -> CommandResult<FinancePortfolioRow> {
    let (result, updates) = state.finance_portfolio_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_investment_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceInvestmentCreateParams,
) -> CommandResult<FinanceInvestmentRow> {
    let (result, updates) = state.finance_investment_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn finance_investment_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceInvestmentUpdateParams,
) -> CommandResult<FinanceInvestmentRow> {
    let (result, updates) = state.finance_investment_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Allocation Targets ──────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn finance_allocation_target_upsert(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceAllocationTargetUpsertParams,
) -> CommandResult<FinanceAllocationTargetRow> {
    let (result, updates) = state.finance_allocation_target_upsert(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_allocation_targets(
    state: State<'_, Arc<AppCore>>,
    portfolio_id: String,
) -> CommandResult<Vec<FinanceAllocationTargetRow>> {
    state.finance_allocation_targets(portfolio_id).await
}

// ── Investment Transactions ─────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn finance_investment_tx_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: FinanceInvestmentTxCreateParams,
) -> CommandResult<FinanceInvestmentTxRow> {
    let (result, updates) = state.finance_investment_tx_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_investment_txs(
    state: State<'_, Arc<AppCore>>,
    investment_id: String,
) -> CommandResult<Vec<FinanceInvestmentTxRow>> {
    state.finance_investment_txs(investment_id).await
}

// ── Reports ─────────────────────────────────────────────────────────────

#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_report_spending(
    state: State<'_, Arc<AppCore>>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> CommandResult<FinanceCategoryReportResponse> {
    state.finance_report_spending(date_from, date_to).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_report_income(
    state: State<'_, Arc<AppCore>>,
    date_from: Option<String>,
    date_to: Option<String>,
) -> CommandResult<FinanceCategoryReportResponse> {
    state.finance_report_income(date_from, date_to).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_report_trends(
    state: State<'_, Arc<AppCore>>,
    metric: String,
    periods: Option<i64>,
) -> CommandResult<Vec<FinanceTrendPoint>> {
    state.finance_report_trends(metric, periods).await
}

#[tauri::command]
#[specta::specta]
pub async fn finance_monthly_summary(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<FinanceMonthlySummaryResponse> {
    state.finance_monthly_summary().await
}

#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_daily_spending(
    state: State<'_, Arc<AppCore>>,
    date_from: String,
    date_to: String,
) -> CommandResult<FinanceDailySpendingResponse> {
    state.finance_daily_spending(date_from, date_to).await
}

#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn finance_period_summary(
    state: State<'_, Arc<AppCore>>,
    date_from: String,
    date_to: String,
) -> CommandResult<FinancePeriodSummaryResponse> {
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
    "finance_allocation_target_upsert",
    "finance_allocation_targets",
    "finance_investment_tx_create",
    "finance_investment_txs",
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
) -> Option<CommandResult<serde_json::Value>> {
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
        "finance_allocation_target_upsert" => dev::val_rh(
            core.finance_allocation_target_upsert(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_allocation_targets" => {
            let portfolio_id = try_field!(dev::get_str(body, "portfolioId"));
            dev::val(core.finance_allocation_targets(portfolio_id).await)
        }
        "finance_investment_tx_create" => dev::val_rh(
            core.finance_investment_tx_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "finance_investment_txs" => {
            let investment_id = try_field!(dev::get_str(body, "investmentId"));
            dev::val(core.finance_investment_txs(investment_id).await)
        }
        // Reports
        "finance_report_spending" => dev::val(
            core.finance_report_spending(dev::get(body, "dateFrom"), dev::get(body, "dateTo"))
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
            let date_from = try_field!(dev::get_str(body, "dateFrom"));
            let date_to = try_field!(dev::get_str(body, "dateTo"));
            dev::val(core.finance_daily_spending(date_from, date_to).await)
        }
        "finance_period_summary" => {
            let date_from = try_field!(dev::get_str(body, "dateFrom"));
            let date_to = try_field!(dev::get_str(body, "dateTo"));
            dev::val(core.finance_period_summary(date_from, date_to).await)
        }
        _ => return None,
    })
}
