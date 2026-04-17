use chrono::{DateTime, Utc};
use common::time::bridge::jiff_to_chrono;
use sqlx::SqlitePool;
use tracing::warn;

use crate::types::TimeEntry;

#[derive(sqlx::FromRow)]
struct TimeEntryRow {
    id: Option<i64>,
    description: String,
    category_id: Option<String>,
    project_id: Option<String>,
    started_at: String,
    duration_secs: i64,
    source: String,
    created_at: String,
}

impl From<TimeEntryRow> for TimeEntry {
    fn from(row: TimeEntryRow) -> Self {
        let started_at = common::parse_datetime(&row.started_at, "UTC").unwrap_or_else(|| {
            warn!(raw = %row.started_at, "unparseable started_at in time_entries");
            jiff_to_chrono(jiff::Timestamp::now())
        });
        let created_at = common::parse_datetime(&row.created_at, "UTC").unwrap_or_else(|| {
            warn!(raw = %row.created_at, "unparseable created_at in time_entries");
            jiff_to_chrono(jiff::Timestamp::now())
        });
        Self {
            id: row.id,
            description: row.description,
            category_id: row.category_id,
            project_id: row.project_id,
            started_at,
            duration_secs: row.duration_secs,
            source: row.source,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeEntryRepo {
    pool: SqlitePool,
}

impl TimeEntryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, entry: &TimeEntry) -> common::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO time_entries (description, category_id, project_id, started_at, duration_secs, source)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               RETURNING id"#,
        )
        .bind(&entry.description)
        .bind(&entry.category_id)
        .bind(&entry.project_id)
        .bind(entry.started_at)
        .bind(entry.duration_secs)
        .bind(&entry.source)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn list_range(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> common::Result<Vec<TimeEntry>> {
        let rows = sqlx::query_as::<_, TimeEntryRow>(
            r#"SELECT id, description, category_id, project_id, started_at, duration_secs, source, created_at
               FROM time_entries
               WHERE started_at >= ?1 AND started_at < ?2
               ORDER BY started_at ASC"#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(TimeEntry::from).collect())
    }

    pub async fn delete(&self, id: i64) -> common::Result<bool> {
        let result = sqlx::query("DELETE FROM time_entries WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}
