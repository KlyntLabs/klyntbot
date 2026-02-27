//! Execute action — evaluate task complexity and start or suggest a plan.

use common::{Result, ToolError};
use storage::TodoPatch;
use tools_core::ParamExtractor;

use super::super::TodoTool;
use crate::task_complexity::evaluate_task_complexity;

impl TodoTool {
    pub(crate) async fn handle_execute(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let task = self
            .repo
            .get(id)
            .await?
            .ok_or_else(|| ToolError::InvalidParams(format!("Task not found: {}", id)))?;

        let signals = evaluate_task_complexity(&self.repo, id).await;

        if signals.is_plan_worthy(3) {
            Ok(format!(
                "Task '{}' is complex (score: {}/6). It has {} subtask(s) and {} dependencies. \
                 I recommend executing this as a plan. Say 'create a plan for task {}' to proceed.",
                task.title,
                signals.complexity_score(),
                signals.subtask_count,
                if signals.has_dependencies {
                    "active"
                } else {
                    "no"
                },
                id
            ))
        } else {
            // Simple task — mark as doing
            self.repo
                .update(&TodoPatch {
                    id: id.to_string(),
                    status: Some("doing".to_string()),
                    ..Default::default()
                })
                .await?;
            Ok(format!(
                "Started working on task '{}'. Marked as doing.",
                task.title
            ))
        }
    }
}
