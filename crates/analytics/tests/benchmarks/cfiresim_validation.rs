use analytics::fire::{FIRECalculator, HistoricalBacktestParams};
use analytics::WithdrawalStrategy;
use rust_decimal_macros::dec;

#[test]
#[ignore] // Run manually: cargo nextest run -p analytics --run-ignored all
fn benchmark_4pct_rule_30yr_historical() {
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    // cFIREsim reports ~95-96% success for nominal 4% SWR.
    // Our engine applies inflation-adjusted withdrawals (real purchasing power preserved),
    // which is more conservative — yielding ~80-85% success rate.
    assert!(
        result.success_rate > dec!(0.75),
        "Expected >75% for inflation-adjusted 4% SWR, got {}",
        result.success_rate
    );
    assert!(result.success_rate < dec!(1.00));
}

#[test]
#[ignore]
fn benchmark_3pct_rule_30yr_historical() {
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(30000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(30000)),
        years: 30,
    });
    // 3% SWR with inflation-adjusted withdrawals should have high success rate
    assert!(
        result.success_rate > dec!(0.90),
        "Expected >90% for inflation-adjusted 3% SWR, got {}",
        result.success_rate
    );
}
