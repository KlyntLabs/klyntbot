use std::collections::HashMap;

use chrono::{DateTime, Utc};
use storage::{StorageError, StoragePool};

use crate::types::{ActivityActor, ActivityLogEntry, ActivitySource};

const INSERT_SQL: &str = r#"INSERT INTO unified_activity_log
    (id, timestamp, source, actor, resource_type, resource_id, resource_name,
     action, content_preview, content_hash, metadata, app_name, project_id,
     work_context_id, embedding_id, duration_secs, session_key, is_sensitive)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"#;

const SELECT_COLS: &str = "id, timestamp, source, actor, resource_type, resource_id, resource_name, action, content_preview, content_hash, metadata, app_name, project_id, work_context_id, embedding_id, duration_secs, session_key, is_sensitive";

pub struct ActivityLogRepo;

impl ActivityLogRepo {
    pub async fn insert(pool: &StoragePool, entry: &ActivityLogEntry) -> common::Result<()> {
        let metadata_str = entry.metadata.as_ref().map(|v| v.to_string());
        sqlx::query(INSERT_SQL)
            .bind(&entry.id)
            .bind(entry.timestamp.to_rfc3339())
            .bind(entry.source.as_str())
            .bind(entry.actor.as_str())
            .bind(&entry.resource_type)
            .bind(&entry.resource_id)
            .bind(&entry.resource_name)
            .bind(&entry.action)
            .bind(&entry.content_preview)
            .bind(&entry.content_hash)
            .bind(&metadata_str)
            .bind(&entry.app_name)
            .bind(&entry.project_id)
            .bind(&entry.work_context_id)
            .bind(&entry.embedding_id)
            .bind(entry.duration_secs)
            .bind(&entry.session_key)
            .bind(entry.is_sensitive)
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn insert_batch(
        pool: &StoragePool,
        entries: &[ActivityLogEntry],
    ) -> common::Result<usize> {
        let mut count = 0;
        let mut tx = pool.inner().begin().await.map_err(StorageError::from)?;

        for entry in entries {
            let metadata_str = entry.metadata.as_ref().map(|v| v.to_string());
            sqlx::query(INSERT_SQL)
                .bind(&entry.id)
                .bind(entry.timestamp.to_rfc3339())
                .bind(entry.source.as_str())
                .bind(entry.actor.as_str())
                .bind(&entry.resource_type)
                .bind(&entry.resource_id)
                .bind(&entry.resource_name)
                .bind(&entry.action)
                .bind(&entry.content_preview)
                .bind(&entry.content_hash)
                .bind(&metadata_str)
                .bind(&entry.app_name)
                .bind(&entry.project_id)
                .bind(&entry.work_context_id)
                .bind(&entry.embedding_id)
                .bind(entry.duration_secs)
                .bind(&entry.session_key)
                .bind(entry.is_sensitive)
                .execute(&mut *tx)
                .await
                .map_err(StorageError::from)?;
            count += 1;
        }

        tx.commit().await.map_err(StorageError::from)?;
        Ok(count)
    }

    pub async fn query_range(
        pool: &StoragePool,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE timestamp >= ?1 AND timestamp <= ?2 \
             ORDER BY timestamp DESC LIMIT ?3 OFFSET ?4"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn query_by_source(
        pool: &StoragePool,
        source: ActivitySource,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE source = ?1 AND timestamp >= ?2 AND timestamp <= ?3 \
             ORDER BY timestamp DESC LIMIT ?4"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(source.as_str())
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .bind(limit)
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn query_by_resource(
        pool: &StoragePool,
        resource_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE resource_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3 \
             ORDER BY timestamp DESC"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(resource_id)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn query_by_session(
        pool: &StoragePool,
        session_key: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE session_key = ?1 AND timestamp >= ?2 AND timestamp <= ?3 \
             ORDER BY timestamp DESC"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(session_key)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn query_by_project(
        pool: &StoragePool,
        project_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE project_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3 \
             ORDER BY timestamp DESC"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(project_id)
            .bind(start.to_rfc3339())
            .bind(end.to_rfc3339())
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn exists_by_hash(pool: &StoragePool, content_hash: &str) -> common::Result<bool> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM unified_activity_log WHERE content_hash = ?1)",
        )
        .bind(content_hash)
        .fetch_one(pool.inner())
        .await
        .map_err(StorageError::from)?;

        Ok(row.0)
    }

