//! PlanTool — Tool interface for plan management with dependency inversion.

use async_trait::async_trait;
use common::Result;
use plan::Plan;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::{RoutingContext, Tool};
use common::ToolError;

/// PlanHandler trait for dependency inversion.
/// Implemented by PlanHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait PlanHandler: Send + Sync {
    async fn create_plan(
        &self,
        title: &str,
        description: &str,
        session_key: &str,
        goal_id: Option<Uuid>,
    ) -> Result<Plan>;
    async fn get_plan(&self, id: &Uuid) -> Result<Option<Plan>>;
    async fn get_active_plan(&self, session_key: &str) -> Result<Option<Plan>>;
    async fn approve_plan(&self, id: &Uuid) -> Result<Plan>;
    async fn abandon_plan(&self, id: &Uuid) -> Result<()>;
    async fn get_step_context(&self, id: &Uuid) -> Result<String>;
}

/// PlanTool — Tool interface for multi-step plan management.
pub struct PlanTool {
    pub(crate) handler: Option<Arc<dyn PlanHandler>>,
}

impl PlanTool {
    pub fn new(handler: Option<Arc<dyn PlanHandler>>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl Tool for PlanTool {
    fn name(&self) -> &str {
        "plan"
    }

    fn description(&self) -> &str {
        "Manage multi-step execution plans. Actions: create, show, approve, abandon, status."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "show", "approve", "abandon", "status"],
                    "description": "The plan action to perform"
                },
                "title": {
                    "type": "string",
                    "description": "Plan title (for create)"
                },
                "description": {
                    "type": "string",
                    "description": "Plan description (for create)"
                },
                "plan_id": {
                    "type": "string",
                    "description": "Plan ID (for show, approve, abandon)"
                },
                "goal_id": {
                    "type": "string",
                    "description": "Optional goal ID to link plan to (for create)"
                },
                "session_key": {
                    "type": "string",
                    "description": "Session key for plan isolation (for create, status)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("PlanHandler not configured".into()))?;

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing action".into()))?;

        match action {
            "create" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParams("Missing title for create".into()))?;

                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let goal_id = args
                    .get("goal_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());

                let session_key = args
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or(ctx.chat_id.as_str());

                let plan = handler
                    .create_plan(title, description, session_key, goal_id)
                    .await?;
                Ok(format!(
                    "Created plan '{}' (id: {}, status: {:?})",
                    plan.title, plan.id, plan.status
                ))
            }

            "show" => {
                let plan_id = parse_plan_id(&args)?;
                let plan = handler
                    .get_plan(&plan_id)
                    .await?
                    .ok_or_else(|| ToolError::ExecutionFailed(format!("Plan {} not found", plan_id)))?;

                let mut lines = vec![
                    format!("Plan: {}", plan.title),
                    format!("ID: {}", plan.id),
                    format!("Status: {:?}", plan.status),
                ];
                if !plan.description.is_empty() {
                    lines.push(format!("Description: {}", plan.description));
                }
                if let Some(goal_id) = plan.goal_id {
                    lines.push(format!("Linked Goal: {}", goal_id));
                }
                lines.push(format!("Steps: {}", plan.steps.len()));
                lines.push(format!(
                    "Progress: {}/{}",
                    plan.current_step_index,
                    plan.steps.len()
                ));
                lines.push(format!("Created: {}", plan.created_at.format("%Y-%m-%d %H:%M")));
                Ok(lines.join("\n"))
            }

            "approve" => {
                let plan_id = parse_plan_id(&args)?;
                let plan = handler.approve_plan(&plan_id).await?;
                Ok(format!(
                    "Approved plan '{}' (status: {:?})",
                    plan.title, plan.status
                ))
            }

            "abandon" => {
                let plan_id = parse_plan_id(&args)?;
                handler.abandon_plan(&plan_id).await?;
                Ok(format!("Abandoned plan {}", plan_id))
            }

            "status" => {
                let session_key = args
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or(ctx.chat_id.as_str());

                let plan = handler.get_active_plan(session_key).await?;
                match plan {
                    Some(p) => Ok(format!(
                        "Active plan: '{}' (id: {}, status: {:?}, progress: {}/{})",
                        p.title, p.id, p.status, p.current_step_index, p.steps.len()
                    )),
                    None => Ok("No active plan for this session.".to_string()),
                }
            }

            other => Err(ToolError::InvalidParams(format!("Unknown action: {}", other)).into()),
        }
    }
}

/// Parse plan_id from args, returning a UUID.
fn parse_plan_id(args: &Value) -> Result<Uuid> {
    let id_str = args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidParams("Missing plan_id".into()))?;
    Uuid::parse_str(id_str)
        .map_err(|_| ToolError::InvalidParams(format!("Invalid plan_id: {}", id_str)).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;

    #[test]
    fn test_plan_tool_metadata() {
        let tool = PlanTool::new(None);
        assert_eq!(tool.name(), "plan");
        assert!(tool.description().contains("plan"));
        assert!(tool.parameters().get("properties").is_some());
    }

    #[test]
    fn test_plan_tool_parameters_schema() {
        let tool = PlanTool::new(None);
        let params = tool.parameters();
        let props = params.get("properties").unwrap();

        assert!(props.get("action").is_some());
        let action_enum = props["action"]["enum"].as_array().unwrap();
        assert!(action_enum.contains(&serde_json::json!("create")));
        assert!(action_enum.contains(&serde_json::json!("show")));
        assert!(action_enum.contains(&serde_json::json!("approve")));
        assert!(action_enum.contains(&serde_json::json!("abandon")));
        assert!(action_enum.contains(&serde_json::json!("status")));
    }

    #[tokio::test]
    async fn test_execute_without_handler() {
        let tool = PlanTool::new(None);
        let args = serde_json::json!({"action": "status"});
        let ctx = RoutingContext::new(common::ChannelName::new("cli"), "test".into());
        assert!(tool.execute(args, &ctx).await.is_err());
    }
}
