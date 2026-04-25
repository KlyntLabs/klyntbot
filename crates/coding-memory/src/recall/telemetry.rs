//! Recall invocation telemetry — every passive/active recall lands a row here.
//!
//! Reads are paginated by `(session_id, occurred_at desc)` and `(repo_id, occurred_at desc)`;
//! the workbench Recall Tool Log panel + Session Replay overlay both consume this table.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use storage::StoragePool;
use uuid::Uuid;

/// One recall invocation row. `result_ids` and `metadata` round-trip as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallInvocationRow {
    /// Stable id.
    pub id: Uuid,
    /// When the recall fired.
    pub occurred_at: Timestamp,
    /// Optional session id.
    pub session_id: Option<String>,
    /// Optional turn id.
    pub turn_id: Option<String>,
    /// Optional repo scope.
    pub repo_id: Option<String>,
    /// Layer label — see migration comment for closed set.
    pub layer: String,
    /// Original query string.
    pub query: String,
    /// Coverage score if scoring ran.
    pub coverage_score: Option<f32>,
    /// CSV of skill names if escalation ran.
    pub skill_used: Option<String>,
    /// Wall-clock latency.
    pub latency_ms: i64,
    /// Memory ids returned.
    pub result_ids: Vec<Uuid>,
    /// Token count for inject layers.
    pub rendered_tokens: Option<i64>,
    /// Free-form metadata JSON.
    pub metadata: serde_json::Value,
}

/// Repo wrapper over `recall_invocations`.
#[derive(Debug, Clone)]
pub struct RecallInvocationRepo {
    pool: StoragePool,
}

impl RecallInvocationRepo {
    /// Construct.
    #[must_use]
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Insert a row.
    pub async fn insert(&self, row: &RecallInvocationRow) -> common::Result<()> {
        let result_ids = serde_json::to_string(&row.result_ids)
            .map_err(|e| common::KlyntbotError::Storage(format!("serialize: {e}")))?;
        let metadata = serde_json::to_string(&row.metadata)
            .map_err(|e| common::KlyntbotError::Storage(format!("serialize: {e}")))?;
        sqlx::query(
            "INSERT INTO recall_invocations
             (id, occurred_at, session_id, turn_id, repo_id, layer, query,
              coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(row.id.to_string())
        .bind(row.occurred_at.to_string())
        .bind(&row.session_id)
        .bind(&row.turn_id)
        .bind(&row.repo_id)
        .bind(&row.layer)
        .bind(&row.query)
        .bind(row.coverage_score.map(|v| v as f64))
        .bind(&row.skill_used)
        .bind(row.latency_ms)
        .bind(result_ids)
        .bind(row.rendered_tokens)
        .bind(metadata)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("insert recall_invocation: {e}")))?;
        Ok(())
    }

    /// List by session, paginated newest-first.
    pub async fn list_by_session(
        &self,
        session_id: &str,
        limit: i64,
        offset: i64,
    ) -> common::Result<Vec<RecallInvocationRow>> {
        let rows = sqlx::query_as::<_, RawRow>(
            "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                    coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
             FROM recall_invocations
             WHERE session_id = ?
             ORDER BY occurred_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("list recall_invocation: {e}")))?;
        rows.into_iter().map(RawRow::into_row).collect()
    }

    /// List recent invocations across all sessions, paginated.
    pub async fn list_recent(
        &self,
        limit: i64,
        offset: i64,
        layer_filter: Option<&str>,
    ) -> common::Result<Vec<RecallInvocationRow>> {
        let rows: Vec<RawRow> = if let Some(layer) = layer_filter {
            sqlx::query_as::<_, RawRow>(
                "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                        coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
                 FROM recall_invocations
                 WHERE layer = ?
                 ORDER BY occurred_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(layer)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.pool.inner())
            .await
        } else {
            sqlx::query_as::<_, RawRow>(
                "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                        coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
                 FROM recall_invocations
                 ORDER BY occurred_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.pool.inner())
            .await
        }
        .map_err(|e| common::KlyntbotError::Storage(format!("list recent: {e}")))?;
        rows.into_iter().map(RawRow::into_row).collect()
    }
}

#[derive(sqlx::FromRow)]
struct RawRow {
    id: String,
    occurred_at: String,
    session_id: Option<String>,
    turn_id: Option<String>,
    repo_id: Option<String>,
    layer: String,
    query: String,
    coverage_score: Option<f64>,
    skill_used: Option<String>,
    latency_ms: i64,
    result_ids: String,
    rendered_tokens: Option<i64>,
    metadata: String,
}

impl RawRow {
    fn into_row(self) -> common::Result<RecallInvocationRow> {
        Ok(RecallInvocationRow {
            id: self
                .id
                .parse()
                .map_err(|e| common::KlyntbotError::Storage(format!("uuid parse: {e}")))?,
            occurred_at: self
                .occurred_at
                .parse()
                .map_err(|e| common::KlyntbotError::Storage(format!("ts parse: {e}")))?,
            session_id: self.session_id,
            turn_id: self.turn_id,
            repo_id: self.repo_id,
            layer: self.layer,
            query: self.query,
            coverage_score: self.coverage_score.map(|v| v as f32),
            skill_used: self.skill_used,
            latency_ms: self.latency_ms,
            result_ids: serde_json::from_str(&self.result_ids)
                .map_err(|e| common::KlyntbotError::Storage(format!("ids parse: {e}")))?,
            rendered_tokens: self.rendered_tokens,
            metadata: serde_json::from_str(&self.metadata)
                .map_err(|e| common::KlyntbotError::Storage(format!("meta parse: {e}")))?,
        })
    }
}
