//! Sensitivity analysis for withdrawal success rates.

use common::Decimal;
use serde::Serialize;

use crate::monte_carlo::{MonteCarloEngine, ReturnModel, SimulationConfig, WithdrawalStrategy};

use super::withdrawal::WithdrawalParams;

/// Configuration for sensitivity analysis.
#[derive(Debug, Clone)]
pub struct SensitivityConfig {
    pub runs_per_point: u32,
    pub seed: Option<u64>,
}

/// A single point in the sensitivity grid.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityPoint {
    pub withdrawal_rate: Decimal,
    pub expected_return: Decimal,
    pub success_rate: Decimal,
}

/// Result of sensitivity analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityResult {
    pub grid: Vec<Vec<SensitivityPoint>>,
    pub withdrawal_rates: Vec<Decimal>,
    pub return_rates: Vec<Decimal>,
}

impl super::FIRECalculator {
    /// Run sensitivity analysis: success rate for each (withdrawal_rate, return_rate) pair.
    ///
    /// For each combination, runs a Monte Carlo simulation with the given number of runs.
    /// The withdrawal amount is computed as `portfolio * withdrawal_rate`.
    pub fn sensitivity_withdrawal_vs_return(
        base: &WithdrawalParams,
        withdrawal_rates: &[Decimal],
        return_rates: &[Decimal],
        config: &SensitivityConfig,
    ) -> common::Result<SensitivityResult> {
        let mut grid: Vec<Vec<SensitivityPoint>> = Vec::with_capacity(withdrawal_rates.len());

        for (wr_idx, &wr) in withdrawal_rates.iter().enumerate() {
            let mut row: Vec<SensitivityPoint> = Vec::with_capacity(return_rates.len());

            for (rr_idx, &rr) in return_rates.iter().enumerate() {
                let annual_withdrawal = base.portfolio * wr;

                // Derive a unique seed for each grid cell for reproducibility
                let seed = config
                    .seed
                    .map(|s| s.wrapping_add(wr_idx as u64 * 1000 + rr_idx as u64));

                let sim_config = SimulationConfig {
                    runs: config.runs_per_point,
                    years: base.years,
                    initial_portfolio: base.portfolio,
                    annual_contribution: Decimal::ZERO,
                    annual_withdrawal,
                    withdrawal_strategy: WithdrawalStrategy::FixedDollar(annual_withdrawal),
                    return_model: ReturnModel::LogNormal {
                        mean_return: rr,
                        std_dev: match &base.return_model {
                            ReturnModel::LogNormal { std_dev, .. } => *std_dev,
                            _ => Decimal::new(15, 2), // 0.15 default
                        },
                    },
                    inflation: base.inflation.clone(),
                    seed,
                };

                let result = MonteCarloEngine::run(&sim_config)?;

                row.push(SensitivityPoint {
                    withdrawal_rate: wr,
                    expected_return: rr,
                    success_rate: result.success_rate,
                });
            }

            grid.push(row);
        }

        Ok(SensitivityResult {
            grid,
            withdrawal_rates: withdrawal_rates.to_vec(),
            return_rates: return_rates.to_vec(),
        })
    }
}
