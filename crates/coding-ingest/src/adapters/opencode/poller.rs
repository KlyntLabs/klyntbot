//! Long-lived task that polls an opencode SQLite DB at a configured interval.
//!
//! Diffs the `messages` table per session, emitting `AgentEvent`s into the
//! daemon's event channel. Persists `last_seen_id` in memory only (resets on
//! restart — acceptable for best-effort ingestion).

use crate::adapters::opencode::normalize;
use crate::event::AgentEvent;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// SQLite-backed poller for opencode.
pub struct OpencodePoller {
    db_path: PathBuf,
    interval: std::time::Duration,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    last_seen: Arc<AtomicI64>,
}

impl OpencodePoller {
    /// Create a new poller. Does not start until [`spawn`](Self::spawn) is called.
    pub fn new(
        db_path: PathBuf,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            db_path,
            interval,
            event_tx,
            last_seen: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Spawn the polling loop as a detached tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.poll_once().await {
                    tracing::warn!(error = %e, "opencode poll failed");
                }
            }
        })
    }

    async fn poll_once(&self) -> common::Result<()> {
        if !self.db_path.exists() {
            return Ok(());
        }
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&self.db_path)
            .read_only(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_with(opts)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("opencode connect: {e}")))?;

        let last = self.last_seen.load(Ordering::SeqCst);
        let rows: Vec<super::schema::MessageRow> = sqlx::query_as(
            "SELECT id, session_id, role, content, tool_calls, tool_call_id, metadata, created_at \
             FROM messages WHERE id > ?1 ORDER BY id ASC",
        )
        .bind(last)
        .fetch_all(&pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("opencode query: {e}")))?;

        let mut max_id = last;
        for row in rows {
            if row.id > max_id {
                max_id = row.id;
            }
            match normalize::row_to_event(row) {
                Ok(Some(v1)) => {
                    let _ = self.event_tx.send(AgentEvent::V1(v1));
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "opencode normalize failed"),
            }
        }

        self.last_seen.store(max_id, Ordering::SeqCst);
        Ok(())
    }
}
