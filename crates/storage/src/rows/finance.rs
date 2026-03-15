//! Row structs for all finance tables:
//! `finance_accounts`, `finance_transactions`, `finance_budgets`,
//! `finance_portfolios`, `finance_investments`, `finance_investment_transactions`,
//! `finance_goals`, `finance_liabilities`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::FromRow;

// ============================================================
// Row structs
// ============================================================

/// Row struct for the `finance_accounts` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountRow {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub currency: String,
    pub balance: i64,
    pub institution: Option<String>,
    pub notes: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_balance: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_transactions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionRow {
    pub id: String,
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: String,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub notes: Option<String>,
    pub tx_date: NaiveDate,
    pub transfer_id: Option<String>,
    pub is_recurring: bool,
    pub recurring_rule: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_budgets` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetRow {
    pub id: String,
    pub name: String,
    pub amount: i64,
    pub currency: String,
    pub period: String,
    pub category: Option<String>,
    pub method: String,
    pub jar_type: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub alert_threshold: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_portfolios` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Row struct for the `finance_investments` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentRow {
    pub id: String,
    pub portfolio_id: String,
    pub asset_type: String,
    pub symbol: Option<String>,
    pub name: String,
    pub quantity: String,
    pub cost_basis: i64,
    pub currency: String,
    pub current_price: Option<i64>,
    pub current_value: Option<i64>,
    pub purchase_date: Option<NaiveDate>,
    pub asset_class: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub market_currency: Option<String>,
    pub base_cost_basis: i64,
    pub base_current_value: i64,
    pub base_currency: String,
    pub purchase_rate: f64,
    pub market_rate: f64,
}

impl FinanceInvestmentRow {
    /// Parse the string `quantity` to `f64`, returning `0.0` on parse failure.
    pub fn quantity_f64(&self) -> f64 {
        self.quantity.parse().unwrap_or(0.0)
    }
}

/// Row struct for the `finance_investment_transactions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentTxRow {
    pub id: String,
    pub investment_id: String,
    pub tx_type: String,
    pub quantity: Option<f64>,
    pub price_per_unit: Option<i64>,
    pub total_amount: i64,
    pub currency: String,
    pub fees: i64,
    pub tx_date: NaiveDate,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub base_total_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_goals` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalRow {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub current_amount: i64,
    pub currency: String,
    pub status: String,
    pub deadline: Option<NaiveDate>,
    pub monthly_contribution: Option<i64>,
    pub expected_return_rate: Option<f64>,
    pub inflation_rate: Option<f64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_target_amount: i64,
    pub base_current_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_liabilities` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityRow {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub remaining: i64,
    pub currency: String,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_principal: i64,
    pub base_remaining: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
}

/// Row struct for the `finance_exchange_rates` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceExchangeRateRow {
    pub from_currency: String,
    pub to_currency: String,
    pub rate: f64,
    pub fetched_at: String,
}

/// Row struct for the `finance_allocation_targets` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAllocationTargetRow {
    pub id: String,
    pub portfolio_id: String,
    pub asset_class: String,
    pub target_weight: String,
    pub tolerance_band: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Row struct for the `finance_net_worth_snapshots` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceNetWorthSnapshotRow {
    pub id: String,
    pub snapshot_date: String,
    pub currency: String,
    pub accounts_total: i64,
    pub investments_total: i64,
    pub liabilities_total: i64,
    pub net_worth: i64,
    pub breakdown: String,
    pub created_at: DateTime<Utc>,
}

// ============================================================
// Patch structs (partial update)
// ============================================================

/// Partial update for `finance_accounts`.
#[derive(Debug, Default, Clone)]
pub struct FinanceAccountPatch {
    pub id: String,
    /// Set a new name.
    pub name: Option<String>,
    /// Directly set the balance (not a delta).
    pub balance: Option<i64>,
    /// `None` = don't change, `Some(None)` = set NULL, `Some(Some(v))` = set value.
    pub institution: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = set NULL, `Some(Some(v))` = set value.
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
    pub base_balance: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
}

/// Partial update for `finance_transactions`.
#[derive(Debug, Default, Clone)]
pub struct FinanceTransactionPatch {
    pub id: String,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<NaiveDate>,
    pub base_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
}

/// Partial update for `finance_budgets`.
#[derive(Debug, Default, Clone)]
pub struct FinanceBudgetPatch {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
    pub base_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
}

/// Partial update for `finance_investments`.
#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentPatch {
    pub id: String,
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<String>,
    /// Set a new total cost basis. `None` leaves it unchanged.
    pub cost_basis: Option<i64>,
    pub notes: Option<Option<String>>,
    pub market_currency: Option<Option<String>>,
    pub base_cost_basis: Option<i64>,
    pub base_current_value: Option<i64>,
    pub base_currency: Option<String>,
    pub purchase_rate: Option<f64>,
    pub market_rate: Option<f64>,
}

/// Partial update for `finance_goals`.
#[derive(Debug, Default, Clone)]
pub struct FinanceGoalPatch {
    pub id: String,
    pub name: Option<String>,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub expected_return_rate: Option<Option<f64>>,
    pub inflation_rate: Option<Option<f64>>,
    pub deadline: Option<Option<NaiveDate>>,
    pub status: Option<String>,
    pub base_target_amount: Option<i64>,
    pub base_current_amount: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
}

/// Partial update for `finance_liabilities`.
#[derive(Debug, Default, Clone)]
pub struct FinanceLiabilityPatch {
    pub id: String,
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
    pub base_principal: Option<i64>,
    pub base_remaining: Option<i64>,
    pub base_currency: Option<String>,
    pub exchange_rate: Option<f64>,
}

// ============================================================
// Filter structs
// ============================================================

/// Filter for listing/searching `finance_transactions`.
#[derive(Debug, Default, Clone)]
pub struct FinanceTransactionFilter {
    pub account_id: Option<String>,
    pub tx_type: Option<String>,
    pub category: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub amount_min: Option<i64>,
    pub amount_max: Option<i64>,
    /// LIKE search across notes, counterparty, and category.
    pub query: Option<String>,
    /// Defaults to 20, max 100 (enforced in repo).
    pub limit: Option<i64>,
}

/// Filter for listing `finance_investments`.
#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentFilter {
    pub portfolio_id: Option<String>,
    pub asset_type: Option<String>,
    /// When `Some(true)`, only returns investments where `symbol IS NOT NULL`.
    pub has_symbol: Option<bool>,
}

// ============================================================
// Aggregation row types
// ============================================================

/// Result of the `budget_usage` join query: all budget fields plus the
/// `spent` amount (sum of matching expense transactions in the current period).
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetUsageRow {
    pub id: String,
    pub name: String,
    pub amount: i64,
    pub currency: String,
    pub period: String,
    pub category: Option<String>,
    pub method: String,
    pub jar_type: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub alert_threshold: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub base_amount: i64,
    pub base_currency: String,
    pub exchange_rate: f64,
    /// Sum of expense amounts matching this budget's category in the current period.
    pub spent: i64,
}

/// Aggregated portfolio summary (holdings count + cost/value totals).
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummaryRow {
    pub portfolio_id: String,
    pub total_cost_basis: i64,
    pub total_current_value: i64,
    pub holding_count: i64,
}
