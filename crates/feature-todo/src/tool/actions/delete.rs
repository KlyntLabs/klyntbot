//! Delete and delete_recurring action handlers.

use common::{Result, ToolError};
use tools_core::ParamExtractor;

use super::super::TodoTool;

impl TodoTool {
    pub(crate) async fn handle_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let result = if self.repo.delete(id).await? {
            Ok(format!("Deleted task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
        };

        if result.is_ok() {
            self.trigger_sync_async().await;
        }

        result
    }

    pub(crate) async fn handle_delete_recurring(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let template_id = p.required_str("template_id")?;

        match self.repo.get(template_id).await? {
            Some(t) if t.is_template => {
                self.repo.delete(template_id).await?;
                Ok(format!(
                    "Recurring template deleted: [{}] {}. Existing instances are now standalone tasks.",
                    t.id, t.title
                ))
            }
            Some(_) => Err(ToolError::InvalidParams(format!(
                "Task {} is not a recurring template. Use 'delete' for regular tasks.",
                template_id
            ))
            .into()),
            None => Err(
                ToolError::ExecutionFailed(format!("Template not found: {}", template_id)).into(),
            ),
        }
    }
}
