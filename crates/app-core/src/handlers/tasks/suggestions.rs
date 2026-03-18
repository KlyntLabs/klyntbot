use chrono::Utc;
use desktop_shared::commands::{SuggestionResponse, TaskResponse};
use desktop_shared::errors::ApiError;
use feature_tasks::types::{SuggestionAction, SuggestionTrigger, Task};
use storage::TaskSuggestionRow;
use tracing::warn;
use uuid::Uuid;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

use super::converters::row_to_task;

impl AppCore {
    /// Generate AI suggestions for a specific task.
    ///
    /// Loads the task, calls `ProactiveHandler::evaluate_task` with a `UserRequested`
    /// trigger, persists each candidate, and returns all pending suggestions for the task.
    pub async fn task_get_suggestions(
        &self,
        task_id: String,
    ) -> Result<Vec<SuggestionResponse>, ApiError> {
        let task_row = self
            .repos
            .tasks
            .get_or_err(&task_id)
            .await
            .map_err(map_storage_err)?;

        let task = Task::from(task_row);

        let handler = self.proactive_handler()?;
        let candidates = handler
            .evaluate_task(&task, &SuggestionTrigger::UserRequested)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        // Persist each candidate
        for candidate in &candidates {
            let row = TaskSuggestionRow {
                id: Uuid::new_v4().to_string(),
                task_id: candidate.task_id.clone(),
                suggestion_type: format!("{:?}", candidate.suggestion_type),
                title: candidate.title.clone(),
                description: candidate.description.clone(),
                confidence: candidate.confidence,
                action_payload: candidate
                    .action
                    .as_ref()
                    .and_then(|a| serde_json::to_string(a).ok()),
                status: "pending".to_string(),
                trigger: candidate.trigger.as_ref().map(|t| format!("{t:?}")),
                created_at: Utc::now(),
                resolved_at: None,
            };

            if let Err(e) = self.repos.tasks.create_suggestion(&row).await {
                warn!("Failed to persist suggestion for task {task_id}: {e}");
            }
        }

        // Return all pending suggestions for this task
        let rows = self
            .repos
            .tasks
            .list_pending_suggestions(Some(&task_id))
            .await
            .map_err(map_storage_err)?;

        Ok(rows.iter().map(suggestion_row_to_response).collect())
    }

    /// Apply a pending suggestion, executing its action and returning the updated task.
    pub async fn task_apply_suggestion(
        &self,
        suggestion_id: String,
    ) -> HandlerResult<TaskResponse> {
        let suggestion = self
            .repos
            .tasks
            .get_pending_suggestion(&suggestion_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| {
                ApiError::new(
                    "NOT_FOUND",
                    format!("Pending suggestion {suggestion_id} not found"),
                )
            })?;

        let action_json = suggestion.action_payload.as_deref().ok_or_else(|| {
            ApiError::new("INVALID_STATE", "Suggestion has no action payload to apply")
        })?;

        let action: SuggestionAction = serde_json::from_str(action_json).map_err(|e| {
            ApiError::new("INTERNAL", format!("Failed to parse action payload: {e}"))
        })?;

        let applier = self.suggestion_applier()?;
        applier
            .apply(&suggestion_id, suggestion.task_id.as_deref(), &action)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        self.repos
            .tasks
            .resolve_suggestion(&suggestion_id, "applied")
            .await
            .map_err(map_storage_err)?;

        // Re-fetch task and return with entity update
        let task_id = suggestion
            .task_id
            .ok_or_else(|| ApiError::new("INVALID_STATE", "Suggestion has no associated task"))?;

        let updated_row = self
            .repos
            .tasks
            .get_or_err(&task_id)
            .await
            .map_err(map_storage_err)?;

        let response = row_to_task(&self.repos, &updated_row).await?;

        let updates = vec![EntityUpdate {
            kind: desktop_shared::types::EntityKind::Task,
            id: task_id,
        }];

        Ok((response, updates))
    }

    /// Dismiss a pending suggestion without applying it.
    pub async fn task_dismiss_suggestion(&self, suggestion_id: String) -> Result<(), ApiError> {
        self.repos
            .tasks
            .resolve_suggestion(&suggestion_id, "dismissed")
            .await
            .map_err(map_storage_err)?;
        Ok(())
    }
}

fn suggestion_row_to_response(row: &TaskSuggestionRow) -> SuggestionResponse {
    SuggestionResponse {
        id: row.id.clone(),
        suggestion_type: row.suggestion_type.clone(),
        title: row.title.clone(),
        description: row.description.clone(),
        confidence: row.confidence,
        status: row.status.clone(),
        created_at: row.created_at.to_rfc3339(),
    }
}
