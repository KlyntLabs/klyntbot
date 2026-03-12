//! Task hierarchy operations (parent/child, subtrees, move, cascade).

use std::collections::HashMap;

use super::TaskRepo;
use crate::error::{OptionExt, StorageError};
use crate::rows::task::TaskRow;

impl TaskRepo {
    /// Get immediate children of a task.
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE parent_id = ?1 ORDER BY created_at",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count immediate children, returning (total, completed) using the `completed` bool column.
    pub async fn count_children(&self, parent_id: &str) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) FROM tasks WHERE parent_id = ?1",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Bulk count immediate children for multiple parents, returning a map of id -> (total, completed).
    pub async fn count_children_bulk(
        &self,
        parent_ids: &[String],
    ) -> Result<HashMap<String, (i64, i64)>, StorageError> {
        if parent_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT parent_id, COUNT(*), SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) FROM tasks WHERE parent_id IN (",
        );
        let mut sep = qb.separated(", ");
        for id in parent_ids {
            sep.push_bind(id);
        }
        qb.push(") GROUP BY parent_id");

        let rows = qb
            .build_query_as::<(String, i64, i64)>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, total, done)| (id, (total, done)))
            .collect())
    }

    /// Get full subtree of a task (recursive CTE).
    pub async fn get_subtree(&self, root_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT * FROM tasks WHERE id = ?1
                UNION ALL
                SELECT t.* FROM tasks t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            SELECT * FROM subtree ORDER BY created_at
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Move a task to a new parent and/or project.
    pub async fn move_task(
        &self,
        id: &str,
        new_parent_id: Option<&str>,
        new_project_id: Option<&str>,
    ) -> Result<TaskRow, StorageError> {
        if let Some(parent_id) = new_parent_id {
            if self.would_create_parent_cycle(id, parent_id).await? {
                return Err(StorageError::Conflict(format!(
                    "Setting parent {parent_id} for task {id} would create a cycle"
                )));
            }
        }

        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            UPDATE tasks
            SET parent_id = ?2, project_id = ?3, updated_at = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new_parent_id)
        .bind(new_project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("task {id}"))?;

        Ok(row)
    }

    async fn would_create_parent_cycle(
        &self,
        child_id: &str,
        new_parent_id: &str,
    ) -> Result<bool, StorageError> {
        if child_id == new_parent_id {
            return Ok(true);
        }

        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ancestors AS (
                SELECT parent_id FROM tasks WHERE id = ?2
                UNION ALL
                SELECT t.parent_id FROM tasks t
                INNER JOIN ancestors a ON t.id = a.parent_id
                WHERE a.parent_id IS NOT NULL
            )
            SELECT EXISTS(SELECT 1 FROM ancestors WHERE parent_id = ?1)
            "#,
        )
        .bind(child_id)
        .bind(new_parent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    /// Mark a task and all children as completed recursively using the `completed` column.
    pub async fn cascade_complete(&self, root_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT id FROM tasks WHERE id = ?1
                UNION ALL
                SELECT t.id FROM tasks t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            UPDATE tasks SET completed = 1, status = 'done', completed_at = datetime('now'), updated_at = datetime('now')
            WHERE id IN (SELECT id FROM subtree) AND completed = 0
            "#,
        )
        .bind(root_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
