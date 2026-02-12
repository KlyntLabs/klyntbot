//! Spawn tool for creating subagents.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{Tool, ToolContext};
use crate::agent::SubagentManager;
use crate::error::{Result, ToolError};

/// Tool to spawn subagents for background task execution.
pub struct SpawnTool {
    subagent_manager: Option<Arc<SubagentManager>>,
    context: ToolContext,
}

impl SpawnTool {
    pub fn new(context: ToolContext) -> Self {
        Self {
            subagent_manager: None,
            context,
        }
    }

    /// Create with a SubagentManager reference
    pub fn with_manager(manager: Arc<SubagentManager>, context: ToolContext) -> Self {
        Self {
            subagent_manager: Some(manager),
            context,
        }
    }
}

impl Default for SpawnTool {
    fn default() -> Self {
        Self::new(ToolContext::new())
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

    async fn execute(&self, args: Value) -> Result<String> {
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

        // Get current context (this is where results will be routed back)
        let (ctx_channel, ctx_chat_id) = self.context.get();

        let channel = if ctx_channel.is_empty() {
            "cli".to_string()
        } else {
            ctx_channel
        };

        let chat_id = if ctx_chat_id.is_empty() {
            "default".to_string()
        } else {
            ctx_chat_id
        };

        let result = manager
            .spawn(task.to_string(), label, channel, chat_id)
            .await;

        Ok(result)
    }

    /// Set the origin context for routing subagent results
    fn set_context(&self, channel: &str, chat_id: &str) {
        self.context.set(channel, chat_id);
    }
}
