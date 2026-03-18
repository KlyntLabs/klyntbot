//! Repository for the `tool_usage` table.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::tool_usage::{ToolUsageRow, ToolUsageStatsRow};

#[derive(Debug, Clone)]
pub struct ToolUsageRepo {
    pool: SqlitePool,
}

impl ToolUsageRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a tool usage record.
    pub async fn insert(&self, row: &ToolUsageRow) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO tool_usage (id, tool_name, action, session_key, channel,
                intent_category, success, duration_ms, error_message, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&row.id)
        .bind(&row.tool_name)
        .bind(&row.action)
        .bind(&row.session_key)
        .bind(&row.channel)
        .bind(&row.intent_category)
        .bind(row.success)
        .bind(row.duration_ms)
        .bind(&row.error_message)
        .bind(row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    delete_older_than_impl!("tool_usage", "created_at");

    /// Aggregate tool usage stats grouped by tool name.
    pub async fn aggregate_by_tool(
        &self,
        days: Option<i64>,
    ) -> Result<Vec<ToolUsageStatsRow>, StorageError> {
        let rows = if let Some(d) = days {
            let cutoff = Utc::now() - chrono::Duration::days(d);
            sqlx::query_as::<_, ToolUsageStatsRow>(
                r#"
                SELECT tool_name,
                       COUNT(*) AS call_count,
                       SUM(CASE WHEN success THEN 1 ELSE 0 END) AS success_count,
                       AVG(duration_ms) AS avg_duration_ms
                FROM tool_usage
                WHERE created_at >= ?1
                GROUP BY tool_name
                ORDER BY call_count DESC
                "#,
            )
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ToolUsageStatsRow>(
                r#"
                SELECT tool_name,
                       COUNT(*) AS call_count,
                       SUM(CASE WHEN success THEN 1 ELSE 0 END) AS success_count,
                       AVG(duration_ms) AS avg_duration_ms
                FROM tool_usage
                GROUP BY tool_name
                ORDER BY call_count DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }
}
