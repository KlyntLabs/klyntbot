//! Plan repository — plans + plan_steps tables.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::plan::{PlanRow, PlanStepRow};

/// Repository for plan persistence.
#[derive(Debug, Clone)]
pub struct PlanRepo {
    pool: SqlitePool,
}

impl PlanRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new plan.
    pub async fn create(&self, row: &PlanRow) -> Result<PlanRow, StorageError> {
        let result = sqlx::query_as::<_, PlanRow>(
            "INSERT INTO plans (id, session_key, goal_id, title, description, status,
                                current_step_index, iteration_limit, backtrack_history,
                                visibility, task_id,
                                created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             RETURNING *",
        )
        .bind(row.id)
        .bind(&row.session_key)
        .bind(row.goal_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.current_step_index)
        .bind(row.iteration_limit)
        .bind(&row.backtrack_history)
        .bind(&row.visibility)
        .bind(&row.task_id)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Upsert a plan (insert or update on conflict).
    pub async fn upsert(&self, row: &PlanRow) -> Result<PlanRow, StorageError> {
        let result = sqlx::query_as::<_, PlanRow>(
            "INSERT INTO plans (id, session_key, goal_id, title, description, status,
                                current_step_index, iteration_limit, backtrack_history,
                                visibility, task_id,
                                created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                status = EXCLUDED.status,
                current_step_index = EXCLUDED.current_step_index,
                iteration_limit = EXCLUDED.iteration_limit,
                backtrack_history = EXCLUDED.backtrack_history,
                visibility = EXCLUDED.visibility,
                task_id = EXCLUDED.task_id,
                updated_at = EXCLUDED.updated_at,
                completed_at = EXCLUDED.completed_at
             RETURNING *",
        )
        .bind(row.id)
        .bind(&row.session_key)
        .bind(row.goal_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.current_step_index)
        .bind(row.iteration_limit)
        .bind(&row.backtrack_history)
        .bind(&row.visibility)
        .bind(&row.task_id)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get a plan by ID.
    pub async fn get(&self, id: Uuid) -> Result<PlanRow, StorageError> {
        sqlx::query_as::<_, PlanRow>("SELECT * FROM plans WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("plan '{}'", id)))
    }

    /// List plans with optional filters.
    ///
    /// `visibility` controls filtering:
    /// - `None` (default): excludes `silent` plans
    /// - `Some("all")`: returns all plans regardless of visibility
    /// - `Some(value)`: filters to only that visibility level
    pub async fn list(
        &self,
        status: Option<&str>,
        session_key: Option<&str>,
        goal_id: Option<Uuid>,
        visibility: Option<&str>,
    ) -> Result<Vec<PlanRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM plans WHERE 1=1");

        if let Some(s) = status {
            qb.push(" AND status = ");
            qb.push_bind(s);
        }
        if let Some(sk) = session_key {
            qb.push(" AND session_key = ");
            qb.push_bind(sk);
        }
        if let Some(gid) = goal_id {
            qb.push(" AND goal_id = ");
            qb.push_bind(gid);
        }

        match visibility {
            Some("all") => { /* no filter */ }
            Some(v) => {
                qb.push(" AND visibility = ");
                qb.push_bind(v);
            }
            None => {
                qb.push(" AND visibility != 'silent'");
            }
        }

        qb.push(" ORDER BY created_at DESC");

        let rows = qb.build_query_as::<PlanRow>().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Update plan mutable fields.
    pub async fn update(&self, row: &PlanRow) -> Result<PlanRow, StorageError> {
        let now = Utc::now();
        let result = sqlx::query_as::<_, PlanRow>(
            "UPDATE plans SET title = ?1, description = ?2, status = ?3,
                              current_step_index = ?4, iteration_limit = ?5,
                              backtrack_history = ?6, visibility = ?7, task_id = ?8,
                              updated_at = ?9, completed_at = ?10
             WHERE id = ?11
             RETURNING *",
        )
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.current_step_index)
        .bind(row.iteration_limit)
        .bind(&row.backtrack_history)
        .bind(&row.visibility)
        .bind(&row.task_id)
        .bind(now)
        .bind(row.completed_at)
        .bind(row.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("plan '{}'", row.id)))?;
        Ok(result)
    }

    /// Delete a plan by ID (cascades to plan_steps).
    pub async fn delete(&self, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM plans WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update plan status with timestamp bookkeeping.
    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<(), StorageError> {
        let now = Utc::now();
        let completed_at = match status.to_lowercase().as_str() {
            "completed" | "failed" | "abandoned" => Some(now),
            _ => None,
        };
        let result = sqlx::query(
            "UPDATE plans SET status = ?1, updated_at = ?2, completed_at = COALESCE(?3, completed_at)
             WHERE id = ?4",
        )
        .bind(status)
        .bind(now)
        .bind(completed_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("plan '{}'", id)));
        }
        Ok(())
    }

    // ── Plan Steps ──────────────────────────────────────────────

    /// Add a step to a plan.
    pub async fn add_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError> {
        let result = sqlx::query_as::<_, PlanStepRow>(
            "INSERT INTO plan_steps (id, plan_id, step_index, description, reasoning,
                                     expected_tools, status, attempt_count, max_attempts,
                                     result, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             RETURNING *",
        )
        .bind(step.id)
        .bind(step.plan_id)
        .bind(step.step_index)
        .bind(&step.description)
        .bind(&step.reasoning)
        .bind(sqlx::types::Json(&step.expected_tools))
        .bind(&step.status)
        .bind(step.attempt_count)
        .bind(step.max_attempts)
        .bind(&step.result)
        .bind(step.started_at)
        .bind(step.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Update a plan step.
    pub async fn update_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError> {
        let result = sqlx::query_as::<_, PlanStepRow>(
            "UPDATE plan_steps SET status = ?1, attempt_count = ?2, result = ?3,
                                   started_at = ?4, completed_at = ?5
             WHERE id = ?6
             RETURNING *",
        )
        .bind(&step.status)
        .bind(step.attempt_count)
        .bind(&step.result)
        .bind(step.started_at)
        .bind(step.completed_at)
        .bind(step.id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("plan_step '{}'", step.id)))?;
        Ok(result)
    }

    /// Upsert a plan step (insert or update on conflict).
    pub async fn upsert_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError> {
        let result = sqlx::query_as::<_, PlanStepRow>(
            "INSERT INTO plan_steps (id, plan_id, step_index, description, reasoning,
                                     expected_tools, status, attempt_count, max_attempts,
                                     result, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                status = EXCLUDED.status,
                attempt_count = EXCLUDED.attempt_count,
                result = EXCLUDED.result,
                started_at = EXCLUDED.started_at,
                completed_at = EXCLUDED.completed_at
             RETURNING *",
        )
        .bind(step.id)
        .bind(step.plan_id)
        .bind(step.step_index)
        .bind(&step.description)
        .bind(&step.reasoning)
        .bind(sqlx::types::Json(&step.expected_tools))
        .bind(&step.status)
        .bind(step.attempt_count)
        .bind(step.max_attempts)
        .bind(&step.result)
        .bind(step.started_at)
        .bind(step.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get all steps for a plan, ordered by step_index.
    pub async fn get_steps(&self, plan_id: Uuid) -> Result<Vec<PlanStepRow>, StorageError> {
        let rows = sqlx::query_as::<_, PlanStepRow>(
            "SELECT * FROM plan_steps WHERE plan_id = ?1 ORDER BY step_index ASC",
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_plan_row() -> PlanRow {
        let now = Utc::now();
        PlanRow {
            id: Uuid::new_v4(),
            session_key: "test-session".to_string(),
            goal_id: None,
            title: "Test plan".to_string(),
            description: "A test plan".to_string(),
            status: "draft".to_string(),
            current_step_index: 0,
            iteration_limit: 20,
            backtrack_history: serde_json::json!([]),
            visibility: "transparent".to_string(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn plan_visibility_and_task_id() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = PlanRepo::new(pool.inner().clone());

        // Create plan with on_failure visibility (task_id left None to avoid FK)
        let mut row = test_plan_row();
        row.visibility = "on_failure".to_string();

        let created = repo.create(&row).await.unwrap();
        assert_eq!(created.visibility, "on_failure");
        assert_eq!(created.task_id, None);

        // Verify get returns the new fields
        let fetched = repo.get(created.id).await.unwrap();
        assert_eq!(fetched.visibility, "on_failure");

        // Create a silent plan
        let mut silent_row = test_plan_row();
        silent_row.id = Uuid::new_v4();
        silent_row.visibility = "silent".to_string();
        repo.create(&silent_row).await.unwrap();

        // list with default visibility excludes silent
        let visible = repo.list(None, None, None, None).await.unwrap();
        assert!(visible.iter().all(|p| p.visibility != "silent"));

        // list with "all" returns everything
        let all = repo.list(None, None, None, Some("all")).await.unwrap();
        assert!(all.len() > visible.len());

        // list with specific visibility
        let silent_only = repo.list(None, None, None, Some("silent")).await.unwrap();
        assert!(silent_only.iter().all(|p| p.visibility == "silent"));
        assert_eq!(silent_only.len(), 1);
    }
}
