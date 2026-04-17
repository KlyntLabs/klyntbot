//! Recurring charge detection for spending data.

use std::collections::BTreeMap;

use common::Decimal;
use jiff::{civil::Date, Span};
use serde::Serialize;

use crate::input_types::{RecurringFrequency, SpendingRecord, SpendingType};

use super::anomaly::SpendingAnalyzer;
use super::compute_median;

/// Calendar days from `from` to `to` (positive if `to` is later).
fn days_between(from: Date, to: Date) -> i64 {
    to.to_zoned(jiff::tz::TimeZone::UTC)
        .and_then(|zto| {
            from.to_zoned(jiff::tz::TimeZone::UTC)
                .map(|zfrom| (zto.timestamp().as_second() - zfrom.timestamp().as_second()) / 86400)
        })
        .unwrap_or(0)
}

/// Configuration for recurring charge detection.
#[derive(Debug, Clone)]
pub struct RecurringConfig {
    /// Minimum number of transactions to consider a charge recurring.
    pub min_occurrences: usize,
    /// Maximum percentage deviation from the median amount to consider consistent.
    pub amount_tolerance_pct: Decimal,
    /// Maximum number of days to look back from `as_of`.
    pub max_lookback_days: i64,
}

impl Default for RecurringConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 3,
            amount_tolerance_pct: Decimal::new(10, 2), // 0.10 = 10%
            max_lookback_days: 365,
        }
    }
}

/// A detected recurring charge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringCharge {
    pub counterparty: String,
    pub frequency: RecurringFrequency,
    pub average_amount: Decimal,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: Decimal,
    /// Estimated annual cost.
    pub annual_cost: Decimal,
    /// Date of the most recent transaction.
    pub last_date: Date,
    /// Whether the charge appears overdue based on expected interval.
    pub is_overdue: bool,
    /// Number of matching occurrences found.
    pub occurrences: usize,
}

impl SpendingAnalyzer {
    /// Detect recurring charges from spending records.
    ///
    /// Groups expenses by counterparty, checks amount consistency and interval
    /// regularity, and classifies the frequency of each recurring charge.
    pub fn detect_recurring(
        txs: &[SpendingRecord],
        config: &RecurringConfig,
        as_of: Date,
    ) -> Vec<RecurringCharge> {
        let cutoff = as_of
            .checked_sub(Span::new().days(config.max_lookback_days))
            .unwrap_or(as_of);

        // 1. Group expenses by counterparty (skip None)
        let mut by_counterparty: BTreeMap<String, Vec<&SpendingRecord>> = BTreeMap::new();
        for tx in txs {
            if tx.tx_type != SpendingType::Expense {
                continue;
            }
            if tx.date < cutoff {
                continue;
            }
            if let Some(ref cp) = tx.counterparty {
                by_counterparty.entry(cp.clone()).or_default().push(tx);
            }
        }

        let mut results = Vec::new();

        for (counterparty, mut txns) in by_counterparty {
            if txns.len() < config.min_occurrences {
                continue;
            }

            // Sort by date ascending
            txns.sort_by_key(|t| t.date);

            let amounts: Vec<Decimal> = txns.iter().map(|t| t.amount).collect();
            let median_amount = compute_median(&amounts);

            // Check amount consistency: count how many are within tolerance
            let consistent_count = amounts
                .iter()
                .filter(|&&a| {
                    if median_amount == Decimal::ZERO {
                        a == Decimal::ZERO
                    } else {
                        let deviation = (a - median_amount).abs() / median_amount;
                        deviation <= config.amount_tolerance_pct
                    }
                })
                .count();

            let amount_consistency =
                Decimal::new(consistent_count as i64, 0) / Decimal::new(amounts.len() as i64, 0);

            // Compute inter-transaction intervals in days
            let intervals: Vec<i64> = txns
                .windows(2)
                .map(|pair| days_between(pair[0].date, pair[1].date))
                .collect();

            if intervals.is_empty() {
                continue;
            }

            let median_interval = compute_median_i64(&intervals);

            // Classify frequency
            let frequency = classify_frequency(median_interval);

            // Compute interval regularity: what fraction of intervals are within 30% of median
            let interval_consistency = if median_interval > 0 {
                let tolerance = (median_interval as f64 * 0.3).max(1.0) as i64;
                let regular_count = intervals
                    .iter()
                    .filter(|&&i| (i - median_interval).abs() <= tolerance)
                    .count();
                Decimal::new(regular_count as i64, 0) / Decimal::new(intervals.len() as i64, 0)
            } else {
                Decimal::ZERO
            };

            // Confidence: weighted combination of amount and interval consistency
            let confidence = {
                let raw = amount_consistency * Decimal::new(4, 1)
                    + interval_consistency * Decimal::new(6, 1);
                // Clamp to [0, 1]
                if raw > Decimal::ONE {
                    Decimal::ONE
                } else if raw < Decimal::ZERO {
                    Decimal::ZERO
                } else {
                    raw
                }
            };

            // Average amount
            let sum: Decimal = amounts.iter().copied().sum();
            let average_amount = sum / Decimal::new(amounts.len() as i64, 0);

            // Annual cost
            let multiplier = frequency_multiplier(frequency);
            let annual_cost = average_amount * Decimal::new(multiplier, 0);

            // Last date
            let last_date = txns.last().map(|t| t.date).unwrap_or(as_of);

            // Is overdue: days since last > expected_interval * 1.5
            let days_since_last = days_between(last_date, as_of);
            let expected_interval = frequency_days(frequency);
            let is_overdue = days_since_last > (expected_interval * 3) / 2;

            results.push(RecurringCharge {
                counterparty,
                frequency,
                average_amount,
                confidence,
                annual_cost,
                last_date,
                is_overdue,
                occurrences: txns.len(),
            });
        }

        // Sort by annual cost descending
        results.sort_by(|a, b| {
            b.annual_cost
                .partial_cmp(&a.annual_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

/// Classify frequency from median interval in days.
fn classify_frequency(median_days: i64) -> RecurringFrequency {
    if median_days <= 10 {
        RecurringFrequency::Weekly
    } else if median_days <= 21 {
        RecurringFrequency::Biweekly
    } else if median_days <= 60 {
        RecurringFrequency::Monthly
    } else if median_days <= 180 {
        RecurringFrequency::Quarterly
    } else {
        RecurringFrequency::Annual
    }
}

/// Number of occurrences per year for a frequency.
fn frequency_multiplier(freq: RecurringFrequency) -> i64 {
    match freq {
        RecurringFrequency::Weekly => 52,
        RecurringFrequency::Biweekly => 26,
        RecurringFrequency::Monthly => 12,
        RecurringFrequency::Quarterly => 4,
        RecurringFrequency::Annual => 1,
    }
}

/// Expected interval in days for a frequency.
fn frequency_days(freq: RecurringFrequency) -> i64 {
    match freq {
        RecurringFrequency::Weekly => 7,
        RecurringFrequency::Biweekly => 14,
        RecurringFrequency::Monthly => 30,
        RecurringFrequency::Quarterly => 90,
        RecurringFrequency::Annual => 365,
    }
}

/// Compute the median of a slice of i64 values.
fn compute_median_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    }
}
