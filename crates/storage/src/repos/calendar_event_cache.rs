//! Calendar event cache repository — calendar_event_cache table.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::calendar::CalendarEventCacheRow;

/// Repository for cached calendar events.
#[derive(Debug, Clone)]
pub struct CalendarEventCacheRepo {
    pool: SqlitePool,
}

impl CalendarEventCacheRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a cached event by UID (across all providers).
    pub async fn get_by_uid(&self, uid: &str) -> Result<CalendarEventCacheRow, StorageError> {
        sqlx::query_as::<_, CalendarEventCacheRow>(
            "SELECT * FROM calendar_event_cache WHERE uid = ?1 LIMIT 1",
        )
        .bind(uid)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("cached event '{uid}'")))
    }

    /// List cached events for a provider, ordered by start time.
    pub async fn list_by_provider(
        &self,
        provider_id: &str,
    ) -> Result<Vec<CalendarEventCacheRow>, StorageError> {
        let rows = sqlx::query_as::<_, CalendarEventCacheRow>(
            "SELECT * FROM calendar_event_cache WHERE provider_id = ?1 ORDER BY start_at",
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List upcoming events (start >= now) across all providers, ordered by start time.
    pub async fn list_upcoming(
        &self,
        limit: i64,
    ) -> Result<Vec<CalendarEventCacheRow>, StorageError> {
        let rows = sqlx::query_as::<_, CalendarEventCacheRow>(
            "SELECT * FROM calendar_event_cache \
             WHERE start_at >= datetime('now') \
             ORDER BY start_at \
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a cached event (insert or update on conflict).
    #[allow(clippy::too_many_arguments)] // Maps 1:1 to table columns
    pub async fn upsert(
        &self,
        uid: &str,
        provider_id: &str,
        summary: &str,
        description: Option<&str>,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        source: &str,
        etag: Option<&str>,
        status: Option<&str>,
    ) -> Result<CalendarEventCacheRow, StorageError> {
        let row = sqlx::query_as::<_, CalendarEventCacheRow>(
            "INSERT INTO calendar_event_cache \
                (uid, provider_id, summary, description, start_at, end_at, source, etag, status, cached_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now')) \
             ON CONFLICT (uid, provider_id) DO UPDATE SET \
                 summary = EXCLUDED.summary, \
                 description = EXCLUDED.description, \
                 start_at = EXCLUDED.start_at, \
                 end_at = EXCLUDED.end_at, \
                 source = EXCLUDED.source, \
                 etag = EXCLUDED.etag, \
                 status = EXCLUDED.status, \
                 cached_at = EXCLUDED.cached_at \
             RETURNING *",
        )
        .bind(uid)
        .bind(provider_id)
        .bind(summary)
        .bind(description)
        .bind(start_at)
        .bind(end_at)
        .bind(source)
        .bind(etag)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete all cached events for a provider.
    pub async fn delete_by_provider(&self, provider_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM calendar_event_cache WHERE provider_id = ?1")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete a specific cached event.
    pub async fn delete(&self, uid: &str, provider_id: &str) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM calendar_event_cache WHERE uid = ?1 AND provider_id = ?2")
                .bind(uid)
                .bind(provider_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get the timestamp of the most recent cache entry for a provider.
    pub async fn last_cached_at(
        &self,
        provider_id: &str,
    ) -> Result<Option<DateTime<Utc>>, StorageError> {
        let row: (Option<DateTime<Utc>>,) = sqlx::query_as(
            "SELECT MAX(cached_at) FROM calendar_event_cache WHERE provider_id = ?1",
        )
        .bind(provider_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}
