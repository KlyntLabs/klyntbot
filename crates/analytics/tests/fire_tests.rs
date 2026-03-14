use analytics::fire::{
    CoastFIREParams, FIRECalculator, FIREParams, HistoricalBacktestParams, WithdrawalParams,
};
use analytics::{InflationModel, ReturnModel, WithdrawalStrategy};
use rust_decimal_macros::dec;

#[test]
fn fire_number_basic() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(0),
        monthly_savings: dec!(2000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert_eq!(result.fire_numbers[0].fire_number, dec!(1000000));
}

#[test]
fn fire_number_multiple_swr() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(3000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04), dec!(0.035), dec!(0.03)],
    });
    assert_eq!(result.fire_numbers.len(), 3);
    assert_eq!(result.fire_numbers[0].fire_number, dec!(1000000));
    assert!(result.fire_numbers[1].fire_number > dec!(1000000));
    assert!(result.fire_numbers[2].fire_number > result.fire_numbers[1].fire_number);
}

#[test]
fn fire_already_reached() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(1500000),
        monthly_savings: dec!(3000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert_eq!(result.months_to_fire, Some(0));
    assert!(result.current_progress >= dec!(1));
}

#[test]
fn fire_zero_savings_unreachable() {
    let result = FIRECalculator::traditional(&FIREParams {
        annual_expenses: dec!(40000),
        current_portfolio: dec!(0),
        monthly_savings: dec!(0),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rates: vec![dec!(0.04)],
    });
    assert!(result.months_to_fire.is_none());
}

#[test]
fn coast_fire_already_coasting() {
    let result = FIRECalculator::coast(&CoastFIREParams {
        current_portfolio: dec!(500000),
        current_age: 30,
        target_age: 65,
        annual_expenses_at_retirement: dec!(40000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    assert!(result.is_coast_fire);
    assert!(result.surplus_or_deficit > dec!(0));
}

#[test]
fn coast_fire_negative_real_return_unreachable() {
    let result = FIRECalculator::coast(&CoastFIREParams {
        current_portfolio: dec!(100000),
        current_age: 30,
        target_age: 65,
        annual_expenses_at_retirement: dec!(40000),
        expected_return: dec!(0.02),
        inflation_rate: dec!(0.05),
        withdrawal_rate: dec!(0.04),
    });
    assert!(!result.is_coast_fire);
    assert!(result.years_to_coast.is_none());
}

#[test]
fn lean_fire_uses_essentials_only() {
    let lean = FIRECalculator::lean(&analytics::fire::LeanFIREParams {
        essential_expenses: dec!(25000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(2000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    assert_eq!(lean.fire_numbers[0].fire_number, dec!(625000));
}

#[test]
fn fat_fire_uses_full_lifestyle() {
    let fat = FIRECalculator::fat(&analytics::fire::FatFIREParams {
        desired_annual_spending: dec!(100000),
        current_portfolio: dec!(500000),
        monthly_savings: dec!(5000),
        expected_return: dec!(0.07),
        inflation_rate: dec!(0.03),
        withdrawal_rate: dec!(0.04),
    });
    assert_eq!(fat.fire_numbers[0].fire_number, dec!(2500000));
}

#[test]
fn withdrawal_sim_high_success_with_low_rate() {
    let result = FIRECalculator::withdrawal_simulation(&WithdrawalParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(30000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(30000)),
        years: 30,
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 1000,
        seed: Some(42),
    });
    // Log-normal with 15% std dev produces a wide return distribution;
    // 3% withdrawal over 30yr yields ~80-85% success in this model.
    assert!(result.success_rate > dec!(0.75));
}

#[test]
fn withdrawal_sim_immediate_ruin() {
    let result = FIRECalculator::withdrawal_simulation(&WithdrawalParams {
        portfolio: dec!(100),
        annual_withdrawal: dec!(50000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(50000)),
        years: 30,
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        monte_carlo_runs: 100,
        seed: Some(42),
    });
    assert_eq!(result.success_rate, dec!(0));
}

#[test]
fn backtest_loads_embedded_data() {
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    assert!(result.total_periods > 0);
    assert!(result.success_rate > dec!(0));
}

// ===========================================================================
// Property-based tests
// ===========================================================================

use proptest::prelude::*;

proptest! {
    #[test]
    fn fire_number_is_expenses_over_rate(
        expenses in 1000i64..1_000_000,
        rate in 1u32..20
    ) {
        let e = rust_decimal::Decimal::new(expenses, 0);
        let r = rust_decimal::Decimal::new(rate as i64, 2);
        let result = FIRECalculator::traditional(&FIREParams {
            annual_expenses: e,
            current_portfolio: dec!(0),
            monthly_savings: dec!(1000),
            expected_return: dec!(0.07),
            inflation_rate: dec!(0.03),
            withdrawal_rates: vec![r],
        });
        let expected = e / r;
        prop_assert_eq!(result.fire_numbers[0].fire_number, expected);
    }
}
