//! Withdrawal simulation — delegates to Monte Carlo engine.

use common::Decimal;
use serde::Serialize;

use crate::monte_carlo::{
    InflationModel, MonteCarloEngine, ReturnModel, SimulationConfig, SimulationResult,
    WithdrawalStrategy,
};

/// Parameters for withdrawal simulation.
#[derive(Debug, Clone)]
pub struct WithdrawalParams {
    pub portfolio: Decimal,
    pub annual_withdrawal: Decimal,
    pub strategy: WithdrawalStrategy,
    pub years: u32,
    pub return_model: ReturnModel,
    pub inflation: InflationModel,
    pub monte_carlo_runs: u32,
    pub seed: Option<u64>,
}

/// Result of withdrawal simulation (wrapper around SimulationResult).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalResult {
    pub success_rate: Decimal,
    pub simulation: SimulationResult,
}

impl super::FIRECalculator {
    /// Run a withdrawal simulation using Monte Carlo.
    pub fn withdrawal_simulation(params: &WithdrawalParams) -> WithdrawalResult {
        let config = SimulationConfig {
            runs: params.monte_carlo_runs,
            years: params.years,
            initial_portfolio: params.portfolio,
            annual_contribution: Decimal::ZERO,
            annual_withdrawal: params.annual_withdrawal,
            withdrawal_strategy: params.strategy.clone(),
            return_model: params.return_model.clone(),
            inflation: params.inflation.clone(),
            seed: params.seed,
        };

        let result = MonteCarloEngine::run(&config).expect("Monte Carlo simulation failed");

        WithdrawalResult {
            success_rate: result.success_rate,
            simulation: result,
        }
    }
}
