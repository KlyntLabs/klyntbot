//! Spawn tool for creating subagents.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{RoutingContext, Tool};
use common::{Result, ToolError};

/// Trait for spawning subagents (dependency inversion to avoid circular dependencies).
/// Implemented by klyntbot-agent's SubagentManager.
#[async_trait]
pub trait SpawnHandler: Send + Sync {
    /// Spawn a subagent to handle a task
    async fn spawn(
        &self,
        task: String,
        label: Option<String>,
        origin_channel: String,
        origin_chat_id: String,
    ) -> String;
}

/// Tool to spawn subagents for background task execution.
pub struct SpawnTool {
    handler: Option<Arc<dyn SpawnHandler>>,
}

impl SpawnTool {
    /// Create a new spawn tool without a handler (will error if used)
    pub fn new() -> Self {
        Self { handler: None }
    }

    /// Create with a SpawnHandler implementation
    pub fn with_handler(handler: Arc<dyn SpawnHandler>) -> Self {
        Self {
            handler: Some(handler),
        }
    }
}

impl Default for SpawnTool {
    fn default() -> Self {
        Self::new()
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

        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("SpawnHandler not available".to_string()))?;

        // Use routing context for result routing
        let result = handler
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
