//! Spawn tool for creating subagents.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{Tool, RoutingContext};
use crate::agent::SubagentManager;
use crate::error::{Result, ToolError};

/// Tool to spawn subagents for background task execution.
pub struct SpawnTool {
    subagent_manager: Option<Arc<SubagentManager>>,
}

impl SpawnTool {
    /// Create with a SubagentManager reference
    pub fn with_manager(manager: Arc<SubagentManager>) -> Self {
        Self {
            subagent_manager: Some(manager),
        }
    }
}

#[async_trait]
impl Tool for SpawnTool {
    fn name(&self) -> &str {
        "spawn"
    }

    fn description(&self) -> &str {
        "Spawn a subagent to handle a task in the background. Use this for complex or time-consuming tasks that can run independently. The subagent will complete the task and report back when done."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task for the subagent to complete"
                },
                "label": {
                    "type": "string",
                    "description": "Optional short label for the task (for display)"
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'task' parameter".to_string()))?;

        let label = args
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        debug!("Spawning subagent for task: {}", task);

        let manager = self.subagent_manager.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("SubagentManager not available".to_string())
        })?;

        // Use routing context for result routing
        let result = manager
            .spawn(
                task.to_string(),
                label,
                ctx.channel.as_str().to_string(),
                ctx.chat_id.as_str().to_string(),
            )
            .await;

        Ok(result)
    }
}
