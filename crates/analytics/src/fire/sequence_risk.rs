//! Historical backtesting for sequence-of-returns risk.

use std::sync::OnceLock;

use common::Decimal;
use rust_decimal::prelude::*;
use serde::Serialize;

use crate::monte_carlo::WithdrawalStrategy;

static STOCK_RETURNS: OnceLock<Vec<(u32, Decimal)>> = OnceLock::new();
static INFLATION_RATES: OnceLock<Vec<(u32, Decimal)>> = OnceLock::new();

/// Parameters for historical backtesting.
#[derive(Debug, Clone)]
pub struct HistoricalBacktestParams {
    pub portfolio: Decimal,
    pub annual_withdrawal: Decimal,
    pub strategy: WithdrawalStrategy,
    pub years: u32,
}

/// Result of historical backtesting.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalBacktestResult {
    pub success_rate: Decimal,
    pub total_periods: u32,
    pub successful_periods: u32,
    pub failed_periods: u32,
    pub worst_start_year: u32,
    pub worst_end_balance: Decimal,
    pub best_start_year: u32,
    pub best_end_balance: Decimal,
}

/// A single period result.
#[derive(Debug, Clone)]
struct PeriodResult {
    start_year: u32,
    end_balance: Decimal,
    survived: bool,
}

impl super::FIRECalculator {
    /// Run historical backtesting using embedded Shiller data.
    pub fn historical_backtest(params: &HistoricalBacktestParams) -> HistoricalBacktestResult {
        let stock_returns = STOCK_RETURNS
            .get_or_init(|| parse_csv(include_str!("../data/us_stock_returns_1928_2024.csv")));
        let inflation_rates = INFLATION_RATES
            .get_or_init(|| parse_csv(include_str!("../data/us_inflation_1928_2024.csv")));

        let years = params.years as usize;
        let one = Decimal::ONE;
        let zero = Decimal::ZERO;

        // Run rolling-window backtests
        let max_start = stock_returns.len().saturating_sub(years);
        let mut results: Vec<PeriodResult> = Vec::new();

        for start_idx in 0..=max_start {
            let start_year = stock_returns[start_idx].0;
            let mut portfolio = params.portfolio;
            let mut survived = true;

            for y in 0..years {
                if start_idx + y >= stock_returns.len() {
                    break;
                }

                let stock_return = stock_returns[start_idx + y].1;
                let inflation = if start_idx + y < inflation_rates.len() {
                    inflation_rates[start_idx + y].1
                } else {
                    Decimal::new(3, 2) // 3% default
                };

                // Apply return
                portfolio *= one + stock_return;

                // Apply withdrawal (inflation-adjusted)
                let withdrawal = match &params.strategy {
                    WithdrawalStrategy::FixedDollar(amount) => {
                        let inflation_factor = (one + inflation).powu(y as u64 + 1);
                        *amount * inflation_factor
                    }
                    WithdrawalStrategy::FixedRate(rate) => portfolio * *rate,
                    _ => {
                        // Default to fixed dollar for backtest
                        let inflation_factor = (one + inflation).powu(y as u64 + 1);
                        params.annual_withdrawal * inflation_factor
                    }
                };

                portfolio -= withdrawal;

                if portfolio < zero {
                    portfolio = zero;
                    survived = false;
                    break;
                }
            }

            results.push(PeriodResult {
                start_year,
                end_balance: portfolio,
                survived,
            });
        }

        let total = results.len() as u32;
        let successful = results.iter().filter(|r| r.survived).count() as u32;
        let failed = total - successful;

        let worst = results
            .iter()
            .min_by(|a, b| a.end_balance.cmp(&b.end_balance))
            .unwrap();
        let best = results
            .iter()
            .max_by(|a, b| a.end_balance.cmp(&b.end_balance))
            .unwrap();

        HistoricalBacktestResult {
            success_rate: if total > 0 {
                Decimal::new(successful as i64, 0) / Decimal::new(total as i64, 0)
            } else {
                zero
            },
            total_periods: total,
            successful_periods: successful,
            failed_periods: failed,
            worst_start_year: worst.start_year,
            worst_end_balance: worst.end_balance,
            best_start_year: best.start_year,
            best_end_balance: best.end_balance,
        }
    }
}

/// Parse a CSV of "year,return" into (year, Decimal) pairs.
fn parse_csv(csv: &str) -> Vec<(u32, Decimal)> {
    csv.lines()
        .filter(|line| !line.starts_with("year") && !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let year = parts[0].trim().parse::<u32>().ok()?;
                let ret = Decimal::from_str(parts[1].trim()).ok()?;
                Some((year, ret))
            } else {
                None
            }
        })
        .collect()
}
