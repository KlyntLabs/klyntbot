use analytics::portfolio::{AssetCorrelationConfig, PortfolioAnalyzer, RebalanceStrategy};
use analytics::{AllocationTarget, Holding, InvestmentCashFlow, PriceSeries};
use jiff::civil::{date, Date};
use rust_decimal_macros::dec;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_holding(name: &str, asset_class: &str, value: rust_decimal::Decimal) -> Holding {
    Holding {
        name: name.to_string(),
        symbol: Some(name.to_uppercase()),
        asset_class: asset_class.to_string(),
        current_value: value,
        cost_basis: value,
        quantity: dec!(1),
    }
}

fn make_target(
    asset_class: &str,
    weight: rust_decimal::Decimal,
    tolerance: rust_decimal::Decimal,
) -> AllocationTarget {
    AllocationTarget {
        asset_class: asset_class.to_string(),
        target_weight: weight,
        tolerance_band: tolerance,
    }
}

// ─── Drift tests ─────────────────────────────────────────────────────────────

#[test]
fn drift_balanced_portfolio() {
    // 60/40 portfolio exactly on target → no drift.
    let holdings = vec![
        make_holding("VTI", "stocks", dec!(60000)),
        make_holding("BND", "bonds", dec!(40000)),
    ];
    let targets = vec![
        make_target("stocks", dec!(0.60), dec!(0.05)),
        make_target("bonds", dec!(0.40), dec!(0.05)),
    ];

    let result = PortfolioAnalyzer::allocation_drift(&holdings, &targets);

    assert!(!result.needs_rebalancing);
    assert_eq!(result.drift_score, dec!(0));
    assert_eq!(result.allocations.len(), 2);

    let stocks = &result.allocations[0];
    assert_eq!(stocks.current_weight, dec!(0.60));
    assert_eq!(stocks.drift, dec!(0));
    assert!(!stocks.exceeds_band);
}

#[test]
fn drift_needs_rebalancing() {
    // Stocks grew to 75%, bonds dropped to 25% — well beyond 5% tolerance.
    let holdings = vec![
        make_holding("VTI", "stocks", dec!(75000)),
        make_holding("BND", "bonds", dec!(25000)),
    ];
    let targets = vec![
        make_target("stocks", dec!(0.60), dec!(0.05)),
        make_target("bonds", dec!(0.40), dec!(0.05)),
    ];

    let result = PortfolioAnalyzer::allocation_drift(&holdings, &targets);

    assert!(result.needs_rebalancing);
    // drift_score = |0.75-0.60| + |0.25-0.40| = 0.15 + 0.15 = 0.30
    assert_eq!(result.drift_score, dec!(0.30));

    let stocks = &result.allocations[0];
    assert_eq!(stocks.drift, dec!(0.15));
    assert!(stocks.exceeds_band);

    let bonds = &result.allocations[1];
    assert_eq!(bonds.drift, dec!(-0.15));
    assert!(bonds.exceeds_band);
}

#[test]
fn drift_single_holding() {
    // Single holding, single target → drift = 0 (100% weight matches 100% target).
    let holdings = vec![make_holding("VTI", "stocks", dec!(50000))];
    let targets = vec![make_target("stocks", dec!(1.0), dec!(0.05))];

    let result = PortfolioAnalyzer::allocation_drift(&holdings, &targets);

    assert!(!result.needs_rebalancing);
    assert_eq!(result.drift_score, dec!(0));
    assert_eq!(result.allocations[0].current_weight, dec!(1));
    assert_eq!(result.allocations[0].drift, dec!(0));
}

// ─── Rebalancing tests ──────────────────────────────────────────────────────

