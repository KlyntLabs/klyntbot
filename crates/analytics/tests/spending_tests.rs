use analytics::spending::anomaly::SpendingAnalyzer;
use analytics::spending::{AnomalyConfig, CorrelationConfig, RecurringConfig, TrendConfig};
use analytics::{AnomalyDirection, SpendingRecord, SpendingType, TrendDirection};
use jiff::civil::{date, Date};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build monthly expense records in the same category, one per month starting Jan 2025.
fn monthly_expenses(category: &str, amounts: &[i64]) -> Vec<SpendingRecord> {
    amounts
        .iter()
        .enumerate()
        .map(|(i, &amt)| SpendingRecord {
            date: Date::new((2025) as i16, ((i as u32 % 12) + 1) as i8, (15) as i8).unwrap(),
            amount: Decimal::new(amt, 0),
            tx_type: SpendingType::Expense,
            category: Some(category.to_string()),
            counterparty: None,
        })
        .collect()
}

/// Build recurring expense records from a given counterparty on specific dates.
fn recurring_expenses(counterparty: &str, amount: i64, dates: &[Date]) -> Vec<SpendingRecord> {
    dates
        .iter()
        .map(|&d| SpendingRecord {
            date: d,
            amount: Decimal::new(amount, 0),
            tx_type: SpendingType::Expense,
            category: Some("subscriptions".to_string()),
            counterparty: Some(counterparty.to_string()),
        })
        .collect()
}

/// Build monthly expense records across two categories for correlation tests.
fn correlated_expenses(
    cat_a: &str,
    amounts_a: &[i64],
    cat_b: &str,
    amounts_b: &[i64],
) -> Vec<SpendingRecord> {
    let mut records = Vec::new();
    for (i, &amt) in amounts_a.iter().enumerate() {
        records.push(SpendingRecord {
            date: Date::new((2025) as i16, ((i as u32 % 12) + 1) as i8, (10) as i8).unwrap(),
            amount: Decimal::new(amt, 0),
            tx_type: SpendingType::Expense,
            category: Some(cat_a.to_string()),
            counterparty: None,
        });
    }
    for (i, &amt) in amounts_b.iter().enumerate() {
        records.push(SpendingRecord {
            date: Date::new((2025) as i16, ((i as u32 % 12) + 1) as i8, (20) as i8).unwrap(),
            amount: Decimal::new(amt, 0),
            tx_type: SpendingType::Expense,
            category: Some(cat_b.to_string()),
            counterparty: None,
        });
    }
    records
}

// ===========================================================================
// Anomaly detection tests
// ===========================================================================

