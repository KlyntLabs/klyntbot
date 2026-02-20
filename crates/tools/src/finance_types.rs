//! Finance domain types
//!
//! Domain enums (with `from_str_loose` / `as_str` / `Display`), domain structs
//! (with enum fields), bidirectional Row↔Domain `From` impls, and domain-level
//! filter types with `to_storage_filter()`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use storage::rows::finance::*;

// ============================================================
// Domain Enums
// ============================================================

/// Type of financial account.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Cash,
    Bank,
    Ewallet,
    CryptoWallet,
    Brokerage,
    #[default]
    Other,
}

impl AccountType {
    /// Parse case-insensitively; returns `None` for unknown strings.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cash" => Some(Self::Cash),
            "bank" => Some(Self::Bank),
            "ewallet" | "e_wallet" => Some(Self::Ewallet),
            "crypto_wallet" | "cryptowallet" => Some(Self::CryptoWallet),
            "brokerage" => Some(Self::Brokerage),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Canonical snake_case string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Bank => "bank",
            Self::Ewallet => "ewallet",
            Self::CryptoWallet => "crypto_wallet",
            Self::Brokerage => "brokerage",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Type of financial transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Income,
    #[default]
    Expense,
    Transfer,
}

impl TransactionType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "income" => Some(Self::Income),
            "expense" => Some(Self::Expense),
            "transfer" => Some(Self::Transfer),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Expense => "expense",
            Self::Transfer => "transfer",
        }
    }
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Budget period.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    #[default]
    Monthly,
    Weekly,
    Yearly,
    Custom,
}

impl BudgetPeriod {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "monthly" | "month" => Some(Self::Monthly),
            "weekly" | "week" => Some(Self::Weekly),
            "yearly" | "year" | "annual" => Some(Self::Yearly),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Weekly => "weekly",
            Self::Yearly => "yearly",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Budget allocation method.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMethod {
    #[default]
    Standard,
    SixJar,
}

impl BudgetMethod {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "six_jar" | "sixjar" | "6jar" => Some(Self::SixJar),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::SixJar => "six_jar",
        }
    }
}

impl std::fmt::Display for BudgetMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Six-Jar budget category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JarType {
    Essentials,
    Savings,
    Investment,
    Education,
    Entertainment,
    Charity,
}

impl JarType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "essentials" | "necessities" => Some(Self::Essentials),
            "savings" | "saving" => Some(Self::Savings),
            "investment" | "investments" => Some(Self::Investment),
            "education" => Some(Self::Education),
            "entertainment" | "play" => Some(Self::Entertainment),
            "charity" | "give" => Some(Self::Charity),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Essentials => "essentials",
            Self::Savings => "savings",
            Self::Investment => "investment",
            Self::Education => "education",
            Self::Entertainment => "entertainment",
            Self::Charity => "charity",
        }
    }
}

impl std::fmt::Display for JarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Asset type — covers financial asset categories and price-routing variants.
///
/// Used for both investment holdings (Stock, Etf, Crypto, RealEstate, Bond, Other)
/// and by `PriceService` for HTTP routing (ExchangeRate).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Stock,
    Etf,
    Crypto,
    RealEstate,
    Bond,
    #[default]
    Other,
    /// Exchange rate — used by `PriceService` to route to forex API.
    ExchangeRate,
}

impl AssetType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stock" | "stocks" | "equity" => Some(Self::Stock),
            "etf" => Some(Self::Etf),
            "crypto" | "cryptocurrency" => Some(Self::Crypto),
            "real_estate" | "realestate" | "property" => Some(Self::RealEstate),
            "bond" | "bonds" | "fixed_income" => Some(Self::Bond),
            "other" => Some(Self::Other),
            "exchange_rate" | "exchangerate" | "forex" | "fx" => Some(Self::ExchangeRate),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stock => "stock",
            Self::Etf => "etf",
            Self::Crypto => "crypto",
            Self::RealEstate => "real_estate",
            Self::Bond => "bond",
            Self::Other => "other",
            Self::ExchangeRate => "exchange_rate",
        }
    }
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Investment transaction type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InvestmentTxType {
    #[default]
    Buy,
    Sell,
    Dividend,
    RentalIncome,
    Interest,
    Split,
}