#[test]
fn rebalance_full() {
    // Stocks are overweight (70/30) vs target (60/40). Full rebalance to fix it.
    let holdings = vec![
        make_holding("VTI", "stocks", dec!(70000)),
        make_holding("BND", "bonds", dec!(30000)),
    ];
    let targets = vec![
        make_target("stocks", dec!(0.60), dec!(0.05)),
        make_target("bonds", dec!(0.40), dec!(0.05)),
    ];

    let result = PortfolioAnalyzer::rebalance_suggestions(
        &holdings,
        &targets,
        RebalanceStrategy::FullRebalance,
        dec!(0), // no new money
        dec!(100),
    );

    assert_eq!(result.total_portfolio_value, dec!(100000));
    assert_eq!(result.suggestions.len(), 2);

    // Stocks should sell: 70000 - 60000 = 10000.
    let stocks_sug = result
        .suggestions
        .iter()
        .find(|s| s.asset_class == "stocks")
        .unwrap();
    assert_eq!(stocks_sug.action, "sell");
    assert_eq!(stocks_sug.amount, dec!(10000));

    // Bonds should buy: 40000 - 30000 = 10000.
    let bonds_sug = result
        .suggestions
        .iter()
        .find(|s| s.asset_class == "bonds")
        .unwrap();
    assert_eq!(bonds_sug.action, "buy");
    assert_eq!(bonds_sug.amount, dec!(10000));
}

#[test]
fn rebalance_contribution_only() {
    // Stocks overweight (70/30), add 10000 contribution. Only buy orders.
    let holdings = vec![
        make_holding("VTI", "stocks", dec!(70000)),
        make_holding("BND", "bonds", dec!(30000)),
    ];
    let targets = vec![
        make_target("stocks", dec!(0.60), dec!(0.05)),
        make_target("bonds", dec!(0.40), dec!(0.05)),
    ];

    let result = PortfolioAnalyzer::rebalance_suggestions(
        &holdings,
        &targets,
        RebalanceStrategy::ContributionOnly,
        dec!(10000),
        dec!(100),
    );

    // All suggestions should be buy.
    for s in &result.suggestions {
        assert_eq!(s.action, "buy");
    }

    // Bonds are underweight, so they should get most/all of the contribution.
    let bonds_sug = result
        .suggestions
        .iter()
        .find(|s| s.asset_class == "bonds")
        .unwrap();
    assert!(bonds_sug.amount > dec!(0));
}

#[test]
fn rebalance_min_trade_filter() {
    // Nearly balanced portfolio with high min_trade_amount → no suggestions.
    let holdings = vec![
        make_holding("VTI", "stocks", dec!(60500)),
        make_holding("BND", "bonds", dec!(39500)),
    ];
    let targets = vec![
        make_target("stocks", dec!(0.60), dec!(0.05)),
        make_target("bonds", dec!(0.40), dec!(0.05)),
    ];

    let result = PortfolioAnalyzer::rebalance_suggestions(
        &holdings,
        &targets,
        RebalanceStrategy::FullRebalance,
        dec!(0),
        dec!(1000), // high min trade → filter out small adjustments
    );

    // Drift is only 500, below min_trade_amount of 1000.
    assert!(result.suggestions.is_empty());
}

// ─── Returns tests ──────────────────────────────────────────────────────────

#[test]
fn returns_no_cash_flows() {
    // Simple 10% gain over 1 year, no cash flows.
    let start = date(2024, 1, 1);
    let end = date(2025, 1, 1);

    let result = PortfolioAnalyzer::returns(dec!(100000), dec!(110000), &[], start, end);

    // TWR = (110000 - 100000) / 100000 = 0.10
    assert_eq!(result.twr, dec!(0.1));
    assert_eq!(result.total_gain, dec!(10000));
    assert_eq!(result.total_invested, dec!(0));

    // MWR should also converge close to 0.10.
    let mwr = result.mwr.unwrap();
    let mwr_f64 = mwr.to_string().parse::<f64>().unwrap();
    assert!(
        (mwr_f64 - 0.10).abs() < 0.01,
        "MWR should be ~0.10, got {mwr_f64}"
    );
}

#[test]
fn returns_with_contribution() {
    // Start 100k, contribute 10k halfway, end at 121k.
    let start = date(2024, 1, 1);
    let mid = date(2024, 7, 1);
    let end = date(2025, 1, 1);

    let cash_flows = vec![InvestmentCashFlow {
        date: mid,
        amount: dec!(10000),
        holding_symbol: None,
    }];

    let result = PortfolioAnalyzer::returns(dec!(100000), dec!(121000), &cash_flows, start, end);

    // gain = 121000 - 100000 - 10000 = 11000 (invested 10k, gained 11k above start+contributions)
    assert_eq!(result.total_gain, dec!(11000));
    assert_eq!(result.total_invested, dec!(10000));

    // Modified Dietz TWR: (121000 - 100000 - 10000) / (100000 + 10000 * 0.5) ≈ 0.1047...
    let twr_f64 = result.twr.to_string().parse::<f64>().unwrap();
    assert!(
        (twr_f64 - 0.1048).abs() < 0.01,
        "TWR should be ~0.105, got {twr_f64}"
    );
}

