use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::types::ActivityEvent;

#[derive(Debug, Clone)]
pub struct ActivityEventRepo {
    pool: SqlitePool,
}

const INSERT_SQL: &str = r#"INSERT INTO activity_events (app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#;

fn bind_event<'a>(
    query: sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>>,
    event: &'a ActivityEvent,
) -> sqlx::query::Query<'a, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
    query
        .bind(&event.app_name)
        .bind(&event.window_title)
        .bind(&event.site_name)
        .bind(&event.bundle_id)
        .bind(&event.url)
        .bind(&event.category_id)
        .bind(event.started_at)
        .bind(event.ended_at)
        .bind(event.duration_secs)
        .bind(event.is_idle)
        .bind(&event.metadata)
        .bind(&event.project_id)
}

impl ActivityEventRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn insert(&self, event: &ActivityEvent) -> common::Result<i64> {
        let insert_returning = format!("{INSERT_SQL} RETURNING id");
        let row = bind_event(sqlx::query(&insert_returning), event)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.get::<i64, _>("id"))
    }

    pub async fn insert_batch(&self, events: &[ActivityEvent]) -> common::Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        for event in events {
            bind_event(sqlx::query(INSERT_SQL), event)
                .execute(&mut *tx)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
        limit: Option<i64>,
    ) -> common::Result<Vec<ActivityEvent>> {
        self.list_range_offset(start, end, limit, None).await
    }

    pub async fn list_range_offset(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> common::Result<Vec<ActivityEvent>> {
        let limit = limit.unwrap_or(10_000);
        let offset = offset.unwrap_or(0).max(0);
        let rows = sqlx::query_as::<_, ActivityEvent>(
            r#"SELECT id, app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2
               ORDER BY started_at ASC
               LIMIT ?3 OFFSET ?4"#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    /// Returns the most recent events (newest first), limited by `limit`.
    pub async fn list_recent(&self, limit: i64) -> common::Result<Vec<ActivityEvent>> {
        let rows = sqlx::query_as::<_, ActivityEvent>(
            r#"SELECT id, app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id
               FROM activity_events
               ORDER BY started_at DESC
               LIMIT ?1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn purge_before(&self, before: &DateTime<Utc>) -> common::Result<u64> {
        let result = sqlx::query("DELETE FROM activity_events WHERE started_at < ?1")
            .bind(before)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn count_context_switches(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> common::Result<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM (
                   SELECT app_name,
                          LAG(app_name) OVER (ORDER BY started_at) AS prev
                   FROM activity_events
                   WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE
               )
               WHERE prev IS NOT NULL AND app_name IS NOT prev"#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(count)
    }

    pub async fn aggregate_by_category(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> common::Result<Vec<(Option<String>, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            category_id: Option<String>,
            total_secs: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT category_id, COALESCE(SUM(duration_secs), 0) as total_secs
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE
               GROUP BY category_id
               ORDER BY total_secs DESC"#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.category_id, r.total_secs))
            .collect())
    }

    /// Returns top apps/sites grouped by `COALESCE(site_name, app_name)`.
    /// For browsers this gives site-level granularity (e.g. "YouTube" instead
    /// of "Google Chrome"); for native apps it stays as the app name.
    pub async fn top_apps(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
        limit: i64,
    ) -> common::Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            display_name: String,
            total_secs: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT COALESCE(site_name, app_name) AS display_name,
                      COALESCE(SUM(duration_secs), 0) AS total_secs
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE
               GROUP BY display_name
               ORDER BY total_secs DESC
               LIMIT ?3"#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.display_name, r.total_secs))
            .collect())
    }

    pub async fn total_idle_secs(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> common::Result<i64> {
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(duration_secs), 0)
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = TRUE"#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(total)
    }

    pub async fn total_active_secs(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> common::Result<i64> {
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(duration_secs), 0)
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE"#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(total)
    }
}
