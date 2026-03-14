use serde::{Deserialize, Serialize};

// ── Finance ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub total_value: i64,
    pub total_cost_basis: i64,
    pub holding_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceNetWorthResponse {
    pub totals_by_currency: Vec<CurrencyNetWorth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyNetWorth {
    pub currency: String,
    pub accounts: i64,
    pub investments: i64,
    pub liabilities: i64,
    pub net: i64,
}

impl CurrencyNetWorth {
    pub fn zero(currency: String) -> Self {
        Self {
            currency,
            accounts: 0,
            investments: 0,
            liabilities: 0,
            net: 0,
        }
    }
}

// ── Finance Mutation Params ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountCreateParams {
    pub name: String,
    pub account_type: String,
    pub currency: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAccountUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub balance: Option<i64>,
    pub institution: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionCreateParams {
    pub account_id: String,
    pub tx_type: String,
    pub amount: i64,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub counterparty: Option<String>,
    pub tx_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionUpdateParams {
    pub id: String,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub subcategory: Option<Option<String>>,
    pub counterparty: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub tx_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetCreateParams {
    pub name: String,
    pub amount: i64,
    pub period: String,
    pub currency: Option<String>,
    pub category: Option<String>,
    pub method: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub alert_threshold: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub amount: Option<i64>,
    pub category: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalCreateParams {
    pub name: String,
    pub goal_type: String,
    pub target_amount: i64,
    pub currency: Option<String>,
    pub current_amount: Option<i64>,
    pub deadline: Option<String>,
    pub monthly_contribution: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceGoalUpdateParams {
    pub id: String,
    pub current_amount: Option<i64>,
    pub target_amount: Option<i64>,
    pub monthly_contribution: Option<Option<i64>>,
    pub deadline: Option<Option<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityCreateParams {
    pub name: String,
    pub liability_type: String,
    pub principal: i64,
    pub currency: Option<String>,
    pub remaining: Option<i64>,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLiabilityUpdateParams {
    pub id: String,
    pub remaining: Option<i64>,
    pub monthly_payment: Option<Option<i64>>,
    pub interest_rate: Option<Option<f64>>,
    pub notes: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePortfolioCreateParams {
    pub name: String,
    pub description: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentCreateParams {
    pub portfolio_id: String,
    pub asset_type: String,
    pub cost_basis: i64,
    pub quantity: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub currency: Option<String>,
    pub purchase_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInvestmentUpdateParams {
    pub id: String,
    pub current_price: Option<Option<i64>>,
    pub current_value: Option<Option<i64>>,
    pub quantity: Option<String>,
    pub notes: Option<Option<String>>,
}

// ── Finance Filter Params ────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTransactionFilterParams {
    pub account_id: Option<String>,
    pub tx_type: Option<String>,
    pub category: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
}

// ── Finance Report Responses ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryReportResponse {
    pub total: i64,
    pub breakdown: Vec<FinanceCategoryBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryBreakdown {
    pub category: String,
    pub amount: i64,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceTrendPoint {
    pub period: String,
    pub value: i64,
    pub change_pct: Option<f64>,
}
