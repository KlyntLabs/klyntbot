//! Idempotency gate for notification deliveries.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::notification_log::NotificationLogRow;

#[derive(Debug, Clone)]
pub struct NotificationLogRepo {
    pool: SqlitePool,
}

impl NotificationLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn try_insert(
        &self,
        alarm_id: &str,
        channel: &str,
        sent_at_ms: i64,
    ) -> Result<bool, StorageError> {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO notification_log (alarm_id, channel, sent_at_ms) \
             VALUES (?, ?, ?)",
        )
        .bind(alarm_id)
        .bind(channel)
        .bind(sent_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    pub async fn record_error(
        &self,
        alarm_id: &str,
        channel: &str,
        error: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE notification_log SET error = ? WHERE alarm_id = ? AND channel = ?")
            .bind(error)
            .bind(alarm_id)
            .bind(channel)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn record_ack(
        &self,
        alarm_id: &str,
        channel: &str,
        ack_at_ms: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE notification_log SET ack_at_ms = ? WHERE alarm_id = ? AND channel = ?")
            .bind(ack_at_ms)
            .bind(alarm_id)
            .bind(channel)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        alarm_id: &str,
        channel: &str,
    ) -> Result<Option<NotificationLogRow>, StorageError> {
        let row = sqlx::query_as::<_, NotificationLogRow>(
            "SELECT alarm_id, channel, sent_at_ms, ack_at_ms, error \
             FROM notification_log WHERE alarm_id = ? AND channel = ?",
        )
        .bind(alarm_id)
        .bind(channel)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}
