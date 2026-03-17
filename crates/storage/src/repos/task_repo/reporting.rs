//! Task aggregation and reporting (summary, overdue, context string).

use std::collections::HashMap;

use super::TaskRepo;
use crate::error::StorageError;
use crate::repos::{compute_summary, TaskSummary};
use crate::rows::task::TaskRow;

impl TaskRepo {
    /// Count tasks by status.
    pub async fn summary(&self) -> Result<TaskSummary, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM tasks
            WHERE is_template = FALSE
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(compute_summary(&rows))
    }

    /// Aggregate task counts by status group (via status_labels JOIN).
    pub async fn summary_by_group(&self) -> Result<HashMap<String, i64>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT sl.status_group, COUNT(*) as cnt
            FROM tasks t
            JOIN status_labels sl ON t.status_label_id = sl.id
            WHERE t.is_template = 0
            GROUP BY sl.status_group
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// Get overdue tasks.
    pub async fn overdue(&self) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT * FROM tasks
            WHERE due_date < datetime('now')
              AND completed = 0
              AND is_template = FALSE
            ORDER BY due_date
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Build a context string of active tasks for LLM context injection.
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
                SELECT t.title, t.status, t.priority, t.focused_at, ar.name
                FROM tasks t
                JOIN areas ar ON t.area_id = ar.id
                WHERE t.status IN ('todo', 'doing')
                  AND t.is_template = FALSE
                ORDER BY
                    CASE WHEN t.focused_at IS NOT NULL THEN 0 ELSE 1 END,
                    t.priority ASC NULLS LAST,
                    t.created_at
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
}
