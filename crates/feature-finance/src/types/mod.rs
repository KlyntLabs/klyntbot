//! Finance domain types for feature-finance.
//!
//! Domain enums (with `DomainEnum` derive macro), domain structs,
//! bidirectional Row↔Domain `From` impls, and domain-level filter types.

mod conversions;
mod domain;

pub use domain::*;

use chrono::NaiveDate;
use common::time::bridge::chrono_date_to_jiff;
use storage::rows::finance::FinanceInvestmentFilter;

// ============================================================
// Domain Filters
// ============================================================

/// Domain-level filter for finance transactions.
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
    pub fn to_storage_filter(&self) -> storage::rows::finance::FinanceTransactionFilter {
        storage::rows::finance::FinanceTransactionFilter {
            account_id: self.account_id.clone(),
            tx_type: self.tx_type.map(|t| t.as_str().to_string()),
            category: self.category.clone(),
            date_from: self.date_from.map(|d| chrono_date_to_jiff(d).into()),
            date_to: self.date_to.map(|d| chrono_date_to_jiff(d).into()),
            amount_min: self.amount_min,
            amount_max: self.amount_max,
            query: self.query.clone(),
            limit: self.limit.map(|l| l as i64),
        }
    }
}

/// Domain-level filter for finance investments.
#[derive(Debug, Default, Clone)]
pub struct FinanceInvestmentDomainFilter {
    pub portfolio_id: Option<String>,
    pub asset_type: Option<AssetType>,
    pub has_symbol: Option<bool>,
}

impl FinanceInvestmentDomainFilter {
    pub fn to_storage_filter(&self) -> FinanceInvestmentFilter {
        FinanceInvestmentFilter {
            portfolio_id: self.portfolio_id.clone(),
            asset_type: self.asset_type.map(|t| t.as_str().to_string()),
            has_symbol: self.has_symbol,
        }
    }
}
