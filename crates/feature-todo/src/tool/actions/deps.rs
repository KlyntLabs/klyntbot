//! Dependency management action handlers (add_dependency, remove_dependency).

use common::Result;
use tools_core::ParamExtractor;

use super::super::TaskTool;

impl TaskTool {
    pub(crate) async fn handle_add_dependency(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let task_id = p.required_str("task_id")?;
        let blocked_by = p.required_str("blocked_by")?;

        self.repo.add_dependency(task_id, blocked_by).await?;

        let task = self.repo.get(task_id).await?.unwrap();
        let blocker = self.repo.get(blocked_by).await?.unwrap();

        Ok(format!(
            "Dependency added: [{}] {} is now blocked by [{}] {}",
            task.id, task.title, blocker.id, blocker.title
        ))
    }

    pub(crate) async fn handle_remove_dependency(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let task_id = p.required_str("task_id")?;
        let blocked_by = p.required_str("blocked_by")?;

        self.repo.remove_dependency(task_id, blocked_by).await?;

        Ok(format!(
            "Dependency removed: {} is no longer blocked by {}",
            task_id, blocked_by
        ))
    }
}
