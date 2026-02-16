//! GoalTool — Tool interface for goal management with dependency inversion.

use async_trait::async_trait;
use common::Result;
use goal::{Goal, GoalProgress, GoalStatus};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::{RoutingContext, Tool};
use common::ToolError;

/// GoalHandler trait for dependency inversion.
/// Implemented by GoalHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait GoalHandler: Send + Sync {
    async fn create_goal(&self, goal: Goal) -> Result<Uuid>;
    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>>;
    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>>;
    async fn update_goal(&self, goal: Goal) -> Result<()>;
    async fn delete_goal(&self, id: &Uuid) -> Result<()>;
    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress>;
}

/// GoalTool — Tool interface for strategic goal management.
pub struct GoalTool {
    pub(crate) handler: Option<Arc<dyn GoalHandler>>,
}

impl GoalTool {
    pub fn new(handler: Option<Arc<dyn GoalHandler>>) -> Self {
        Self { handler }
    }
}

fn parse_goal_status(s: &str) -> GoalStatus {
    match s {
        "active" => GoalStatus::Active,
        "paused" => GoalStatus::Paused,
        "achieved" => GoalStatus::Achieved,
        "abandoned" => GoalStatus::Abandoned,
        _ => GoalStatus::Active,
    }
}

#[async_trait]
impl Tool for GoalTool {
    fn name(&self) -> &str {
        "goal"
    }

    fn description(&self) -> &str {
        "Manage strategic goals that span multiple projects. Actions: create, list, show, update, delete, progress."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "show", "update", "delete", "progress"],
                    "description": "The goal action to perform"
                },
                "title": {
                    "type": "string",
                    "description": "Goal title (for create, update)"
                },
                "description": {
                    "type": "string",
                    "description": "Goal description (optional)"
                },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "description": "Goal priority 1-5 (optional)"
                },
                "goal_id": {
                    "type": "string",
                    "description": "Goal ID (for show, update, delete, progress)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "achieved", "abandoned"],
                    "description": "Filter by status (for list)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("GoalHandler not configured".into()))?;

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing action".into()))?;

        match action {
            "list" => {
                let status = args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(parse_goal_status);
                let goals = handler.list_goals(status).await?;
                Ok(format!("Found {} goals", goals.len()))
            }
            _ => Ok(format!("Action '{}' not yet implemented", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_tool_creation_without_handler() {
        let tool = GoalTool::new(None);
        assert!(tool.handler.is_none());
    }

    #[test]
    fn test_goal_tool_creation_with_handler() {
        use crate::Tool;

        // We can't easily mock GoalHandler here without a full mock,
        // but we can verify the tool's metadata
        let tool = GoalTool::new(None);
        assert_eq!(tool.name(), "goal");
        assert!(tool.description().contains("goal"));
        assert!(tool.parameters().get("properties").is_some());
    }

    #[test]
    fn test_goal_tool_parameters_schema() {
        use crate::Tool;

        let tool = GoalTool::new(None);
        let params = tool.parameters();
        let props = params.get("properties").unwrap();

        // Required action field
        assert!(props.get("action").is_some());
        let action = props.get("action").unwrap();
        let action_enum = action.get("enum").unwrap().as_array().unwrap();
        assert!(action_enum.contains(&serde_json::json!("create")));
        assert!(action_enum.contains(&serde_json::json!("list")));
        assert!(action_enum.contains(&serde_json::json!("show")));
        assert!(action_enum.contains(&serde_json::json!("update")));
        assert!(action_enum.contains(&serde_json::json!("delete")));
        assert!(action_enum.contains(&serde_json::json!("progress")));

        // Other fields
        assert!(props.get("title").is_some());
        assert!(props.get("goal_id").is_some());
        assert!(props.get("status").is_some());
    }

    #[tokio::test]
    async fn test_goal_tool_execute_without_handler() {
        use crate::{RoutingContext, Tool};
        use common::ChannelName;

        let tool = GoalTool::new(None);
        let ctx = RoutingContext::new(ChannelName::new("cli"), "test".into());
        let args = serde_json::json!({"action": "list"});

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }
}
