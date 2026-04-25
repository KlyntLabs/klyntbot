use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingMemoryStatusResponse {
    pub daemon_alive: bool,
    pub buffered_event_count: i64,
    pub unprocessed_event_count: i64,
    pub socket_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliHealthRow {
    pub cli: String,
    pub enabled: bool,
    pub last_event_at: Option<String>,
    pub event_count_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReplayEntry {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub kind: String,
    pub occurred_at: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnoseResult {
    pub ok: bool,
    pub message: String,
}

/// One row in the Memory Browser panel — flat triple from `semantic_facts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBrowserRow {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// One bucket in the Activity Timeline panel — daily ingest event count.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    /// ISO-8601 date (YYYY-MM-DD).
    pub date: String,
    pub count: i64,
}

/// Per-(date × model) cost row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdownRow {
    /// ISO-8601 date (YYYY-MM-DD).
    pub date: String,
    /// Model id, or `"unknown"` if `sessionStart` did not specify one.
    pub model: String,
    /// Source of the row: `"klynt-cli"` (already-priced) or `"hooks"` (derived).
    pub source: String,
    /// Number of assistant messages (only meaningful for `source = "hooks"`).
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_tokens: i64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub total_cost_usd: f64,
}

/// Cost Tracker response — per-row breakdown + aggregate totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub rows: Vec<CostBreakdownRow>,
    pub total_cost_usd: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cached_tokens: i64,
    pub total_requests: i64,
    /// Number of distinct (date, model) buckets.
    pub bucket_count: i64,
    /// Number of distinct models seen.
    pub model_count: i64,
    /// Number of distinct dates seen.
    pub day_count: i64,
}

/// One row in the Sensitivity Inspector panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityRow {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// `"public" | "redactable" | "secret"` — extracted from `metadata.sensitivity`
    /// when present, else `"public"`.
    pub sensitivity: String,
}
