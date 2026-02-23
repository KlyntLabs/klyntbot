//! Calendar sync repository — calendar_sync_state table.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::calendar::CalendarSyncStateRow;

/// Repository for calendar sync state persistence.
#[derive(Debug, Clone)]
pub struct CalendarSyncRepo {
    pool: SqlitePool,
}

impl CalendarSyncRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get sync state for a provider.
    pub async fn get(&self, provider_id: &str) -> Result<CalendarSyncStateRow, StorageError> {
        sqlx::query_as::<_, CalendarSyncStateRow>(
            "SELECT * FROM calendar_sync_state WHERE provider_id = ?1",
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("calendar sync state '{}'", provider_id)))
    }

    /// Upsert sync state for a provider.
    pub async fn upsert(
        &self,
        provider_id: &str,
        sync_token: Option<&str>,
        last_sync_at: Option<DateTime<Utc>>,
    ) -> Result<CalendarSyncStateRow, StorageError> {
        let row = sqlx::query_as::<_, CalendarSyncStateRow>(
            "INSERT INTO calendar_sync_state (provider_id, sync_token, last_sync_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (provider_id) DO UPDATE SET
                 sync_token = EXCLUDED.sync_token,
                 last_sync_at = EXCLUDED.last_sync_at
             RETURNING *",
        )
        .bind(provider_id)
        .bind(sync_token)
        .bind(last_sync_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all sync states.
    pub async fn list(&self) -> Result<Vec<CalendarSyncStateRow>, StorageError> {
        let rows = sqlx::query_as::<_, CalendarSyncStateRow>(
            "SELECT * FROM calendar_sync_state ORDER BY provider_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete sync state for a provider.
    pub async fn delete(&self, provider_id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM calendar_sync_state WHERE provider_id = ?1")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
