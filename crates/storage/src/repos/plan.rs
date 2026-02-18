//! Plan repository — plans + plan_steps tables.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::plan::{PlanRow, PlanStepRow};

/// Repository for plan persistence.
#[derive(Debug, Clone)]
pub struct PlanRepo {
    pool: PgPool,
}

impl PlanRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new plan.
    pub async fn create(&self, row: &PlanRow) -> Result<PlanRow, StorageError> {
        let result = sqlx::query_as::<_, PlanRow>(
            "INSERT INTO plans (id, session_key, goal_id, title, description, status,
                                current_step_index, iteration_limit, backtrack_history,
                                created_at, updated_at, completed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get a plan by ID.
    pub async fn get(&self, id: Uuid) -> Result<PlanRow, StorageError> {
        sqlx::query_as::<_, PlanRow>("SELECT * FROM plans WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("plan '{}'", id)))
    }

    /// List plans with optional filters.
    pub async fn list(
        &self,
        status: Option<&str>,
        session_key: Option<&str>,
        goal_id: Option<Uuid>,
    ) -> Result<Vec<PlanRow>, StorageError> {
        // Build dynamic query
        let mut query = String::from("SELECT * FROM plans WHERE 1=1");
        let mut param_idx = 1u32;

        if status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_idx));
            param_idx += 1;
        }
        if session_key.is_some() {
            query.push_str(&format!(" AND session_key = ${}", param_idx));
            param_idx += 1;
        }
        if goal_id.is_some() {
            query.push_str(&format!(" AND goal_id = ${}", param_idx));
        }
        query.push_str(" ORDER BY created_at DESC");

        let mut q = sqlx::query_as::<_, PlanRow>(&query);
        if let Some(s) = status {
            q = q.bind(s);
        }
        if let Some(sk) = session_key {
            q = q.bind(sk);
        }
        if let Some(gid) = goal_id {
            q = q.bind(gid);
        }

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Update plan mutable fields.
    pub async fn update(&self, row: &PlanRow) -> Result<PlanRow, StorageError> {
        let now = Utc::now();
        let result = sqlx::query_as::<_, PlanRow>(
            "UPDATE plans SET status = $1, current_step_index = $2, iteration_limit = $3,
                              backtrack_history = $4, updated_at = $5, completed_at = $6
             WHERE id = $7
             RETURNING *",
        )
        .bind(&row.status)
        .bind(row.current_step_index)
        .bind(row.iteration_limit)
        .bind(&row.backtrack_history)
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
        let result = sqlx::query("DELETE FROM plans WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update plan status with timestamp bookkeeping.
    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<(), StorageError> {
        let now = Utc::now();
        let completed_at = match status {
            "Completed" | "Failed" | "Abandoned" => Some(now),
            _ => None,
        };
        let result = sqlx::query(
            "UPDATE plans SET status = $1, updated_at = $2, completed_at = COALESCE($3, completed_at)
             WHERE id = $4",
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
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             RETURNING *",
        )
        .bind(step.id)
        .bind(step.plan_id)
        .bind(step.step_index)
        .bind(&step.description)
        .bind(&step.reasoning)
        .bind(&step.expected_tools)
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
            "UPDATE plan_steps SET status = $1, attempt_count = $2, result = $3,
                                   started_at = $4, completed_at = $5
             WHERE id = $6
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

    /// Get all steps for a plan, ordered by step_index.
    pub async fn get_steps(&self, plan_id: Uuid) -> Result<Vec<PlanStepRow>, StorageError> {
        let rows = sqlx::query_as::<_, PlanStepRow>(
            "SELECT * FROM plan_steps WHERE plan_id = $1 ORDER BY step_index ASC",
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
