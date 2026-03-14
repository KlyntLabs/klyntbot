//! Spending analytics — anomaly detection, trends, recurring charges, correlations.

pub mod anomaly;
pub mod correlation;
pub mod recurring;
pub mod trends;

pub use anomaly::{AnomalyConfig, SpendingAnalyzer};
pub use correlation::CorrelationConfig;
pub use recurring::RecurringConfig;
pub use trends::{TrendConfig, TrendReport};

use common::Decimal;

/// Compute the median of a slice of Decimals. Shared across spending sub-modules.
pub(crate) fn compute_median(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / Decimal::new(2, 0)
    }
}
