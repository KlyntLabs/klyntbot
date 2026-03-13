//! LLM-backed TaskExecutionHandler implementation.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use chrono::Utc;
use common::Result;
use storage::TaskRepo;
use tracing::{debug, info, warn};
use uuid::Uuid;

use feature_tasks::handlers::TaskExecutionHandler;
use feature_tasks::types::{
    ExecuteResult, ExecutionConfig, ExecutionState, Task, TaskExecution, TaskType,
};

/// Cost budget tiers by complexity score.
fn budget_for_complexity(score: Option<i32>) -> f64 {
    match score.unwrap_or(0) {
        0..=2 => 0.25,
        3..=4 => 1.00,
        5..=6 => 3.00,
        _ => 5.00,
    }
}

/// LLM-powered task execution handler.
pub struct LlmTaskExecutionHandler {
    repo: TaskRepo,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl LlmTaskExecutionHandler {
    pub fn new(repo: TaskRepo, domain_bus: Option<Arc<DomainEventBus>>) -> Self {
        Self { repo, domain_bus }
    }

    async fn fetch_execution(&self, execution_id: &str) -> Result<storage::TaskExecutionRow> {
        self.repo.get_execution(execution_id).await?.ok_or_else(|| {
            common::KlyntbotError::StorageNotFound(format!("Execution {execution_id} not found"))
        })
    }

    fn emit(&self, event: DomainEvent) {
        if let Some(bus) = &self.domain_bus {
            bus.publish(event);
        }
    }
}

#[async_trait]
impl TaskExecutionHandler for LlmTaskExecutionHandler {
    async fn execute(&self, task: &Task, config: &ExecutionConfig) -> Result<ExecuteResult> {
        // Validate task type
        if task.task_type == TaskType::Manual {
            return Err(common::ToolError::InvalidParams(
                "Cannot execute manual tasks. Set task_type to 'agentic' or 'hybrid'.".into(),
            )
            .into());
        }

        // Check execution state
        if !matches!(
            task.execution_state,
            ExecutionState::Idle | ExecutionState::Failed
        ) {
            return Err(common::ToolError::InvalidParams(format!(
                "Task execution_state must be 'idle' or 'failed', got '{}'",
                task.execution_state
            ))
            .into());
        }

        // Approval gate for hybrid tasks or explicit require_approval
        if task.task_type == TaskType::Hybrid || config.require_approval {
            let suggestion_id = Uuid::new_v4().to_string();
            let sugg_row = storage::TaskSuggestionRow {
                id: suggestion_id.clone(),
                task_id: Some(task.id.clone()),
                suggestion_type: "Execute".to_string(),
                title: format!("Approve execution of: {}", task.title),
                description: Some("This task requires approval before agent execution.".into()),
                confidence: 1.0,
                action_payload: Some(serde_json::to_string(config).unwrap_or_else(|e| {
                    warn!("Failed to serialize ExecutionConfig: {e}");
                    "{}".to_string()
                })),
                status: "pending".to_string(),
                trigger: Some("execution_request".to_string()),
                created_at: Utc::now(),
                resolved_at: None,
            };
            self.repo.create_suggestion(&sugg_row).await?;

            // Update execution state
            let patch = storage::TaskPatch {
                id: task.id.clone(),
                execution_state: Some(ExecutionState::AwaitingApproval.to_string()),
                ..Default::default()
            };
            self.repo.update(&patch).await?;

            info!(task_id = %task.id, "Execution awaiting approval");
            return Ok(ExecuteResult::AwaitingApproval { suggestion_id });
        }

        // Compute budget
        let max_cost = config
            .max_cost_usd
            .unwrap_or_else(|| budget_for_complexity(task.complexity_score));

        // Create execution record
        let execution_id = Uuid::new_v4().to_string();
        let exec_row = storage::TaskExecutionRow {
            id: execution_id.clone(),
            task_id: task.id.clone(),
            status: "pending".to_string(),
            agent_profile: config.agent_profile.clone(),
            started_at: Some(Utc::now()),
            completed_at: None,
            duration_secs: None,
            tokens_used: None,
            cost_usd: Some(0.0),
            input_context: task
                .context_snapshot
                .as_ref()
                .and_then(|cs| serde_json::to_string(cs).ok()),
            output_summary: None,
            error_message: None,
            artifacts: None,
            metrics: None,
            retry_count: 0,
            created_at: Utc::now(),
        };
        self.repo.create_execution(&exec_row).await?;

        // Update task state to running
        let patch = storage::TaskPatch {
            id: task.id.clone(),
            execution_state: Some(ExecutionState::Running.to_string()),
            spawned_execution_id: Some(Some(execution_id.clone())),
            ..Default::default()
        };
        self.repo.update(&patch).await?;

        // Log activity
        self.repo
            .log_activity(
                &task.id,
                "execution_started",
                None,
                None,
                None,
                "agent",
                Some(&format!(
                    "Execution {} started (budget: ${:.2})",
                    execution_id, max_cost
                )),
            )
            .await?;

        // Emit domain event
        self.emit(DomainEvent::TaskExecutionStarted {
            task_id: task.id.clone(),
            execution_id: execution_id.clone(),
            agent_profile: config
                .agent_profile
                .clone()
                .unwrap_or_else(|| "task".to_string()),
        });

        debug!(task_id = %task.id, execution_id = %execution_id, "Execution started");

        // NOTE: Actual subagent spawning will be integrated when SpawnHandler
        // is wired into this handler. For now, we create the execution record
        // and update state — the spawn integration is a follow-up task.

        Ok(ExecuteResult::Started { execution_id })
    }

