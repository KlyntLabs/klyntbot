//! Finance system configuration and sub-structs.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Finance system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceConfig {
    /// Enable/disable the finance system (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default ISO 4217 currency code (default: "USD").
    #[serde(default = "default_finance_currency")]
    pub default_currency: String,
    /// Proactivity level: "full", "moderate", or "reactive" (default: "full").
    #[serde(default = "default_proactivity_level")]
    pub proactivity_level: String,
    #[serde(default)]
    pub inflation: FinanceInflationConfig,
    #[serde(default)]
    pub expected_returns: FinanceExpectedReturnsConfig,
    #[serde(default)]
    pub budgeting: FinanceBudgetingConfig,
    #[serde(default)]
    pub price_refresh: FinancePriceRefreshConfig,
    #[serde(default)]
    pub scheduling: FinanceSchedulingConfig,
    #[serde(default)]
    pub categories: FinanceCategoryConfig,
    #[serde(default)]
    pub fire: FireConfig,
    /// Manual exchange rates mapping currency codes to their VND equivalent.
    /// E.g. `{"USD": 25500, "USDT": 25500}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_rates: Option<HashMap<String, f64>>,
}

impl Default for FinanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_currency: default_finance_currency(),
            proactivity_level: default_proactivity_level(),
            inflation: Default::default(),
            expected_returns: Default::default(),
            budgeting: Default::default(),
            price_refresh: Default::default(),
            scheduling: Default::default(),
            categories: Default::default(),
            fire: Default::default(),
            exchange_rates: None,
        }
    }
}

fn default_finance_currency() -> String {
    "USD".to_string()
}

fn default_proactivity_level() -> String {
    "full".to_string()
}

/// Inflation assumption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceInflationConfig {
    /// Annual inflation rate as a percentage (default: 3.3).
    #[serde(default = "default_inflation_rate")]
    pub rate: f64,
    /// Source of inflation data: "manual" or "api" (default: "manual").
    #[serde(default = "default_inflation_source")]
    pub source: String,
}

impl Default for FinanceInflationConfig {
    fn default() -> Self {
        Self {
            rate: default_inflation_rate(),
            source: default_inflation_source(),
        }
    }
}

fn default_inflation_rate() -> f64 {
    3.3
}

fn default_inflation_source() -> String {
    "manual".to_string()
}

/// Expected annual return rates by asset class (as percentages).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceExpectedReturnsConfig {
    #[serde(default = "default_stock_return")]
    pub stocks: f64,
    #[serde(default = "default_crypto_return")]
    pub crypto: f64,
    #[serde(default = "default_real_estate_return")]
    pub real_estate: f64,
    #[serde(default = "default_bond_return")]
    pub bonds: f64,
}

impl Default for FinanceExpectedReturnsConfig {
    fn default() -> Self {
        Self {
            stocks: default_stock_return(),
            crypto: default_crypto_return(),
            real_estate: default_real_estate_return(),
            bonds: default_bond_return(),
        }
    }
}

fn default_stock_return() -> f64 {
    10.0
}

fn default_crypto_return() -> f64 {
    15.0
}

fn default_real_estate_return() -> f64 {
    8.0
}

fn default_bond_return() -> f64 {
    5.0
}

/// Budgeting method and alert configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBudgetingConfig {
    /// Default budgeting method: "standard" or "six_jar" (default: "standard").
    #[serde(default = "default_budget_method")]
    pub default_method: String,
    /// Percentage threshold for budget alerts (0-100, default: 80).
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold: u8,
    #[serde(default)]
    pub six_jar_ratios: SixJarRatios,
}

impl Default for FinanceBudgetingConfig {
    fn default() -> Self {
        Self {
            default_method: default_budget_method(),
            alert_threshold: default_alert_threshold(),
            six_jar_ratios: Default::default(),
        }
    }
}

fn default_budget_method() -> String {
    "standard".to_string()
}

fn default_alert_threshold() -> u8 {
    80
}

/// Six Jar allocation ratios (percentages, should sum to 100).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SixJarRatios {
    #[serde(default = "default_jar_essentials")]
    pub essentials: u8,
    #[serde(default = "default_jar_small")]
    pub savings: u8,
    #[serde(default = "default_jar_small")]
    pub investment: u8,
    #[serde(default = "default_jar_small")]
    pub education: u8,
    #[serde(default = "default_jar_small")]
    pub entertainment: u8,
    #[serde(default = "default_jar_charity")]
    pub charity: u8,
}

impl Default for SixJarRatios {
    fn default() -> Self {
        Self {
            essentials: 55,
            savings: 10,
            investment: 10,
            education: 10,
            entertainment: 10,
            charity: 5,
        }
    }
}

fn default_jar_essentials() -> u8 {
    55
}

fn default_jar_small() -> u8 {
    10
}

fn default_jar_charity() -> u8 {
    5
}

/// Price refresh / cache configuration for market data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePriceRefreshConfig {
    /// Enable automatic price refresh (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How often to refresh prices, in hours (default: 4).
    #[serde(default = "default_price_interval_hours")]
    pub interval_hours: u32,
    /// How long to cache prices, in minutes (default: 15).
    #[serde(default = "default_price_cache_ttl")]
    pub cache_ttl_minutes: u32,
}

impl Default for FinancePriceRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: default_price_interval_hours(),
            cache_ttl_minutes: default_price_cache_ttl(),
        }
    }
}

