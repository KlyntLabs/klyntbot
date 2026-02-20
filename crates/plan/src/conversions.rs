//! Domain ↔ SQL row conversion helpers for plans.
//!
//! Extracted from the removed PlanStore to support direct PlanRepo usage.

use crate::types::{BacktrackEntry, Plan, PlanStatus, PlanStep, StepStatus};
use uuid::Uuid;

/// Convert PlanStatus to database string.
pub fn plan_status_to_str(status: &PlanStatus) -> &'static str {
    match status {
        PlanStatus::Draft => "draft",
        PlanStatus::Approved => "approved",
        PlanStatus::Executing => "executing",
        PlanStatus::Completed => "completed",
        PlanStatus::Failed => "failed",
        PlanStatus::Abandoned => "abandoned",
    }
}

/// Convert database string to PlanStatus.
pub fn str_to_plan_status(s: &str) -> PlanStatus {
    match s.to_lowercase().as_str() {
        "draft" => PlanStatus::Draft,
        "approved" => PlanStatus::Approved,
        "executing" => PlanStatus::Executing,
        "completed" => PlanStatus::Completed,
        "failed" => PlanStatus::Failed,
        "abandoned" => PlanStatus::Abandoned,
        _ => PlanStatus::Draft,
    }
}

/// Convert StepStatus to database string.
pub fn step_status_to_str(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::Executing => "executing",
        StepStatus::Completed => "completed",
        StepStatus::Failed => "failed",
        StepStatus::Skipped => "skipped",
    }
}

/// Convert database string to StepStatus.
pub fn str_to_step_status(s: &str) -> StepStatus {
    match s.to_lowercase().as_str() {
        "pending" => StepStatus::Pending,
        "executing" => StepStatus::Executing,
        "completed" => StepStatus::Completed,
        "failed" => StepStatus::Failed,
        "skipped" => StepStatus::Skipped,
        _ => StepStatus::Pending,
    }
}

/// Convert a Plan domain type to a PlanRow for SQL persistence.
pub fn plan_to_row(plan: &Plan) -> storage::PlanRow {
    storage::PlanRow {
        id: plan.id,
        session_key: plan.session_key.clone(),
        goal_id: plan.goal_id,
        title: plan.title.clone(),
        description: plan.description.clone(),
        status: plan_status_to_str(&plan.status).to_string(),
        current_step_index: plan.current_step_index as i32,
        iteration_limit: plan.iteration_limit as i32,
        backtrack_history: serde_json::to_value(&plan.backtrack_history).unwrap_or_default(),
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
        status: step_status_to_str(&step.status).to_string(),
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
            status: str_to_step_status(&sr.status),
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
        status: str_to_plan_status(&row.status),
        steps,
        current_step_index: row.current_step_index as usize,
        iteration_limit: row.iteration_limit as usize,
        backtrack_history: serde_json::from_value::<Vec<BacktrackEntry>>(row.backtrack_history)
            .unwrap_or_default(),
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
/// Returns Draft, Approved, or Executing plans only, sorted by most recent first.
pub async fn get_active_plan(
    repo: &storage::PlanRepo,
    session_key: &str,
) -> common::Result<Option<Plan>> {
    let mut candidates = Vec::new();
    for status in &["draft", "approved", "executing"] {
        let rows = repo.list(Some(status), Some(session_key), None).await?;
        for row in rows {
            let steps = repo.get_steps(row.id).await?;
            candidates.push(row_to_plan(row, steps));
        }
    }
    candidates.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(candidates.into_iter().next())
}
