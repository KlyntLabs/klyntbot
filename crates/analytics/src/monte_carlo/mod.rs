//! Monte Carlo simulation engine for financial projections.

pub mod distributions;
pub mod sampling;

use common::Decimal;
use rust_decimal::prelude::*;
use serde::Serialize;

use crate::types::PercentileBands;
use distributions::{draw_bootstrap, draw_correlated_returns, draw_log_normal};
use sampling::{cholesky_decompose, create_rng};

/// Configuration for a Monte Carlo simulation run.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub runs: u32,
    pub years: u32,
    pub initial_portfolio: Decimal,
    pub annual_contribution: Decimal,
    pub annual_withdrawal: Decimal,
    pub withdrawal_strategy: WithdrawalStrategy,
    pub return_model: ReturnModel,
    pub inflation: InflationModel,
    pub seed: Option<u64>,
}

/// How portfolio returns are modelled each year.
#[derive(Debug, Clone)]
pub enum ReturnModel {
    /// Parametric log-normal with given mean and standard deviation.
    LogNormal {
        mean_return: Decimal,
        std_dev: Decimal,
    },
    /// Resample from a vector of historical annual returns.
    HistoricalBootstrap { returns: Vec<Decimal> },
    /// Multi-asset with correlation structure.
    AssetAllocation { assets: Vec<AssetClass> },
}

/// A single asset class in a multi-asset allocation model.
#[derive(Debug, Clone)]
pub struct AssetClass {
    pub name: String,
    pub weight: Decimal,
    pub mean_return: Decimal,
    pub std_dev: Decimal,
    pub correlation_row: Vec<Decimal>,
}

/// How inflation is modelled.
#[derive(Debug, Clone)]
pub enum InflationModel {
    /// Constant annual inflation rate.
    Fixed(Decimal),
    /// Stochastic inflation drawn from a normal distribution.
    Variable { mean: Decimal, std_dev: Decimal },
}

/// Withdrawal strategy during retirement.
#[derive(Debug, Clone)]
pub enum WithdrawalStrategy {
    /// Constant percentage of current portfolio each year.
    FixedRate(Decimal),
    /// Constant dollar amount (inflation-adjusted).
    FixedDollar(Decimal),
    /// Guyton-Klinger guardrails strategy.
    GuytonKlinger {
        initial_rate: Decimal,
        ceiling_rate: Decimal,
        floor_rate: Decimal,
        capital_preservation_threshold: Decimal,
    },
    /// Variable Percentage Withdrawal based on remaining life expectancy.
    VPW { age: u32 },
}

/// Summary of the configuration used in a simulation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSummary {
    pub runs: u32,
    pub years: u32,
    pub initial_portfolio: Decimal,
    pub annual_withdrawal: Decimal,
}

/// Full result of a Monte Carlo simulation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationResult {
    pub config_summary: ConfigSummary,
    pub success_rate: Decimal,
    pub percentile_bands: PercentileBands,
    pub terminal_values: TerminalStats,
    pub worst_sequence: WorstSequence,
}

/// Statistics about the distribution of terminal portfolio values.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStats {
    pub median: Decimal,
    pub mean: Decimal,
    pub p5: Decimal,
    pub p95: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    pub ruin_count: u32,
}

/// The worst simulation path (earliest ruin or lowest terminal value).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstSequence {
    pub seed_index: u32,
    pub portfolio_by_year: Vec<Decimal>,
    pub ruin_year: Option<u32>,
}

/// Stateless Monte Carlo simulation engine.
pub struct MonteCarloEngine;

