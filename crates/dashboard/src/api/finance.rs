//! Finance API handlers — accounts, transactions, budgets, investments, goals, liabilities.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;

use storage::rows::finance::{
    BudgetUsageRow, FinanceAccountPatch, FinanceAccountRow, FinanceBudgetPatch, FinanceBudgetRow,
    FinanceGoalPatch, FinanceGoalRow, FinanceInvestmentFilter, FinanceInvestmentPatch,
    FinanceInvestmentRow, FinanceLiabilityPatch, FinanceLiabilityRow, FinanceTransactionFilter,
    FinanceTransactionPatch, FinanceTransactionRow,
};

use crate::error::ApiError;
use crate::state::AppState;

// ============================================================
// Accounts
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountRequest {
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub balance: Option<i64>,
    pub institution: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchAccountRequest {
    pub name: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

/// GET /api/finance/accounts
pub async fn list_accounts(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceAccountRow>>, ApiError> {
    let rows = state
        .repos
        .finance_accounts
        .list(false)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/accounts
pub async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<FinanceAccountRow>, ApiError> {
    let now = Utc::now();
    let row = FinanceAccountRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        account_type: req.account_type,
        currency: req.currency,
        balance: req.balance.unwrap_or(0),
        institution: req.institution,
        notes: req.notes,
        is_archived: false,
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_accounts
        .add(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// GET /api/finance/accounts/:id
pub async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FinanceAccountRow>, ApiError> {
    let row = state
        .repos
        .finance_accounts
        .get(&id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("account '{id}' not found")))?;
    Ok(Json(row))
}

/// PATCH /api/finance/accounts/:id
pub async fn patch_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchAccountRequest>,
) -> Result<Json<FinanceAccountRow>, ApiError> {
    let patch = FinanceAccountPatch {
        id,
        name: req.name,
        balance: req.balance,
        institution: req.institution,
        notes: req.notes,
        is_archived: req.is_archived,
    };
    let updated = state
        .repos
        .finance_accounts
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/accounts/:id
pub async fn delete_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_accounts
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("account '{id}' not found")))
    }
}

// ============================================================
// Transactions
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionRequest {
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: String,
    pub tx_date: NaiveDate,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub notes: Option<String>,
    pub transfer_id: Option<String>,
    pub is_recurring: Option<bool>,
    pub recurring_rule: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchTransactionRequest {
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<NaiveDate>,
}

/// GET /api/finance/transactions
pub async fn list_transactions(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceTransactionRow>>, ApiError> {
    let rows = state
        .repos
        .finance_transactions
        .list(&FinanceTransactionFilter::default())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/transactions
pub async fn create_transaction(
    State(state): State<AppState>,
    Json(req): Json<CreateTransactionRequest>,
) -> Result<Json<FinanceTransactionRow>, ApiError> {
    let now = Utc::now();
    let row = FinanceTransactionRow {
        id: uuid::Uuid::new_v4().to_string(),
        account_id: req.account_id,
        tx_type: req.tx_type,
        amount: req.amount,
        currency: req.currency,
        tx_date: req.tx_date,
        category: req.category,
        subcategory: req.subcategory,
        counterparty: req.counterparty,
        notes: req.notes,
        transfer_id: req.transfer_id,
        is_recurring: req.is_recurring.unwrap_or(false),
        recurring_rule: req.recurring_rule,
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_transactions
        .add(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// GET /api/finance/transactions/:id
pub async fn get_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FinanceTransactionRow>, ApiError> {
    let row = state
        .repos
        .finance_transactions
        .get(&id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("transaction '{id}' not found")))?;
    Ok(Json(row))
}

/// PATCH /api/finance/transactions/:id
pub async fn patch_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchTransactionRequest>,
) -> Result<Json<FinanceTransactionRow>, ApiError> {
    let patch = FinanceTransactionPatch {
        id,
        amount: req.amount,
        category: req.category,
        subcategory: req.subcategory,
        counterparty: req.counterparty,
        notes: req.notes,
        tx_date: req.tx_date,
    };
    let updated = state
        .repos
        .finance_transactions
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/transactions/:id
pub async fn delete_transaction(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_transactions
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted.is_some() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("transaction '{id}' not found")))
    }
}

// ============================================================
// Budgets
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBudgetRequest {
    pub name: String,
    pub amount: i64,
    pub currency: String,
    pub period: String,
    pub category: Option<String>,
    pub method: Option<String>,
    pub jar_type: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub alert_threshold: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchBudgetRequest {
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
}

/// GET /api/finance/budgets
pub async fn list_budgets(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceBudgetRow>>, ApiError> {
    let rows = state
        .repos
        .finance_budgets
        .list_active()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/budgets
pub async fn create_budget(
    State(state): State<AppState>,
    Json(req): Json<CreateBudgetRequest>,
) -> Result<Json<FinanceBudgetRow>, ApiError> {
    let now = Utc::now();
    let row = FinanceBudgetRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        amount: req.amount,
        currency: req.currency,
        period: req.period,
        category: req.category,
        method: req.method.unwrap_or_else(|| "envelope".to_string()),
        jar_type: req.jar_type,
        start_date: req.start_date,
        end_date: req.end_date,
        is_active: true,
        alert_threshold: req.alert_threshold.unwrap_or(80),
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_budgets
        .add(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// PATCH /api/finance/budgets/:id
pub async fn patch_budget(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchBudgetRequest>,
) -> Result<Json<FinanceBudgetRow>, ApiError> {
    let patch = FinanceBudgetPatch {
        id,
        name: req.name,
        amount: req.amount,
        category: req.category,
        is_active: req.is_active,
    };
    let updated = state
        .repos
        .finance_budgets
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/budgets/:id
pub async fn delete_budget(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_budgets
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("budget '{id}' not found")))
    }
}

/// GET /api/finance/budgets/usage
pub async fn get_budget_usage(
    State(state): State<AppState>,
) -> Result<Json<Vec<BudgetUsageRow>>, ApiError> {
    let rows = state
        .repos
        .finance_budgets
        .all_budget_usage()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

// ============================================================
// Investments
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInvestmentRequest {
    pub portfolio_id: String,
    pub asset_type: String,
    pub symbol: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub cost_basis: i64,
    pub currency: String,
    pub current_price: Option<i64>,
    pub current_value: Option<i64>,
    pub purchase_date: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchInvestmentRequest {
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<f64>,
    pub cost_basis: Option<i64>,
    pub notes: Option<Option<String>>,
}

/// GET /api/finance/investments
pub async fn list_investments(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceInvestmentRow>>, ApiError> {
    let rows = state
        .repos
        .finance_investments
        .list_investments(&FinanceInvestmentFilter::default())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/investments
pub async fn create_investment(
    State(state): State<AppState>,
    Json(req): Json<CreateInvestmentRequest>,
) -> Result<Json<FinanceInvestmentRow>, ApiError> {
    let now = Utc::now();
    let row = FinanceInvestmentRow {
        id: uuid::Uuid::new_v4().to_string(),
        portfolio_id: req.portfolio_id,
        asset_type: req.asset_type,
        symbol: req.symbol,
        name: req.name,
        quantity: req.quantity,
        cost_basis: req.cost_basis,
        currency: req.currency,
        current_price: req.current_price,
        current_value: req.current_value,
        purchase_date: req.purchase_date,
        notes: req.notes,
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_investments
        .add_investment(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// PATCH /api/finance/investments/:id
pub async fn patch_investment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchInvestmentRequest>,
) -> Result<Json<FinanceInvestmentRow>, ApiError> {
    let patch = FinanceInvestmentPatch {
        id,
        current_price: req.current_price,
        current_value: req.current_value,
        quantity: req.quantity,
        cost_basis: req.cost_basis,
        notes: req.notes,
    };
    let updated = state
        .repos
        .finance_investments
        .update_investment(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/investments/:id
pub async fn delete_investment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_investments
        .delete_investment(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("investment '{id}' not found")))
    }
}

// ============================================================
// Goals
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoalRequest {
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub current_amount: Option<i64>,
    pub currency: String,
    pub deadline: Option<NaiveDate>,
    pub monthly_contribution: Option<i64>,
    pub expected_return_rate: Option<f64>,
    pub inflation_rate: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchGoalRequest {
    pub name: Option<String>,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub expected_return_rate: Option<Option<f64>>,
    pub inflation_rate: Option<Option<f64>>,
    pub deadline: Option<Option<NaiveDate>>,
    pub status: Option<String>,
}

/// GET /api/finance/goals
pub async fn list_goals(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceGoalRow>>, ApiError> {
    let rows = state
        .repos
        .finance_goals
        .list_active()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/goals
pub async fn create_goal(
    State(state): State<AppState>,
    Json(req): Json<CreateGoalRequest>,
) -> Result<Json<FinanceGoalRow>, ApiError> {
    let now = Utc::now();
    let row = FinanceGoalRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        goal_type: req.goal_type,
        target_amount: req.target_amount,
        current_amount: req.current_amount.unwrap_or(0),
        currency: req.currency,
        status: "active".to_string(),
        deadline: req.deadline,
        monthly_contribution: req.monthly_contribution,
        expected_return_rate: req.expected_return_rate,
        inflation_rate: req.inflation_rate,
        notes: req.notes,
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_goals
        .add(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// PATCH /api/finance/goals/:id
pub async fn patch_goal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchGoalRequest>,
) -> Result<Json<FinanceGoalRow>, ApiError> {
    let patch = FinanceGoalPatch {
        id,
        name: req.name,
        current_amount: req.current_amount,
        target_amount: req.target_amount,
        monthly_contribution: req.monthly_contribution,
        expected_return_rate: req.expected_return_rate,
        inflation_rate: req.inflation_rate,
        deadline: req.deadline,
        status: req.status,
    };
    let updated = state
        .repos
        .finance_goals
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/goals/:id
pub async fn delete_goal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_goals
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("goal '{id}' not found")))
    }
}

// ============================================================
// Liabilities
// ============================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLiabilityRequest {
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub remaining: Option<i64>,
    pub currency: String,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchLiabilityRequest {
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
}

/// GET /api/finance/liabilities
pub async fn list_liabilities(
    State(state): State<AppState>,
) -> Result<Json<Vec<FinanceLiabilityRow>>, ApiError> {
    let rows = state
        .repos
        .finance_liabilities
        .list_all()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/finance/liabilities
pub async fn create_liability(
    State(state): State<AppState>,
    Json(req): Json<CreateLiabilityRequest>,
) -> Result<Json<FinanceLiabilityRow>, ApiError> {
    let now = Utc::now();
    let principal = req.principal;
    let row = FinanceLiabilityRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        liability_type: req.liability_type,
        principal,
        remaining: req.remaining.unwrap_or(principal),
        currency: req.currency,
        interest_rate: req.interest_rate,
        monthly_payment: req.monthly_payment,
        due_date: req.due_date,
        notes: req.notes,
        created_at: now,
        updated_at: now,
    };
    let created = state
        .repos
        .finance_liabilities
        .add(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// PATCH /api/finance/liabilities/:id
pub async fn patch_liability(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchLiabilityRequest>,
) -> Result<Json<FinanceLiabilityRow>, ApiError> {
    let patch = FinanceLiabilityPatch {
        id,
        remaining: req.remaining,
        monthly_payment: req.monthly_payment,
        interest_rate: req.interest_rate,
        notes: req.notes,
    };
    let updated = state
        .repos
        .finance_liabilities
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// DELETE /api/finance/liabilities/:id
pub async fn delete_liability(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .finance_liabilities
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("liability '{id}' not found")))
    }
}
