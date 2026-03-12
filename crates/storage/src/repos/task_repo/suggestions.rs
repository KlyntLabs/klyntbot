//! Task suggestion management operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskSuggestionRow;

impl TaskRepo {
    /// Create a suggestion record.
    pub async fn create_suggestion(
        &self,
        row: &TaskSuggestionRow,
    ) -> Result<TaskSuggestionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskSuggestionRow>(
            r#"
            INSERT INTO task_suggestions (
                id, task_id, suggestion_type, title, description,
                confidence, action_payload, status, trigger
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.suggestion_type)
        .bind(&row.title)
        .bind(&row.description)
        .bind(row.confidence)
        .bind(&row.action_payload)
        .bind(&row.status)
        .bind(&row.trigger)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Resolve a suggestion by updating its status and resolved_at timestamp.
    pub async fn resolve_suggestion(&self, id: &str, status: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE task_suggestions SET status = ?2, resolved_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a single pending suggestion by ID.
    pub async fn get_pending_suggestion(
        &self,
        id: &str,
    ) -> Result<Option<TaskSuggestionRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskSuggestionRow>(
            "SELECT * FROM task_suggestions WHERE id = ?1 AND status = 'pending'",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List pending suggestions, optionally filtered by task_id.
    pub async fn list_pending_suggestions(
        &self,
        task_id: Option<&str>,
    ) -> Result<Vec<TaskSuggestionRow>, StorageError> {
        if let Some(tid) = task_id {
            let rows = sqlx::query_as::<_, TaskSuggestionRow>(
                "SELECT * FROM task_suggestions WHERE status = 'pending' AND task_id = ?1 ORDER BY created_at DESC",
            )
            .bind(tid)
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        } else {
            let rows = sqlx::query_as::<_, TaskSuggestionRow>(
                "SELECT * FROM task_suggestions WHERE status = 'pending' ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        }
    }

    /// Expire all pending suggestions for a task.
    pub async fn expire_suggestions_for_task(&self, task_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE task_suggestions SET status = 'expired', resolved_at = datetime('now') WHERE task_id = ?1 AND status = 'pending'",
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
