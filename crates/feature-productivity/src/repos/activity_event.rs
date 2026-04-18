use sqlx::{Row, SqlitePool};
use tracing::warn;

use crate::types::ActivityEvent;

#[derive(sqlx::FromRow)]
struct ActivityEventRow {
    id: Option<i64>,
    app_name: String,
    window_title: Option<String>,
    site_name: Option<String>,
    bundle_id: Option<String>,
    url: Option<String>,
    category_id: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    duration_secs: Option<i64>,
    is_idle: bool,
    metadata: Option<String>,
    project_id: Option<String>,
    focus_session_id: Option<String>,
}

impl From<ActivityEventRow> for ActivityEvent {
    fn from(row: ActivityEventRow) -> Self {
        let started_at = common::parse_datetime_jiff(&row.started_at, "UTC").unwrap_or_else(|| {
            warn!(raw = %row.started_at, "unparseable started_at in activity_events, using now()");
            jiff::Timestamp::now()
        });
        let ended_at = row
            .ended_at
            .as_deref()
            .and_then(|s| common::parse_datetime_jiff(s, "UTC"));
        Self {
            id: row.id,
            app_name: row.app_name,
            window_title: row.window_title,
            site_name: row.site_name,
            bundle_id: row.bundle_id,
            url: row.url,
            category_id: row.category_id,
            started_at,
            ended_at,
            duration_secs: row.duration_secs,
            is_idle: row.is_idle,
            metadata: row.metadata,
            project_id: row.project_id,
            focus_session_id: row.focus_session_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivityEventRepo {
    pool: SqlitePool,
}

const INSERT_SQL: &str = r#"INSERT INTO activity_events (app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id, focus_session_id)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#;

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
        .bind(event.started_at.to_string())
        .bind(event.ended_at.map(|t| t.to_string()))
        .bind(event.duration_secs)
        .bind(event.is_idle)
        .bind(&event.metadata)
        .bind(&event.project_id)
        .bind(&event.focus_session_id)
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
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
        limit: Option<i64>,
    ) -> common::Result<Vec<ActivityEvent>> {
        self.list_range_offset(start, end, limit, None).await
    }

    pub async fn list_range_offset(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> common::Result<Vec<ActivityEvent>> {
        let limit = limit.unwrap_or(10_000);
        let offset = offset.unwrap_or(0).max(0);
        let rows = sqlx::query_as::<_, ActivityEventRow>(
            r#"SELECT id, app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id, focus_session_id
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2
               ORDER BY started_at ASC
               LIMIT ?3 OFFSET ?4"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(ActivityEvent::from).collect())
    }

    /// Returns the most recent events (newest first), limited by `limit`.
    pub async fn list_recent(&self, limit: i64) -> common::Result<Vec<ActivityEvent>> {
        let rows = sqlx::query_as::<_, ActivityEventRow>(
            r#"SELECT id, app_name, window_title, site_name, bundle_id, url, category_id, started_at, ended_at, duration_secs, is_idle, metadata, project_id, focus_session_id
               FROM activity_events
               ORDER BY started_at DESC
               LIMIT ?1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(ActivityEvent::from).collect())
    }

    pub async fn purge_before(&self, before: &jiff::Timestamp) -> common::Result<u64> {
        let result = sqlx::query("DELETE FROM activity_events WHERE started_at < ?1")
            .bind(before.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    pub async fn count_context_switches(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
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
        .bind(start.to_string())
        .bind(end.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(count)
    }

    pub async fn aggregate_by_category(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
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
        .bind(start.to_string())
        .bind(end.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.category_id, r.total_secs))
            .collect())
    }

    /// Aggregate activity seconds by category type (productive/neutral/distracting).
    /// JOINs with `activity_categories` to resolve category_id → category_type.
    /// Returns (productive_secs, neutral_secs, distracting_secs).
    pub async fn aggregate_by_type(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
    ) -> common::Result<(i64, i64, i64)> {
        #[derive(sqlx::FromRow)]
        struct Row {
            category_type: Option<String>,
            total_secs: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT ac.category_type, COALESCE(SUM(ae.duration_secs), 0) as total_secs
               FROM activity_events ae
               LEFT JOIN activity_categories ac ON ac.id = ae.category_id
               WHERE ae.started_at >= ?1 AND ae.started_at < ?2 AND ae.is_idle = FALSE
               GROUP BY ac.category_type"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let mut productive = 0i64;
        let mut neutral = 0i64;
        let mut distracting = 0i64;
        for r in rows {
            match r.category_type.as_deref() {
                Some("productive") => productive += r.total_secs,
                Some("distracting") => distracting += r.total_secs,
                _ => neutral += r.total_secs,
            }
        }
        Ok((productive, neutral, distracting))
    }

    /// Returns top apps/sites grouped by `COALESCE(site_name, app_name)`.
    /// For browsers this gives site-level granularity (e.g. "YouTube" instead
    /// of "Google Chrome"); for native apps it stays as the app name.
    pub async fn top_apps(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
        limit: i64,
    ) -> common::Result<Vec<(String, i64, Option<String>)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            display_name: String,
            total_secs: i64,
            category: Option<String>,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT COALESCE(e.site_name, e.app_name) AS display_name,
                      COALESCE(SUM(e.duration_secs), 0) AS total_secs,
                      MAX(c.name) AS category
               FROM activity_events e
               LEFT JOIN activity_categories c ON e.category_id = c.id
               WHERE e.started_at >= ?1 AND e.started_at < ?2 AND e.is_idle = FALSE
               GROUP BY display_name
               ORDER BY total_secs DESC
               LIMIT ?3"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.display_name, r.total_secs, r.category))
            .collect())
    }

    pub async fn total_idle_secs(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
    ) -> common::Result<i64> {
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(duration_secs), 0)
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = TRUE"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(total)
    }

    pub async fn top_projects(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
        limit: i64,
    ) -> common::Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            project_id: Option<String>,
            total_secs: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT project_id, COALESCE(SUM(duration_secs), 0) AS total_secs
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE
                 AND project_id IS NOT NULL
               GROUP BY project_id
               ORDER BY total_secs DESC
               LIMIT ?3"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.project_id.map(|pid| (pid, r.total_secs)))
            .collect())
    }

