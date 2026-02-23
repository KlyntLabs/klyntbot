//! Goal repository — goals + goal_project_links tables.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::goal::{GoalProjectLinkRow, GoalRow};

/// Repository for goal persistence.
#[derive(Debug, Clone)]
pub struct GoalRepo {
    pool: SqlitePool,
}

impl GoalRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new goal.
    pub async fn create(&self, row: &GoalRow) -> Result<GoalRow, StorageError> {
        let result = sqlx::query_as::<_, GoalRow>(
            "INSERT INTO goals (id, title, description, status, priority, target_date,
                                created_at, updated_at, metrics, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             RETURNING *",
        )
        .bind(row.id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.priority)
        .bind(row.target_date)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(&row.metrics)
        .bind(&row.metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get a goal by ID.
    pub async fn get(&self, id: Uuid) -> Result<GoalRow, StorageError> {
        sqlx::query_as::<_, GoalRow>("SELECT * FROM goals WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("goal '{}'", id)))
    }

    /// List goals with optional status filter.
    pub async fn list(&self, status: Option<&str>) -> Result<Vec<GoalRow>, StorageError> {
        let rows =
            match status {
                Some(s) => sqlx::query_as::<_, GoalRow>(
                    "SELECT * FROM goals WHERE status = ?1 ORDER BY priority ASC, created_at DESC",
                )
                .bind(s)
                .fetch_all(&self.pool)
                .await?,
                None => {
                    sqlx::query_as::<_, GoalRow>(
                        "SELECT * FROM goals ORDER BY priority ASC, created_at DESC",
                    )
                    .fetch_all(&self.pool)
                    .await?
                }
            };
        Ok(rows)
    }

    /// Update a goal (full replace of mutable fields).
    pub async fn update(&self, row: &GoalRow) -> Result<GoalRow, StorageError> {
        let now = Utc::now();
        let result = sqlx::query_as::<_, GoalRow>(
            "UPDATE goals SET title = ?1, description = ?2, status = ?3, priority = ?4,
                              target_date = ?5, updated_at = ?6, metrics = ?7, metadata = ?8
             WHERE id = ?9
             RETURNING *",
        )
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.priority)
        .bind(row.target_date)
        .bind(now)
        .bind(&row.metrics)
        .bind(&row.metadata)
        .bind(row.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("goal '{}'", row.id)))?;
        Ok(result)
    }

    /// Delete a goal by ID.
    pub async fn delete(&self, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM goals WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update metrics (JSON) for a goal.
    pub async fn update_metrics(
        &self,
        id: Uuid,
        metrics: &serde_json::Value,
    ) -> Result<(), StorageError> {
        let result = sqlx::query("UPDATE goals SET metrics = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(metrics)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("goal '{}'", id)));
        }
        Ok(())
    }

    /// Link a project to a goal.
    pub async fn link_project(&self, goal_id: Uuid, project_id: &str) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO goal_project_links (goal_id, project_id)
             VALUES (?1, ?2)
             ON CONFLICT DO NOTHING",
        )
        .bind(goal_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Unlink a project from a goal.
    pub async fn unlink_project(
        &self,
        goal_id: Uuid,
        project_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM goal_project_links WHERE goal_id = ?1 AND project_id = ?2")
                .bind(goal_id)
                .bind(project_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all project links for a goal.
    pub async fn get_project_links(
        &self,
        goal_id: Uuid,
    ) -> Result<Vec<GoalProjectLinkRow>, StorageError> {
        let rows = sqlx::query_as::<_, GoalProjectLinkRow>(
            "SELECT * FROM goal_project_links WHERE goal_id = ?1",
        )
        .bind(goal_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
