//! Domain ↔ SQL row conversion helpers for goals.
//!
//! Extracted from the removed GoalStore to support direct GoalRepo usage.

use crate::types::{Goal, GoalProgress, GoalStatus};
use std::str::FromStr;
use uuid::Uuid;

/// Convert a Goal domain type to a GoalRow for SQL persistence.
pub fn goal_to_row(goal: &Goal) -> storage::GoalRow {
    storage::GoalRow {
        id: goal.id,
        title: goal.title.clone(),
        description: goal.description.clone(),
        status: goal.status.to_string(),
        priority: goal.priority as i16,
        target_date: goal.target_date,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
        plans_completed: goal.plans_completed,
        plans_failed: goal.plans_failed,
        avg_duration_ms: goal.avg_duration_ms,
        last_plan_at: goal.last_plan_at,
    }
}

/// Convert a GoalRow + linked project IDs back to a Goal domain type.
pub fn row_to_goal(row: storage::GoalRow, linked_project_ids: Vec<Uuid>) -> Goal {
    Goal {
        id: row.id,
        title: row.title,
        description: row.description,
        status: GoalStatus::from_str(&row.status).unwrap_or_default(),
        priority: row.priority as u8,
        target_date: row.target_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
        plans_completed: row.plans_completed,
        plans_failed: row.plans_failed,
        avg_duration_ms: row.avg_duration_ms,
        last_plan_at: row.last_plan_at,
        linked_project_ids,
    }
}

/// Compute a GoalProgress from a Goal based on plan completion statistics.
pub fn compute_progress(goal: &Goal) -> GoalProgress {
    let total = goal.plans_completed + goal.plans_failed;
    let completion = if total == 0 {
        0.0
    } else {
        (goal.plans_completed as f64 / total as f64) * 100.0
    };

    let summary = if total == 0 {
        "No plans executed yet".to_string()
    } else {
        format!(
            "{:.0}% completion rate ({} of {} plans succeeded)",
            completion, goal.plans_completed, total
        )
    };

    GoalProgress {
        goal_id: goal.id,
        completion_percentage: completion,
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
