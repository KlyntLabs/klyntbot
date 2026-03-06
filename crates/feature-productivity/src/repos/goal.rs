use chrono::Utc;
use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{GoalMetric, GoalType, ProductivityGoal};

#[derive(sqlx::FromRow)]
struct GoalRow {
    id: Option<i64>,
    goal_type: String,
    metric: String,
    target_value: f64,
    enabled: bool,
    project_id: Option<String>,
    created_at: String,
}

impl From<GoalRow> for ProductivityGoal {
    fn from(row: GoalRow) -> Self {
        let goal_type = row.goal_type.parse::<GoalType>().unwrap_or_else(|_| {
            warn!(raw = %row.goal_type, "unknown goal_type in DB, defaulting to Daily");
            GoalType::Daily
        });
        let metric = row.metric.parse::<GoalMetric>().unwrap_or_else(|_| {
            warn!(raw = %row.metric, "unknown metric in DB, defaulting to ProductiveHours");
            GoalMetric::ProductiveHours
        });
        let created_at = common::utils::date::parse_datetime(&row.created_at, "UTC")
            .unwrap_or_else(|| {
                warn!(raw = %row.created_at, "unparseable created_at in productivity_goals");
                Utc::now()
            });
        Self {
            id: row.id,
            goal_type,
            metric,
            target_value: row.target_value,
            enabled: row.enabled,
            project_id: row.project_id,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoalRepo {
    pool: SqlitePool,
}

impl GoalRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, goal: &ProductivityGoal) -> common::Result<i64> {
        let goal_type = goal.goal_type.to_string();
        let metric = goal.metric.to_string();
        let id = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO productivity_goals (goal_type, metric, target_value, enabled, project_id)
               VALUES (?1, ?2, ?3, ?4, ?5)
               RETURNING id"#,
        )
        .bind(&goal_type)
        .bind(&metric)
        .bind(goal.target_value)
        .bind(goal.enabled)
        .bind(&goal.project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn list_enabled(&self) -> common::Result<Vec<ProductivityGoal>> {
        let rows = sqlx::query_as::<_, GoalRow>(
            r#"SELECT id, goal_type, metric, target_value, enabled, project_id, created_at
               FROM productivity_goals WHERE enabled = TRUE ORDER BY goal_type, metric"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(ProductivityGoal::from).collect())
    }

    pub async fn delete(&self, id: i64) -> common::Result<bool> {
        let result = sqlx::query("DELETE FROM productivity_goals WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_enabled(&self, id: i64, enabled: bool) -> common::Result<()> {
        sqlx::query("UPDATE productivity_goals SET enabled = ?2 WHERE id = ?1")
            .bind(id)
            .bind(enabled)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}
