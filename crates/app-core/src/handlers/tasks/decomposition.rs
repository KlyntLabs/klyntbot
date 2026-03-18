use desktop_shared::commands::{
    DecompositionResponse, PlannedSubtaskResponse, TaskCreateParams, TaskResponse,
};
use desktop_shared::errors::ApiError;
use feature_tasks::types::{DecompositionContext, DecompositionTree, PlannedSubtask, Task};
use storage::TaskDecompositionRow;
use tracing::warn;
use uuid::Uuid;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

impl AppCore {
    /// Decompose a task into subtasks using AI.
    ///
    /// If the decomposition confidence is >= 0.90, subtasks are auto-created.
    /// Otherwise, the plan is stored as pending for user review.
    pub async fn task_decompose(&self, task_id: String) -> HandlerResult<DecompositionResponse> {
        let task_row = self
            .repos
            .tasks
            .get_or_err(&task_id)
            .await
            .map_err(map_storage_err)?;

        let task = Task::from(task_row);

        // Get existing children titles for context
        let children = self
            .repos
            .tasks
            .get_children(&task_id)
            .await
            .map_err(map_storage_err)?;
        let existing_titles: Vec<String> = children.iter().map(|c| c.title.clone()).collect();

        let context = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 7,
            existing_subtasks: existing_titles,
            ..Default::default()
        };

        let handler = self.decomposition_handler()?;
        let result = handler
            .decompose(&task, &context)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let warnings: Vec<String> = result
            .validation_warnings
            .iter()
            .map(|w| w.message.clone())
            .collect();

        let subtask_responses = map_planned_subtasks(&result.tree.subtasks);

        if result.confidence >= 0.90 {
            // Auto-apply: create subtasks immediately
            let mut created_tasks = Vec::new();
            let mut updates = Vec::new();
            self.create_subtasks_from_plan(
                &task_id,
                &result.tree.subtasks,
                &mut created_tasks,
                &mut updates,
            )
            .await?;

            let decomposition_id = Uuid::new_v4().to_string();

            let response = DecompositionResponse {
                id: decomposition_id,
                confidence: result.confidence,
                reasoning: result.reasoning,
                subtasks: subtask_responses,
                total_estimated_mins: result.tree.total_estimated_mins,
                warnings,
                auto_applied: true,
            };

            // Also add the parent task as updated
            updates.push(EntityUpdate {
                kind: desktop_shared::types::EntityKind::Task,
                id: task_id,
            });

            Ok((response, updates))
        } else {
            // Store as pending for user review
            let decomposition_id = Uuid::new_v4().to_string();
            let plan_json = serde_json::to_string(&result.tree)
                .map_err(|e| ApiError::new("INTERNAL", format!("Failed to serialize plan: {e}")))?;

            let row = TaskDecompositionRow {
                id: decomposition_id.clone(),
                task_id: task_id.clone(),
                plan: plan_json,
                confidence: result.confidence,
                status: "pending".to_string(),
                reasoning: Some(result.reasoning.clone()),
                created_at: chrono::Utc::now(),
                applied_at: None,
            };

            if let Err(e) = self.repos.tasks.create_decomposition(&row).await {
                warn!("Failed to persist decomposition for task {task_id}: {e}");
            }

            let response = DecompositionResponse {
                id: decomposition_id,
                confidence: result.confidence,
                reasoning: result.reasoning,
                subtasks: subtask_responses,
                total_estimated_mins: result.tree.total_estimated_mins,
                warnings,
                auto_applied: false,
            };

            Ok((response, vec![]))
        }
    }

    /// Apply a pending decomposition plan, creating the subtasks.
    pub async fn task_apply_decomposition(
        &self,
        decomposition_id: String,
    ) -> HandlerResult<Vec<TaskResponse>> {
        let row = self
            .repos
            .tasks
            .get_decomposition(&decomposition_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| {
                ApiError::new(
                    "NOT_FOUND",
                    format!("Decomposition {decomposition_id} not found"),
                )
            })?;

        if row.status != "pending" {
            return Err(ApiError::new(
                "INVALID_STATE",
                format!("Decomposition is already {}", row.status),
            ));
        }

        let tree: DecompositionTree = serde_json::from_str(&row.plan).map_err(|e| {
            ApiError::new(
                "INTERNAL",
                format!("Failed to parse decomposition plan: {e}"),
            )
        })?;

        let mut created_tasks = Vec::new();
        let mut updates = Vec::new();
        self.create_subtasks_from_plan(
            &row.task_id,
            &tree.subtasks,
            &mut created_tasks,
            &mut updates,
        )
        .await?;

        // Mark decomposition as applied
        self.repos
            .tasks
            .apply_decomposition(&decomposition_id)
            .await
            .map_err(map_storage_err)?;

        // Add parent task as updated
        updates.push(EntityUpdate {
            kind: desktop_shared::types::EntityKind::Task,
            id: row.task_id,
        });

        Ok((created_tasks, updates))
    }

    /// Reject a pending decomposition plan.
    pub async fn task_reject_decomposition(
        &self,
        decomposition_id: String,
    ) -> Result<(), ApiError> {
        self.repos
            .tasks
            .reject_decomposition(&decomposition_id)
            .await
            .map_err(map_storage_err)?;
        Ok(())
    }

    /// Recursively create subtasks from a decomposition plan.
    fn create_subtasks_from_plan<'a>(
        &'a self,
        parent_id: &'a str,
        planned: &'a [PlannedSubtask],
        created_tasks: &'a mut Vec<TaskResponse>,
        updates: &'a mut Vec<EntityUpdate>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ApiError>> + Send + 'a>>
    {
        Box::pin(async move {
            for subtask in planned {
                let params = TaskCreateParams {
                    title: subtask.title.clone(),
                    area_id: None,
                    project_id: None,
                    priority: subtask.priority,
                    due_date: None,
                    tags: None,
                    parent_id: Some(parent_id.into()),
                    status_label_id: None,
                    group_id: None,
                    task_type: None,
                    acceptance_criteria: None,
                    energy_level: subtask
                        .energy_level
                        .as_ref()
                        .map(|e| format!("{e:?}").to_lowercase()),
                    estimated_minutes: subtask.estimated_minutes,
                };

                let (task_response, task_updates) = self.task_create(params).await?;
                let new_task_id = task_response.id.clone();

                updates.extend(task_updates);
                created_tasks.push(task_response);

                // Recurse for children
                if !subtask.children.is_empty() {
                    self.create_subtasks_from_plan(
                        &new_task_id,
                        &subtask.children,
                        created_tasks,
                        updates,
                    )
                    .await?;
                }
            }
            Ok(())
        })
    }
}

/// Map `PlannedSubtask` domain types to `PlannedSubtaskResponse` for the frontend.
fn map_planned_subtasks(subtasks: &[PlannedSubtask]) -> Vec<PlannedSubtaskResponse> {
    subtasks
        .iter()
        .map(|s| PlannedSubtaskResponse {
            temp_id: s.temp_id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            estimated_minutes: s.estimated_minutes,
            energy_level: s
                .energy_level
                .as_ref()
                .map(|e| format!("{e:?}").to_lowercase()),
            priority: s.priority,
            children: map_planned_subtasks(&s.children),
        })
        .collect()
}
