//! Applies accepted suggestion actions to tasks.
//!
//! Each SuggestionAction variant maps to a specific repo operation.
//! This handler is intentionally simple — it delegates to TaskRepo
//! and returns a human-readable summary.

use async_trait::async_trait;
use common::Result;
use tracing::info;

use feature_tasks::types::SuggestionAction;
use feature_tasks::SuggestionApplier;
use storage::{TaskPatch, TaskRepo};

pub struct TaskSuggestionApplier {
    repo: TaskRepo,
}

impl TaskSuggestionApplier {
    pub fn new(repo: TaskRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl SuggestionApplier for TaskSuggestionApplier {
    async fn apply(
        &self,
        suggestion_id: &str,
        task_id: Option<&str>,
        action: &SuggestionAction,
    ) -> Result<String> {
        info!(suggestion_id, ?task_id, ?action, "Applying suggestion");

        let summary = match action {
            SuggestionAction::SetPriority { priority } => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        priority: Some(Some(*priority)),
                        ..Default::default()
                    })
                    .await?;
                format!("Set priority to {priority}")
            }
            SuggestionAction::SetDueDate { due_date } => {
                let tid = require_task_id(task_id)?;
                let dt = chrono::DateTime::parse_from_rfc3339(due_date)
                    .or_else(|_| {
                        // Try YYYY-MM-DD format
                        chrono::NaiveDate::parse_from_str(due_date, "%Y-%m-%d")
                            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().fixed_offset())
                    })
                    .map_err(|e| {
                        common::ToolError::ExecutionFailed(format!("Invalid date: {e}"))
                    })?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        due_date: Some(Some(common::time::bridge::chrono_to_jiff(dt.with_timezone(&chrono::Utc)))),
                        ..Default::default()
                    })
                    .await?;
                format!("Set due date to {due_date}")
            }
            SuggestionAction::TriggerDecomposition => {
                let tid = require_task_id(task_id)?;
                // The actual decomposition is triggered by the caller (tool action or cron)
                format!("Marked task {tid} for decomposition")
            }
            SuggestionAction::ConvertToAgentic => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        task_type: Some("agentic".into()),
                        ..Default::default()
                    })
                    .await?;
                format!("Converted task {tid} to agentic type")
            }
            SuggestionAction::Archive => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        status: Some("archived".into()),
                        ..Default::default()
                    })
                    .await?;
                format!("Archived task {tid}")
            }
            SuggestionAction::MergeInto { target_task_id } => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        status: Some("archived".into()),
                        ..Default::default()
                    })
                    .await?;
                format!("Merged task {tid} into {target_task_id}")
            }
            SuggestionAction::RemoveBlocker { blocker_id } => {
                let tid = require_task_id(task_id)?;
                self.repo.remove_dependency(tid, blocker_id).await?;
                format!("Removed blocker {blocker_id} from task {tid}")
            }
            SuggestionAction::UpdateEstimationBaseline { minutes } => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        estimated_minutes: Some(Some(*minutes)),
                        ..Default::default()
                    })
                    .await?;
                format!("Updated estimation to {minutes}min")
            }
            SuggestionAction::SetEnergyLevel { level } => {
                let tid = require_task_id(task_id)?;
                self.repo
                    .update(&TaskPatch {
                        id: tid.into(),
                        energy_level: Some(Some(level.to_string())),
                        ..Default::default()
                    })
                    .await?;
                format!("Set energy level to {level}")
            }
            SuggestionAction::Informational => "Informational suggestion — no action taken".into(),
        };

        // Mark suggestion as applied
        self.repo
            .resolve_suggestion(suggestion_id, "applied")
            .await?;

        Ok(summary)
    }
}

fn require_task_id(task_id: Option<&str>) -> Result<&str> {
    task_id.ok_or_else(|| {
        common::ToolError::ExecutionFailed("task_id required for this action".into()).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_task_id_some() {
        assert_eq!(require_task_id(Some("t1")).unwrap(), "t1");
    }

    #[test]
    fn test_require_task_id_none() {
        assert!(require_task_id(None).is_err());
    }
}
