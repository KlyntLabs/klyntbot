//! Task attachment operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskAttachmentRow;

impl TaskRepo {
    /// Add an attachment to a task.
    pub async fn add_attachment(
        &self,
        task_id: &str,
        attachment_type: &str,
        value: &str,
        title: Option<&str>,
        tags: &[String],
        source: &str,
    ) -> Result<TaskAttachmentRow, StorageError> {
        let id = uuid::Uuid::new_v4();
        let row = sqlx::query_as::<_, TaskAttachmentRow>(
            r#"
            INSERT INTO task_attachments (id, task_id, attachment_type, value, title, tags, source)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(task_id)
        .bind(attachment_type)
        .bind(value)
        .bind(title)
        .bind(sqlx::types::Json(tags))
        .bind(source)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Remove an attachment by its UUID.
    pub async fn remove_attachment(
        &self,
        task_id: &str,
        attachment_id: uuid::Uuid,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM task_attachments WHERE id = ?1 AND task_id = ?2")
            .bind(attachment_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all attachments for a task.
    pub async fn list_attachments(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskAttachmentRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskAttachmentRow>(
            "SELECT * FROM task_attachments WHERE task_id = ?1 ORDER BY created_at",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
