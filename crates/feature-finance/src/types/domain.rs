//! Finance domain enums and structs.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tools_core::DomainEnum;

// ============================================================
// Domain Enums
// ============================================================

/// Type of financial account.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Cash,
    Bank,
    #[aliases("e_wallet")]
    Ewallet,
    #[aliases("cryptowallet")]
    CryptoWallet,
    Brokerage,
    #[default]
    Other,
}

// ---------------------------------------------------------------------------

/// Type of financial transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Income,
    #[default]
    Expense,
    Transfer,
}

// ---------------------------------------------------------------------------

/// Budget period.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    #[aliases("month")]
    #[default]
    Monthly,
    #[aliases("week")]
    Weekly,
    #[aliases("year", "annual")]
    Yearly,
    Custom,
}

// ---------------------------------------------------------------------------

/// Budget allocation method.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMethod {
    #[default]
    Standard,
    #[aliases("sixjar", "6jar")]
    SixJar,
}

// ---------------------------------------------------------------------------

/// Six-Jar budget category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum JarType {
    #[aliases("necessities")]
    Essentials,
    #[aliases("saving")]
    Savings,
    #[aliases("investments")]
    Investment,
    Education,
    #[aliases("play")]
    Entertainment,
    #[aliases("give")]
    Charity,
}

// ---------------------------------------------------------------------------

/// Asset type — covers financial asset categories and price-routing variants.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    #[aliases("stocks", "equity")]
    Stock,
    Etf,
    #[aliases("cryptocurrency")]
    Crypto,
    #[aliases("realestate", "property")]
    RealEstate,
    #[aliases("bonds", "fixed_income")]
    Bond,
    #[default]
    Other,
    /// Exchange rate — used by `PriceService` to route to forex API.
    #[aliases("exchangerate", "forex", "fx")]
    ExchangeRate,
}

// ---------------------------------------------------------------------------

/// Investment transaction type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum InvestmentTxType {
    #[aliases("purchase")]
    #[default]
    Buy,
    #[aliases("sale")]
    Sell,
    Dividend,
    #[aliases("rental", "rent")]
    RentalIncome,
    Interest,
    Split,
}

// ---------------------------------------------------------------------------

/// Financial goal type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    #[aliases("saving", "emergency_fund")]
    #[default]
    Savings,
    #[aliases("buy")]
    Purchase,
    #[aliases("debt", "payoff")]
    DebtPayoff,
    #[aliases("financial_independence")]
    Fire,
    Custom,
}

// ---------------------------------------------------------------------------

/// Status of a financial goal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    #[aliases("in_progress")]
    #[default]
    Active,
    #[aliases("completed", "done")]
    Achieved,
    #[aliases("cancelled")]
    Abandoned,
}

// ---------------------------------------------------------------------------

/// Type of financial liability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, DomainEnum)]
#[serde(rename_all = "snake_case")]
pub enum LiabilityType {
    #[aliases("home_loan")]
    Mortgage,
    #[aliases("creditcard", "cc")]
    CreditCard,
    #[aliases("personal")]
    PersonalLoan,
    #[aliases("student", "education_loan")]
    StudentLoan,
    #[default]
    Other,
}

// ============================================================
// Domain Structs
// ============================================================

/// Domain representation of a finance account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceAccount {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub balance: i64,
    pub institution: Option<String>,
    pub notes: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of a finance transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceTransaction {
    pub id: String,
    pub account_id: String,
    pub tx_type: TransactionType,
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
}

/// Domain representation of a budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceBudget {
    pub id: String,
    pub name: String,
    pub amount: i64,
    pub currency: String,
    pub period: BudgetPeriod,
    pub category: Option<String>,
    pub method: BudgetMethod,
    pub jar_type: Option<JarType>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub alert_threshold: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of an investment portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancePortfolio {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of an investment holding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceInvestment {
    pub id: String,
    pub portfolio_id: String,
    pub asset_type: AssetType,
    pub symbol: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub cost_basis: i64,
    pub currency: String,
    pub current_price: Option<i64>,
    pub current_value: Option<i64>,
    pub purchase_date: Option<NaiveDate>,
    pub asset_class: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of an investment transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceInvestmentTx {
    pub id: String,
    pub investment_id: String,
    pub tx_type: InvestmentTxType,
    pub quantity: Option<f64>,
    pub price_per_unit: Option<i64>,
    pub total_amount: i64,
    pub currency: String,
    pub fees: i64,
    pub tx_date: NaiveDate,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Domain representation of a financial goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceGoal {
    pub id: String,
    pub name: String,
    pub goal_type: GoalType,
    pub target_amount: i64,
    pub current_amount: i64,
    pub currency: String,
    pub status: GoalStatus,
    pub deadline: Option<NaiveDate>,
    pub monthly_contribution: Option<i64>,
    pub expected_return_rate: Option<f64>,
    pub inflation_rate: Option<f64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of a financial liability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceLiability {
    pub id: String,
    pub name: String,
    pub liability_type: LiabilityType,
    pub principal: i64,
    pub remaining: i64,
    pub currency: String,
    pub interest_rate: Option<f64>,
    pub monthly_payment: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
