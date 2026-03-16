//! Shared output types used across analytics modules.

use chrono::NaiveDate;
use common::Decimal;
use serde::Serialize;

/// Percentile bands from Monte Carlo or sensitivity analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentileBands {
    pub p5: Vec<Decimal>,
    pub p25: Vec<Decimal>,
    pub p50: Vec<Decimal>,
    pub p75: Vec<Decimal>,
    pub p95: Vec<Decimal>,
    pub survival_rate: Vec<Decimal>,
    pub labels: Vec<String>,
}

/// Generic time series for trend analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeries {
    pub points: Vec<(NaiveDate, Decimal)>,
    pub label: String,
}

/// Correlation matrix (spending categories or assets).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelationMatrix {
    pub labels: Vec<String>,
    pub coefficients: Vec<Vec<Decimal>>,
}

/// Severity of a detected anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

/// A detected anomaly in spending.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Anomaly {
    pub date: NaiveDate,
    pub category: String,
    pub amount: Decimal,
    pub z_score: Decimal,
    pub severity: AnomalySeverity,
    pub explanation: String,
}

/// Direction of anomaly detection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnomalyDirection {
    #[default]
    SpikesOnly,
    DropsOnly,
    Both,
}

/// Trend direction classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}
