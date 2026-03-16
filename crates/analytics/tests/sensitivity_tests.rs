use analytics::fire::{FIRECalculator, SensitivityConfig, WithdrawalParams};
use analytics::{InflationModel, ReturnModel, WithdrawalStrategy};
use rust_decimal_macros::dec;

fn base_withdrawal_params() -> WithdrawalParams {
    WithdrawalParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 500,
        seed: Some(42),
    }
}

#[test]
fn sensitivity_grid_dimensions() {
    let base = base_withdrawal_params();
    let withdrawal_rates = vec![dec!(0.03), dec!(0.04), dec!(0.05)];
    let return_rates = vec![dec!(0.05), dec!(0.07)];
    let config = SensitivityConfig {
        runs_per_point: 100,
        seed: Some(42),
    };

    let result = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();

    assert_eq!(result.grid.len(), 3);
    assert_eq!(result.grid[0].len(), 2);
    assert_eq!(result.withdrawal_rates.len(), 3);
    assert_eq!(result.return_rates.len(), 2);
}

#[test]
fn sensitivity_higher_withdrawal_lower_success() {
    let base = base_withdrawal_params();
    let withdrawal_rates = vec![dec!(0.03), dec!(0.06)];
    let return_rates = vec![dec!(0.07)];
    let config = SensitivityConfig {
        runs_per_point: 200,
        seed: Some(42),
    };

    let result = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();

    let low_wr_success = result.grid[0][0].success_rate;
    let high_wr_success = result.grid[1][0].success_rate;
    assert!(
        low_wr_success >= high_wr_success,
        "3% withdrawal ({}) should have >= success than 6% ({})",
        low_wr_success,
        high_wr_success
    );
}

#[test]
fn sensitivity_higher_return_higher_success() {
    let base = base_withdrawal_params();
    let withdrawal_rates = vec![dec!(0.04)];
    let return_rates = vec![dec!(0.04), dec!(0.10)];
    let config = SensitivityConfig {
        runs_per_point: 200,
        seed: Some(42),
    };

    let result = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();

    let low_return_success = result.grid[0][0].success_rate;
    let high_return_success = result.grid[0][1].success_rate;
    assert!(
        high_return_success >= low_return_success,
        "10% return ({}) should have >= success than 4% ({})",
        high_return_success,
        low_return_success
    );
}

#[test]
fn sensitivity_deterministic_with_seed() {
    let base = base_withdrawal_params();
    let withdrawal_rates = vec![dec!(0.04)];
    let return_rates = vec![dec!(0.07)];
    let config = SensitivityConfig {
        runs_per_point: 100,
        seed: Some(42),
    };

    let r1 = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();
    let r2 = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();

    assert_eq!(r1.grid[0][0].success_rate, r2.grid[0][0].success_rate);
}

#[test]
fn sensitivity_point_fields_populated() {
    let base = base_withdrawal_params();
    let withdrawal_rates = vec![dec!(0.04)];
    let return_rates = vec![dec!(0.07)];
    let config = SensitivityConfig {
        runs_per_point: 100,
        seed: Some(42),
    };

    let result = FIRECalculator::sensitivity_withdrawal_vs_return(
        &base,
        &withdrawal_rates,
        &return_rates,
        &config,
    )
    .unwrap();

    let point = &result.grid[0][0];
    assert_eq!(point.withdrawal_rate, dec!(0.04));
    assert_eq!(point.expected_return, dec!(0.07));
    assert!(point.success_rate >= dec!(0));
    assert!(point.success_rate <= dec!(1));
}