impl InvestmentTxType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "buy" | "purchase" => Some(Self::Buy),
            "sell" | "sale" => Some(Self::Sell),
            "dividend" => Some(Self::Dividend),
            "rental_income" | "rental" | "rent" => Some(Self::RentalIncome),
            "interest" => Some(Self::Interest),
            "split" => Some(Self::Split),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Dividend => "dividend",
            Self::RentalIncome => "rental_income",
            Self::Interest => "interest",
            Self::Split => "split",
        }
    }
}

impl std::fmt::Display for InvestmentTxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Financial goal type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    #[default]
    Savings,
    Purchase,
    DebtPayoff,
    Fire,
    Custom,
}

impl GoalType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "savings" | "saving" | "emergency_fund" => Some(Self::Savings),
            "purchase" | "buy" => Some(Self::Purchase),
            "debt_payoff" | "debt" | "payoff" => Some(Self::DebtPayoff),
            "fire" | "financial_independence" => Some(Self::Fire),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Savings => "savings",
            Self::Purchase => "purchase",
            Self::DebtPayoff => "debt_payoff",
            Self::Fire => "fire",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for GoalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Status of a financial goal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    #[default]
    Active,
    Achieved,
    Abandoned,
}

impl GoalStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" | "in_progress" => Some(Self::Active),
            "achieved" | "completed" | "done" => Some(Self::Achieved),
            "abandoned" | "cancelled" => Some(Self::Abandoned),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Achieved => "achieved",
            Self::Abandoned => "abandoned",
        }
    }
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------

/// Type of financial liability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LiabilityType {
    Mortgage,
    CreditCard,
    PersonalLoan,
    StudentLoan,
    #[default]
    Other,
}

impl LiabilityType {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mortgage" | "home_loan" => Some(Self::Mortgage),
            "credit_card" | "creditcard" | "cc" => Some(Self::CreditCard),
            "personal_loan" | "personal" => Some(Self::PersonalLoan),
            "student_loan" | "student" | "education_loan" => Some(Self::StudentLoan),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mortgage => "mortgage",
            Self::CreditCard => "credit_card",
            Self::PersonalLoan => "personal_loan",
            Self::StudentLoan => "student_loan",
            Self::Other => "other",
        }
    }
}

impl std::fmt::Display for LiabilityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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

// ============================================================
// From impls — Row ↔ Domain (16 total)
// ============================================================

// ── FinanceAccount ──────────────────────────────────────────

