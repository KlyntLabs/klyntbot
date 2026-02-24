//! Domain ↔ SQL row conversion helpers for goals.
//!
//! Extracted from the removed GoalStore to support direct GoalRepo usage.

use crate::types::{Goal, GoalProgress, GoalStatus};
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

/// Convert a Goal domain type to a GoalRow for SQL persistence.
///
/// Note: The domain `Goal` still carries `metrics` and `metadata` fields, but
/// those are no longer persisted in `GoalRow`. Plan-completion columns are
/// populated from `metadata` when available, defaulting to zero/None otherwise.
pub fn goal_to_row(goal: &Goal) -> storage::GoalRow {
    // Extract plan-completion values from domain metadata when present.
    let plans_completed: i32 = goal
        .metadata
        .get("plans_completed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let plans_failed: i32 = goal
        .metadata
        .get("plans_failed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let avg_duration_ms: Option<i64> = goal
        .metadata
        .get("avg_duration_ms")
        .and_then(|v| v.parse().ok());
    let last_plan_at = goal
        .metadata
        .get("last_plan_at")
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&Utc));

    storage::GoalRow {
        id: goal.id,
        title: goal.title.clone(),
        description: goal.description.clone(),
        status: goal.status.to_string(),
        priority: goal.priority as i16,
        target_date: goal.target_date,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
        plans_completed,
        plans_failed,
        avg_duration_ms,
        last_plan_at,
    }
}

/// Convert a GoalRow + linked project IDs back to a Goal domain type.
///
/// Plan-completion typed columns are projected back into `metadata` so
/// that callers (e.g. `PlanCompletionHandlerImpl`) can read them without
/// changing their API until the domain type is updated (Task 9).
pub fn row_to_goal(row: storage::GoalRow, linked_project_ids: Vec<Uuid>) -> Goal {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("plans_completed".into(), row.plans_completed.to_string());
    metadata.insert("plans_failed".into(), row.plans_failed.to_string());
    if let Some(avg) = row.avg_duration_ms {
        metadata.insert("avg_duration_ms".into(), avg.to_string());
    }
    if let Some(last) = row.last_plan_at {
        metadata.insert("last_plan_at".into(), last.to_rfc3339());
    }

    Goal {
        id: row.id,
        title: row.title,
        description: row.description,
        status: GoalStatus::from_str(&row.status).unwrap_or_default(),
        priority: row.priority as u8,
        target_date: row.target_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
        metrics: vec![],
        linked_project_ids,
        metadata,
    }
}

/// Compute a GoalProgress from a Goal.
pub fn compute_progress(goal: &Goal) -> GoalProgress {
    let completion = if goal.metrics.is_empty() {
        0.0
    } else {
        let sum: f64 = goal.metrics.iter().map(|m| m.progress_percentage()).sum();
        sum / goal.metrics.len() as f64
    };

    let summary = if goal.metrics.is_empty() {
        "No metrics defined".to_string()
    } else {
        format!(
            "{:.0}% complete across {} metric(s)",
            completion,
            goal.metrics.len()
        )
    };

    GoalProgress {
        goal_id: goal.id,
        completion_percentage: completion,
        metrics: goal.metrics.clone(),
        summary,
    }
}

/// Load a Goal from the repo by ID. Returns None if not found.
pub async fn load_goal(repo: &storage::GoalRepo, id: &Uuid) -> common::Result<Option<Goal>> {
    match repo.get(*id).await {
        Ok(row) => {
            let links = repo.get_project_links(*id).await?;
            let project_ids = links
                .into_iter()
                .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                .collect();
            Ok(Some(row_to_goal(row, project_ids)))
        }
        Err(storage::StorageError::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Save a Goal to the repo (create or update), including project links.
pub async fn save_goal(repo: &storage::GoalRepo, goal: &Goal) -> common::Result<()> {
    let row = goal_to_row(goal);
    // Try update first, fall back to create
    match repo.update(&row).await {
        Ok(_) => {}
        Err(storage::StorageError::NotFound(_)) => {
            repo.create(&row).await?;
        }
        Err(e) => return Err(e.into()),
    }

    // Sync project links: clear existing and re-link
    let existing = repo.get_project_links(goal.id).await?;
    for link in &existing {
        let _ = repo.unlink_project(goal.id, &link.project_id).await;
    }
    for pid in &goal.linked_project_ids {
        repo.link_project(goal.id, &pid.to_string()).await?;
    }
    Ok(())
}
