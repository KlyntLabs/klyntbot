//! Task decomposition plan operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskDecompositionRow;

impl TaskRepo {
    /// Create a decomposition plan record.
    pub async fn create_decomposition(
        &self,
        row: &TaskDecompositionRow,
    ) -> Result<TaskDecompositionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskDecompositionRow>(
            r#"
            INSERT INTO task_decompositions (id, task_id, plan, confidence, status, reasoning)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.plan)
        .bind(row.confidence)
        .bind(&row.status)
        .bind(&row.reasoning)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get a decomposition by ID.
    pub async fn get_decomposition(
        &self,
        id: &str,
    ) -> Result<Option<TaskDecompositionRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskDecompositionRow>(
            "SELECT * FROM task_decompositions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List pending decompositions for a task.
    pub async fn list_pending_decompositions(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskDecompositionRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskDecompositionRow>(
            "SELECT * FROM task_decompositions WHERE task_id = ?1 AND status = 'pending' ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark a decomposition as applied.
    pub async fn apply_decomposition(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE task_decompositions SET status = 'applied', applied_at = (unixepoch('now') * 1000) WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reject a pending decomposition.
    pub async fn reject_decomposition(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE task_decompositions SET status = 'rejected', applied_at = (unixepoch('now') * 1000) WHERE id = ?1 AND status = 'pending'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