impl MonteCarloEngine {
    /// Run a simulation using the config's seed (or a time-based seed if None).
    pub fn run(config: &SimulationConfig) -> common::Result<SimulationResult> {
        let seed = config.seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(42)
        });
        Self::run_with_seed(config, seed)
    }

    /// Run a simulation with an explicit seed for full reproducibility.
    pub fn run_with_seed(config: &SimulationConfig, seed: u64) -> common::Result<SimulationResult> {
        // Pre-compute Cholesky if AssetAllocation model
        let cholesky_l = match &config.return_model {
            ReturnModel::AssetAllocation { assets } => {
                let n = assets.len();
                let mut corr_matrix = vec![vec![Decimal::ZERO; n]; n];
                for (i, row) in corr_matrix.iter_mut().enumerate().take(n) {
                    for (j, cell) in row.iter_mut().enumerate().take(n) {
                        *cell = assets[i].correlation_row[j];
                    }
                }
                let l = cholesky_decompose(&corr_matrix).ok_or_else(|| {
                    common::ToolError::ExecutionFailed(
                        "Correlation matrix is not positive definite".to_string(),
                    )
                })?;
                Some(l)
            }
            _ => None,
        };

        let runs = config.runs;
        let years = config.years as usize;
        let one = Decimal::ONE;
        let zero = Decimal::ZERO;

        // Pre-compute asset allocation vectors outside the simulation loops
        let asset_alloc_precomputed = match &config.return_model {
            ReturnModel::AssetAllocation { assets } => Some((
                assets.iter().map(|a| a.mean_return).collect::<Vec<_>>(),
                assets.iter().map(|a| a.std_dev).collect::<Vec<_>>(),
                assets.iter().map(|a| a.weight).collect::<Vec<_>>(),
            )),
            _ => None,
        };

        let mut all_terminal: Vec<Decimal> = Vec::with_capacity(runs as usize);
        let mut all_yearly: Vec<Vec<Decimal>> = Vec::with_capacity(runs as usize);
        let mut ruin_count: u32 = 0;
        let mut worst_terminal = Decimal::MAX;
        let mut worst_idx: u32 = 0;
        let mut worst_ruin_year: Option<u32> = None;
        let mut worst_yearly: Vec<Decimal> = Vec::new();

        for run_idx in 0..runs {
            let mut rng = create_rng(seed, run_idx);
            let mut portfolio = config.initial_portfolio;
            let mut yearly_values = Vec::with_capacity(years);
            let mut this_ruin_year: Option<u32> = None;

            for year in 0..years {
                let annual_return = match &config.return_model {
                    ReturnModel::LogNormal {
                        mean_return,
                        std_dev,
                    } => draw_log_normal(&mut rng, *mean_return, *std_dev),
                    ReturnModel::HistoricalBootstrap { returns } => {
                        draw_bootstrap(&mut rng, returns)
                    }
                    ReturnModel::AssetAllocation { .. } => {
                        let (means, std_devs, weights) = asset_alloc_precomputed.as_ref().unwrap();
                        let returns = draw_correlated_returns(
                            &mut rng,
                            means,
                            std_devs,
                            cholesky_l.as_ref().unwrap(),
                        );
                        weights
                            .iter()
                            .zip(returns.iter())
                            .map(|(w, r)| *w * *r)
                            .sum()
                    }
                };

                let inflation = match &config.inflation {
                    InflationModel::Fixed(rate) => *rate,
                    InflationModel::Variable { mean, std_dev } => {
                        draw_log_normal(&mut rng, *mean, *std_dev)
                    }
                };

                portfolio *= one + annual_return;

                if config.annual_withdrawal > zero {
                    let withdrawal = match &config.withdrawal_strategy {
                        WithdrawalStrategy::FixedDollar(amount) => {
                            let inflation_factor = (one + inflation).powu(year as u64 + 1);
                            *amount * inflation_factor
                        }
                        WithdrawalStrategy::FixedRate(rate) => portfolio * *rate,
                        WithdrawalStrategy::GuytonKlinger {
                            initial_rate,
                            ceiling_rate,
                            floor_rate,
                            capital_preservation_threshold,
                        } => {
                            let base_withdrawal = config.initial_portfolio * *initial_rate;
                            let inflation_factor = (one + inflation).powu(year as u64 + 1);
                            let mut w = base_withdrawal * inflation_factor;
                            let current_rate = w / portfolio;
                            if current_rate > *initial_rate + *ceiling_rate {
                                w *= one - *floor_rate;
                            } else if current_rate < *initial_rate - *floor_rate
                                && portfolio
                                    > config.initial_portfolio * *capital_preservation_threshold
                            {
                                w *= one + *ceiling_rate;
                            }
                            w
                        }
                        WithdrawalStrategy::VPW { age } => {
                            let remaining_years =
                                Decimal::new(100i64.saturating_sub(*age as i64 + year as i64), 0);
                            if remaining_years <= zero {
                                portfolio
                            } else {
                                portfolio / remaining_years
                            }
                        }
                    };
                    portfolio -= withdrawal;
                } else if config.annual_contribution > zero {
                    let inflation_factor = (one + inflation).powu(year as u64 + 1);
                    portfolio += config.annual_contribution * inflation_factor;
                }

                if portfolio < zero {
                    portfolio = zero;
                }

                yearly_values.push(portfolio);

                if portfolio == zero && config.annual_withdrawal > zero {
                    this_ruin_year = Some(year as u32);
                    ruin_count += 1;
                    for _ in (year + 1)..years {
                        yearly_values.push(zero);
                    }
                    break;
                }
            }

            let terminal = portfolio;
            all_terminal.push(terminal);

            let is_worse = terminal < worst_terminal
                || (terminal == zero
                    && worst_ruin_year.is_none_or(|wy| this_ruin_year.is_some_and(|ry| ry < wy)));
            if is_worse {
                worst_terminal = terminal;
                worst_idx = run_idx;
                worst_ruin_year = this_ruin_year;
                worst_yearly = yearly_values.clone();
            }

            all_yearly.push(yearly_values);
        }

        all_terminal.sort();

        let runs_dec = Decimal::new(runs as i64, 0);
        let success_rate = one - Decimal::new(ruin_count as i64, 0) / runs_dec;

        let terminal_stats = TerminalStats {
            median: percentile_sorted(&all_terminal, 50),
            mean: all_terminal.iter().copied().sum::<Decimal>() / runs_dec,
            p5: percentile_sorted(&all_terminal, 5),
            p95: percentile_sorted(&all_terminal, 95),
            min: all_terminal.first().copied().unwrap_or(zero),
            max: all_terminal.last().copied().unwrap_or(zero),
            ruin_count,
        };

        let mut percentile_bands = PercentileBands {
            p5: Vec::with_capacity(years),
            p25: Vec::with_capacity(years),
            p50: Vec::with_capacity(years),
            p75: Vec::with_capacity(years),
            p95: Vec::with_capacity(years),
            survival_rate: Vec::with_capacity(years),
            labels: (0..years).map(|y| format!("Year {}", y + 1)).collect(),
        };

        for year_idx in 0..years {
            let mut year_values: Vec<Decimal> =
                all_yearly.iter().map(|run| run[year_idx]).collect();
            year_values.sort();

            percentile_bands.p5.push(percentile_sorted(&year_values, 5));
            percentile_bands
                .p25
                .push(percentile_sorted(&year_values, 25));
            percentile_bands
                .p50
                .push(percentile_sorted(&year_values, 50));
            percentile_bands
                .p75
                .push(percentile_sorted(&year_values, 75));
            percentile_bands
                .p95
                .push(percentile_sorted(&year_values, 95));

            let alive = year_values.iter().filter(|v| **v > zero).count();
            percentile_bands
                .survival_rate
                .push(Decimal::new(alive as i64, 0) / runs_dec);
        }

        Ok(SimulationResult {
            config_summary: ConfigSummary {
                runs,
                years: config.years,
                initial_portfolio: config.initial_portfolio,
                annual_withdrawal: config.annual_withdrawal,
            },
            success_rate,
            percentile_bands,
            terminal_values: terminal_stats,
            worst_sequence: WorstSequence {
                seed_index: worst_idx,
                portfolio_by_year: worst_yearly,
                ruin_year: worst_ruin_year,
            },
        })
    }
}

/// Get the value at a given percentile from a pre-sorted slice.
fn percentile_sorted(sorted: &[Decimal], pct: u32) -> Decimal {
    if sorted.is_empty() {
        return Decimal::ZERO;
    }
    let idx = (sorted.len() as f64 * pct as f64 / 100.0).ceil() as usize;
    let idx = idx.min(sorted.len()).max(1) - 1;
    sorted[idx]
}
