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

/// Args for `coding_memory_recall_index`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecallIndexArgs {
    pub query: String,
    pub repo: Option<String>,
    pub kinds: Option<Vec<String>>,
    pub days: Option<u32>,
    #[serde(default = "default_limit_20")]
    pub limit: u32,
}

/// Args for `coding_memory_recall_timeline`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecallTimelineArgs {
    pub query: String,
    pub repo: Option<String>,
    pub days: Option<u32>,
    #[serde(default = "default_limit_50_u32")]
    pub limit: u32,
}

/// Args for `coding_memory_recall_fetch`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecallFetchArgs {
    pub ids: Vec<String>,
    #[serde(default)]
    pub include_provenance: bool,
    #[serde(default)]
    pub include_causal_graph: bool,
}

/// Args for `coding_memory_check_dead_ends`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndArgs {
    pub problem: String,
    pub repo: Option<String>,
}

/// Args for `coding_memory_recall_facts_as_of`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FactsAsOfArgs {
    pub subject: String,
    pub predicate: Option<String>,
    /// RFC-3339 timestamp.
    pub as_of: String,
    pub repo: Option<String>,
}

/// Args for `coding_memory_recall_change_history`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryArgs {
    pub subject: String,
    pub predicate: String,
    pub repo: Option<String>,
    #[serde(default = "default_limit_50_u32")]
    pub limit: u32,
}

/// Args for `coding_memory_recall_decision_points`.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPointsArgs {
    pub query: String,
    pub repo: Option<String>,
    pub days: Option<u32>,
    #[serde(default = "default_limit_20")]
    pub limit: u32,
}

fn default_limit_20() -> u32 {
    20
}
fn default_limit_50_u32() -> u32 {
    50
}

/// One telemetry row in the Recall Log panel — mirrors
/// `coding_memory::RecallInvocationRow`. View-shaped DTO for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallToolInvocation {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub layer: String,
    pub query: String,
    pub repo: Option<String>,
    pub coverage_score: Option<f64>,
    pub duration_ms: i64,
    pub skill_used: Option<String>,
    pub created_at: String,
}

/// Paginated response for `coding_memory_recall_log`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallLogPage {
    pub rows: Vec<RecallToolInvocation>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Args for `coding_memory_recall_log`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecallLogArgs {
    pub layer: Option<String>,
    #[serde(default = "default_limit_50")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
fn default_limit_50() -> i64 {
    50
}

/// Args for `coding_memory_session_replay_recall_overlay`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecallOverlayArgs {
    pub session_id: String,
    #[serde(default = "default_limit_200")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
fn default_limit_200() -> i64 {
    200
}