fn default_price_interval_hours() -> u32 {
    4
}

fn default_price_cache_ttl() -> u32 {
    15
}

/// Finance review and reporting schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSchedulingConfig {
    /// Time for daily financial review in HH:MM format (default: "21:00").
    #[serde(default = "default_daily_review_time")]
    pub daily_review_time: String,
    /// Day of week for weekly report (default: "monday").
    #[serde(default = "default_weekly_report_day")]
    pub weekly_report_day: String,
    /// Time for daily budget check in HH:MM format (default: "09:00").
    #[serde(default = "default_budget_check_time")]
    pub budget_check_time: String,
    /// IANA timezone override (falls back to system timezone when None).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl Default for FinanceSchedulingConfig {
    fn default() -> Self {
        Self {
            daily_review_time: default_daily_review_time(),
            weekly_report_day: default_weekly_report_day(),
            budget_check_time: default_budget_check_time(),
            timezone: None,
        }
    }
}

fn default_daily_review_time() -> String {
    "21:00".to_string()
}

fn default_weekly_report_day() -> String {
    "monday".to_string()
}

fn default_budget_check_time() -> String {
    "09:00".to_string()
}

/// Transaction auto-categorization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCategoryConfig {
    /// Enable automatic transaction categorization (default: true).
    #[serde(default = "default_true")]
    pub auto_categorize: bool,
    /// Minimum confidence to auto-apply a category (0.0-1.0, default: 0.8).
    #[serde(default = "default_finance_category_threshold")]
    pub confidence_threshold: f64,
}

impl Default for FinanceCategoryConfig {
    fn default() -> Self {
        Self {
            auto_categorize: true,
            confidence_threshold: default_finance_category_threshold(),
        }
    }
}

fn default_finance_category_threshold() -> f64 {
    0.8
}

/// FIRE (Financial Independence, Retire Early) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FireConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_age: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_retirement_age: Option<u32>,
    /// Annual expenses in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annual_expenses: Option<i64>,
    /// Safe withdrawal rate as percentage (default: 4.0).
    #[serde(default = "default_swr")]
    pub safe_withdrawal_rate: f64,
    /// FIRE type: "lean", "regular", "fat", or "coast".
    #[serde(default = "default_fire_type")]
    pub fire_type: String,
    /// Target FIRE number in cents (auto-calculated or manual override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_number: Option<i64>,
    /// Monthly savings rate in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly_savings_rate: Option<i64>,
    /// Snapshot of current net worth in cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_net_worth: Option<i64>,
}

impl Default for FireConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            current_age: None,
            target_retirement_age: None,
            annual_expenses: None,
            safe_withdrawal_rate: default_swr(),
            fire_type: default_fire_type(),
            target_number: None,
            monthly_savings_rate: None,
            current_net_worth: None,
        }
    }
}

fn default_swr() -> f64 {
    4.0
}

fn default_fire_type() -> String {
    "regular".to_string()
}

/// Default finance transaction/budget categories.
pub fn default_finance_categories() -> Vec<FinanceDefaultCategory> {
    vec![
        // Income
        FinanceDefaultCategory::new("income", "Salary", "income"),
        FinanceDefaultCategory::new("income", "Freelance", "income"),
        FinanceDefaultCategory::new("income", "Investments", "income"),
        FinanceDefaultCategory::new("income", "Rental", "income"),
        FinanceDefaultCategory::new("income", "Side Business", "income"),
        FinanceDefaultCategory::new("income", "Gifts", "income"),
        FinanceDefaultCategory::new("income", "Refunds", "income"),
        // Essential
        FinanceDefaultCategory::new("essential", "Housing", "expense"),
        FinanceDefaultCategory::new("essential", "Utilities", "expense"),
        FinanceDefaultCategory::new("essential", "Groceries", "expense"),
        FinanceDefaultCategory::new("essential", "Transportation", "expense"),
        FinanceDefaultCategory::new("essential", "Insurance", "expense"),
        FinanceDefaultCategory::new("essential", "Healthcare", "expense"),
        FinanceDefaultCategory::new("essential", "Debt Payments", "expense"),
        // Lifestyle
        FinanceDefaultCategory::new("lifestyle", "Dining Out", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Entertainment", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Shopping", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Personal Care", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Subscriptions", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Travel", "expense"),
        FinanceDefaultCategory::new("lifestyle", "Fitness", "expense"),
        // Savings
        FinanceDefaultCategory::new("savings", "Emergency Fund", "transfer"),
        FinanceDefaultCategory::new("savings", "Retirement", "transfer"),
        FinanceDefaultCategory::new("savings", "Investments", "transfer"),
        FinanceDefaultCategory::new("savings", "Education Fund", "transfer"),
        // Giving
        FinanceDefaultCategory::new("giving", "Charity", "expense"),
        FinanceDefaultCategory::new("giving", "Gifts", "expense"),
        FinanceDefaultCategory::new("giving", "Family Support", "expense"),
    ]
}

/// A default category entry used by the wizard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDefaultCategory {
    pub group: String,
    pub name: String,
    pub tx_type: String,
}

impl FinanceDefaultCategory {
    pub fn new(group: &str, name: &str, tx_type: &str) -> Self {
        Self {
            group: group.to_string(),
            name: name.to_string(),
            tx_type: tx_type.to_string(),
        }
    }
}