    async fn get_execution(&self, execution_id: &str) -> Result<TaskExecution> {
        let row = self.fetch_execution(execution_id).await?;
        Ok(TaskExecution::from(row))
    }

    async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        let row = self.fetch_execution(execution_id).await?;

        if row.status != "running" && row.status != "pending" {
            return Err(common::ToolError::InvalidParams(format!(
                "Cannot cancel execution in '{}' state",
                row.status
            ))
            .into());
        }

        self.repo
            .update_execution(execution_id, "cancelled", None, None, None)
            .await?;

        // Reset task execution state
        let patch = storage::TaskPatch {
            id: row.task_id.clone(),
            execution_state: Some(ExecutionState::Idle.to_string()),
            spawned_execution_id: Some(None),
            ..Default::default()
        };
        self.repo.update(&patch).await?;

        self.repo
            .log_activity(
                &row.task_id,
                "execution_cancelled",
                None,
                None,
                None,
                "user",
                Some(&format!("Execution {execution_id} cancelled")),
            )
            .await?;

        Ok(())
    }

    async fn retry_execution(&self, execution_id: &str) -> Result<ExecuteResult> {
        let row = self.fetch_execution(execution_id).await?;

        if row.status != "failed" {
            return Err(common::ToolError::InvalidParams(format!(
                "Can only retry failed executions, got '{}'",
                row.status
            ))
            .into());
        }

        // Get the task to re-execute
        let task_row = self.repo.get_or_err(&row.task_id).await?;
        let task = Task::from(task_row);

        let config = ExecutionConfig {
            agent_profile: row.agent_profile.clone(),
            max_cost_usd: None, // Fresh budget — don't reuse prior spend as cap
            require_approval: false,
            ..Default::default()
        };

        self.execute(&task, &config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_for_complexity() {
        assert_eq!(budget_for_complexity(Some(0)), 0.25);
        assert_eq!(budget_for_complexity(Some(2)), 0.25);
        assert_eq!(budget_for_complexity(Some(3)), 1.00);
        assert_eq!(budget_for_complexity(Some(5)), 3.00);
        assert_eq!(budget_for_complexity(Some(7)), 5.00);
        assert_eq!(budget_for_complexity(None), 0.25);
    }
}
