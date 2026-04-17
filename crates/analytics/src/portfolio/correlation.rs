use std::collections::BTreeMap;

use common::Decimal;
use jiff::civil::Date;
use rust_decimal::prelude::*;

use crate::types::CorrelationMatrix;
use crate::PriceSeries;

/// Configuration for asset correlation.
pub struct AssetCorrelationConfig {
    /// Minimum number of overlapping months required to compute correlation.
    pub min_overlap_months: usize,
}

impl Default for AssetCorrelationConfig {
    fn default() -> Self {
        Self {
            min_overlap_months: 12,
        }
    }
}

impl super::PortfolioAnalyzer {
    /// Compute correlation matrix between assets based on monthly returns.
    pub fn asset_correlation(
        price_series: &[PriceSeries],
        config: &AssetCorrelationConfig,
    ) -> CorrelationMatrix {
        let n = price_series.len();
        if n <= 1 {
            return CorrelationMatrix {
                labels: price_series.iter().map(|s| s.symbol.clone()).collect(),
                coefficients: if n == 1 {
                    vec![vec![Decimal::ONE]]
                } else {
                    vec![]
                },
            };
        }

        // Compute monthly returns for each series.
        let monthly_returns: Vec<BTreeMap<(i32, u32), Decimal>> = price_series
            .iter()
            .map(|ps| Self::compute_monthly_returns(&ps.prices))
            .collect();

        let labels: Vec<String> = price_series.iter().map(|s| s.symbol.clone()).collect();

        // Build the symmetric correlation matrix.
        let mut coefficients = vec![vec![Decimal::ZERO; n]; n];

        for i in 0..n {
            coefficients[i][i] = Decimal::ONE;

            for j in (i + 1)..n {
                let corr =
                    Self::pearson_correlation(&monthly_returns[i], &monthly_returns[j], config);
                coefficients[i][j] = corr;
                coefficients[j][i] = corr;
            }
        }

        CorrelationMatrix {
            labels,
            coefficients,
        }
    }

    /// Compute monthly returns from a price series.
    /// Returns a map of (year, month) -> monthly return (% change).
    fn compute_monthly_returns(prices: &[(Date, Decimal)]) -> BTreeMap<(i32, u32), Decimal> {
        // Group prices by (year, month), taking the last price in each month.
        let mut monthly_prices: BTreeMap<(i32, u32), Decimal> = BTreeMap::new();
        for (date, price) in prices {
            let key = (date.year() as i32, date.month() as u32);
            // Keep the latest price per month (overwrite since we iterate chronologically).
            monthly_prices.insert(key, *price);
        }

        // Compute returns from consecutive months.
        let months: Vec<((i32, u32), Decimal)> = monthly_prices.into_iter().collect();
        let mut returns = BTreeMap::new();

        for window in months.windows(2) {
            let prev_price = window[0].1;
            let curr_price = window[1].1;
            let month_key = window[1].0;

            if prev_price != Decimal::ZERO {
                let ret = (curr_price - prev_price) / prev_price;
                returns.insert(month_key, ret);
            }
        }

        returns
    }

    /// Compute Pearson correlation coefficient between two monthly return series.
    fn pearson_correlation(
        returns_a: &BTreeMap<(i32, u32), Decimal>,
        returns_b: &BTreeMap<(i32, u32), Decimal>,
        config: &AssetCorrelationConfig,
    ) -> Decimal {
        // Find overlapping months.
        let mut paired: Vec<(f64, f64)> = Vec::new();
        for (month_key, ret_a) in returns_a {
            if let Some(ret_b) = returns_b.get(month_key) {
                let a = ret_a.to_f64().unwrap_or(0.0);
                let b = ret_b.to_f64().unwrap_or(0.0);
                paired.push((a, b));
            }
        }

        if paired.len() < config.min_overlap_months {
            return Decimal::ZERO;
        }

        let n = paired.len() as f64;
        let mean_a: f64 = paired.iter().map(|(a, _)| a).sum::<f64>() / n;
        let mean_b: f64 = paired.iter().map(|(_, b)| b).sum::<f64>() / n;

        let mut cov = 0.0_f64;
        let mut var_a = 0.0_f64;
        let mut var_b = 0.0_f64;

        for (a, b) in &paired {
            let da = a - mean_a;
            let db = b - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        let denom = (var_a * var_b).sqrt();
        if denom < 1e-12 {
            return Decimal::ZERO;
        }

        let r = cov / denom;
        Decimal::from_f64_retain(r).unwrap_or(Decimal::ZERO)
    }
}
