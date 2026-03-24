//! Retrieval audit trail for InsightForge.

use std::collections::HashMap;

/// Single audit entry capturing the full retrieval pipeline decisions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RetrievalAuditEntry {
    pub query: String,
    pub enriched_query: Option<String>,
    pub sub_queries: Vec<String>,
    pub sources_queried: Vec<String>,
    pub results_per_source: HashMap<String, usize>,
    pub final_results: usize,
    pub circuit_breaker_fallback: bool,
    pub total_latency_ms: u64,
}
