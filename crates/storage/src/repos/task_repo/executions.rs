//! Task execution tracking operations.

use super::TaskRepo;
use crate::error::{OptionExt, StorageError};
use crate::rows::task::TaskExecutionRow;

impl TaskRepo {
    /// Create an execution record.
    pub async fn create_execution(
        &self,
        row: &TaskExecutionRow,
    ) -> Result<TaskExecutionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskExecutionRow>(
            r#"
            INSERT INTO task_executions (
                id, task_id, status, agent_profile, started_at, completed_at,
                duration_secs, tokens_used, cost_usd, input_context,
                output_summary, error_message, artifacts, metrics, retry_count
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.status)
        .bind(&row.agent_profile)
        .bind(row.started_at)
        .bind(row.completed_at)
        .bind(row.duration_secs)
        .bind(row.tokens_used)
        .bind(row.cost_usd)
        .bind(&row.input_context)
        .bind(&row.output_summary)
        .bind(&row.error_message)
        .bind(&row.artifacts)
        .bind(&row.metrics)
        .bind(row.retry_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Update an execution record's status and optional output fields.
    pub async fn update_execution(
        &self,
        id: &str,
        status: &str,
        output_summary: Option<&str>,
        error_message: Option<&str>,
        metrics: Option<&str>,
    ) -> Result<TaskExecutionRow, StorageError> {
        let row = sqlx::query_as::<_, TaskExecutionRow>(
            r#"
            UPDATE task_executions
            SET status = ?2,
                output_summary = COALESCE(?3, output_summary),
                error_message = COALESCE(?4, error_message),
                metrics = COALESCE(?5, metrics),
                completed_at = CASE WHEN ?2 IN ('completed', 'failed') THEN (unixepoch('now') * 1000) ELSE completed_at END
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(output_summary)
        .bind(error_message)
        .bind(metrics)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("execution {id}"))?;
        Ok(row)
    }

    /// Get a single execution by ID.
    pub async fn get_execution(&self, id: &str) -> Result<Option<TaskExecutionRow>, StorageError> {
        let row =
            sqlx::query_as::<_, TaskExecutionRow>("SELECT * FROM task_executions WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// List executions for a task.
    pub async fn list_executions(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskExecutionRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskExecutionRow>(
            "SELECT * FROM task_executions WHERE task_id = ?1 ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
