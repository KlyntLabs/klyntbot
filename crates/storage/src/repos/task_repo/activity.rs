//! Task activity log operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskActivityRow;

impl TaskRepo {
    /// Log an activity entry for a task.
    #[allow(clippy::too_many_arguments)]
    pub async fn log_activity(
        &self,
        task_id: &str,
        activity_type: &str,
        field_changed: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        actor_type: &str,
        summary: Option<&str>,
    ) -> Result<TaskActivityRow, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, TaskActivityRow>(
            r#"
            INSERT INTO task_activity (id, task_id, activity_type, field_changed, old_value, new_value, actor_type, summary)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(task_id)
        .bind(activity_type)
        .bind(field_changed)
        .bind(old_value)
        .bind(new_value)
        .bind(actor_type)
        .bind(summary)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List activity entries for a task, most recent first, up to `limit`.
    pub async fn list_activity(
        &self,
        task_id: &str,
        limit: i64,
    ) -> Result<Vec<TaskActivityRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskActivityRow>(
            r#"
            SELECT * FROM task_activity
            WHERE task_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
