//! `ingest_event_log` persistence — the append-only AgentEvent buffer
//! the daemon writes to and the Distiller (Phase 3) reads from.

use crate::event::{AgentEvent, EventKind};
use common::{KlyntbotError, Result};
use sqlx::{Row, SqlitePool};

/// A single decoded row from `ingest_event_log`.
#[derive(Debug, Clone)]
pub struct IngestEventLogRow {
    /// UUID (matches `AgentEventV1.id`).
    pub id: String,
    /// Source CLI name (`claude-code`, `codex`, ...).
    pub source: String,
    /// Session id as assigned by the CLI.
    pub session_id: String,
    /// Turn id when present.
    pub turn_id: Option<String>,
    /// Repo canonical id when resolved.
    pub repo_id: Option<String>,
    /// `EventKind` discriminant (`userPrompt`, `toolCall`, ...).
    pub kind: String,
    /// Serialized `AgentEvent` JSON.
    pub payload: String,
    /// Whether Distiller has consumed this row.
    pub processed: bool,
    /// RFC3339 occurred-at.
    pub occurred_at: String,
}

/// Repository for `ingest_event_log`.
#[derive(Debug, Clone)]
pub struct IngestEventLogRepo {
    pool: SqlitePool,
}

impl IngestEventLogRepo {
    /// Construct over a `SqlitePool`.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert one event. The `AgentEvent` is serialized as JSON into `payload`.
    pub async fn insert(&self, event: &AgentEvent) -> Result<()> {
        let AgentEvent::V1(v1) = event;
        let payload = serde_json::to_string(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize event: {e}")))?;
        let kind = event_kind_tag(&v1.kind);
        let repo_id = v1.repo.as_ref().map(|r| r.repo_id.clone());
        let cwd = v1.cwd.to_string_lossy().to_string();
        let occurred = v1.occurred_at.to_string();
        let source = agent_source_slug(v1.source);

        sqlx::query(
            "INSERT INTO ingest_event_log
             (id, source, session_id, turn_id, cwd, repo_id, occurred_at, kind, payload)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(v1.id.to_string())
        .bind(source)
        .bind(&v1.session_id)
        .bind(v1.turn_id.as_deref())
        .bind(cwd)
        .bind(repo_id)
        .bind(occurred)
        .bind(kind)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log insert: {e}")))?;
        Ok(())
    }

    /// Fetch up to `limit` unprocessed rows ordered by `received_at`.
    pub async fn list_unprocessed(&self, limit: i64) -> Result<Vec<IngestEventLogRow>> {
        let rows = sqlx::query(
            "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
             FROM ingest_event_log
             WHERE processed = 0
             ORDER BY received_at ASC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log list: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| IngestEventLogRow {
                id: r.get("id"),
                source: r.get("source"),
                session_id: r.get("session_id"),
                turn_id: r.get("turn_id"),
                repo_id: r.get("repo_id"),
                kind: r.get("kind"),
                payload: r.get("payload"),
                processed: r.get::<bool, _>("processed"),
                occurred_at: r.get("occurred_at"),
            })
            .collect())
    }

    /// Count rows for a session (processed + unprocessed).
    pub async fn count_by_session(&self, session_id: &str) -> Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ingest_event_log WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log count: {e}")))?;
        Ok(row.0)
    }

    /// Mark rows as processed by id. Returns rows affected.
    ///
    /// Idempotent — already-processed ids are no-ops. Empty input is a no-op.
    pub async fn mark_processed(&self, ids: &[&str]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("UPDATE ingest_event_log SET processed = 1 WHERE id IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for id in ids {
            q = q.bind(*id);
        }
        let res = q
            .execute(&self.pool)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log mark_processed: {e}")))?;
        Ok(res.rows_affected())
    }

    /// Fetch every event row for a given (session, turn) pair, ordered by `occurred_at`.
    /// `turn_id = None` matches rows where the column is NULL.
    pub async fn fetch_turn(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
    ) -> Result<Vec<IngestEventLogRow>> {
        let rows = match turn_id {
            Some(tid) => {
                sqlx::query(
                    "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
                     FROM ingest_event_log
                     WHERE session_id = ? AND turn_id = ?
                     ORDER BY occurred_at ASC",
                )
                .bind(session_id)
                .bind(tid)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
                     FROM ingest_event_log
                     WHERE session_id = ? AND turn_id IS NULL
                     ORDER BY occurred_at ASC",
                )
                .bind(session_id)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| KlyntbotError::Storage(format!("fetch_turn: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| IngestEventLogRow {
                id: r.get("id"),
                source: r.get("source"),
                session_id: r.get("session_id"),
                turn_id: r.get("turn_id"),
                repo_id: r.get("repo_id"),
                kind: r.get("kind"),
                payload: r.get("payload"),
                processed: r.get::<bool, _>("processed"),
                occurred_at: r.get("occurred_at"),
            })
            .collect())
    }

