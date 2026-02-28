//! Goal repository — goals + goal_project_links tables.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{OptionExt, StorageError};
use crate::rows::goal::{GoalProjectLinkRow, GoalRow};

/// Explicit column list for the `goals` table.
///
/// The table still has dead `metrics` and `metadata` JSON columns from the
/// original schema (SQLite cannot DROP COLUMN). We list columns explicitly so
/// that `FromRow` derivation on `GoalRow` never encounters them.
const GOAL_COLS: &str = "id, title, description, status, priority, target_date, \
                         created_at, updated_at, plans_completed, plans_failed, \
                         avg_duration_ms, last_plan_at";

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
        let query = format!(
            "INSERT INTO goals ({}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
             RETURNING {}",
            GOAL_COLS, GOAL_COLS
        );
        let result = sqlx::query_as::<_, GoalRow>(&query)
            .bind(row.id)
            .bind(&row.title)
            .bind(&row.description)
            .bind(&row.status)
            .bind(row.priority)
            .bind(row.target_date)
            .bind(row.created_at)
            .bind(row.updated_at)
            .bind(row.plans_completed)
            .bind(row.plans_failed)
            .bind(row.avg_duration_ms)
            .bind(row.last_plan_at)
            .fetch_one(&self.pool)
            .await?;
        Ok(result)
    }

    /// Get a goal by ID.
    pub async fn get(&self, id: Uuid) -> Result<GoalRow, StorageError> {
        let query = format!("SELECT {} FROM goals WHERE id = ?1", GOAL_COLS);
        sqlx::query_as::<_, GoalRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_not_found(&format!("goal '{}'", id))
    }

    /// List goals with optional status filter.
    pub async fn list(&self, status: Option<&str>) -> Result<Vec<GoalRow>, StorageError> {
        let rows = match status {
            Some(s) => {
                let query = format!(
                    "SELECT {} FROM goals WHERE status = ?1 ORDER BY priority ASC, created_at DESC",
                    GOAL_COLS
                );
                sqlx::query_as::<_, GoalRow>(&query)
                    .bind(s)
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                let query = format!(
                    "SELECT {} FROM goals ORDER BY priority ASC, created_at DESC",
                    GOAL_COLS
                );
                sqlx::query_as::<_, GoalRow>(&query)
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(rows)
    }

    /// Update a goal (full replace of mutable fields).
    pub async fn update(&self, row: &GoalRow) -> Result<GoalRow, StorageError> {
        let now = Utc::now();
        let query = format!(
            "UPDATE goals SET title = ?1, description = ?2, status = ?3, priority = ?4, \
             target_date = ?5, updated_at = ?6, plans_completed = ?7, plans_failed = ?8, \
             avg_duration_ms = ?9, last_plan_at = ?10 \
             WHERE id = ?11 \
             RETURNING {}",
            GOAL_COLS
        );
        let result = sqlx::query_as::<_, GoalRow>(&query)
            .bind(&row.title)
            .bind(&row.description)
            .bind(&row.status)
            .bind(row.priority)
            .bind(row.target_date)
            .bind(now)
            .bind(row.plans_completed)
            .bind(row.plans_failed)
            .bind(row.avg_duration_ms)
            .bind(row.last_plan_at)
            .bind(row.id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_not_found(&format!("goal '{}'", row.id))?;
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

    /// Increment plans_completed and update avg_duration_ms with rolling average.
    pub async fn increment_completed(
        &self,
        id: Uuid,
        plan_duration_ms: i64,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE goals SET \
             plans_completed = plans_completed + 1, \
             avg_duration_ms = CASE \
               WHEN avg_duration_ms IS NULL THEN ?1 \
               ELSE ((avg_duration_ms * plans_completed) + ?1) / (plans_completed + 1) \
             END, \
             last_plan_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), \
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?2",
        )
        .bind(plan_duration_ms)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("Goal {}", id)));
        }
        Ok(())
    }

    /// Increment plans_failed counter.
    pub async fn increment_failed(&self, id: Uuid) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE goals SET \
             plans_failed = plans_failed + 1, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("Goal {}", id)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> (StoragePool, GoalRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = GoalRepo::new(pool.inner().clone());
        (pool, repo)
    }

    fn make_goal(id: uuid::Uuid) -> GoalRow {
        GoalRow {
            id,
            title: "Test goal".to_string(),
            description: "A test".to_string(),
            status: "Active".to_string(),
            priority: 3,
            target_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            plans_completed: 0,
            plans_failed: 0,
            avg_duration_ms: None,
            last_plan_at: None,
        }
    }

    #[tokio::test]
    async fn test_goal_typed_columns_roundtrip() {
        let (_pool, repo) = setup().await;
        let id = uuid::Uuid::new_v4();
        let row = make_goal(id);
        repo.create(&row).await.unwrap();

        let fetched = repo.get(id).await.unwrap();
        assert_eq!(fetched.plans_completed, 0);
        assert_eq!(fetched.plans_failed, 0);
        assert_eq!(fetched.avg_duration_ms, None);
        assert_eq!(fetched.last_plan_at, None);
    }

    #[tokio::test]
    async fn test_increment_completed_rolling_average() {
        let (_pool, repo) = setup().await;
        let id = uuid::Uuid::new_v4();
        repo.create(&make_goal(id)).await.unwrap();

        // First: 1000ms
        repo.increment_completed(id, 1000).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_completed, 1);
        assert_eq!(g.avg_duration_ms, Some(1000));

        // Second: 3000ms -> avg = (1000*1 + 3000) / 2 = 2000
        repo.increment_completed(id, 3000).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_completed, 2);
        assert_eq!(g.avg_duration_ms, Some(2000));

        // Third: 2000ms -> avg = (2000*2 + 2000) / 3 = 2000
        repo.increment_completed(id, 2000).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_completed, 3);
        assert_eq!(g.avg_duration_ms, Some(2000));
    }

    #[tokio::test]
    async fn test_increment_failed() {
        let (_pool, repo) = setup().await;
        let id = uuid::Uuid::new_v4();
        repo.create(&make_goal(id)).await.unwrap();

        repo.increment_failed(id).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_failed, 1);

        repo.increment_failed(id).await.unwrap();
        let g = repo.get(id).await.unwrap();
        assert_eq!(g.plans_failed, 2);
        assert_eq!(g.plans_completed, 0); // unchanged
    }

    #[tokio::test]
    async fn test_increment_completed_not_found() {
        let (_pool, repo) = setup().await;
        let result = repo.increment_completed(uuid::Uuid::new_v4(), 100).await;
        assert!(result.is_err());
    }
}