#[test]
fn returns_twr_annualized() {
    // 21% total return over 2 years → annualized ≈ 10%.
    let start = date(2023, 1, 1);
    let end = date(2025, 1, 1);

    let result = PortfolioAnalyzer::returns(dec!(100000), dec!(121000), &[], start, end);

    // TWR = 21%
    assert_eq!(result.twr, dec!(0.21));

    // Annualized ≈ (1.21)^(1/2) - 1 ≈ 0.10
    let ann_f64 = result.twr_annualized.to_string().parse::<f64>().unwrap();
    assert!(
        (ann_f64 - 0.10).abs() < 0.005,
        "Annualized TWR should be ~0.10, got {ann_f64}"
    );
}

// ─── Correlation tests ──────────────────────────────────────────────────────

/// Generate a price series with monthly prices starting from a base, applying monthly returns.
fn make_price_series(
    symbol: &str,
    start_year: i32,
    months: usize,
    base_price: f64,
    monthly_returns: &[f64],
) -> PriceSeries {
    use rust_decimal::Decimal;

    let mut prices = Vec::new();
    let mut price = base_price;

    for i in 0..months {
        let year = start_year + (i as i32 / 12);
        let month = (i % 12) as u32 + 1;
        let date = Date::new(year as i16, month as i8, 15).unwrap();

        prices.push((
            date,
            Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO),
        ));

        if i < monthly_returns.len() {
            price *= 1.0 + monthly_returns[i];
        } else {
            // Cycle the returns.
            price *= 1.0 + monthly_returns[i % monthly_returns.len()];
        }
    }

    PriceSeries {
        symbol: symbol.to_string(),
        asset_class: "equity".to_string(),
        prices,
    }
}

#[test]
fn correlation_two_assets() {
    // Two assets with positively correlated returns.
    let returns_a = vec![
        0.02, 0.03, -0.01, 0.04, 0.01, -0.02, 0.03, 0.02, -0.01, 0.05, 0.01, -0.03, 0.02,
    ];
    // B moves in a similar direction but with different magnitude.
    let returns_b: Vec<f64> = returns_a.iter().map(|r| r * 0.8 + 0.005).collect();

    let series_a = make_price_series("AAPL", 2023, 14, 100.0, &returns_a);
    let series_b = make_price_series("MSFT", 2023, 14, 200.0, &returns_b);

    let config = AssetCorrelationConfig {
        min_overlap_months: 6,
    };

    let matrix = PortfolioAnalyzer::asset_correlation(&[series_a, series_b], &config);

    assert_eq!(matrix.labels, vec!["AAPL", "MSFT"]);
    assert_eq!(matrix.coefficients.len(), 2);

    // Diagonal should be 1.0.
    assert_eq!(matrix.coefficients[0][0], dec!(1));
    assert_eq!(matrix.coefficients[1][1], dec!(1));

    // Should be positively correlated (> 0.5).
    let corr = matrix.coefficients[0][1]
        .to_string()
        .parse::<f64>()
        .unwrap();
    assert!(
        corr > 0.5,
        "Expected positive correlation > 0.5, got {corr}"
    );

    // Matrix should be symmetric.
    assert_eq!(matrix.coefficients[0][1], matrix.coefficients[1][0]);
}

#[test]
fn correlation_single_asset_empty() {
    // Single price series → 1x1 matrix with diagonal 1.
    let series = make_price_series("AAPL", 2023, 14, 100.0, &[0.01, 0.02, -0.01]);
    let config = AssetCorrelationConfig::default();

    let matrix = PortfolioAnalyzer::asset_correlation(&[series], &config);

    assert_eq!(matrix.labels.len(), 1);
    assert_eq!(matrix.coefficients.len(), 1);
    assert_eq!(matrix.coefficients[0][0], dec!(1));
}

