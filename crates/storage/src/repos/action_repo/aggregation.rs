//! Aggregation, analytics, recurring templates, and LLM context for `actions`.

use crate::error::StorageError;
use crate::rows::action::ActionRow;

use super::{ActionRepo, ActionSummary};

impl ActionRepo {
    /// Count actions by status.
    pub async fn summary(&self) -> Result<ActionSummary, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM actions
            WHERE is_template = FALSE
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut summary = ActionSummary::default();
        for (status, count) in &rows {
            match status.as_str() {
                "todo" => summary.todo = *count,
                "doing" => summary.doing = *count,
                "done" => summary.done = *count,
                _ => {}
            }
            summary.total += count;
        }
        Ok(summary)
    }

    /// Aggregate task counts by status group (via status_labels JOIN).
    pub async fn summary_by_group(
        &self,
    ) -> Result<std::collections::HashMap<String, i64>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT sl.status_group, COUNT(*) as cnt
            FROM actions a
            JOIN status_labels sl ON a.status_label_id = sl.id
            WHERE a.is_template = 0
            GROUP BY sl.status_group
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// Get overdue actions.
    pub async fn overdue(&self) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT * FROM actions
            WHERE due_date < datetime('now')
              AND status != 'done'
              AND is_template = FALSE
            ORDER BY due_date
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Build a context string of active actions for LLM context injection.
    #[allow(clippy::type_complexity)]
    pub async fn to_context_string(&self) -> Result<String, StorageError> {
        let rows: Vec<(
            String,
            String,
            Option<i16>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        )> = sqlx::query_as(
            r#"
                SELECT a.title, a.status, a.priority, a.focused_at, ar.name
                FROM actions a
                JOIN areas ar ON a.area_id = ar.id
                WHERE a.status IN ('todo', 'doing')
                  AND a.is_template = FALSE
                ORDER BY
                    CASE WHEN a.focused_at IS NOT NULL THEN 0 ELSE 1 END,
                    a.priority ASC NULLS LAST,
                    a.created_at
                "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok("No active tasks.".to_string());
        }

        let mut out = String::from("Active tasks:\n");
        for (title, status, priority, focused_at, area_name) in &rows {
            let focus_marker = if focused_at.is_some() {
                " [FOCUSED]"
            } else {
                ""
            };
            let priority_str = priority.map(|p| format!(" P{p}")).unwrap_or_default();
            out.push_str(&format!(
                "- [{}]{}{} {} ({})\n",
                status, focus_marker, priority_str, title, area_name
            ));
        }
        Ok(out)
    }

    /// Count total and completed actions for a key result.
    /// Returns `(total, completed)`.
    pub async fn count_by_kr(&self, kr_id: &str) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END) \
             FROM actions WHERE key_result_id = ?1 AND is_template = FALSE",
        )
        .bind(kr_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((row.0, row.1))
    }

    /// Add a recurring template.
    pub async fn add_template(&self, row: &ActionRow) -> Result<ActionRow, StorageError> {
        self.add(row).await
    }

    /// Delete a recurring template.
    pub async fn delete_template(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM actions WHERE id = ?1 AND is_template = TRUE")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
