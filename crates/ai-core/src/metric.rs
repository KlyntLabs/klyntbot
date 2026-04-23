/// How a metric is aggregated over its sample window.
///
/// `Avg` is the natural fit for rates (0/1 samples) and bias metrics.
/// `Sum` is the natural fit for counts with a value (e.g. total amount transacted).
/// `Count` ignores the sample value and returns the number of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Avg,
    Sum,
    Count,
}

impl Aggregation {
    /// The SQL aggregate expression over the `value` column of `ai_metric_samples`.
    /// Used by `MetricRepo::aggregate_metric` — keep the string stable; tests pin it.
    pub const fn as_sql_expr(&self) -> &'static str {
        match self {
            Aggregation::Avg => "AVG(value)",
            Aggregation::Sum => "SUM(value)",
            Aggregation::Count => "CAST(COUNT(*) AS REAL)",
        }
    }
}

/// Compile-time spec for a behavioural metric, emitted by `#[derive(AiEvent)]` when
/// a variant carries `#[ai(metric(...))]`. The runtime registry is a `Vec<&'static MetricSpec>`;
/// there is never a heap-allocated `MetricSpec`.
#[derive(Debug, Clone, Copy)]
pub struct MetricSpec {
    pub name: &'static str,
    pub window_secs: u64,
    pub min_samples: u32,
    pub aggregation: Aggregation,
}

/// A single sample emitted into `AiSignal::metric_samples` by the generated `to_signal()`.
/// Copied into `ai_metric_samples` by `MetricHarvestConsumer`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricSample {
    pub name: &'static str,
    pub value: f64,
}