    /// Atomically flip `processing` from 0→1 for every row in the turn. Returns the count flipped.
    /// Already-processing rows are skipped — making the call idempotent.
    pub async fn mark_processing(&self, session_id: &str, turn_id: Option<&str>) -> Result<u64> {
        let res = match turn_id {
            Some(tid) => {
                sqlx::query(
                    "UPDATE ingest_event_log SET processing = 1
                     WHERE session_id = ? AND turn_id = ?
                       AND processed = 0 AND processing = 0",
                )
                .bind(session_id)
                .bind(tid)
                .execute(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "UPDATE ingest_event_log SET processing = 1
                     WHERE session_id = ? AND turn_id IS NULL
                       AND processed = 0 AND processing = 0",
                )
                .bind(session_id)
                .execute(&self.pool)
                .await
            }
        }
        .map_err(|e| KlyntbotError::Storage(format!("mark_processing: {e}")))?;
        Ok(res.rows_affected())
    }

    /// Mark a set of row ids as `processed=1, processing=0` — called after a successful distill cycle.
    pub async fn mark_processed_iter<'a, I: IntoIterator<Item = &'a str>>(
        &self,
        ids: I,
    ) -> Result<u64> {
        let ids: Vec<&str> = ids.into_iter().collect();
        if ids.is_empty() {
            return Ok(0);
        }
        let mut total: u64 = 0;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed tx: {e}")))?;
        for id in ids {
            let res = sqlx::query(
                "UPDATE ingest_event_log SET processed = 1, processing = 0 WHERE id = ?",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed row: {e}")))?;
            total += res.rows_affected();
        }
        tx.commit()
            .await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed commit: {e}")))?;
        Ok(total)
    }

    /// Latest distillation timestamp — `received_at` of the most recently
    /// processed row, in RFC3339. `None` if nothing has been distilled yet.
    pub async fn last_distilled_at(&self) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT MAX(received_at) FROM ingest_event_log WHERE processed = 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    KlyntbotError::Storage(format!("ingest_event_log last_distilled_at: {e}"))
                })?;
        Ok(row.and_then(|(ts,)| ts))
    }

    /// Count unprocessed rows (buffered events awaiting distillation).
    pub async fn count_unprocessed(&self) -> Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ingest_event_log WHERE processed = 0")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    KlyntbotError::Storage(format!("ingest_event_log count_unprocessed: {e}"))
                })?;
        Ok(row.0)
    }
}

fn event_kind_tag(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::SessionStart { .. } => "sessionStart",
        EventKind::SessionEnd { .. } => "sessionEnd",
        EventKind::UserPrompt { .. } => "userPrompt",
        EventKind::AssistantMsg { .. } => "assistantMsg",
        EventKind::ToolCall { .. } => "toolCall",
        EventKind::FileEdit { .. } => "fileEdit",
        EventKind::TestRun { .. } => "testRun",
        EventKind::CompactEvent { .. } => "compactEvent",
        EventKind::Error { .. } => "error",
        EventKind::SkillActivated { .. } => "skillActivated",
        EventKind::RecallInjected { .. } => "recallInjected",
        EventKind::ApprovalDecision { .. } => "approvalDecision",
        EventKind::SandboxApplied { .. } => "sandboxApplied",
        EventKind::FileEditEnriched { .. } => "fileEditEnriched",
        EventKind::TestRunEnriched { .. } => "testRunEnriched",
        EventKind::ProviderCall { .. } => "providerCall",
        EventKind::CompressionApplied { .. } => "compressionApplied",
        EventKind::MirrorAlert { .. } => "mirrorAlert",
        EventKind::SkillRoutingTrace { .. } => "skillRoutingTrace",
        EventKind::GitCommit { .. } => "gitCommit",
        EventKind::BackgroundJobLifecycle { .. } => "backgroundJobLifecycle",
        EventKind::BackgroundJobOutputBisect { .. } => "backgroundJobOutputBisect",
    }
}

fn agent_source_slug(src: crate::event::AgentSource) -> &'static str {
    use crate::event::AgentSource::*;
    match src {
        ClaudeCode => "claude-code",
        Codex => "codex",
        KimiCli => "kimi-cli",
        OpenCode => "opencode",
        KlyntCli => "klynt-cli",
    }
}
