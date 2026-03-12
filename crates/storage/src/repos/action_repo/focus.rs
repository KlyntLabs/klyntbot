//! Focus-slot management for the `actions` table.

use chrono::{DateTime, Utc};

use crate::error::StorageError;
use crate::rows::action::ActionRow;

use super::ActionRepo;

impl ActionRepo {
    /// Focus an action. Returns true if the focus was set, false if at max_slots.
    pub async fn focus(
        &self,
        id: &str,
        max_slots: i64,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            r#"
            UPDATE actions
            SET focused_at = datetime('now'), focus_deadline = ?3, updated_at = datetime('now')
            WHERE id = ?1
              AND focused_at IS NULL
              AND (SELECT COUNT(*) FROM actions WHERE focused_at IS NOT NULL) < ?2
            "#,
        )
        .bind(id)
        .bind(max_slots)
        .bind(deadline)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Unfocus an action.
    pub async fn unfocus(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE actions SET focused_at = NULL, focus_deadline = NULL, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List currently focused actions.
    pub async fn list_focused(&self) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            "SELECT * FROM actions WHERE focused_at IS NOT NULL ORDER BY focused_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
