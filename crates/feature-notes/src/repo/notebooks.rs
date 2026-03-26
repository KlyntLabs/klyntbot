use std::collections::HashMap;

use storage::StorageError;

use super::{nullable_to_sentinel, NoteRepo};
use crate::models::NotebookRow;

impl NoteRepo {
    // ── Notebooks ────────────────────────────────────

    pub async fn create_notebook(&self, row: &NotebookRow) -> Result<NotebookRow, StorageError> {
        let result = sqlx::query_as::<_, NotebookRow>(
            "INSERT INTO notebooks (id, parent_id, title, icon, color, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.parent_id)
        .bind(&row.title)
        .bind(&row.icon)
        .bind(&row.color)
        .bind(row.sort_order)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn list_notebooks(&self) -> Result<Vec<NotebookRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, NotebookRow>("SELECT * FROM notebooks ORDER BY sort_order, title")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn update_notebook(
        &self,
        id: &str,
        title: Option<&str>,
        icon: Option<Option<&str>>,
        color: Option<Option<&str>>,
        parent_id: Option<Option<&str>>,
    ) -> Result<NotebookRow, StorageError> {
        let pid_sentinel = nullable_to_sentinel(parent_id);
        let icon_sentinel = nullable_to_sentinel(icon);
        let color_sentinel = nullable_to_sentinel(color);
        let row = sqlx::query_as::<_, NotebookRow>(
            "UPDATE notebooks SET
                title = COALESCE(?2, title),
                icon = CASE
                    WHEN ?3 IS NULL THEN icon
                    WHEN ?3 = '' THEN NULL
                    ELSE ?3
                END,
                color = CASE
                    WHEN ?4 IS NULL THEN color
                    WHEN ?4 = '' THEN NULL
                    ELSE ?4
                END,
                parent_id = CASE
                    WHEN ?5 IS NULL THEN parent_id
                    WHEN ?5 = '' THEN NULL
                    ELSE ?5
                END,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(title)
        .bind(icon_sentinel)
        .bind(color_sentinel)
        .bind(pid_sentinel)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn count_notes_in_notebook(&self, notebook_id: &str) -> Result<i64, StorageError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM notes WHERE notebook_id = ?1 AND archived = 0")
                .bind(notebook_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count.0)
    }

    /// Count notes per notebook in a single query. Returns notebook_id → count.
    pub async fn count_notes_by_notebook(&self) -> Result<HashMap<String, i64>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT notebook_id, COUNT(*) FROM notes WHERE notebook_id IS NOT NULL AND archived = 0 GROUP BY notebook_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn delete_notebook(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM notebooks WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Check if setting `notebook_id`'s parent to `proposed_parent_id` would create a cycle.
    /// Returns `true` if a cycle would be created.
    pub async fn would_create_cycle(
        &self,
        notebook_id: &str,
        proposed_parent_id: &str,
    ) -> Result<bool, StorageError> {
        // Trivial self-cycle
        if notebook_id == proposed_parent_id {
            return Ok(true);
        }
        // Walk up the ancestor chain from proposed_parent_id.
        // If we encounter notebook_id, setting it as parent would create a cycle.
        let result = sqlx::query_scalar::<_, i32>(
            "WITH RECURSIVE ancestors(nid) AS (
                SELECT parent_id FROM notebooks WHERE id = ?1
                UNION ALL
                SELECT nb.parent_id FROM notebooks nb JOIN ancestors a ON nb.id = a.nid
                WHERE nb.parent_id IS NOT NULL
            )
            SELECT 1 FROM ancestors WHERE nid = ?2 LIMIT 1",
        )
        .bind(proposed_parent_id)
        .bind(notebook_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    /// Find a notebook by parent_id and title. Used for deduplication during import.
    pub async fn find_notebook_by_parent_and_title(
        &self,
        parent_id: Option<&str>,
        title: &str,
    ) -> Result<Option<NotebookRow>, StorageError> {
        let row = sqlx::query_as::<_, NotebookRow>(
            "SELECT * FROM notebooks WHERE title = ?1 AND (parent_id IS ?2) LIMIT 1",
        )
        .bind(title)
        .bind(parent_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Find note IDs by their titles (case-insensitive). Returns (title, id) pairs.
    pub async fn resolve_titles_to_ids(
        &self,
        titles: &[String],
    ) -> Result<Vec<(String, String)>, StorageError> {
        if titles.is_empty() {
            return Ok(vec![]);
        }
        // Build dynamic IN clause
        let placeholders: Vec<String> = titles
            .iter()
            .enumerate()
            .map(|(i, _)| format!("LOWER(?{})", i + 1))
            .collect();
        let sql = format!(
            "SELECT title, id FROM notes WHERE LOWER(title) IN ({}) AND archived = 0",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(&sql);
        for title in titles {
            query = query.bind(title.to_lowercase());
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }
}
