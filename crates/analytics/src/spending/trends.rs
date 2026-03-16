//! Trend analysis for spending data — monthly totals, moving averages, period-over-period changes.

use std::collections::BTreeMap;

use chrono::Datelike;
use common::Decimal;
use serde::Serialize;

use crate::input_types::{SpendingRecord, SpendingType};
use crate::types::TrendDirection;

use super::anomaly::SpendingAnalyzer;

/// Configuration for trend analysis.
#[derive(Debug, Clone)]
pub struct TrendConfig {
    /// Moving average window in months.
    pub window_months: u32,
    /// Minimum months of data needed for analysis.
    pub min_months: u32,
}

impl Default for TrendConfig {
    fn default() -> Self {
        Self {
            window_months: 3,
            min_months: 3,
        }
    }
}

/// Summary report of spending trends.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendReport {
    /// Overall direction of spending.
    pub overall_direction: TrendDirection,
    /// Monthly totals as `("YYYY-MM", amount)` pairs.
    pub monthly_totals: Vec<(String, Decimal)>,
    /// Moving average series.
    pub moving_average: Vec<(String, Decimal)>,
    /// Month-over-month percentage change.
    pub period_over_period: Vec<(String, Decimal)>,
    /// Per-category trend breakdown.
    pub category_trends: Vec<CategoryTrend>,
}

/// Trend data for a single spending category.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTrend {
    pub category: String,
    pub direction: TrendDirection,
    pub average_monthly: Decimal,
    pub latest_monthly: Decimal,
    pub change_pct: Decimal,
}

impl SpendingAnalyzer {
    /// Analyse spending trends over time.
    ///
    /// Computes monthly totals, a moving average, period-over-period changes,
    /// and per-category trends.
    pub fn trends(txs: &[SpendingRecord], config: &TrendConfig) -> TrendReport {
        // 1. Filter expenses, group by month
        let expenses: Vec<&SpendingRecord> = txs
            .iter()
            .filter(|t| t.tx_type == SpendingType::Expense)
            .collect();

        let mut monthly: BTreeMap<String, Decimal> = BTreeMap::new();
        let mut category_monthly: BTreeMap<String, BTreeMap<String, Decimal>> = BTreeMap::new();

        for tx in &expenses {
            let month_key = format!("{}-{:02}", tx.date.year(), tx.date.month());
            *monthly.entry(month_key.clone()).or_insert(Decimal::ZERO) += tx.amount;

            if let Some(ref cat) = tx.category {
                *category_monthly
                    .entry(cat.clone())
                    .or_default()
                    .entry(month_key)
                    .or_insert(Decimal::ZERO) += tx.amount;
            }
        }

        let monthly_totals: Vec<(String, Decimal)> =
            monthly.iter().map(|(k, v)| (k.clone(), *v)).collect();

        // Insufficient data
        if (monthly_totals.len() as u32) < config.min_months {
            return TrendReport {
                overall_direction: TrendDirection::Stable,
                monthly_totals,
                moving_average: Vec::new(),
                period_over_period: Vec::new(),
                category_trends: Vec::new(),
            };
        }

        // 3. Moving average
        let window = config.window_months as usize;
        let moving_average: Vec<(String, Decimal)> = if monthly_totals.len() >= window {
            (0..=monthly_totals.len() - window)
                .map(|i| {
                    let sum: Decimal = monthly_totals[i..i + window].iter().map(|(_, v)| *v).sum();
                    let avg = sum / Decimal::new(window as i64, 0);
                    (monthly_totals[i + window - 1].0.clone(), avg)
                })
                .collect()
        } else {
            Vec::new()
        };

        // 4. Period-over-period % change
        let period_over_period: Vec<(String, Decimal)> = monthly_totals
            .windows(2)
            .filter_map(|pair| {
                let prev = pair[0].1;
                let curr = pair[1].1;
                if prev == Decimal::ZERO {
                    None
                } else {
                    let pct = (curr - prev) / prev * Decimal::new(100, 0);
                    Some((pair[1].0.clone(), pct))
                }
            })
            .collect();

        // 5. Classify overall direction
        let overall_direction = if moving_average.len() >= 2 {
            let first_ma = moving_average
                .first()
                .map(|(_, v)| *v)
                .unwrap_or(Decimal::ZERO);
            let last_ma = moving_average
                .last()
                .map(|(_, v)| *v)
                .unwrap_or(Decimal::ZERO);
            if first_ma == Decimal::ZERO {
                TrendDirection::Stable
            } else {
                let change = (last_ma - first_ma) / first_ma * Decimal::new(100, 0);
                if change > Decimal::new(5, 0) {
                    TrendDirection::Increasing
                } else if change < Decimal::new(-5, 0) {
                    TrendDirection::Decreasing
                } else {
                    TrendDirection::Stable
                }
            }
        } else {
            TrendDirection::Stable
        };

        // 6. Per-category trends
        let all_months: Vec<&String> = monthly.keys().collect();
        let category_trends: Vec<CategoryTrend> = category_monthly
            .iter()
            .map(|(cat, months_map)| {
                let totals: Vec<Decimal> = all_months
                    .iter()
                    .filter_map(|m| months_map.get(*m).copied())
                    .collect();
                let count = totals.len();
                let sum: Decimal = totals.iter().copied().sum();
                let average_monthly = if count > 0 {
                    sum / Decimal::new(count as i64, 0)
                } else {
                    Decimal::ZERO
                };
                let latest_monthly = totals.last().copied().unwrap_or(Decimal::ZERO);
                let change_pct = if average_monthly == Decimal::ZERO {
                    Decimal::ZERO
                } else {
                    (latest_monthly - average_monthly) / average_monthly * Decimal::new(100, 0)
                };
                let direction = if change_pct > Decimal::new(5, 0) {
                    TrendDirection::Increasing
                } else if change_pct < Decimal::new(-5, 0) {
                    TrendDirection::Decreasing
                } else {
                    TrendDirection::Stable
                };
                CategoryTrend {
                    category: cat.clone(),
                    direction,
                    average_monthly,
                    latest_monthly,
                    change_pct,
                }
            })
            .collect();

        TrendReport {
            overall_direction,
            monthly_totals,
            moving_average,
            period_over_period,
            category_trends,
        }
    }
}
