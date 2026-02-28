//! AgentTaskHandler implementation — bridges tools layer to storage layer.
//!
//! Implements the `AgentTaskHandler` trait (defined in `tools` crate)
//! by delegating to `AgentTaskRepo` (defined in `storage` crate).

use async_trait::async_trait;
use storage::AgentTaskRepo;
use tools::agent_task_tool::AgentTaskHandler;

/// Adapter that implements `AgentTaskHandler` using `AgentTaskRepo`.
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
    async fn list_tasks(&self, session_key: &str) -> common::Result<String> {
        let tasks = self.repo.list_by_session(session_key).await?;
        if tasks.is_empty() {
            return Ok("No tasks on the board.".to_string());
        }

        let mut lines = vec![format!("Task board ({} tasks):", tasks.len())];
        for t in &tasks {
            let owner = t
                .owner_agent_id
                .as_deref()
                .unwrap_or("unassigned");
            let blocked: Vec<String> =
                serde_json::from_str(&t.blocked_by).unwrap_or_default();
            let blocked_str = if blocked.is_empty() {
                String::new()
            } else {
                format!(" [blocked by: {}]", blocked.join(", "))
            };
            lines.push(format!(
                "  {} [{}] owner={} — {}{}",
                t.id, t.status, owner, t.description, blocked_str
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn claim_task(&self, task_id: &str, agent_id: &str) -> common::Result<String> {
        let task = self.repo.claim(task_id, agent_id).await?;
        Ok(format!(
            "Claimed task '{}' ({}). Status: {}",
            task.description, task.id, task.status
        ))
    }

    async fn update_task(
        &self,
        task_id: &str,
        status: &str,
        result: Option<&str>,
    ) -> common::Result<String> {
        let task = self
            .repo
            .update_status(task_id, status, result, None)
            .await?;
        Ok(format!(
            "Updated task '{}' — status: {}",
            task.description, task.status
        ))
    }

    async fn complete_task(&self, task_id: &str, result: &str) -> common::Result<String> {
        let task = self
            .repo
            .update_status(task_id, "completed", Some(result), None)
            .await?;
        Ok(format!(
            "Completed task '{}': {}",
            task.description,
            result
        ))
    }

    async fn fail_task(&self, task_id: &str, error: &str) -> common::Result<String> {
        let task = self
            .repo
            .update_status(task_id, "failed", None, Some(error))
            .await?;
        Ok(format!(
            "Failed task '{}': {}",
            task.description, error
        ))
    }
}
