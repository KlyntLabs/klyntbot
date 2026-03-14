use analytics::fire::{FIRECalculator, HistoricalBacktestParams};
use analytics::WithdrawalStrategy;
use rust_decimal_macros::dec;

#[test]
#[ignore]
fn trinity_4pct_stocks_30yr() {
    // Trinity Study: 4% SWR, 100% stocks, 30 years
    // Original study found ~95% success rate for nominal withdrawals.
    // Our engine uses inflation-adjusted withdrawals, yielding ~80-85%.
    let result = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    assert!(
        result.success_rate > dec!(0.75),
        "Expected >75% for inflation-adjusted 4% SWR, got {}",
        result.success_rate
    );
    assert!(
        result.total_periods > 50,
        "Should have at least 50 rolling periods"
    );
}

#[test]
#[ignore]
fn trinity_5pct_stocks_30yr() {
    // 5% SWR should have lower success than 4%
    let result_4 = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(40000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        years: 30,
    });
    let result_5 = FIRECalculator::historical_backtest(&HistoricalBacktestParams {
        portfolio: dec!(1000000),
        annual_withdrawal: dec!(50000),
        strategy: WithdrawalStrategy::FixedDollar(dec!(50000)),
        years: 30,
    });
    assert!(result_4.success_rate >= result_5.success_rate);
}
