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
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ingest_event_log WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log count: {e}")))?;
        Ok(row.0)
    }

    /// Count unprocessed rows (buffered events awaiting distillation).
    pub async fn count_unprocessed(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM ingest_event_log WHERE processed = 0",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("ingest_event_log count_unprocessed: {e}")))?;
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
