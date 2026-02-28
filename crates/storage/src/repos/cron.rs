//! Cron job repository — cron_jobs table.

use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::cron::CronJobRow;

/// Repository for cron job persistence.
#[derive(Debug, Clone)]
pub struct CronRepo {
    pool: SqlitePool,
}

impl CronRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create or replace a cron job.
    pub async fn upsert(&self, row: &CronJobRow) -> Result<CronJobRow, StorageError> {
        let result = sqlx::query_as::<_, CronJobRow>(
            "INSERT INTO cron_jobs (id, name, enabled, schedule, payload, next_run_at_ms,
                                    last_run_at_ms, last_status, last_error,
                                    created_at_ms, updated_at_ms, delete_after_run)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (id) DO UPDATE SET
                 name = EXCLUDED.name,
                 enabled = EXCLUDED.enabled,
                 schedule = EXCLUDED.schedule,
                 payload = EXCLUDED.payload,
                 next_run_at_ms = EXCLUDED.next_run_at_ms,
                 last_run_at_ms = EXCLUDED.last_run_at_ms,
                 last_status = EXCLUDED.last_status,
                 last_error = EXCLUDED.last_error,
                 updated_at_ms = EXCLUDED.updated_at_ms,
                 delete_after_run = EXCLUDED.delete_after_run
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(row.enabled)
        .bind(&row.schedule)
        .bind(&row.payload)
        .bind(row.next_run_at_ms)
        .bind(row.last_run_at_ms)
        .bind(&row.last_status)
        .bind(&row.last_error)
        .bind(row.created_at_ms)
        .bind(row.updated_at_ms)
        .bind(row.delete_after_run)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get a cron job by ID.
    pub async fn get(&self, id: &str) -> Result<CronJobRow, StorageError> {
        sqlx::query_as::<_, CronJobRow>("SELECT * FROM cron_jobs WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_not_found(&format!("cron job '{}'", id))
    }

    /// List all cron jobs.
    pub async fn list(&self) -> Result<Vec<CronJobRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, CronJobRow>("SELECT * FROM cron_jobs ORDER BY created_at_ms ASC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// List only enabled cron jobs.
    pub async fn list_active(&self) -> Result<Vec<CronJobRow>, StorageError> {
        let rows = sqlx::query_as::<_, CronJobRow>(
            "SELECT * FROM cron_jobs WHERE enabled = TRUE ORDER BY next_run_at_ms ASC NULLS LAST",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Enable or disable a cron job.
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), StorageError> {
        let result = sqlx::query("UPDATE cron_jobs SET enabled = ?1 WHERE id = ?2")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("cron job '{}'", id)));
        }
        Ok(())
    }

    /// Update run state after execution.
    pub async fn update_run_state(
        &self,
        id: &str,
        last_run_at_ms: i64,
        next_run_at_ms: Option<i64>,
        last_status: &str,
        last_error: Option<&str>,
        updated_at_ms: i64,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE cron_jobs SET last_run_at_ms = ?1, next_run_at_ms = ?2,
                                  last_status = ?3, last_error = ?4, updated_at_ms = ?5
             WHERE id = ?6",
        )
        .bind(last_run_at_ms)
        .bind(next_run_at_ms)
        .bind(last_status)
        .bind(last_error)
        .bind(updated_at_ms)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("cron job '{}'", id)));
        }
        Ok(())
    }

    /// Delete a cron job by ID.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM cron_jobs WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