impl From<FinanceAccountRow> for FinanceAccount {
    fn from(row: FinanceAccountRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            account_type: AccountType::from_str_loose(&row.account_type).unwrap_or_default(),
            currency: row.currency,
            balance: row.balance,
            institution: row.institution,
            notes: row.notes,
            is_archived: row.is_archived,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceAccount> for FinanceAccountRow {
    fn from(account: &FinanceAccount) -> Self {
        Self {
            id: account.id.clone(),
            name: account.name.clone(),
            account_type: account.account_type.as_str().to_string(),
            currency: account.currency.clone(),
            balance: account.balance,
            institution: account.institution.clone(),
            notes: account.notes.clone(),
            is_archived: account.is_archived,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

// ── FinanceTransaction ──────────────────────────────────────

impl From<FinanceTransactionRow> for FinanceTransaction {
    fn from(row: FinanceTransactionRow) -> Self {
        Self {
            id: row.id,
            account_id: row.account_id,
            tx_type: TransactionType::from_str_loose(&row.tx_type).unwrap_or_default(),
            amount: row.amount,
            currency: row.currency,
            category: row.category,
            subcategory: row.subcategory,
            counterparty: row.counterparty,
            notes: row.notes,
            tx_date: row.tx_date,
            transfer_id: row.transfer_id,
            is_recurring: row.is_recurring,
            recurring_rule: row.recurring_rule,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceTransaction> for FinanceTransactionRow {
    fn from(tx: &FinanceTransaction) -> Self {
        Self {
            id: tx.id.clone(),
            account_id: tx.account_id.clone(),
            tx_type: tx.tx_type.as_str().to_string(),
            amount: tx.amount,
            currency: tx.currency.clone(),
            category: tx.category.clone(),
            subcategory: tx.subcategory.clone(),
            counterparty: tx.counterparty.clone(),
            notes: tx.notes.clone(),
            tx_date: tx.tx_date,
            transfer_id: tx.transfer_id.clone(),
            is_recurring: tx.is_recurring,
            recurring_rule: tx.recurring_rule.clone(),
            created_at: tx.created_at,
            updated_at: tx.updated_at,
        }
    }
}

// ── FinanceBudget ────────────────────────────────────────────

impl From<FinanceBudgetRow> for FinanceBudget {
    fn from(row: FinanceBudgetRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            amount: row.amount,
            currency: row.currency,
            period: BudgetPeriod::from_str_loose(&row.period).unwrap_or_default(),
            category: row.category,
            method: BudgetMethod::from_str_loose(&row.method).unwrap_or_default(),
            jar_type: row.jar_type.as_deref().and_then(JarType::from_str_loose),
            start_date: row.start_date,
            end_date: row.end_date,
            is_active: row.is_active,
            alert_threshold: row.alert_threshold,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceBudget> for FinanceBudgetRow {
    fn from(budget: &FinanceBudget) -> Self {
        Self {
            id: budget.id.clone(),
            name: budget.name.clone(),
            amount: budget.amount,
            currency: budget.currency.clone(),
            period: budget.period.as_str().to_string(),
            category: budget.category.clone(),
            method: budget.method.as_str().to_string(),
            jar_type: budget.jar_type.map(|j| j.as_str().to_string()),
            start_date: budget.start_date,
            end_date: budget.end_date,
            is_active: budget.is_active,
            alert_threshold: budget.alert_threshold,
            created_at: budget.created_at,
            updated_at: budget.updated_at,
        }
    }
}

// ── FinancePortfolio ─────────────────────────────────────────

impl From<FinancePortfolioRow> for FinancePortfolio {
    fn from(row: FinancePortfolioRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            currency: row.currency,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinancePortfolio> for FinancePortfolioRow {
    fn from(portfolio: &FinancePortfolio) -> Self {
        Self {
            id: portfolio.id.clone(),
            name: portfolio.name.clone(),
            description: portfolio.description.clone(),
            currency: portfolio.currency.clone(),
            created_at: portfolio.created_at,
            updated_at: portfolio.updated_at,
        }
    }
}

// ── FinanceInvestment ─────────────────────────────────────────

impl From<FinanceInvestmentRow> for FinanceInvestment {
    fn from(row: FinanceInvestmentRow) -> Self {
        Self {
            id: row.id,
            portfolio_id: row.portfolio_id,
            asset_type: AssetType::from_str_loose(&row.asset_type).unwrap_or_default(),
            symbol: row.symbol,
            name: row.name,
            quantity: row.quantity,
            cost_basis: row.cost_basis,
            currency: row.currency,
            current_price: row.current_price,
            current_value: row.current_value,
            purchase_date: row.purchase_date,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceInvestment> for FinanceInvestmentRow {
    fn from(inv: &FinanceInvestment) -> Self {
        Self {
            id: inv.id.clone(),
            portfolio_id: inv.portfolio_id.clone(),
            asset_type: inv.asset_type.as_str().to_string(),
            symbol: inv.symbol.clone(),
            name: inv.name.clone(),
            quantity: inv.quantity,
            cost_basis: inv.cost_basis,
            currency: inv.currency.clone(),
            current_price: inv.current_price,
            current_value: inv.current_value,
            purchase_date: inv.purchase_date,
            notes: inv.notes.clone(),
            created_at: inv.created_at,
            updated_at: inv.updated_at,
        }
    }
}

// ── FinanceInvestmentTx ───────────────────────────────────────

impl From<FinanceInvestmentTxRow> for FinanceInvestmentTx {
    fn from(row: FinanceInvestmentTxRow) -> Self {
        Self {
            id: row.id,
            investment_id: row.investment_id,
            tx_type: InvestmentTxType::from_str_loose(&row.tx_type).unwrap_or_default(),
            quantity: row.quantity,
            price_per_unit: row.price_per_unit,
            total_amount: row.total_amount,
            currency: row.currency,
            fees: row.fees,
            tx_date: row.tx_date,
            notes: row.notes,
            created_at: row.created_at,
        }
    }
}

impl From<&FinanceInvestmentTx> for FinanceInvestmentTxRow {
    fn from(tx: &FinanceInvestmentTx) -> Self {
        Self {
            id: tx.id.clone(),
            investment_id: tx.investment_id.clone(),
            tx_type: tx.tx_type.as_str().to_string(),
            quantity: tx.quantity,
            price_per_unit: tx.price_per_unit,
            total_amount: tx.total_amount,
            currency: tx.currency.clone(),
            fees: tx.fees,
            tx_date: tx.tx_date,
            notes: tx.notes.clone(),
            created_at: tx.created_at,
        }
    }
}

// ── FinanceGoal ───────────────────────────────────────────────

impl From<FinanceGoalRow> for FinanceGoal {
    fn from(row: FinanceGoalRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            goal_type: GoalType::from_str_loose(&row.goal_type).unwrap_or_default(),
            target_amount: row.target_amount,
            current_amount: row.current_amount,
            currency: row.currency,
            status: GoalStatus::from_str_loose(&row.status).unwrap_or_default(),
            deadline: row.deadline,
            monthly_contribution: row.monthly_contribution,
            expected_return_rate: row.expected_return_rate,
            inflation_rate: row.inflation_rate,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceGoal> for FinanceGoalRow {
    fn from(goal: &FinanceGoal) -> Self {
        Self {
            id: goal.id.clone(),
            name: goal.name.clone(),
            goal_type: goal.goal_type.as_str().to_string(),
            target_amount: goal.target_amount,
            current_amount: goal.current_amount,
            currency: goal.currency.clone(),
            status: goal.status.as_str().to_string(),
            deadline: goal.deadline,
            monthly_contribution: goal.monthly_contribution,
            expected_return_rate: goal.expected_return_rate,
            inflation_rate: goal.inflation_rate,
            notes: goal.notes.clone(),
            created_at: goal.created_at,
            updated_at: goal.updated_at,
        }
    }
}

// ── FinanceLiability ──────────────────────────────────────────

impl From<FinanceLiabilityRow> for FinanceLiability {
    fn from(row: FinanceLiabilityRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            liability_type: LiabilityType::from_str_loose(&row.liability_type).unwrap_or_default(),
            principal: row.principal,
            remaining: row.remaining,
            currency: row.currency,
            interest_rate: row.interest_rate,
            monthly_payment: row.monthly_payment,
            due_date: row.due_date,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<&FinanceLiability> for FinanceLiabilityRow {
    fn from(liability: &FinanceLiability) -> Self {
        Self {
            id: liability.id.clone(),
            name: liability.name.clone(),
            liability_type: liability.liability_type.as_str().to_string(),
            principal: liability.principal,
            remaining: liability.remaining,
            currency: liability.currency.clone(),
            interest_rate: liability.interest_rate,
            monthly_payment: liability.monthly_payment,
            due_date: liability.due_date,
            notes: liability.notes.clone(),
            created_at: liability.created_at,
            updated_at: liability.updated_at,
        }
    }
}

// ============================================================
// Domain Filters
// ============================================================

/// Domain-level filter for finance transactions.
///
/// Uses typed enums instead of raw strings.
/// Call `to_storage_filter()` to convert for the repo layer.
#[derive(Debug, Default, Clone)]
pub struct FinanceTransactionFilter {
    pub account_id: Option<String>,
    pub tx_type: Option<TransactionType>,
    pub category: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub amount_min: Option<i64>,
    pub amount_max: Option<i64>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}

impl FinanceTransactionFilter {
    /// Convert to the storage-layer filter (raw strings + i64 limit).
    pub fn to_storage_filter(&self) -> storage::rows::finance::FinanceTransactionFilter {
        storage::rows::finance::FinanceTransactionFilter {
            account_id: self.account_id.clone(),
            tx_type: self.tx_type.map(|t| t.as_str().to_string()),
            category: self.category.clone(),
            date_from: self.date_from,
            date_to: self.date_to,
            amount_min: self.amount_min,
            amount_max: self.amount_max,
            query: self.query.clone(),
            limit: self.limit.map(|l| l as i64),
        }
    }
}

/// Domain-level filter for finance investments.
///
/// Uses typed `AssetType` enum; converts to storage filter via `to_storage_filter()`.
#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentDomainFilter {
    pub portfolio_id: Option<String>,
    pub asset_type: Option<AssetType>,
    pub has_symbol: Option<bool>,
}

impl FinanceInvestmentDomainFilter {
    /// Convert to the storage-layer filter (raw strings).
    pub fn to_storage_filter(&self) -> FinanceInvestmentFilter {
        FinanceInvestmentFilter {
            portfolio_id: self.portfolio_id.clone(),
            asset_type: self.asset_type.map(|t| t.as_str().to_string()),
            has_symbol: self.has_symbol,
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
#[path = "finance_types_tests.rs"]
mod tests;
