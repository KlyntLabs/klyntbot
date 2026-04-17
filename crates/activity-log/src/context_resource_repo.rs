use jiff::Timestamp;
use storage::{StorageError, StoragePool};

use crate::types::WorkResource;
use crate::work_resource_repo::WrRawRow;

pub struct ContextResourceRepo;

impl ContextResourceRepo {
    pub async fn link(
        pool: &StoragePool,
        context_id: &str,
        resource_id: &str,
        relevance_score: f64,
    ) -> common::Result<()> {
        let now = Timestamp::now().to_string();
        sqlx::query(
            "INSERT INTO work_context_resources (context_id, resource_id, relevance_score, \
             first_associated_at, last_associated_at) \
             VALUES (?1,?2,?3,?4,?5) \
             ON CONFLICT(context_id, resource_id) DO UPDATE SET \
             relevance_score = excluded.relevance_score, \
             last_associated_at = excluded.last_associated_at",
        )
        .bind(context_id)
        .bind(resource_id)
        .bind(relevance_score)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn unlink(
        pool: &StoragePool,
        context_id: &str,
        resource_id: &str,
    ) -> common::Result<()> {
        sqlx::query(
            "DELETE FROM work_context_resources WHERE context_id = ?1 AND resource_id = ?2",
        )
        .bind(context_id)
        .bind(resource_id)
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn list_for_context(
        pool: &StoragePool,
        context_id: &str,
    ) -> common::Result<Vec<(WorkResource, f64)>> {
        let rows: Vec<WrWithScore> = sqlx::query_as(
            "SELECT r.id, r.resource_type, r.resource_name, r.resource_path, r.resource_uri, \
             r.first_seen_at, r.last_seen_at, r.access_count, r.embedding_id, \
             cr.relevance_score \
             FROM work_resources r \
             INNER JOIN work_context_resources cr ON cr.resource_id = r.id \
             WHERE cr.context_id = ?1 \
             ORDER BY cr.relevance_score DESC",
        )
        .bind(context_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let score = r.relevance_score;
                let raw = WrRawRow {
                    id: r.id,
                    resource_type: r.resource_type,
                    resource_name: r.resource_name,
                    resource_path: r.resource_path,
                    resource_uri: r.resource_uri,
                    first_seen_at: r.first_seen_at,
                    last_seen_at: r.last_seen_at,
                    access_count: r.access_count,
                    embedding_id: r.embedding_id,
                };
                (WorkResource::from(raw), score)
            })
            .collect())
    }

    pub async fn list_contexts_for_resource(
        pool: &StoragePool,
        resource_id: &str,
    ) -> common::Result<Vec<(String, f64)>> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT context_id, relevance_score FROM work_context_resources \
             WHERE resource_id = ?1 ORDER BY relevance_score DESC",
        )
        .bind(resource_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows)
    }

    pub async fn update_relevance(
        pool: &StoragePool,
        context_id: &str,
        resource_id: &str,
        relevance_score: f64,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE work_context_resources SET relevance_score = ?3, last_associated_at = ?4 \
             WHERE context_id = ?1 AND resource_id = ?2",
        )
        .bind(context_id)
        .bind(resource_id)
        .bind(relevance_score)
        .bind(Timestamp::now().to_string())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn count_for_context(pool: &StoragePool, context_id: &str) -> i64 {
        let row: Result<(i64,), _> =
            sqlx::query_as("SELECT COUNT(*) FROM work_context_resources WHERE context_id = ?1")
                .bind(context_id)
                .fetch_one(pool.inner())
                .await;
        row.map(|(c,)| c).unwrap_or(0)
    }

    pub async fn list_resource_ids_for_context(
        pool: &StoragePool,
        context_id: &str,
    ) -> common::Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT resource_id FROM work_context_resources WHERE context_id = ?1")
                .bind(context_id)
                .fetch_all(pool.inner())
                .await
                .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[derive(sqlx::FromRow)]
struct WrWithScore {
    id: String,
    resource_type: String,
    resource_name: String,
    resource_path: Option<String>,
    resource_uri: Option<String>,
    first_seen_at: String,
    last_seen_at: String,
    access_count: i64,
    embedding_id: Option<String>,
    relevance_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_context_repo::tests::make_context;
    use crate::work_context_repo::WorkContextRepo;
    use crate::work_resource_repo::tests::make_resource;
    use crate::work_resource_repo::WorkResourceRepo;

    #[tokio::test]
    async fn test_link_and_list() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Test");
        let res = make_resource("main.rs", None);
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        ContextResourceRepo::link(&pool, &ctx.id, &res.id, 0.8)
            .await
            .unwrap();

        let resources = ContextResourceRepo::list_for_context(&pool, &ctx.id)
            .await
            .unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].0.resource_name, "main.rs");
        assert!((resources[0].1 - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_unlink() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Test");
        let res = make_resource("lib.rs", None);
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        ContextResourceRepo::link(&pool, &ctx.id, &res.id, 0.5)
            .await
            .unwrap();
        ContextResourceRepo::unlink(&pool, &ctx.id, &res.id)
            .await
            .unwrap();

        let resources = ContextResourceRepo::list_for_context(&pool, &ctx.id)
            .await
            .unwrap();
        assert!(resources.is_empty());
    }

    #[tokio::test]
    async fn test_update_relevance() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Test");
        let res = make_resource("test.rs", None);
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        ContextResourceRepo::link(&pool, &ctx.id, &res.id, 0.5)
            .await
            .unwrap();
        ContextResourceRepo::update_relevance(&pool, &ctx.id, &res.id, 0.9)
            .await
            .unwrap();

        let resources = ContextResourceRepo::list_for_context(&pool, &ctx.id)
            .await
            .unwrap();
        assert!((resources[0].1 - 0.9).abs() < f64::EPSILON);
    }
}
