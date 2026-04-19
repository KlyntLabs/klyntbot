//! Repository for `held_notifications` — quiet-hours-suppressed deliveries awaiting release.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::held_notification::HeldNotificationRow;

#[derive(Debug, Clone)]
pub struct HeldNotificationsRepo {
    pool: SqlitePool,
}

impl HeldNotificationsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new held notification.
    pub async fn insert(&self, row: &HeldNotificationRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO held_notifications \
             (id, alarm_id, channels, payload, release_at_ms, released, held_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&row.id)
        .bind(&row.alarm_id)
        .bind(row.channels.to_string())
        .bind(row.payload.to_string())
        .bind(row.release_at_ms)
        .bind(row.released as i64)
        .bind(row.held_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all pending (unreleased) rows with `release_at_ms <= ts_ms`, oldest first.
    pub async fn list_pending_before(
        &self,
        ts_ms: i64,
    ) -> Result<Vec<HeldNotificationRow>, StorageError> {
        let rows = sqlx::query_as::<_, HeldNotificationRow>(
            "SELECT id, alarm_id, channels, payload, release_at_ms, released, held_at_ms \
             FROM held_notifications \
             WHERE released = 0 AND release_at_ms <= ?1 \
             ORDER BY release_at_ms ASC",
        )
        .bind(ts_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark a held notification as released.
    pub async fn mark_released(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE held_notifications SET released = 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Earliest pending `release_at_ms`, or `None` if no pending rows exist.
    pub async fn next_release_at_ms(&self) -> Result<Option<i64>, StorageError> {
        let result: Option<i64> = sqlx::query_scalar(
            "SELECT release_at_ms FROM held_notifications \
             WHERE released = 0 ORDER BY release_at_ms ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }
}
