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

/// Workspace-global registry of `MetricSpec`s. Populated explicitly at startup
/// by app-core calling `register_all(Feature::FEATURE_METRICS)` for each feature.
/// Duplicate names are a programming error — fail fast at startup.
#[derive(Debug, Default)]
pub struct MetricRegistry {
    specs: Vec<&'static MetricSpec>,
}

impl MetricRegistry {
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    /// Registers a spec, silently skipping duplicates (same name).
    /// Different variants of the same enum may declare the same metric name
    /// with different value_from expressions (e.g. BudgetAlert vs TransactionRecorded
    /// both sampling budget_overrun_frequency with 1.0 and 0.0 respectively).
    pub fn register(&mut self, spec: &'static MetricSpec) {
        if self.specs.iter().any(|s| s.name == spec.name) {
            return; // skip duplicate
        }
        self.specs.push(spec);
    }

    pub fn register_all(&mut self, specs: &[&'static MetricSpec]) {
        for s in specs {
            self.register(s);
        }
    }

    pub fn all(&self) -> &[&'static MetricSpec] {
        &self.specs
    }

    pub fn get(&self, name: &str) -> Option<&'static MetricSpec> {
        self.specs.iter().copied().find(|s| s.name == name)
    }
}