#[test]
fn correlation_diagonal_one() {
    // Three assets — verify all diagonal entries are 1.0.
    let returns_a = vec![
        0.02, 0.03, -0.01, 0.04, 0.01, -0.02, 0.03, 0.02, -0.01, 0.05, 0.01, -0.03, 0.02,
    ];
    let returns_b = vec![
        -0.01, 0.02, 0.04, -0.03, 0.02, 0.01, -0.01, 0.03, 0.02, -0.02, 0.04, 0.01, -0.01,
    ];
    let returns_c = vec![
        0.01, -0.02, 0.03, 0.01, -0.01, 0.02, -0.03, 0.01, 0.04, -0.01, 0.02, 0.03, 0.01,
    ];

    let series = vec![
        make_price_series("A", 2023, 14, 100.0, &returns_a),
        make_price_series("B", 2023, 14, 150.0, &returns_b),
        make_price_series("C", 2023, 14, 200.0, &returns_c),
    ];

    let config = AssetCorrelationConfig {
        min_overlap_months: 6,
    };

    let matrix = PortfolioAnalyzer::asset_correlation(&series, &config);

    assert_eq!(matrix.labels.len(), 3);
    assert_eq!(matrix.coefficients.len(), 3);

    // All diagonal entries must be 1.0.
    for i in 0..3 {
        assert_eq!(
            matrix.coefficients[i][i],
            dec!(1),
            "Diagonal [{i}][{i}] should be 1.0"
        );
    }

    // Matrix must be symmetric.
    for i in 0..3 {
        for j in 0..3 {
            assert_eq!(
                matrix.coefficients[i][j], matrix.coefficients[j][i],
                "Matrix should be symmetric at [{i}][{j}]"
            );
        }
    }
}

// ===========================================================================
// Property-based tests
// ===========================================================================

use proptest::prelude::*;

proptest! {
    #[test]
    fn drift_sums_to_zero(
        n_assets in 2usize..6
    ) {
        // Create holdings and targets with equal weights
        let weight = rust_decimal::Decimal::ONE / rust_decimal::Decimal::new(n_assets as i64, 0);
        let mut holdings = Vec::new();
        let mut targets = Vec::new();
        for i in 0..n_assets {
            let class = format!("asset_{i}");
            holdings.push(Holding {
                name: class.clone(),
                symbol: None,
                asset_class: class.clone(),
                current_value: rust_decimal::Decimal::new(10000 + (i as i64 * 1000), 0),
                cost_basis: rust_decimal::Decimal::new(9000, 0),
                quantity: rust_decimal::Decimal::ONE,
            });
            targets.push(AllocationTarget {
                asset_class: class,
                target_weight: weight,
                tolerance_band: rust_decimal::Decimal::new(5, 2),
            });
        }
        let result = PortfolioAnalyzer::allocation_drift(&holdings, &targets);
        let drift_sum: rust_decimal::Decimal = result.allocations.iter().map(|a| a.drift).sum();
        // Drift should sum to approximately zero (floating point)
        let abs_sum = drift_sum.abs();
        prop_assert!(abs_sum < rust_decimal::Decimal::new(1, 4), "Drift sum {} too large", abs_sum);
    }

    #[test]
    fn correlation_matrix_symmetric(
        n_months in 6usize..12
    ) {
        // Create two price series with enough months
        let base_date = date(2024, 1, 1);
        let series1 = PriceSeries {
            symbol: "A".to_string(),
            asset_class: "stocks".to_string(),
            prices: (0..n_months).map(|m| {
                let date = base_date.checked_add(jiff::Span::new().months(m as i64)).unwrap();
                (date, rust_decimal::Decimal::new(100 + m as i64, 0))
            }).collect(),
        };
        let series2 = PriceSeries {
            symbol: "B".to_string(),
            asset_class: "bonds".to_string(),
            prices: (0..n_months).map(|m| {
                let date = base_date.checked_add(jiff::Span::new().months(m as i64)).unwrap();
                (date, rust_decimal::Decimal::new(50 + m as i64 * 2, 0))
            }).collect(),
        };
        let config = AssetCorrelationConfig::default();
        let matrix = PortfolioAnalyzer::asset_correlation(&[series1, series2], &config);
        if matrix.labels.len() == 2 {
            prop_assert_eq!(matrix.coefficients[0][1], matrix.coefficients[1][0]);
        }
    }
}
