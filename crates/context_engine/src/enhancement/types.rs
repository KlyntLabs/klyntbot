use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::RwLock;

/// Identifies which pipeline stage produced or transformed a query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuerySource {
    /// No enhancement applied — query passed through as-is.
    Passthrough,
    /// Query enriched with signals from retrieved memories / context.
    SignalEnrichment,
    /// Pseudo-relevance feedback: initial retrieval results used to expand query.
    PseudoRelevanceFeedback,
    /// Multiple query variants generated (LLM or heuristic).
    MultiQuery,
    /// Candidates re-ranked using heuristic scoring.
    HeuristicRerank,
    /// Candidates re-ranked using an LLM call.
    LlmRerank,
}

impl fmt::Display for QuerySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            QuerySource::Passthrough => "passthrough",
            QuerySource::SignalEnrichment => "signal_enrichment",
            QuerySource::PseudoRelevanceFeedback => "pseudo_relevance_feedback",
            QuerySource::MultiQuery => "multi_query",
            QuerySource::HeuristicRerank => "heuristic_rerank",
            QuerySource::LlmRerank => "llm_rerank",
        };
        write!(f, "{}", s)
    }
}

/// The query representation that flows through the enhancement pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryBundle {
    /// The raw user query, unchanged throughout the pipeline.
    pub original: String,
    /// The primary query used for retrieval (may be rewritten).
    pub primary: String,
    /// Additional query variants for multi-retrieval.
    pub variants: Vec<String>,
    /// Confidence in the primary query (0.0–1.0).
    pub confidence: f32,
    /// Which stages contributed to this bundle.
    pub sources: Vec<QuerySource>,
}

impl QueryBundle {
    /// Create a no-op bundle that passes the query through unchanged.
    pub fn passthrough(query: &str) -> Self {
        Self {
            original: query.to_string(),
            primary: query.to_string(),
            variants: Vec::new(),
            confidence: 0.0,
            sources: vec![QuerySource::Passthrough],
        }
    }
}

/// Constraints that limit how much work the enhancement pipeline may do.
#[derive(Debug, Clone)]
pub struct EnhancementBudget {
    /// Maximum wall-clock time allowed for the full pipeline.
    pub max_latency_ms: u64,
    /// Maximum number of LLM calls the pipeline may make.
    pub max_llm_calls: u32,
    /// Maximum number of tokens to spend on query expansion via LLM.
    pub max_expansion_tokens: usize,
}

impl Default for EnhancementBudget {
    /// Defaults to the Normal (fast, no LLM) budget.
    fn default() -> Self {
        Self::normal()
    }
}

impl EnhancementBudget {
    /// Standard budget: fast, no LLM calls, no token spend.
    pub fn normal() -> Self {
        Self {
            max_latency_ms: 100,
            max_llm_calls: 0,
            max_expansion_tokens: 0,
        }
    }

    /// Deep-think budget: moderate latency, up to 2 LLM calls.
    pub fn deep_think() -> Self {
        Self {
            max_latency_ms: 500,
            max_llm_calls: 2,
            max_expansion_tokens: 200,
        }
    }

    /// Ultra budget: generous latency, up to 4 LLM calls.
    pub fn ultra() -> Self {
        Self {
            max_latency_ms: 1000,
            max_llm_calls: 4,
            max_expansion_tokens: 400,
        }
    }
}

/// Outcome of a single pipeline stage execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    /// Stage ran and produced output.
    Ran,
    /// Stage was skipped (reason provided).
    Skipped(String),
    /// Stage attempted but encountered an error.
    Failed(String),
}

impl StageStatus {
    /// Flatten into a (status_label, optional_detail) pair for serialization
    /// to frontend or logs. The label is a stable kebab-case string.
    pub fn to_parts(&self) -> (&'static str, Option<&str>) {
        match self {
            Self::Ran => ("ran", None),
            Self::Skipped(r) => ("skipped", Some(r.as_str())),
            Self::Failed(r) => ("failed", Some(r.as_str())),
        }
    }
}

/// Diagnostic record for one stage in the enhancement pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTrace {
    /// Which stage this trace belongs to.
    pub name: QuerySource,
    /// How the stage resolved.
    pub status: StageStatus,
    /// Wall-clock time taken by the stage.
    pub latency_ms: u64,
    /// Number of LLM calls made by this stage.
    pub llm_calls: u32,
    /// Number of LLM tokens consumed by this stage.
    pub llm_tokens: u32,
    /// Short human-readable description of what the stage produced.
    pub output_summary: String,
}

/// Aggregated diagnostics for the entire enhancement pipeline run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnhancementTrace {
    /// Per-stage trace records, in execution order.
    pub stages: Vec<StageTrace>,
    /// Sum of all stage latencies.
    pub total_latency_ms: u64,
    /// Sum of all LLM calls across stages.
    pub total_llm_calls: u32,
    /// Sum of all LLM tokens across stages.
    pub total_llm_tokens: u32,
}

impl EnhancementTrace {
    /// Record a stage that ran successfully.
    pub fn record_success(
        &mut self,
        name: QuerySource,
        latency_ms: u64,
        llm_calls: u32,
        llm_tokens: u32,
        summary: impl Into<String>,
    ) {
        self.total_latency_ms += latency_ms;
        self.total_llm_calls += llm_calls;
        self.total_llm_tokens += llm_tokens;
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Ran,
            latency_ms,
            llm_calls,
            llm_tokens,
            output_summary: summary.into(),
        });
    }

    /// Record a stage that was skipped.
    pub fn record_skip(&mut self, name: QuerySource, reason: impl Into<String>) {
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Skipped(reason.into()),
            latency_ms: 0,
            llm_calls: 0,
            llm_tokens: 0,
            output_summary: String::new(),
        });
    }

    /// Record a stage that failed.
    pub fn record_failure(&mut self, name: QuerySource, latency_ms: u64, error: impl Into<String>) {
        self.total_latency_ms += latency_ms;
        self.stages.push(StageTrace {
            name,
            status: StageStatus::Failed(error.into()),
            latency_ms,
            llm_calls: 0,
            llm_tokens: 0,
            output_summary: String::new(),
        });
    }
}

/// Final output of the enhancement pipeline.
#[derive(Debug)]
pub struct EnhancementOutput {
    /// The (possibly enhanced) query bundle ready for retrieval.
    pub query: QueryBundle,
    /// Full diagnostic trace of the pipeline run.
    pub trace: EnhancementTrace,
}

impl EnhancementOutput {
    /// Create a no-op output that passes the query through unchanged.
    pub fn passthrough(query: &str) -> Self {
        Self {
            query: QueryBundle::passthrough(query),
            trace: EnhancementTrace::default(),
        }
    }
}

/// A snapshot of the most recent enhancement run, captured for inspection
/// via MCP / debug tooling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementTraceSnapshot {
    pub query: QueryBundle,
    pub trace: EnhancementTrace,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

/// Process-wide store for the most recent enhancement trace. The
/// [`crate::enhancement::QueryPipeline`] writes here on every `enhance()`
/// call; consumers (e.g. the `memory` MCP tool) read snapshots for debugging.
#[derive(Default)]
pub struct LatestEnhancementTrace {
    inner: RwLock<Option<EnhancementTraceSnapshot>>,
}

impl LatestEnhancementTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, snapshot: EnhancementTraceSnapshot) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = Some(snapshot);
        }
    }

    pub fn get(&self) -> Option<EnhancementTraceSnapshot> {
        self.inner.read().ok().and_then(|g| g.clone())
    }
}
