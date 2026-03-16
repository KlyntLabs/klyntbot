use storage::StorageError;
use uuid::Uuid;

use super::{utc_now_str, NoteRepo};
use crate::models::InboxItemRow;

impl NoteRepo {
    pub async fn create_inbox_item(&self, content: &str) -> Result<InboxItemRow, StorageError> {
        let id = Uuid::new_v4().to_string();
        let now = utc_now_str();
        let result = sqlx::query_as::<_, InboxItemRow>(
            "INSERT INTO inbox_items (id, content, status, created_at)
             VALUES (?1, ?2, 'pending', ?3)
             RETURNING *",
        )
        .bind(&id)
        .bind(content)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn list_inbox_items(&self) -> Result<Vec<InboxItemRow>, StorageError> {
        let rows = sqlx::query_as::<_, InboxItemRow>(
            "SELECT * FROM inbox_items WHERE status = 'pending' ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete_inbox_item(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM inbox_items WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_inbox_items(&self) -> Result<i64, StorageError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM inbox_items WHERE status = 'pending'")
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }
}
