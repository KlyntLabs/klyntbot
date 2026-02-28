//! Domain ↔ SQL row conversion helpers for plans.
//!
//! Extracted from the removed PlanStore to support direct PlanRepo usage.

use crate::types::{BacktrackEntry, Plan, PlanStep};
use uuid::Uuid;

/// Convert a Plan domain type to a PlanRow for SQL persistence.
pub fn plan_to_row(plan: &Plan) -> storage::PlanRow {
    storage::PlanRow {
        id: plan.id,
        session_key: plan.session_key.clone(),
        goal_id: plan.goal_id,
        title: plan.title.clone(),
        description: plan.description.clone(),
        status: plan.status.to_string(),
        current_step_index: plan.current_step_index as i32,
        iteration_limit: plan.iteration_limit as i32,
        backtrack_history: serde_json::to_value(&plan.backtrack_history).unwrap_or_default(),
        visibility: plan.visibility.to_string(),
        task_id: plan.task_id.clone(),
        created_at: plan.created_at,
        updated_at: plan.updated_at,
        completed_at: plan.completed_at,
    }
}

/// Convert a PlanStep to a PlanStepRow for SQL persistence.
pub fn step_to_row(step: &PlanStep, plan_id: Uuid) -> storage::PlanStepRow {
    storage::PlanStepRow {
        id: step.id,
        plan_id,
        step_index: step.index as i32,
        description: step.description.clone(),
        reasoning: step.reasoning.clone(),
        expected_tools: step.expected_tools.clone(),
        status: step.status.to_string(),
        attempt_count: step.attempt_count as i16,
        max_attempts: step.max_attempts as i16,
        result: step.result.clone(),
        started_at: step.started_at,
        completed_at: step.completed_at,
    }
}

/// Convert a PlanRow + PlanStepRows back to a Plan domain type.
pub fn row_to_plan(row: storage::PlanRow, step_rows: Vec<storage::PlanStepRow>) -> Plan {
    let steps: Vec<PlanStep> = step_rows
        .into_iter()
        .map(|sr| PlanStep {
            id: sr.id,
            index: sr.step_index as usize,
            description: sr.description,
            reasoning: sr.reasoning,
            expected_tools: sr.expected_tools,
            status: sr.status.parse().unwrap_or_default(),
            attempt_count: sr.attempt_count as u8,
            max_attempts: sr.max_attempts as u8,
            result: sr.result,
            started_at: sr.started_at,
            completed_at: sr.completed_at,
        })
        .collect();

    Plan {
        id: row.id,
        session_key: row.session_key,
        goal_id: row.goal_id,
        title: row.title,
        description: row.description,
        status: row.status.parse().unwrap_or_default(),
        steps,
        current_step_index: row.current_step_index as usize,
        iteration_limit: row.iteration_limit as usize,
        backtrack_history: serde_json::from_value::<Vec<BacktrackEntry>>(row.backtrack_history)
            .unwrap_or_default(),
        visibility: row.visibility.parse().unwrap_or_default(),
        task_id: row.task_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        completed_at: row.completed_at,
    }
}

/// Load a Plan from the repo by ID (including its steps). Returns None if not found.
pub async fn load_plan(repo: &storage::PlanRepo, id: &Uuid) -> common::Result<Option<Plan>> {
    match repo.get(*id).await {
        Ok(row) => {
            let steps = repo.get_steps(*id).await?;
            Ok(Some(row_to_plan(row, steps)))
        }
        Err(storage::StorageError::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Save (upsert) a Plan + all its steps to the repo.
pub async fn save_plan(repo: &storage::PlanRepo, plan: &Plan) -> common::Result<()> {
    let row = plan_to_row(plan);
    repo.upsert(&row).await?;

    for step in &plan.steps {
        let step_row = step_to_row(step, plan.id);
        repo.upsert_step(&step_row).await?;
    }

    Ok(())
}

/// Get the most recent active plan for a session.
/// Returns Draft, Approved, or Executing plans only.
pub async fn get_active_plan(
    repo: &storage::PlanRepo,
    session_key: &str,
) -> common::Result<Option<Plan>> {
    let Some(row) = repo.get_active(session_key).await? else {
        return Ok(None);
    };
    let steps = repo.get_steps(row.id).await?;
    Ok(Some(row_to_plan(row, steps)))
}
