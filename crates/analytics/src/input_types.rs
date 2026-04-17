//! Input types for the analytics crate.
//! These are lightweight structs that feature-finance converts storage Row types into.

use common::Decimal;
use jiff::civil::Date;

/// A financial transaction for spending analysis.
#[derive(Debug, Clone)]
pub struct SpendingRecord {
    pub date: Date,
    pub amount: Decimal,
    pub tx_type: SpendingType,
    pub category: Option<String>,
    pub counterparty: Option<String>,
}

/// Income or Expense classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendingType {
    Income,
    Expense,
}

/// A portfolio holding for drift/returns analysis.
#[derive(Debug, Clone)]
pub struct Holding {
    pub name: String,
    pub symbol: Option<String>,
    pub asset_class: String,
    pub current_value: Decimal,
    pub cost_basis: Decimal,
    pub quantity: Decimal,
}

/// An investment cash flow for returns calculation (TWR/MWR).
#[derive(Debug, Clone)]
pub struct InvestmentCashFlow {
    pub date: Date,
    pub amount: Decimal,
    pub holding_symbol: Option<String>,
}

/// A price time series for correlation analysis.
#[derive(Debug, Clone)]
pub struct PriceSeries {
    pub symbol: String,
    pub asset_class: String,
    pub prices: Vec<(Date, Decimal)>,
}

/// Allocation target for a portfolio.
#[derive(Debug, Clone)]
pub struct AllocationTarget {
    pub asset_class: String,
    pub target_weight: Decimal,
    pub tolerance_band: Decimal,
}

/// Frequency of a recurring charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurringFrequency {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Annual,
}
