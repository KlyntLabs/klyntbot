use chrono::{DateTime, Utc};
use storage::{StorageError, StoragePool};

use crate::normalizers::parse_rfc3339;
use crate::types::{WorkContext, WorkContextStatus, WorkContextType};

const WC_COLS: &str = "id, title, description, status, context_type, embedding_id, \
     linked_project_id, color, tags, confidence, first_seen_at, last_active_at, \
     total_duration_secs, event_count, created_at, updated_at";

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

pub struct WorkContextRepo;

impl WorkContextRepo {
    pub async fn insert(pool: &StoragePool, ctx: &WorkContext) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO work_contexts (id, title, description, status, context_type, \
             embedding_id, linked_project_id, color, tags, confidence, first_seen_at, \
             last_active_at, total_duration_secs, event_count, created_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        )
        .bind(&ctx.id)
        .bind(&ctx.title)
        .bind(&ctx.description)
        .bind(ctx.status.as_str())
        .bind(ctx.context_type.as_str())
        .bind(&ctx.embedding_id)
        .bind(&ctx.linked_project_id)
        .bind(&ctx.color)
        .bind(tags_to_json(&ctx.tags))
        .bind(ctx.confidence)
        .bind(ctx.first_seen_at.to_rfc3339())
        .bind(ctx.last_active_at.to_rfc3339())
        .bind(ctx.total_duration_secs)
        .bind(ctx.event_count)
        .bind(ctx.created_at.to_rfc3339())
        .bind(ctx.updated_at.to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn get(pool: &StoragePool, id: &str) -> common::Result<Option<WorkContext>> {
        let row = sqlx::query_as::<_, WcRawRow>(&format!(
            "SELECT {WC_COLS} FROM work_contexts WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(row.map(Into::into))
    }

    pub async fn update(pool: &StoragePool, ctx: &WorkContext) -> common::Result<()> {
        sqlx::query(
            "UPDATE work_contexts SET title=?2, description=?3, status=?4, context_type=?5, \
             embedding_id=?6, linked_project_id=?7, color=?8, tags=?9, confidence=?10, \
             last_active_at=?11, total_duration_secs=?12, event_count=?13, updated_at=?14 \
             WHERE id=?1",
        )
        .bind(&ctx.id)
        .bind(&ctx.title)
        .bind(&ctx.description)
        .bind(ctx.status.as_str())
        .bind(ctx.context_type.as_str())
        .bind(&ctx.embedding_id)
        .bind(&ctx.linked_project_id)
        .bind(&ctx.color)
        .bind(tags_to_json(&ctx.tags))
        .bind(ctx.confidence)
        .bind(ctx.last_active_at.to_rfc3339())
        .bind(ctx.total_duration_secs)
        .bind(ctx.event_count)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn list_active(pool: &StoragePool) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(&format!(
            "SELECT {WC_COLS} FROM work_contexts WHERE status = 'active' \
             ORDER BY last_active_at DESC"
        ))
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_status(
        pool: &StoragePool,
        status: WorkContextStatus,
        limit: i64,
    ) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(&format!(
            "SELECT {WC_COLS} FROM work_contexts WHERE status = ?1 \
             ORDER BY last_active_at DESC LIMIT ?2"
        ))
        .bind(status.as_str())
        .bind(limit)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_project(
        pool: &StoragePool,
        project_id: &str,
    ) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(&format!(
            "SELECT {WC_COLS} FROM work_contexts WHERE linked_project_id = ?1 \
             ORDER BY last_active_at DESC"
        ))
        .bind(project_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn update_stats(
        pool: &StoragePool,
        id: &str,
        last_active_at: DateTime<Utc>,
        duration_increment: i64,
        event_increment: i64,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE work_contexts SET \
             last_active_at = ?2, \
             total_duration_secs = total_duration_secs + ?3, \
             event_count = event_count + ?4, \
             updated_at = ?5 \
             WHERE id = ?1",
        )
        .bind(id)
        .bind(last_active_at.to_rfc3339())
        .bind(duration_increment)
        .bind(event_increment)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn update_confidence(
        pool: &StoragePool,
        id: &str,
        confidence: f64,
    ) -> common::Result<()> {
        sqlx::query("UPDATE work_contexts SET confidence = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(confidence)
            .bind(Utc::now().to_rfc3339())
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn update_embedding(
        pool: &StoragePool,
        id: &str,
        embedding_id: &str,
    ) -> common::Result<()> {
        sqlx::query("UPDATE work_contexts SET embedding_id = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(embedding_id)
            .bind(Utc::now().to_rfc3339())
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn archive_dormant(pool: &StoragePool, dormancy_days: i64) -> common::Result<u64> {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::days(dormancy_days);
        let result = sqlx::query(
            "UPDATE work_contexts SET status = 'archived', updated_at = ?2 \
             WHERE status = 'active' AND last_active_at < ?1",
        )
        .bind(cutoff.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(result.rows_affected())
    }

    pub async fn merge(
        pool: &StoragePool,
        keep_id: &str,
        remove_id: &str,
        reason: &str,
    ) -> common::Result<()> {
        let mut tx = pool.inner().begin().await.map_err(StorageError::from)?;

        // Transfer resources
        sqlx::query(
            "UPDATE OR IGNORE work_context_resources SET context_id = ?1 WHERE context_id = ?2",
        )
        .bind(keep_id)
        .bind(remove_id)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        // Transfer actions
        sqlx::query(
            "UPDATE OR IGNORE work_context_actions SET context_id = ?1 WHERE context_id = ?2",
        )
        .bind(keep_id)
        .bind(remove_id)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        // Delete orphaned rows that conflicted
        sqlx::query("DELETE FROM work_context_resources WHERE context_id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;
        sqlx::query("DELETE FROM work_context_actions WHERE context_id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;

        // Delete removed context
        sqlx::query("DELETE FROM work_contexts WHERE id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;

        // Record merge event
        sqlx::query(
            "INSERT INTO context_merges (id, keep_id, remove_id, reason, merged_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(crate::normalizers::new_ulid())
        .bind(keep_id)
        .bind(remove_id)
        .bind(reason)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        tx.commit().await.map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn count_merges_since(
        pool: &StoragePool,
        since: DateTime<Utc>,
    ) -> common::Result<i64> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM context_merges WHERE merged_at >= ?1")
                .bind(since.to_rfc3339())
                .fetch_one(pool.inner())
                .await
                .map_err(StorageError::from)?;
        Ok(row.0)
    }

    pub async fn record_inference_run(pool: &StoragePool) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO inference_state (key, value) VALUES ('last_run_at', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn get_last_inference_run(pool: &StoragePool) -> common::Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM inference_state WHERE key = 'last_run_at'")
                .fetch_optional(pool.inner())
                .await
                .map_err(StorageError::from)?;
        Ok(row.map(|r| r.0))
    }

    pub async fn count_by_status(
        pool: &StoragePool,
        status: WorkContextStatus,
    ) -> common::Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM work_contexts WHERE status = ?1")
            .bind(status.as_str())
            .fetch_one(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(row.0)
    }

    pub async fn avg_confidence_active(pool: &StoragePool) -> common::Result<f64> {
        let row: (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(confidence), 0.0) FROM work_contexts WHERE status = 'active'",
        )
        .fetch_one(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(row.0)
    }

    pub async fn search_by_title(
        pool: &StoragePool,
        query: &str,
    ) -> common::Result<Vec<WorkContext>> {
        let pattern = format!("%{query}%");
        let rows = sqlx::query_as::<_, WcRawRow>(&format!(
            "SELECT {WC_COLS} FROM work_contexts WHERE title LIKE ?1 \
             ORDER BY last_active_at DESC"
        ))
        .bind(&pattern)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(sqlx::FromRow)]
struct WcRawRow {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    context_type: String,
    embedding_id: Option<String>,
    linked_project_id: Option<String>,
    color: Option<String>,
    tags: Option<String>,
    confidence: f64,
    first_seen_at: String,
    last_active_at: String,
    total_duration_secs: i64,
    event_count: i64,
    created_at: String,
    updated_at: String,
}

impl From<WcRawRow> for WorkContext {
    fn from(row: WcRawRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            status: WorkContextStatus::parse(&row.status).unwrap_or(WorkContextStatus::Active),
            context_type: WorkContextType::parse(&row.context_type)
                .unwrap_or(WorkContextType::General),
            embedding_id: row.embedding_id,
            linked_project_id: row.linked_project_id,
            color: row.color,
            tags: row
                .tags
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            confidence: row.confidence,
            first_seen_at: parse_rfc3339(&row.first_seen_at),
            last_active_at: parse_rfc3339(&row.last_active_at),
            total_duration_secs: row.total_duration_secs,
            event_count: row.event_count,
            created_at: parse_rfc3339(&row.created_at),
            updated_at: parse_rfc3339(&row.updated_at),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::normalizers::new_ulid;

    pub(crate) fn make_context(title: &str) -> WorkContext {
        let now = Utc::now();
        WorkContext {
            id: new_ulid(),
            title: title.to_string(),
            description: None,
            status: WorkContextStatus::Active,
            context_type: WorkContextType::Coding,
            embedding_id: None,
            linked_project_id: None,
            color: None,
            tags: vec![],
            confidence: 0.7,
            first_seen_at: now,
            last_active_at: now,
            total_duration_secs: 0,
            event_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Test Context");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().title, "Test Context");
    }

    #[tokio::test]
    async fn test_list_active() {
        let pool = crate::test_pool().await;
        let c1 = make_context("Active 1");
        let mut c2 = make_context("Archived");
        c2.status = WorkContextStatus::Archived;
        WorkContextRepo::insert(&pool, &c1).await.unwrap();
        WorkContextRepo::insert(&pool, &c2).await.unwrap();
        let active = WorkContextRepo::list_active(&pool).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].title, "Active 1");
    }

    #[tokio::test]
    async fn test_update_stats() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Stats Test");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        WorkContextRepo::update_stats(&pool, &ctx.id, Utc::now(), 300, 5)
            .await
            .unwrap();
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap().unwrap();
        assert_eq!(loaded.total_duration_secs, 300);
        assert_eq!(loaded.event_count, 5);
    }

    #[tokio::test]
    async fn test_archive_dormant() {
        let pool = crate::test_pool().await;
        let mut ctx = make_context("Old Context");
        ctx.last_active_at = Utc::now() - chrono::Duration::days(30);
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let archived = WorkContextRepo::archive_dormant(&pool, 7).await.unwrap();
        assert_eq!(archived, 1);
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, WorkContextStatus::Archived);
    }

    #[tokio::test]
    async fn test_merge_records_event_and_counts() {
        let pool = crate::test_pool().await;
        let c1 = make_context("Keep Me");
        let c2 = make_context("Remove Me");
        WorkContextRepo::insert(&pool, &c1).await.unwrap();
        WorkContextRepo::insert(&pool, &c2).await.unwrap();

        WorkContextRepo::merge(&pool, &c1.id, &c2.id, "inferred")
            .await
            .unwrap();

        // Removed context is gone
        assert!(WorkContextRepo::get(&pool, &c2.id).await.unwrap().is_none());

        // Merge event recorded
        let since = Utc::now() - chrono::Duration::hours(1);
        let count = WorkContextRepo::count_merges_since(&pool, since)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_search_by_title() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Coding: activity-log crate");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let results = WorkContextRepo::search_by_title(&pool, "activity")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }
}