#[test]
fn anomaly_spike_detected() {
    // 6 normal values around 100, then one huge spike at 500.
    let txs = monthly_expenses("food", &[100, 95, 105, 98, 102, 97, 500]);
    let config = AnomalyConfig {
        z_threshold: dec!(2.0),
        min_data_points: 5,
        direction: AnomalyDirection::SpikesOnly,
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(!anomalies.is_empty(), "should detect the spike");
    // The spike should be the 500 value.
    assert_eq!(anomalies[0].amount, dec!(500));
    assert!(anomalies[0].z_score > dec!(2.0));
}

#[test]
fn anomaly_insufficient_data_skipped() {
    // Only 3 data points (below default min_data_points of 5).
    let txs = monthly_expenses("food", &[100, 200, 100]);
    let config = AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(anomalies.is_empty(), "should skip with insufficient data");
}

#[test]
fn anomaly_zero_mad_handles_gracefully() {
    // All identical values — no anomalies should be detected.
    let txs = monthly_expenses("food", &[100, 100, 100, 100, 100]);
    let config = AnomalyConfig::default();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(
        anomalies.is_empty(),
        "identical values should produce no anomalies"
    );
}

#[test]
fn anomaly_zero_mad_with_deviation() {
    // All identical except one outlier — should detect it even with MAD=0.
    let txs = monthly_expenses("food", &[100, 100, 100, 100, 100, 500]);
    let config = AnomalyConfig {
        z_threshold: dec!(2.0),
        min_data_points: 5,
        direction: AnomalyDirection::SpikesOnly,
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(
        !anomalies.is_empty(),
        "should detect outlier when MAD is zero"
    );
    assert_eq!(anomalies[0].amount, dec!(500));
}

#[test]
fn anomaly_drops_only_mode() {
    // Normal values around 100, with one drop to 10.
    let txs = monthly_expenses("food", &[100, 95, 105, 98, 102, 97, 10]);
    let config = AnomalyConfig {
        z_threshold: dec!(2.0),
        min_data_points: 5,
        direction: AnomalyDirection::DropsOnly,
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(!anomalies.is_empty(), "should detect the drop");
    assert_eq!(anomalies[0].amount, dec!(10));
    // z-score should be negative for a drop.
    assert!(anomalies[0].z_score < Decimal::ZERO);
}

#[test]
fn anomaly_both_direction_mode() {
    // One spike and one drop.
    let txs = monthly_expenses("food", &[100, 95, 105, 98, 102, 97, 500, 10]);
    let config = AnomalyConfig {
        z_threshold: dec!(2.0),
        min_data_points: 5,
        direction: AnomalyDirection::Both,
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(
        anomalies.len() >= 2,
        "should detect both spike and drop, got {}",
        anomalies.len()
    );
}

#[test]
fn anomaly_income_records_excluded() {
    // All income — should produce no anomalies even if they look anomalous.
    let txs: Vec<SpendingRecord> = (0..7)
        .map(|i| SpendingRecord {
            date: Date::new((2025) as i16, ((i % 12) + 1) as i8, (15) as i8).unwrap(),
            amount: if i == 6 { dec!(10000) } else { dec!(100) },
            tx_type: SpendingType::Income,
            category: Some("salary".to_string()),
            counterparty: None,
        })
        .collect();
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &AnomalyConfig::default());
    assert!(anomalies.is_empty());
}

#[test]
fn anomaly_severity_levels() {
    // Use values designed to produce different severity levels.
    // With varied data so MAD > 0 and extreme spike gives High severity.
    let txs = monthly_expenses("food", &[90, 95, 100, 105, 110, 100, 5000]);
    let config = AnomalyConfig {
        z_threshold: dec!(1.0),
        min_data_points: 5,
        direction: AnomalyDirection::SpikesOnly,
    };
    let anomalies = SpendingAnalyzer::detect_anomalies(&txs, &config);
    assert!(!anomalies.is_empty());
    // The 5000 spike is extreme, should be high severity.
    assert_eq!(
        anomalies[0].severity,
        analytics::AnomalySeverity::High,
        "extreme spike should be High severity"
    );
}

// ===========================================================================
// Trend analysis tests
// ===========================================================================

#[test]
fn trends_monthly_totals_correct() {
    let txs = monthly_expenses("food", &[100, 200, 300]);
    let config = TrendConfig {
        window_months: 2,
        min_months: 2,
    };
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(report.monthly_totals.len(), 3);
    // Sorted by month key (BTreeMap): 2025-01, 2025-02, 2025-03
    assert_eq!(report.monthly_totals[0], ("2025-01".to_string(), dec!(100)));
    assert_eq!(report.monthly_totals[1], ("2025-02".to_string(), dec!(200)));
    assert_eq!(report.monthly_totals[2], ("2025-03".to_string(), dec!(300)));
}

#[test]
fn trends_increasing_detected() {
    // Steadily increasing spending.
    let txs = monthly_expenses("food", &[100, 120, 140, 160, 180, 200]);
    let config = TrendConfig::default(); // window=3, min=3
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(
        report.overall_direction,
        TrendDirection::Increasing,
        "steadily increasing spending should be classified as Increasing"
    );
}

#[test]
fn trends_decreasing_detected() {
    // Steadily decreasing spending.
    let txs = monthly_expenses("food", &[200, 180, 160, 140, 120, 100]);
    let config = TrendConfig::default();
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(
        report.overall_direction,
        TrendDirection::Decreasing,
        "steadily decreasing spending should be classified as Decreasing"
    );
}

#[test]
fn trends_stable_detected() {
    // Flat spending.
    let txs = monthly_expenses("food", &[100, 100, 100, 100, 100, 100]);
    let config = TrendConfig::default();
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(
        report.overall_direction,
        TrendDirection::Stable,
        "flat spending should be classified as Stable"
    );
}

#[test]
fn trends_insufficient_data() {
    // Only 2 months, below min_months=3.
    let txs = monthly_expenses("food", &[100, 200]);
    let config = TrendConfig::default();
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(report.overall_direction, TrendDirection::Stable);
    assert!(report.moving_average.is_empty());
    assert!(report.period_over_period.is_empty());
    assert!(report.category_trends.is_empty());
}

#[test]
fn trends_moving_average_computed() {
    let txs = monthly_expenses("food", &[100, 200, 300, 400]);
    let config = TrendConfig {
        window_months: 3,
        min_months: 3,
    };
    let report = SpendingAnalyzer::trends(&txs, &config);
    // MA(3) over [100,200,300,400]: first = (100+200+300)/3 = 200, second = (200+300+400)/3 = 300
    assert_eq!(report.moving_average.len(), 2);
    assert_eq!(report.moving_average[0].1, dec!(200));
    assert_eq!(report.moving_average[1].1, dec!(300));
}

#[test]
fn trends_period_over_period_change() {
    let txs = monthly_expenses("food", &[100, 200, 300]);
    let config = TrendConfig {
        window_months: 2,
        min_months: 2,
    };
    let report = SpendingAnalyzer::trends(&txs, &config);
    // 100 -> 200 = +100%
    assert_eq!(report.period_over_period.len(), 2);
    assert_eq!(report.period_over_period[0].1, dec!(100)); // +100%
    assert_eq!(report.period_over_period[1].1, dec!(50)); // 200->300 = +50%
}

#[test]
fn trends_category_breakdown() {
    let mut txs = monthly_expenses("food", &[100, 200, 300, 400]);
    txs.extend(monthly_expenses("transport", &[50, 50, 50, 50]));
    let config = TrendConfig {
        window_months: 2,
        min_months: 2,
    };
    let report = SpendingAnalyzer::trends(&txs, &config);
    assert_eq!(report.category_trends.len(), 2);
    let food_trend = report
        .category_trends
        .iter()
        .find(|ct| ct.category == "food")
        .expect("should have food trend");
    assert_eq!(food_trend.direction, TrendDirection::Increasing);
}

// ===========================================================================
// Recurring charge detection tests
// ===========================================================================

#[test]
fn recurring_monthly_detected() {
    let dates: Vec<Date> = (1..=6)
        .map(|m| Date::new((2025) as i16, (m) as i8, (1) as i8).unwrap())
        .collect();
    let txs = recurring_expenses("Netflix", 15, &dates);
    let config = RecurringConfig::default();
    let as_of = date(2025, 6, 15);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert_eq!(results.len(), 1);
    let netflix = &results[0];
    assert_eq!(netflix.counterparty, "Netflix");
    assert_eq!(netflix.frequency, analytics::RecurringFrequency::Monthly);
    assert_eq!(netflix.average_amount, dec!(15));
    assert_eq!(netflix.occurrences, 6);
}

#[test]
fn recurring_overdue_flagged() {
    // Monthly charge, last seen 3 months ago -> overdue.
    let dates: Vec<Date> = (1..=4)
        .map(|m| Date::new((2025) as i16, (m) as i8, (1) as i8).unwrap())
        .collect();
    let txs = recurring_expenses("Gym", 50, &dates);
    let config = RecurringConfig::default();
    // Check as of August — last payment was April 1, ~120 days ago, expected ~30 day interval.
    let as_of = date(2025, 8, 1);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert_eq!(results.len(), 1);
    assert!(results[0].is_overdue, "charge should be flagged as overdue");
}

#[test]
fn recurring_not_overdue_when_recent() {
    // Monthly charge, last seen recently -> not overdue.
    let dates: Vec<Date> = (1..=6)
        .map(|m| Date::new((2025) as i16, (m) as i8, (1) as i8).unwrap())
        .collect();
    let txs = recurring_expenses("Spotify", 10, &dates);
    let config = RecurringConfig::default();
    let as_of = date(2025, 6, 15);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert_eq!(results.len(), 1);
    assert!(
        !results[0].is_overdue,
        "recent charge should not be overdue"
    );
}

#[test]
fn recurring_insufficient_occurrences_skipped() {
    // Only 2 occurrences — below min_occurrences of 3.
    let dates = vec![
        date(2025, 1, 1),
        date(2025, 2, 1),
    ];
    let txs = recurring_expenses("SomeService", 20, &dates);
    let config = RecurringConfig::default();
    let as_of = date(2025, 3, 1);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert!(
        results.is_empty(),
        "should skip with insufficient occurrences"
    );
}

#[test]
fn recurring_annual_cost_correct() {
    // Weekly charge of $10 → annual cost should be $520.
    let dates: Vec<Date> = (0..8)
        .map(|w| date(2025, 1, 1) + jiff::Span::new().days(w * 7))
        .collect();
    let txs = recurring_expenses("WeeklyMeal", 10, &dates);
    let config = RecurringConfig {
        min_occurrences: 3,
        amount_tolerance_pct: dec!(0.10),
        max_lookback_days: 365,
    };
    let as_of = date(2025, 3, 1);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].frequency, analytics::RecurringFrequency::Weekly);
    assert_eq!(results[0].annual_cost, dec!(520)); // 10 * 52
}

#[test]
fn recurring_income_excluded() {
    // Income records should be excluded.
    let txs: Vec<SpendingRecord> = (1..=6)
        .map(|m| SpendingRecord {
            date: Date::new((2025) as i16, (m) as i8, (1) as i8).unwrap(),
            amount: dec!(5000),
            tx_type: SpendingType::Income,
            category: Some("salary".to_string()),
            counterparty: Some("Employer".to_string()),
        })
        .collect();
    let config = RecurringConfig::default();
    let as_of = date(2025, 7, 1);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert!(
        results.is_empty(),
        "income should be excluded from recurring detection"
    );
}

#[test]
fn recurring_no_counterparty_skipped() {
    // Records without counterparty should be ignored.
    let txs: Vec<SpendingRecord> = (1..=6)
        .map(|m| SpendingRecord {
            date: Date::new((2025) as i16, (m) as i8, (1) as i8).unwrap(),
            amount: dec!(100),
            tx_type: SpendingType::Expense,
            category: Some("misc".to_string()),
            counterparty: None,
        })
        .collect();
    let config = RecurringConfig::default();
    let as_of = date(2025, 7, 1);
    let results = SpendingAnalyzer::detect_recurring(&txs, &config, as_of);
    assert!(results.is_empty());
}

// ===========================================================================
// Correlation tests
// ===========================================================================

#[test]
fn correlation_two_categories() {
    // Perfectly correlated: both categories increase linearly.
    let txs = correlated_expenses(
        "food",
        &[100, 200, 300, 400, 500, 600],
        "transport",
        &[50, 100, 150, 200, 250, 300],
    );
    let config = CorrelationConfig { min_months: 3 };
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    assert_eq!(matrix.labels.len(), 2);
    assert_eq!(matrix.coefficients.len(), 2);
    // Correlation should be close to 1.0.
    let r = matrix.coefficients[0][1];
    assert!(
        r > dec!(0.99),
        "perfectly correlated series should have r > 0.99, got {r}"
    );
}

#[test]
fn correlation_single_category_empty() {
    let txs = monthly_expenses("food", &[100, 200, 300, 400, 500, 600]);
    let config = CorrelationConfig { min_months: 3 };
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    // Single category: matrix should be 1x1 with [1.0].
    assert_eq!(matrix.labels.len(), 1);
    assert_eq!(matrix.coefficients.len(), 1);
    assert_eq!(matrix.coefficients[0][0], dec!(1));
}

#[test]
fn correlation_diagonal_is_one() {
    let txs = correlated_expenses(
        "food",
        &[100, 200, 300, 400, 500, 600],
        "transport",
        &[50, 100, 150, 200, 250, 300],
    );
    let config = CorrelationConfig { min_months: 3 };
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    for i in 0..matrix.labels.len() {
        assert_eq!(
            matrix.coefficients[i][i],
            dec!(1),
            "diagonal element [{i}][{i}] should be 1.0"
        );
    }
}

#[test]
fn correlation_insufficient_shared_months() {
    // Two categories with only 2 shared months — below min_months=6.
    let mut txs = Vec::new();
    for m in 1..=2 {
        txs.push(SpendingRecord {
            date: Date::new((2025) as i16, (m) as i8, (10) as i8).unwrap(),
            amount: Decimal::new(100, 0),
            tx_type: SpendingType::Expense,
            category: Some("food".to_string()),
            counterparty: None,
        });
        txs.push(SpendingRecord {
            date: Date::new((2025) as i16, (m) as i8, (20) as i8).unwrap(),
            amount: Decimal::new(50, 0),
            tx_type: SpendingType::Expense,
            category: Some("transport".to_string()),
            counterparty: None,
        });
    }
    let config = CorrelationConfig::default(); // min_months=6
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    // Labels still present, but off-diagonal should be 0 (insufficient data).
    assert_eq!(matrix.labels.len(), 2);
    assert_eq!(matrix.coefficients[0][1], dec!(0));
    assert_eq!(matrix.coefficients[1][0], dec!(0));
}

#[test]
fn correlation_negative_correlation() {
    // One goes up, other goes down — should be negative.
    let txs = correlated_expenses(
        "food",
        &[100, 200, 300, 400, 500, 600],
        "transport",
        &[600, 500, 400, 300, 200, 100],
    );
    let config = CorrelationConfig { min_months: 3 };
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    let r = matrix.coefficients[0][1];
    assert!(
        r < dec!(-0.99),
        "negatively correlated series should have r < -0.99, got {r}"
    );
}

#[test]
fn correlation_empty_input() {
    let txs: Vec<SpendingRecord> = Vec::new();
    let config = CorrelationConfig::default();
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    assert!(matrix.labels.is_empty());
    assert!(matrix.coefficients.is_empty());
}

#[test]
fn correlation_symmetric() {
    let txs = correlated_expenses(
        "food",
        &[100, 200, 300, 400, 500, 600],
        "transport",
        &[50, 100, 150, 200, 250, 300],
    );
    let config = CorrelationConfig { min_months: 3 };
    let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
    let n = matrix.labels.len();
    for i in 0..n {
        for j in 0..n {
            assert_eq!(
                matrix.coefficients[i][j], matrix.coefficients[j][i],
                "matrix should be symmetric at [{i}][{j}]"
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
    fn correlation_matrix_is_symmetric(
        n_months in 6usize..13
    ) {
        // Create expenses in two categories
        let mut txs = Vec::new();
        for m in 0..n_months {
            let date = Date::new((2025) as i16, ((m as u32 % 12) + 1) as i8, (15) as i8).unwrap();
            txs.push(SpendingRecord {
                date,
                amount: rust_decimal::Decimal::new(100 + m as i64 * 10, 0),
                tx_type: SpendingType::Expense,
                category: Some("food".to_string()),
                counterparty: None,
            });
            txs.push(SpendingRecord {
                date,
                amount: rust_decimal::Decimal::new(50 + m as i64 * 5, 0),
                tx_type: SpendingType::Expense,
                category: Some("transport".to_string()),
                counterparty: None,
            });
        }
        let config = CorrelationConfig { min_months: 3 };
        let matrix = SpendingAnalyzer::category_correlation(&txs, &config);
        for i in 0..matrix.labels.len() {
            for j in 0..matrix.labels.len() {
                prop_assert_eq!(
                    matrix.coefficients[i][j], matrix.coefficients[j][i],
                    "Matrix not symmetric at [{}][{}]", i, j
                );
            }
        }
    }
}
