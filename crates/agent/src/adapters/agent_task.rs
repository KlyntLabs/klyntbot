//! AgentTaskHandler adapter — delegates to AgentTaskRepo.
//!
//! Follows the same dependency inversion pattern as CronHandlerAdapter:
//! the trait is defined in `tools`, the implementation lives here in `agent`.

use async_trait::async_trait;
use common::Result;
use storage::AgentTaskRepo;
use tools::agent_task_tool::AgentTaskHandler;

/// Adapter that implements `AgentTaskHandler` by delegating to `AgentTaskRepo`.
pub struct AgentTaskHandlerImpl {
    repo: AgentTaskRepo,
}

impl AgentTaskHandlerImpl {
    pub fn new(repo: AgentTaskRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl AgentTaskHandler for AgentTaskHandlerImpl {
    async fn list_tasks(&self, session_key: &str) -> Result<String> {
        let tasks = self.repo.list_by_session(session_key).await?;
        if tasks.is_empty() {
            return Ok("No tasks on the board.".to_string());
        }

        let mut lines = vec![format!("Task board ({} tasks):", tasks.len())];
        for t in &tasks {
            let owner = t.owner_agent_id.as_deref().unwrap_or("unassigned");
            lines.push(format!(
                "  [{}] {} — {} (owner: {})",
                t.status, t.id, t.description, owner
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn claim_task(&self, task_id: &str, agent_id: &str) -> Result<String> {
        let task = self.repo.claim(task_id, agent_id).await?;
        Ok(format!(
            "Claimed task '{}' ({}). Status: {}",
            task.description, task.id, task.status
        ))
    }

    async fn complete_task(&self, task_id: &str, result: &str) -> Result<String> {
        let task = self
            .repo
            .update_status(task_id, "completed", Some(result), None)
            .await?;
        Ok(format!(
            "Task '{}' marked completed. Result: {}",
            task.description, result
        ))
    }

    async fn fail_task(&self, task_id: &str, error: &str) -> Result<String> {
        let task = self
            .repo
            .update_status(task_id, "failed", None, Some(error))
            .await?;
        Ok(format!(
            "Task '{}' marked failed. Error: {}",
            task.description, error
        ))
    }
}
