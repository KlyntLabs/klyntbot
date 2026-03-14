use analytics::{
    InflationModel, MonteCarloEngine, ReturnModel, SimulationConfig, WithdrawalStrategy,
};
use rust_decimal_macros::dec;

fn base_config() -> SimulationConfig {
    SimulationConfig {
        runs: 1000,
        years: 30,
        initial_portfolio: dec!(1000000),
        annual_contribution: dec!(0),
        annual_withdrawal: dec!(40000),
        withdrawal_strategy: WithdrawalStrategy::FixedDollar(dec!(40000)),
        return_model: ReturnModel::LogNormal {
            mean_return: dec!(0.07),
            std_dev: dec!(0.15),
        },
        inflation: InflationModel::Fixed(dec!(0.03)),
        seed: Some(42),
    }
}

#[test]
fn deterministic_with_same_seed() {
    let config = base_config();
    let r1 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let r2 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(r1.success_rate, r2.success_rate);
    assert_eq!(r1.terminal_values.median, r2.terminal_values.median);
    assert_eq!(r1.terminal_values.ruin_count, r2.terminal_values.ruin_count);
}

#[test]
fn different_seeds_produce_different_results() {
    let config = base_config();
    let r1 = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let r2 = MonteCarloEngine::run_with_seed(&config, 999).unwrap();
    assert_ne!(r1.terminal_values.median, r2.terminal_values.median);
}

#[test]
fn zero_portfolio_always_ruins() {
    let mut config = base_config();
    config.initial_portfolio = dec!(0);
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.success_rate, dec!(0));
    assert_eq!(result.terminal_values.ruin_count, 100);
}

#[test]
fn no_withdrawal_never_ruins() {
    let mut config = base_config();
    config.annual_withdrawal = dec!(0);
    config.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(0));
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.success_rate, dec!(1));
    assert_eq!(result.terminal_values.ruin_count, 0);
}

#[test]
fn higher_withdrawal_lower_success() {
    let low = {
        let mut c = base_config();
        c.annual_withdrawal = dec!(30000);
        c.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(30000));
        c.runs = 500;
        MonteCarloEngine::run_with_seed(&c, 42).unwrap()
    };
    let high = {
        let mut c = base_config();
        c.annual_withdrawal = dec!(80000);
        c.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(80000));
        c.runs = 500;
        MonteCarloEngine::run_with_seed(&c, 42).unwrap()
    };
    assert!(low.success_rate >= high.success_rate);
}

#[test]
fn percentile_bands_correct_length() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert_eq!(result.percentile_bands.p50.len(), 30);
    assert_eq!(result.percentile_bands.labels.len(), 30);
    assert_eq!(result.percentile_bands.survival_rate.len(), 30);
}

#[test]
fn percentile_ordering() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    for i in 0..result.percentile_bands.p50.len() {
        assert!(result.percentile_bands.p5[i] <= result.percentile_bands.p25[i]);
        assert!(result.percentile_bands.p25[i] <= result.percentile_bands.p50[i]);
        assert!(result.percentile_bands.p50[i] <= result.percentile_bands.p75[i]);
        assert!(result.percentile_bands.p75[i] <= result.percentile_bands.p95[i]);
    }
}

#[test]
fn terminal_stats_consistency() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert!(result.terminal_values.min <= result.terminal_values.p5);
    assert!(result.terminal_values.p5 <= result.terminal_values.median);
    assert!(result.terminal_values.median <= result.terminal_values.p95);
    assert!(result.terminal_values.p95 <= result.terminal_values.max);
}

#[test]
fn success_rate_derived_from_ruin_count() {
    let config = base_config();
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    let expected = dec!(1)
        - rust_decimal::Decimal::new(result.terminal_values.ruin_count as i64, 0)
            / rust_decimal::Decimal::new(config.runs as i64, 0);
    assert_eq!(result.success_rate, expected);
}

#[test]
fn bootstrap_model_works() {
    let mut config = base_config();
    config.return_model = ReturnModel::HistoricalBootstrap {
        returns: vec![dec!(0.10), dec!(-0.05), dec!(0.15), dec!(0.08), dec!(-0.10)],
    };
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert!(result.success_rate > dec!(0));
}

#[test]
fn contribution_mode_grows_portfolio() {
    let mut config = base_config();
    config.annual_withdrawal = dec!(0);
    config.annual_contribution = dec!(20000);
    config.withdrawal_strategy = WithdrawalStrategy::FixedDollar(dec!(0));
    config.runs = 100;
    let result = MonteCarloEngine::run_with_seed(&config, 42).unwrap();
    assert!(result.terminal_values.median > config.initial_portfolio);
}
