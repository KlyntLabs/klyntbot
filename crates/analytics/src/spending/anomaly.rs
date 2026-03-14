//! Anomaly detection for spending data using modified z-scores.

use common::Decimal;

use crate::input_types::{SpendingRecord, SpendingType};
use crate::types::{Anomaly, AnomalyDirection, AnomalySeverity};

use super::compute_median;

/// Configuration for anomaly detection.
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Z-score threshold beyond which a value is flagged as anomalous.
    pub z_threshold: Decimal,
    /// Minimum number of data points in a category before analysis is applied.
    pub min_data_points: usize,
    /// Which direction of anomalies to detect.
    pub direction: AnomalyDirection,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            z_threshold: Decimal::new(25, 1), // 2.5
            min_data_points: 5,
            direction: AnomalyDirection::default(),
        }
    }
}

/// Spending analyzer providing anomaly detection, trends, recurring charges, and correlations.
pub struct SpendingAnalyzer;

impl SpendingAnalyzer {
    /// Detect anomalous spending using the modified z-score method.
    ///
    /// Groups expenses by category, computes the modified z-score for each
    /// transaction, and flags those exceeding the configured threshold.
    pub fn detect_anomalies(txs: &[SpendingRecord], config: &AnomalyConfig) -> Vec<Anomaly> {
        use std::collections::BTreeMap;

        // 1. Filter expenses only
        let expenses: Vec<&SpendingRecord> = txs
            .iter()
            .filter(|t| t.tx_type == SpendingType::Expense)
            .collect();

        // 2. Group by category
        let mut by_category: BTreeMap<String, Vec<&SpendingRecord>> = BTreeMap::new();
        for tx in &expenses {
            if let Some(ref cat) = tx.category {
                by_category.entry(cat.clone()).or_default().push(tx);
            }
        }

        let mut anomalies = Vec::new();

        // 3. For each category with enough data points
        for (category, txns) in &by_category {
            if txns.len() < config.min_data_points {
                continue;
            }

            let amounts: Vec<Decimal> = txns.iter().map(|t| t.amount).collect();
            let median = compute_median(&amounts);
            let deviations: Vec<Decimal> =
                amounts.iter().map(|a| (*a - median).abs()).collect();
            let mad = compute_median(&deviations);

            // Constant for modified z-score: 0.6745
            let k = Decimal::new(6745, 4);

            for tx in txns {
                let diff = tx.amount - median;
                let z_score = if mad > Decimal::ZERO {
                    k * diff / mad
                } else {
                    // MAD == 0: all values are identical (or nearly so).
                    // Use mean absolute deviation instead.
                    let mean_dev = mean_absolute_deviation(&amounts);
                    if mean_dev > Decimal::ZERO {
                        k * diff / mean_dev
                    } else if diff != Decimal::ZERO {
                        // All values truly identical but this one differs — extreme anomaly
                        if diff > Decimal::ZERO {
                            Decimal::new(100, 0)
                        } else {
                            Decimal::new(-100, 0)
                        }
                    } else {
                        Decimal::ZERO
                    }
                };

                let is_anomaly = match config.direction {
                    AnomalyDirection::SpikesOnly => z_score > config.z_threshold,
                    AnomalyDirection::DropsOnly => z_score < Decimal::ZERO - config.z_threshold,
                    AnomalyDirection::Both => z_score.abs() > config.z_threshold,
                };

                if is_anomaly {
                    let abs_z = z_score.abs();
                    let severity = if abs_z > Decimal::new(5, 0) {
                        AnomalySeverity::High
                    } else if abs_z > Decimal::new(35, 1) {
                        AnomalySeverity::Medium
                    } else {
                        AnomalySeverity::Low
                    };

                    let direction_str = if z_score > Decimal::ZERO {
                        "above"
                    } else {
                        "below"
                    };

                    let explanation = format!(
                        "{} spending of {} is {direction_str} normal (z={z_score:.2}, median={})",
                        category, tx.amount, median,
                    );

                    anomalies.push(Anomaly {
                        date: tx.date,
                        category: category.clone(),
                        amount: tx.amount,
                        z_score,
                        severity,
                        explanation,
                    });
                }
            }
        }

        // Sort by |z_score| descending
        anomalies.sort_by(|a, b| {
            b.z_score
                .abs()
                .partial_cmp(&a.z_score.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        anomalies
    }
}

/// Compute the mean absolute deviation from the median.
fn mean_absolute_deviation(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }
    let median = compute_median(values);
    let sum: Decimal = values.iter().map(|v| (*v - median).abs()).sum();
    sum / Decimal::new(values.len() as i64, 0)
}
