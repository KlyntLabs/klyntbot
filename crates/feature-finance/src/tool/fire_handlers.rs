//! Advanced FIRE planning action handlers for `FinanceTool`.
//!
//! Handles: fire_traditional, fire_coast, fire_lean, fire_fat,
//! fire_withdrawal_sim, fire_backtest, fire_sensitivity.

use common::{Decimal, Result, ToolError};
use rust_decimal::prelude::FromPrimitive;
use serde_json::json;
use tools_core::ParamExtractor;
use tools_core::RoutingContext;

use analytics::fire::{
    CoastFIREParams, FIRECalculator, FIREParams, FatFIREParams, HistoricalBacktestParams,
    LeanFIREParams, SensitivityConfig, WithdrawalParams,
};
use analytics::monte_carlo::{InflationModel, ReturnModel, WithdrawalStrategy};

use super::FinanceTool;

/// Helper to convert an optional f64 parameter to a Decimal with a default.
fn dec_param(val: Option<f64>, default: f64) -> Decimal {
    Decimal::from_f64(val.unwrap_or(default)).unwrap_or(Decimal::ZERO)
}

/// Helper to convert a required i64 amount (in cents/smallest unit) to Decimal.
fn dec_from_i64(val: i64) -> Decimal {
    Decimal::new(val, 0)
}

impl FinanceTool {
    pub(crate) async fn handle_fire(
        &self,
        action: &str,
        p: &ParamExtractor<'_>,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        match action {
            "fire_traditional" => self.fire_traditional(p).await,
            "fire_coast" => self.fire_coast(p).await,
            "fire_lean" => self.fire_lean(p).await,
            "fire_fat" => self.fire_fat(p).await,
            "fire_withdrawal_sim" => self.fire_withdrawal_sim(p).await,
            "fire_backtest" => self.fire_backtest(p).await,
            "fire_sensitivity" => self.fire_sensitivity(p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown FIRE action: {action}")).into()),
        }
    }

    /// Compute total current portfolio from accounts + investments, optionally use provided value.
    async fn compute_current_portfolio(&self, p: &ParamExtractor<'_>) -> Result<Decimal> {
        if let Some(val) = p.optional_i64("current_portfolio")? {
            return Ok(dec_from_i64(val));
        }

        let base = &self.default_currency;
        let (accounts_total, investments_total) = tokio::try_join!(
            self.storage.accounts.total_base_balance(base),
            self.storage.investments.total_base_value(base),
        )?;

        Ok(dec_from_i64(accounts_total + investments_total))
    }

    async fn fire_traditional(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let annual_expenses = dec_from_i64(p.required_i64("annual_expenses")?);
        let current_portfolio = self.compute_current_portfolio(p).await?;
        let monthly_savings = dec_from_i64(p.i64_or("monthly_savings", 0)?);
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);

        // Parse withdrawal_rates or use default [0.04]
        let withdrawal_rates = match p.optional_str("withdrawal_rates")? {
            Some(s) => s
                .split(',')
                .filter_map(|r| r.trim().parse::<f64>().ok())
                .filter_map(Decimal::from_f64)
                .collect::<Vec<_>>(),
            None => vec![Decimal::from_f64(0.04).unwrap_or(Decimal::ZERO)],
        };

        if withdrawal_rates.is_empty() {
            return Err(ToolError::InvalidParams(
                "At least one withdrawal rate is required".into(),
            )
            .into());
        }

        let params = FIREParams {
            annual_expenses,
            current_portfolio,
            monthly_savings,
            expected_return,
            inflation_rate,
            withdrawal_rates,
        };

        let result = FIRECalculator::traditional(&params);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn fire_coast(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let current_portfolio = self.compute_current_portfolio(p).await?;
        let current_age = p.required_i64("current_age")? as u32;
        let target_age = p.i64_or("target_age", 65)? as u32;
        let annual_expenses_at_retirement =
            dec_from_i64(p.required_i64("annual_expenses_at_retirement")?);
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);
        let withdrawal_rate = dec_param(p.optional_f64("withdrawal_rate")?, 0.04);

        let params = CoastFIREParams {
            current_portfolio,
            current_age,
            target_age,
            annual_expenses_at_retirement,
            expected_return,
            inflation_rate,
            withdrawal_rate,
        };

        let result = FIRECalculator::coast(&params);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn fire_lean(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let essential_expenses = dec_from_i64(p.required_i64("essential_expenses")?);
        let current_portfolio = self.compute_current_portfolio(p).await?;
        let monthly_savings = dec_from_i64(p.i64_or("monthly_savings", 0)?);
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);
        let withdrawal_rate = dec_param(p.optional_f64("withdrawal_rate")?, 0.04);

        let params = LeanFIREParams {
            essential_expenses,
            current_portfolio,
            monthly_savings,
            expected_return,
            inflation_rate,
            withdrawal_rate,
        };

