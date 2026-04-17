//! Category correlation analysis for spending data.

use std::collections::{BTreeMap, BTreeSet};

use common::Decimal;

use crate::input_types::{SpendingRecord, SpendingType};
use crate::monte_carlo::sampling::decimal_sqrt;
use crate::types::CorrelationMatrix;

use super::anomaly::SpendingAnalyzer;

/// Configuration for category correlation analysis.
#[derive(Debug, Clone)]
pub struct CorrelationConfig {
    /// Minimum number of shared months for a category pair to be included.
    pub min_months: usize,
}

impl Default for CorrelationConfig {
    fn default() -> Self {
        Self { min_months: 6 }
    }
}

impl SpendingAnalyzer {
    /// Compute a Pearson correlation matrix across spending categories.
    ///
    /// Groups expenses by (year-month, category), then for all category pairs
    /// with at least `min_months` shared months, computes the Pearson correlation
    /// coefficient.
    pub fn category_correlation(
        txs: &[SpendingRecord],
        config: &CorrelationConfig,
    ) -> CorrelationMatrix {
        // 1. Filter expenses, group by (year-month, category)
        let mut category_monthly: BTreeMap<String, BTreeMap<String, Decimal>> = BTreeMap::new();
        let mut all_months: BTreeSet<String> = BTreeSet::new();

        for tx in txs {
            if tx.tx_type != SpendingType::Expense {
                continue;
            }
            if let Some(ref cat) = tx.category {
                let month_key = format!("{}-{:02}", tx.date.year(), tx.date.month());
                all_months.insert(month_key.clone());
                *category_monthly
                    .entry(cat.clone())
                    .or_default()
                    .entry(month_key)
                    .or_insert(Decimal::ZERO) += tx.amount;
            }
        }

        let categories: Vec<String> = category_monthly.keys().cloned().collect();

        // Edge case: single category or no categories
        if categories.len() < 2 {
            return CorrelationMatrix {
                labels: categories,
                coefficients: if category_monthly.len() == 1 {
                    vec![vec![Decimal::ONE]]
                } else {
                    Vec::new()
                },
            };
        }

        let n = categories.len();
        let mut coefficients = vec![vec![Decimal::ZERO; n]; n];

        for i in 0..n {
            coefficients[i][i] = Decimal::ONE;
            for j in (i + 1)..n {
                let cat_a = &categories[i];
                let cat_b = &categories[j];
                let months_a = &category_monthly[cat_a];
                let months_b = &category_monthly[cat_b];

                // Find shared months
                let shared_months: Vec<&String> = all_months
                    .iter()
                    .filter(|m| months_a.contains_key(*m) && months_b.contains_key(*m))
                    .collect();

                if shared_months.len() < config.min_months {
                    // Not enough data; leave as 0
                    continue;
                }

                let vals_a: Vec<Decimal> = shared_months.iter().map(|m| months_a[*m]).collect();
                let vals_b: Vec<Decimal> = shared_months.iter().map(|m| months_b[*m]).collect();

                let r = pearson_correlation(&vals_a, &vals_b);
                coefficients[i][j] = r;
                coefficients[j][i] = r;
            }
        }

        CorrelationMatrix {
            labels: categories,
            coefficients,
        }
    }
}

/// Compute the Pearson correlation coefficient for two equal-length series.
fn pearson_correlation(xs: &[Decimal], ys: &[Decimal]) -> Decimal {
    let n = xs.len();
    if n == 0 {
        return Decimal::ZERO;
    }

    let n_dec = Decimal::new(n as i64, 0);
    let sum_x: Decimal = xs.iter().copied().sum();
    let sum_y: Decimal = ys.iter().copied().sum();
    let mean_x = sum_x / n_dec;
    let mean_y = sum_y / n_dec;

    let mut sum_xy = Decimal::ZERO;
    let mut sum_xx = Decimal::ZERO;
    let mut sum_yy = Decimal::ZERO;

    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        sum_xy += dx * dy;
        sum_xx += dx * dx;
        sum_yy += dy * dy;
    }

    if sum_xx == Decimal::ZERO || sum_yy == Decimal::ZERO {
        return Decimal::ZERO;
    }

    // Pearson r = sum_xy / sqrt(sum_xx * sum_yy)
    let denominator_sq = sum_xx * sum_yy;
    let denominator = match decimal_sqrt(denominator_sq) {
        Some(v) if v > Decimal::ZERO => v,
        _ => return Decimal::ZERO,
    };

    let r = sum_xy / denominator;

    // Clamp to [-1, 1]
    if r > Decimal::ONE {
        Decimal::ONE
    } else if r < Decimal::new(-1, 0) {
        Decimal::new(-1, 0)
    } else {
        r
    }
}
