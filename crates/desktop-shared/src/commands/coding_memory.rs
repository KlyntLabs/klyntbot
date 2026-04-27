use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CodingMemoryStatusResponse {
    pub daemon_alive: bool,
    pub buffered_event_count: i64,
    pub unprocessed_event_count: i64,
    pub socket_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CliHealthRow {
    pub cli: String,
    pub enabled: bool,
    pub last_event_at: Option<String>,
    pub event_count_24h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionReplayEntry {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub kind: String,
    pub occurred_at: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnoseResult {
    pub ok: bool,
    pub message: String,
}

/// One row in the Memory Browser panel — flat triple from `semantic_facts`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBrowserRow {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// One bucket in the Activity Timeline panel — daily ingest event count.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    /// ISO-8601 date (YYYY-MM-DD).
    pub date: String,
    pub count: i64,
}

/// Per-(date × model) cost row.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallIndexArgs {
    pub query: String,
    pub repo: Option<String>,
    pub kinds: Option<Vec<String>>,
    pub days: Option<u32>,
    #[serde(default = "default_limit_20")]
    pub limit: u32,
}

/// Args for `coding_memory_recall_timeline`. One of `ids` or `query` is required.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallTimelineArgs {
    pub ids: Option<Vec<String>>,
    pub query: Option<String>,
    pub repo: Option<String>,
    #[serde(default = "default_days_30")]
    pub days: u32,
}

/// Args for `coding_memory_recall_fetch`. `includeProvenance` defaults to true.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallFetchArgs {
    pub ids: Vec<String>,
    #[serde(default = "default_true")]
    pub include_provenance: bool,
    #[serde(default)]
    pub include_causal_graph: bool,
}

/// Args for `coding_memory_check_dead_ends`. `approach` is the candidate fix
/// being evaluated against historical FixAttempt outcomes.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndArgs {
    pub approach: String,
    pub repo: Option<String>,
}

/// Args for `coding_memory_recall_facts_as_of`.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FactsAsOfArgs {
    pub subject: String,
    pub predicate: String,
    /// RFC-3339 timestamp.
    pub as_of: String,
}

/// Args for `coding_memory_recall_change_history`.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryArgs {
    pub subject: String,
    pub predicate: String,
    pub repo: Option<String>,
    #[serde(default = "default_limit_50_u32")]
    pub limit: u32,
}

/// Args for `coding_memory_recall_decision_points`.
#[derive(Deserialize, Serialize, Debug, Clone, specta::Type)]
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
fn default_days_30() -> u32 {
    30
}
fn default_true() -> bool {
    true
}

/// One telemetry row in the Recall Log panel — mirrors
/// `coding_memory::RecallInvocationRow`. View-shaped DTO for the UI.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallLogPage {
    pub rows: Vec<RecallToolInvocation>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Args for `coding_memory_recall_log`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, specta::Type)]
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
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, specta::Type)]
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

/// Args for `coding_memory_mirror_alerts_feed`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertsFeedArgs {
    /// Filter by kind (string form of `CodingMirrorAlertKind`).
    pub kind: Option<String>,
    /// Filter by severity.
    pub severity: Option<String>,
    /// Filter by repo.
    pub repo: Option<String>,
    /// Pagination limit.
    pub limit: Option<u32>,
}

/// Row in the alerts feed.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertRow {
    /// Snippet id.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Severity.
    pub severity: String,
    /// Headline.
    pub headline: String,
    /// JSON payload.
    pub payload: String,
    /// When created (RFC 3339).
    pub created_at: String,
    /// Dismissed?.
    pub dismissed: bool,
}

/// Args for `coding_memory_mirror_alert_action`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertActionArgs {
    /// Alert id.
    pub id: String,
    /// `approve` | `reject` | `snooze`.
    pub action: String,
}

/// Bucket for the effectiveness chart.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessTrendBucket {
    /// ISO date.
    pub at: String,
    /// Effectiveness after.
    pub score: f32,
}

/// Response for `coding_memory_effectiveness_trends`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessTrendsResponse {
    /// Pattern id.
    pub pattern_id: String,
    /// Pattern name (from `procedural_rules.rule`).
    pub pattern_name: String,
    /// Buckets ordered oldest-first.
    pub buckets: Vec<EffectivenessTrendBucket>,
}

/// Args for `coding_memory_reforge_cycle_diff`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleDiffArgs {
    /// Repo id.
    pub repo_id: String,
    /// Artifact (`claude_md` | `agents_md` | `cursorrules` | `continue_rules`).
    pub artifact: String,
    /// Cycle id (left side).
    pub before_cycle_id: Option<String>,
    /// Cycle id (right side); `None` ⇒ current.
    pub after_cycle_id: Option<String>,
}

/// Response for `coding_memory_reforge_cycle_diff`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleDiffResponse {
    /// Left body.
    pub before_body: String,
    /// Right body.
    pub after_body: String,
    /// Section labels for color-coding the diff.
    pub section_labels: Vec<String>,
}

/// Cycle summary row.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleSummary {
    /// Cycle id.
    pub cycle_id: String,
    /// When run.
    pub ran_at: String,
    /// Repos affected.
    pub repos: Vec<String>,
    /// Artifacts written.
    pub artifacts_written: u32,
}

/// Project-skill row for the in-app skill listing.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSkillRow {
    /// Skill name.
    pub skill_name: String,
    /// Repo id.
    pub repo_id: String,
    /// Active version.
    pub active_version: i64,
    /// Status.
    pub status: String,
    /// Effectiveness score (live).
    pub effectiveness: f32,
}
