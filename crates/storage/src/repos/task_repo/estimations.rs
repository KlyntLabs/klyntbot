//! Task estimation tracking operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskEstimationRow;

impl TaskRepo {
    /// Record an estimation history entry.
    pub async fn record_estimation(
        &self,
        row: &TaskEstimationRow,
    ) -> Result<TaskEstimationRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskEstimationRow>(
            r#"
            INSERT INTO task_estimation_history (
                id, task_id, estimated_minutes, actual_minutes, deviation_pct,
                complexity_score, energy_level, tags, completed_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(row.estimated_minutes)
        .bind(row.actual_minutes)
        .bind(row.deviation_pct)
        .bind(row.complexity_score)
        .bind(&row.energy_level)
        .bind(sqlx::types::Json(&row.tags))
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get estimation accuracy stats: (avg_deviation_pct, count), optionally filtered by tags.
    pub async fn estimation_stats(
        &self,
        tags: Option<&[String]>,
    ) -> Result<(f64, i64), StorageError> {
        if let Some(tag_list) = tags {
            if tag_list.is_empty() {
                let row: (f64, i64) = sqlx::query_as(
                    "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history",
                )
                .fetch_one(&self.pool)
                .await?;
                return Ok(row);
            }

            // Filter by tags: entries must contain ALL specified tags.
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history WHERE 1=1",
            );
            for tag in tag_list {
                qb.push(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ");
                qb.push_bind(tag);
                qb.push(")");
            }
            let row = qb
                .build_query_as::<(f64, i64)>()
                .fetch_one(&self.pool)
                .await?;
            Ok(row)
        } else {
            let row: (f64, i64) = sqlx::query_as(
                "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history",
            )
            .fetch_one(&self.pool)
            .await?;
            Ok(row)
        }
    }

    /// List raw estimation history records within a lookback window.
    pub async fn list_estimation_history(
        &self,
        lookback_days: u32,
    ) -> Result<Vec<TaskEstimationRow>, StorageError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(lookback_days as i64);
        let rows = sqlx::query_as::<_, TaskEstimationRow>(
            "SELECT * FROM task_estimation_history WHERE completed_at >= ?1 ORDER BY completed_at DESC",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
