//! Repository for `scheduled_fires` — the canonical "when to fire" table.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::scheduled_fire::ScheduledFireRow;

#[derive(Debug, Clone)]
pub struct ScheduledFiresRepo {
    pool: SqlitePool,
}

impl ScheduledFiresRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &ScheduledFireRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO scheduled_fires
                 (id, fire_at_ms, kind, ref_id, payload, dedup_prefix,
                  fired, firing_started_at_ms, fired_at_ms, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, ?7)",
        )
        .bind(&row.id)
        .bind(row.fire_at_ms)
        .bind(&row.kind)
        .bind(&row.ref_id)
        .bind(row.payload.to_string())
        .bind(&row.dedup_prefix)
        .bind(row.created_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns fire_at_ms of the earliest pending fire, or None.
    pub async fn next_pending_fire_at_ms(&self) -> Result<Option<i64>, StorageError> {
        let result: Option<i64> = sqlx::query_scalar(
            "SELECT fire_at_ms FROM scheduled_fires WHERE fired = 0 ORDER BY fire_at_ms ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }

    /// List all pending rows with fire_at_ms <= cutoff, oldest first.
    pub async fn list_pending_up_to_ms(
        &self,
        cutoff_ms: i64,
    ) -> Result<Vec<ScheduledFireRow>, StorageError> {
        let rows = sqlx::query_as::<_, ScheduledFireRow>(
            "SELECT * FROM scheduled_fires
             WHERE fired = 0 AND fire_at_ms <= ?1
             ORDER BY fire_at_ms ASC",
        )
        .bind(cutoff_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Two-phase commit phase 1: claim a row for firing. Returns true if newly claimed,
    /// false if another worker already claimed it or it's already fired.
    pub async fn begin_firing(&self, id: &str, now_ms: i64) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE scheduled_fires SET firing_started_at_ms = ?2
             WHERE id = ?1 AND fired = 0 AND firing_started_at_ms IS NULL",
        )
        .bind(id)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Two-phase commit phase 2: mark as fired.
    pub async fn mark_fired(&self, id: &str, now_ms: i64) -> Result<(), StorageError> {
        sqlx::query("UPDATE scheduled_fires SET fired = 1, fired_at_ms = ?2 WHERE id = ?1")
            .bind(id)
            .bind(now_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Rows claimed but never marked fired — happens when the process crashed mid-dispatch.
    pub async fn list_in_flight(&self) -> Result<Vec<ScheduledFireRow>, StorageError> {
        let rows = sqlx::query_as::<_, ScheduledFireRow>(
            "SELECT * FROM scheduled_fires
             WHERE fired = 0 AND firing_started_at_ms IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete all pending fires whose dedup_prefix matches the given literal prefix.
    /// Returns count deleted.
    pub async fn cancel_by_prefix(&self, prefix: &str) -> Result<u64, StorageError> {
        let like = format!("{prefix}%");
        let result =
            sqlx::query("DELETE FROM scheduled_fires WHERE fired = 0 AND dedup_prefix LIKE ?1")
                .bind(like)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    /// Delete all pending fires with the given (kind, ref_id). Used for cron re-sync.
    pub async fn cancel_by_kind_ref(&self, kind: &str, ref_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM scheduled_fires WHERE fired = 0 AND kind = ?1 AND ref_id = ?2",
        )
        .bind(kind)
        .bind(ref_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
