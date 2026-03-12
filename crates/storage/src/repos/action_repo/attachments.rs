//! Attachment operations for the `action_attachments` join table.

use crate::error::StorageError;
use crate::rows::action::ActionAttachmentRow;

use super::ActionRepo;

impl ActionRepo {
    /// Add an attachment to an action.
    pub async fn add_attachment(
        &self,
        action_id: &str,
        attachment_type: &str,
        value: &str,
        title: Option<&str>,
        tags: &[String],
    ) -> Result<ActionAttachmentRow, StorageError> {
        let id = uuid::Uuid::new_v4();
        let row = sqlx::query_as::<_, ActionAttachmentRow>(
            r#"
            INSERT INTO action_attachments (id, action_id, attachment_type, value, title, tags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(action_id)
        .bind(attachment_type)
        .bind(value)
        .bind(title)
        .bind(sqlx::types::Json(tags))
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Remove an attachment by its UUID.
    pub async fn remove_attachment(
        &self,
        action_id: &str,
        attachment_id: uuid::Uuid,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM action_attachments WHERE id = ?1 AND action_id = ?2")
            .bind(attachment_id)
            .bind(action_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all attachments for an action.
    pub async fn list_attachments(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionAttachmentRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionAttachmentRow>(
            "SELECT * FROM action_attachments WHERE action_id = ?1 ORDER BY created_at",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
