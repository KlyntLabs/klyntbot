//! Long-lived task that polls an opencode SQLite DB at a configured interval.
//!
//! Diffs the singular `message` and `part` tables by `time_created`,
//! emitting `AgentEvent`s into the daemon's event channel. Persists
//! `last_seen_ms` in memory only (resets on restart — acceptable for
//! best-effort ingestion).

use crate::adapters::opencode::normalize;
use crate::event::AgentEvent;
use crate::store::IngestEventLogRepo;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// SQLite-backed poller for opencode.
pub struct OpencodePoller {
    db_path: PathBuf,
    interval: std::time::Duration,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    /// Repo for inserting events into `ingest_event_log`.
    repo: Arc<IngestEventLogRepo>,
    last_seen_ms: Arc<AtomicI64>,
}

impl OpencodePoller {
    /// Create a new poller. Does not start until [`spawn`](Self::spawn) is called.
    pub fn new(
        db_path: PathBuf,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        repo: Arc<IngestEventLogRepo>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            db_path,
            interval,
            event_tx,
            repo,
            last_seen_ms: Arc::new(AtomicI64::new(jiff::Timestamp::now().as_millisecond())),
        }
    }

    /// Spawn the polling loop as a detached tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = Self::ensure_indexes(&self.db_path).await {
                tracing::warn!(error = %e, "opencode ensure_indexes failed");
            }

            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&self.db_path)
                .read_only(true);
            let pool = match sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(2))
                .connect_with(opts)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "opencode pool connect failed, poller exiting");
                    return;
                }
            };

            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.poll_once(&pool).await {
                    tracing::warn!(error = %e, "opencode poll failed");
                }
            }
        })
    }

    /// Create indexes on the external opencode DB if they don't exist.
    /// Uses a brief writable connection; the indexes are harmless and
    /// idempotent, but they eliminate full-table scans when polling.
    async fn ensure_indexes(db_path: &PathBuf) -> common::Result<()> {
        if !db_path.exists() {
            return Ok(());
        }
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(false);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_with(opts)
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!("opencode ensure_indexes connect: {e}"))
            })?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_message_time_created ON message(time_created)")
            .execute(&pool)
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!(
                    "opencode create idx_message_time_created: {e}"
                ))
            })?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_part_message_id ON part(message_id)")
            .execute(&pool)
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!("opencode create idx_part_message_id: {e}"))
            })?;
        pool.close().await;
        Ok(())
    }

    async fn poll_once(&self, pool: &sqlx::SqlitePool) -> common::Result<()> {
        if !self.db_path.exists() {
            return Ok(());
        }

        let last = self.last_seen_ms.load(Ordering::SeqCst);

        // Pull every new message since the watermark.
        let messages: Vec<super::schema::MessageRow> = sqlx::query_as(
            "SELECT id, session_id, time_created, time_updated, data \
             FROM message WHERE time_created > ?1 ORDER BY time_created ASC",
        )
        .bind(last)
        .fetch_all(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("opencode message query: {e}")))?;

        if messages.is_empty() {
            return Ok(());
        }

        // Pull all parts for those messages in one query.
        let message_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
        let placeholders = vec!["?"; message_ids.len()].join(",");
        let parts_sql = format!(
            "SELECT id, message_id, session_id, time_created, time_updated, data \
             FROM part WHERE message_id IN ({placeholders}) ORDER BY time_created ASC",
        );
        let mut q = sqlx::query_as::<_, super::schema::PartRow>(&parts_sql);
        for id in &message_ids {
            q = q.bind(id);
        }
        let parts = q
            .fetch_all(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("opencode part query: {e}")))?;

        // Bucket parts by message_id.
        let mut parts_by_msg: HashMap<String, Vec<super::schema::PartRow>> = HashMap::new();
        for p in parts {
            parts_by_msg
                .entry(p.message_id.clone())
                .or_default()
                .push(p);
        }

        let mut max_ms = last;
        for msg in messages {
            if msg.time_created > max_ms {
                max_ms = msg.time_created;
            }
            let parts = parts_by_msg.remove(&msg.id).unwrap_or_default();
            match normalize::message_to_events(msg, parts) {
                Ok(events) => {
                    if !events.is_empty() {
                        tracing::info!(count = events.len(), "opencode emitting events");
                    }
                    for e in events {
                        let event = AgentEvent::V1(e);
                        if let Err(err) = self.repo.insert(&event).await {
                            tracing::warn!(error = %err, "opencode repo.insert failed");
                        }
                        let _ = self.event_tx.send(event);
                    }
                }
                Err(e) => tracing::warn!(error = %e, "opencode normalize failed"),
            }
        }

        self.last_seen_ms.store(max_ms, Ordering::SeqCst);
        Ok(())
    }
}
