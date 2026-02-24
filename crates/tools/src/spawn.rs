//! Spawn tool for creating subagents.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
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
        profile: String,
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

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Admin
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
                },
                "profile": {
                    "type": "string",
                    "description": "Sub-agent specialization profile. Options: general (default, full access), research (web + read-only files), code (files + shell, no web), analyst (read-only files, pure reasoning)",
                    "enum": ["general", "research", "code", "analyst"]
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let task = p.required_str("task")?;
        let label = p.optional_str("label")?.map(|s| s.to_string());
        let profile = args
            .get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_string();

        debug!("Spawning subagent for task: {} (profile: {})", task, profile);

        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("SpawnHandler not available".to_string()))?;

        // Use routing context for result routing
        let result = handler
            .spawn(
                task.to_string(),
                label,
                profile,
                ctx.channel.as_str().to_string(),
                ctx.chat_id.as_str().to_string(),
            )
            .await;

        Ok(result)
    }
}