    pub async fn aggregate_by_project(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> common::Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            project_id: String,
            total_secs: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT project_id, COALESCE(SUM(duration_secs), 0) as total_secs
               FROM activity_events
               WHERE started_at >= ?1
                 AND started_at < date(?2, '+1 day')
                 AND project_id IS NOT NULL
               GROUP BY project_id
               ORDER BY total_secs DESC"#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| (r.project_id, r.total_secs))
            .collect())
    }

    /// Returns all distinct app/site combinations with their category name and total duration.
    pub async fn tracked_apps(&self) -> common::Result<Vec<TrackedAppRow>> {
        #[derive(Debug, Clone, sqlx::FromRow)]
        struct Row {
            display_name: String,
            app_name: String,
            site_name: Option<String>,
            category_id: Option<String>,
            category_name: Option<String>,
            total_secs: i64,
            event_count: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT
                   COALESCE(ae.site_name, ae.app_name) AS display_name,
                   ae.app_name,
                   ae.site_name,
                   ae.category_id,
                   ac.name AS category_name,
                   COALESCE(SUM(ae.duration_secs), 0) AS total_secs,
                   COUNT(*) AS event_count
               FROM activity_events ae
               LEFT JOIN activity_categories ac ON ac.id = ae.category_id
               WHERE ae.is_idle = FALSE
               GROUP BY ae.app_name, ae.site_name
               ORDER BY total_secs DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| TrackedAppRow {
                display_name: r.display_name,
                app_name: r.app_name,
                site_name: r.site_name,
                category_id: r.category_id,
                category_name: r.category_name,
                total_secs: r.total_secs,
                event_count: r.event_count,
            })
            .collect())
    }

    /// Re-assign all historical events for a given app/site to a new category.
    pub async fn recategorize_app(
        &self,
        app_name: &str,
        site_name: Option<&str>,
        new_category_id: &str,
    ) -> common::Result<u64> {
        let result = match site_name {
            Some(site) => {
                sqlx::query(
                    "UPDATE activity_events SET category_id = ?1 WHERE app_name = ?2 AND site_name = ?3",
                )
                .bind(new_category_id)
                .bind(app_name)
                .bind(site)
                .execute(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "UPDATE activity_events SET category_id = ?1 WHERE app_name = ?2 AND site_name IS NULL",
                )
                .bind(new_category_id)
                .bind(app_name)
                .execute(&self.pool)
                .await
            }
        };
        let rows = result
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
            .rows_affected();
        Ok(rows)
    }

    pub async fn total_active_secs(
        &self,
        start: &jiff::Timestamp,
        end: &jiff::Timestamp,
    ) -> common::Result<i64> {
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(duration_secs), 0)
               FROM activity_events
               WHERE started_at >= ?1 AND started_at < ?2 AND is_idle = FALSE"#,
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(total)
    }
}

#[derive(Debug, Clone)]
pub struct TrackedAppRow {
    pub display_name: String,
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub total_secs: i64,
    pub event_count: i64,
}