        let result = FIRECalculator::lean(&params);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn fire_fat(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let desired_annual_spending = dec_from_i64(p.required_i64("desired_annual_spending")?);
        let current_portfolio = self.compute_current_portfolio(p).await?;
        let monthly_savings = dec_from_i64(p.i64_or("monthly_savings", 0)?);
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);
        let withdrawal_rate = dec_param(p.optional_f64("withdrawal_rate")?, 0.04);

        let params = FatFIREParams {
            desired_annual_spending,
            current_portfolio,
            monthly_savings,
            expected_return,
            inflation_rate,
            withdrawal_rate,
        };

        let result = FIRECalculator::fat(&params);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn fire_withdrawal_sim(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio = self.compute_current_portfolio(p).await?;
        let annual_withdrawal = dec_from_i64(p.required_i64("annual_withdrawal")?);
        let years = p.i64_or("years", 30)? as u32;
        let monte_carlo_runs = p.i64_or("monte_carlo_runs", 1000)? as u32;
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let std_dev = dec_param(p.optional_f64("std_dev")?, 0.15);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);
        let seed = p.optional_i64("seed")?.map(|s| s as u64);

        let strategy_str = p.str_or("strategy", "fixed_dollar")?;
        let strategy = match strategy_str {
            "fixed_rate" => {
                let rate = dec_param(p.optional_f64("withdrawal_rate")?, 0.04);
                WithdrawalStrategy::FixedRate(rate)
            }
            _ => WithdrawalStrategy::FixedDollar(annual_withdrawal),
        };

        let params = WithdrawalParams {
            portfolio,
            annual_withdrawal,
            strategy,
            years,
            return_model: ReturnModel::LogNormal {
                mean_return: expected_return,
                std_dev,
            },
            inflation: InflationModel::Fixed(inflation_rate),
            monte_carlo_runs,
            seed,
        };

        let result = FIRECalculator::withdrawal_simulation(&params)?;
        Ok(serde_json::to_string_pretty(&json!({
            "success_rate": result.success_rate.to_string(),
            "median_final": result.simulation.terminal_values.median.to_string(),
            "p5_final": result.simulation.terminal_values.p5.to_string(),
            "p95_final": result.simulation.terminal_values.p95.to_string(),
            "ruin_count": result.simulation.terminal_values.ruin_count,
            "years": years,
            "monte_carlo_runs": monte_carlo_runs,
        }))
        .unwrap())
    }

    async fn fire_backtest(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio = self.compute_current_portfolio(p).await?;
        let annual_withdrawal = dec_from_i64(p.required_i64("annual_withdrawal")?);
        let years = p.i64_or("years", 30)? as u32;

        let strategy_str = p.str_or("strategy", "fixed_dollar")?;
        let strategy = match strategy_str {
            "fixed_rate" => {
                let rate = dec_param(p.optional_f64("withdrawal_rate")?, 0.04);
                WithdrawalStrategy::FixedRate(rate)
            }
            _ => WithdrawalStrategy::FixedDollar(annual_withdrawal),
        };

        let params = HistoricalBacktestParams {
            portfolio,
            annual_withdrawal,
            strategy,
            years,
        };

        let result = FIRECalculator::historical_backtest(&params);
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }

    async fn fire_sensitivity(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let portfolio = self.compute_current_portfolio(p).await?;
        let annual_withdrawal = dec_from_i64(p.i64_or("annual_withdrawal", 0)?);
        let years = p.i64_or("years", 30)? as u32;
        let runs_per_point = p.i64_or("runs_per_point", 500)? as u32;
        let expected_return = dec_param(p.optional_f64("expected_return")?, 0.07);
        let std_dev = dec_param(p.optional_f64("std_dev")?, 0.15);
        let inflation_rate = dec_param(p.optional_f64("inflation_rate")?, 0.03);
        let seed = p.optional_i64("seed")?.map(|s| s as u64);

        let base = WithdrawalParams {
            portfolio,
            annual_withdrawal,
            strategy: WithdrawalStrategy::FixedDollar(annual_withdrawal),
            years,
            return_model: ReturnModel::LogNormal {
                mean_return: expected_return,
                std_dev,
            },
            inflation: InflationModel::Fixed(inflation_rate),
            monte_carlo_runs: runs_per_point,
            seed,
        };

        let config = SensitivityConfig {
            runs_per_point,
            seed,
        };

        // Default sensitivity grid
        let wr: Vec<Decimal> = [0.03, 0.035, 0.04, 0.045, 0.05]
            .iter()
            .filter_map(|&r| Decimal::from_f64(r))
            .collect();
        let rr: Vec<Decimal> = [0.04, 0.05, 0.06, 0.07, 0.08, 0.09, 0.10]
            .iter()
            .filter_map(|&r| Decimal::from_f64(r))
            .collect();

        let result = FIRECalculator::sensitivity_withdrawal_vs_return(&base, &wr, &rr, &config)?;
        Ok(serde_json::to_string_pretty(&result).unwrap())
    }
}