    pub async fn count_by_source(
        pool: &StoragePool,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> common::Result<HashMap<String, i64>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"SELECT source, COUNT(*) FROM unified_activity_log
               WHERE timestamp >= ?1 AND timestamp <= ?2
               GROUP BY source"#,
        )
        .bind(start.to_rfc3339())
        .bind(end.to_rfc3339())
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().collect())
    }

    pub async fn count_by_source_since(
        pool: &StoragePool,
        since: DateTime<Utc>,
    ) -> common::Result<HashMap<String, i64>> {
        Self::count_by_source(pool, since, Utc::now()).await
    }

    pub async fn query_unassigned(
        pool: &StoragePool,
        since: DateTime<Utc>,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE work_context_id IS NULL AND timestamp >= ?1 \
             ORDER BY timestamp ASC LIMIT 500"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(since.to_rfc3339())
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn set_work_context_id(
        pool: &StoragePool,
        event_id: &str,
        context_id: &str,
    ) -> common::Result<()> {
        sqlx::query("UPDATE unified_activity_log SET work_context_id = ?2 WHERE id = ?1")
            .bind(event_id)
            .bind(context_id)
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn query_by_context(
        pool: &StoragePool,
        context_id: &str,
        limit: i64,
    ) -> common::Result<Vec<ActivityLogEntry>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM unified_activity_log \
             WHERE work_context_id = ?1 \
             ORDER BY timestamp DESC LIMIT ?2"
        );
        let rows = sqlx::query_as::<_, RawRow>(&sql)
            .bind(context_id)
            .bind(limit)
            .fetch_all(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn delete_older_than(pool: &StoragePool, days: i64) -> common::Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let result = sqlx::query("DELETE FROM unified_activity_log WHERE timestamp < ?1")
            .bind(cutoff.to_rfc3339())
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;

        Ok(result.rows_affected())
    }
}

// Internal row type for sqlx::FromRow mapping
#[derive(sqlx::FromRow)]
struct RawRow {
    id: String,
    timestamp: String,
    source: String,
    actor: String,
    resource_type: Option<String>,
    resource_id: Option<String>,
    resource_name: Option<String>,
    action: String,
    content_preview: Option<String>,
    content_hash: Option<String>,
    metadata: Option<String>,
    app_name: Option<String>,
    project_id: Option<String>,
    work_context_id: Option<String>,
    embedding_id: Option<String>,
    duration_secs: Option<i64>,
    session_key: Option<String>,
    is_sensitive: bool,
}

impl From<RawRow> for ActivityLogEntry {
    fn from(row: RawRow) -> Self {
        Self {
            id: row.id,
            timestamp: chrono::DateTime::parse_from_rfc3339(&row.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            source: ActivitySource::parse(&row.source).unwrap_or(ActivitySource::DomainEvent),
            actor: ActivityActor::parse(&row.actor).unwrap_or(ActivityActor::System),
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            resource_name: row.resource_name,
            action: row.action,
            content_preview: row.content_preview,
            content_hash: row.content_hash,
            metadata: row.metadata.and_then(|s| serde_json::from_str(&s).ok()),
            app_name: row.app_name,
            project_id: row.project_id,
            work_context_id: row.work_context_id,
            embedding_id: row.embedding_id,
            duration_secs: row.duration_secs,
            session_key: row.session_key,
            is_sensitive: row.is_sensitive,
        }
    }
}
