//! Distillation retry queue — transient failure rehab.
//!
//! LLM timeouts, provider-open-circuit, and other soft errors enqueue a row
//! here. A periodic sweeper (Task 24) re-runs `distill_turn` for every due
//! row. Backoff: 1m, 5m, 30m, then permanent failure.

use common::{KlyntbotError, Result};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Why this turn is in the retry queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryReason {
    /// LLM call timed out.
    LlmTimeout,
    /// Provider returned an error (rate limit, circuit open, …).
    LlmProvider,
    /// Other transient — e.g. DB busy at write.
    Transient,
}

impl RetryReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::LlmTimeout => "llm_timeout",
            Self::LlmProvider => "llm_provider",
            Self::Transient => "transient",
        }
    }
}

/// One retry-queue row.
#[derive(Debug, Clone)]
pub struct RetryRow {
    /// Row id.
    pub id: String,
    /// Session.
    pub session_id: String,
    /// Turn.
    pub turn_id: Option<String>,
    /// How many attempts have been made.
    pub attempt_count: i64,
    /// Reason code.
    pub reason: String,
}

/// Repository for `ingest_distillation_retry`.
#[derive(Debug, Clone)]
pub struct DistillationRetryRepo {
    pool: SqlitePool,
}

impl DistillationRetryRepo {
    /// Construct over a SQLite pool.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Enqueue a turn for retry. Duplicates are allowed — each attempt gets a row.
    pub async fn enqueue(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        reason: RetryReason,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ingest_distillation_retry (id, session_id, turn_id, reason)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(session_id)
        .bind(turn_id)
        .bind(reason.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry enqueue: {e}")))?;
        Ok(())
    }

    /// List rows whose `next_due_at <= now`, up to `limit`.
    pub async fn list_due(&self, limit: i64) -> Result<Vec<RetryRow>> {
        let rows = sqlx::query(
            "SELECT id, session_id, turn_id, attempt_count, reason
             FROM ingest_distillation_retry
             WHERE next_due_at <= datetime('now')
             ORDER BY next_due_at ASC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry list: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| RetryRow {
                id: r.get("id"),
                session_id: r.get("session_id"),
                turn_id: r.get("turn_id"),
                attempt_count: r.get("attempt_count"),
                reason: r.get("reason"),
            })
            .collect())
    }

    /// Record a failed attempt. Backoff: 1m / 5m / 30m.
    pub async fn record_attempt(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE ingest_distillation_retry
             SET attempt_count = attempt_count + 1,
                 next_due_at = CASE attempt_count
                    WHEN 0 THEN datetime('now', '+1 minute')
                    WHEN 1 THEN datetime('now', '+5 minutes')
                    ELSE datetime('now', '+30 minutes')
                 END
             WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry attempt: {e}")))?;
        Ok(())
    }

    /// Remove a row — distillation succeeded.
    pub async fn mark_done(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM ingest_distillation_retry WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("retry done: {e}")))?;
        Ok(())
    }
}
