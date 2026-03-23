use chrono::Utc;
use storage::{StorageError, StoragePool};

/// Links work contexts to tasks (formerly "actions" — renamed for consistency).
/// The underlying table `work_context_actions` still uses `action_id` as column name
/// but all values are task IDs from the `tasks` table.
pub struct ContextTaskRepo;

impl ContextTaskRepo {
    pub async fn link(pool: &StoragePool, context_id: &str, task_id: &str) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO work_context_actions (context_id, action_id, linked_at) \
             VALUES (?1,?2,?3) \
             ON CONFLICT(context_id, action_id) DO NOTHING",
        )
        .bind(context_id)
        .bind(task_id)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn unlink(
        pool: &StoragePool,
        context_id: &str,
        task_id: &str,
    ) -> common::Result<()> {
        sqlx::query("DELETE FROM work_context_actions WHERE context_id = ?1 AND action_id = ?2")
            .bind(context_id)
            .bind(task_id)
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn list_tasks_for_context(
        pool: &StoragePool,
        context_id: &str,
    ) -> common::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT action_id FROM work_context_actions WHERE context_id = ?1 ORDER BY linked_at DESC",
        )
        .bind(context_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn list_contexts_for_task(
        pool: &StoragePool,
        task_id: &str,
    ) -> common::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT context_id FROM work_context_actions WHERE action_id = ?1 ORDER BY linked_at DESC",
        )
        .bind(task_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::new_ulid;
    use crate::work_context_repo::tests::make_context;
    use crate::work_context_repo::WorkContextRepo;

    #[tokio::test]
    async fn test_link_and_list_tasks() {
        let pool = crate::test_pool().await;
        let ctx = make_context("Test");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let task_id = new_ulid();
        ContextTaskRepo::link(&pool, &ctx.id, &task_id)
            .await
            .unwrap();

        let tasks = ContextTaskRepo::list_tasks_for_context(&pool, &ctx.id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0], task_id);
    }

    #[tokio::test]
    async fn test_list_contexts_for_task() {
        let pool = crate::test_pool().await;
        let c1 = make_context("C1");
        let c2 = make_context("C2");
        WorkContextRepo::insert(&pool, &c1).await.unwrap();
        WorkContextRepo::insert(&pool, &c2).await.unwrap();
        let task_id = new_ulid();
        ContextTaskRepo::link(&pool, &c1.id, &task_id)
            .await
            .unwrap();
        ContextTaskRepo::link(&pool, &c2.id, &task_id)
            .await
            .unwrap();

        let contexts = ContextTaskRepo::list_contexts_for_task(&pool, &task_id)
            .await
            .unwrap();
        assert_eq!(contexts.len(), 2);
    }
}
