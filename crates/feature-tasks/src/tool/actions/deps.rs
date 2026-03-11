//! Dependency management action handlers (add_dep, remove_dep).

use common::Result;
use tools_core::ParamExtractor;

use super::super::TaskTool;

impl TaskTool {
    pub(crate) async fn handle_add_dep(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let task_id = p.required_str("task_id")?;
        let blocked_by = p.required_str("blocked_by")?;
        let dep_type = p.optional_str("dep_type")?.unwrap_or("blocks");

        self.repo
            .add_dependency(task_id, blocked_by, dep_type)
            .await?;

        let task = self.repo.get(task_id).await?.unwrap();
        let blocker = self.repo.get(blocked_by).await?.unwrap();

        // Log activity
        if self.config.auto_log_activity {
            let _ = self
                .repo
                .log_activity(
                    task_id,
                    "dependency_added",
                    None,
                    None,
                    Some(&format!("blocked by {} ({})", blocker.id, dep_type)),
                    "user",
                    Some(&format!(
                        "Added {} dependency on {}",
                        dep_type, blocker.title
                    )),
                )
                .await;
        }

        Ok(format!(
            "Dependency added: [{}] {} is now blocked by [{}] {} (type: {})",
            task.id, task.title, blocker.id, blocker.title, dep_type
        ))
    }

    pub(crate) async fn handle_remove_dep(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let task_id = p.required_str("task_id")?;
        let blocked_by = p.required_str("blocked_by")?;

        self.repo.remove_dependency(task_id, blocked_by).await?;

        Ok(format!(
            "Dependency removed: {} is no longer blocked by {}",
            task_id, blocked_by
        ))
    }
}
