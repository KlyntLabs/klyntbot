//! PlanTool — Tool interface for plan management with dependency inversion.

use async_trait::async_trait;
use common::Result;
use plan::Plan;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::{RoutingContext, Tool};
use common::ToolError;

/// PlanCompletionHandler trait for dependency inversion.
/// Called by AgentLoop after a plan finishes (success or failure).
/// Allows the agent crate to update goal metrics without a circular dep.
#[async_trait]
pub trait PlanCompletionHandler: Send + Sync {
    /// Called once when a plan execution finishes.
    ///
    /// - `plan_id`: the plan that just finished
    /// - `goal_id`: the goal this plan was linked to (may be `None`)
    /// - `success`: whether the plan completed successfully
    /// - `summary`: human-readable summary of what happened
    async fn on_plan_completed(
        &self,
        plan_id: &Uuid,
        goal_id: Option<Uuid>,
        success: bool,
        summary: &str,
    ) -> Result<()>;
}

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
    /// Trigger execution of an approved plan. Returns the plan in Executing state.
    async fn execute_plan(&self, id: &Uuid) -> Result<Plan>;
    /// Auto-generate steps for a newly created plan via LLM decomposition.
    /// Saves the generated steps to the database.  Returns `Ok(())` whether or
    /// not steps were generated — callers must not treat an empty result as an
    /// error.
    async fn generate_steps(&self, plan_id: &Uuid) -> Result<()>;
    /// Generate plan steps as a preview without persisting.
    /// Returns a list of step descriptions for user review.
    async fn preview_steps(&self, description: &str) -> Result<Vec<String>>;
}

/// Result of asking the user to approve a plan.
enum PlanApproval {
    Approved,
    Abandoned,
    /// No interaction channel — fall back to conversational approval.
    NoInteraction,
}

/// Ask the user to approve a plan preview via the interaction channel.
async fn ask_plan_approval(ctx: &RoutingContext, preview: &str) -> PlanApproval {
    use common::{
        AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
    };

    let interaction_tx = match &ctx.interaction_tx {
        Some(tx) => tx,
        None => return PlanApproval::NoInteraction,
    };

    let request = InteractionRequest {
        title: "Plan Review".to_string(),
        questions: vec![Question {
            id: "approval".to_string(),
            title: "Plan".to_string(),
            text: format!("{}\n\nDo you want to create this plan?", preview),
            answer_type: AnswerType::SingleSelect {
                options: vec![
                    AnswerOption {
                        value: "approve".to_string(),
                        label: "Approve".to_string(),
                        description: Some("Save and create this plan".to_string()),
                    },
                    AnswerOption {
                        value: "abandon".to_string(),
                        label: "Abandon".to_string(),
                        description: Some("Discard — nothing saved".to_string()),
                    },
                ],
            },
        }],
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    if interaction_tx
        .send(crate::InteractionBundle {
            request,
            response_tx,
        })
        .await
        .is_err()
    {
        return PlanApproval::NoInteraction;
    }

    match response_rx.await {
        Ok(FormResponse::Completed(answers)) => {
            if let Some(answer) = answers.first() {
                match &answer.value {
                    AnswerValue::Selected { value } if value == "approve" => PlanApproval::Approved,
                    AnswerValue::Selected { value } if value == "abandon" => {
                        PlanApproval::Abandoned
                    }
                    AnswerValue::Skipped => PlanApproval::NoInteraction,
                    _ => PlanApproval::Abandoned,
                }
            } else {
                PlanApproval::Abandoned
            }
        }
        Ok(FormResponse::Cancelled) => PlanApproval::Abandoned,
        Err(_) => PlanApproval::NoInteraction,
    }
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
                    "enum": ["create", "show", "approve", "abandon", "status", "execute"],
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

                let default_session_key = format!("{}:{}", ctx.channel, ctx.chat_id);
                let session_key = args
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_session_key);

                // Preview steps via LLM (without persisting)
                let steps = handler.preview_steps(description).await?;

                // Format preview for user
                let mut preview = format!("**Plan: {}**\n", title);
                if !description.is_empty() {
                    preview.push_str(&format!("Description: {}\n", description));
                }
                if steps.is_empty() {
                    preview.push_str("\n(No steps generated — try a more specific description)\n");
                } else {
                    preview.push_str(&format!("\n**Steps ({}):**\n", steps.len()));
                    for (i, step) in steps.iter().enumerate() {
                        preview.push_str(&format!("{}. {}\n", i + 1, step));
                    }
                }

                // Ask user for approval via interaction channel (or fallback to text)
                let approval_result = ask_plan_approval(ctx, &preview).await;

                match approval_result {
                    PlanApproval::Approved => {
                        let plan = handler
                            .create_plan(title, description, session_key, goal_id)
                            .await?;
                        // Generate and save steps
                        let _ = handler.generate_steps(&plan.id).await;
                        Ok(format!(
                            "Created plan '{}' (id: {}, status: {:?})",
                            plan.title, plan.id, plan.status
                        ))
                    }
                    PlanApproval::Abandoned => {
                        Ok("Plan abandoned — nothing was saved.".to_string())
                    }
                    PlanApproval::NoInteraction => {
                        // Non-TTY: present preview and instruct LLM to ask conversationally
                        Ok(format!(
                            "{}\n\nPlease ask the user if they want to approve this plan, \
                             revise the description, or abandon it. Do NOT save the plan until \
                             the user explicitly approves.",
                            preview
                        ))
                    }
                }
            }

            "show" => {
                let plan_id = parse_plan_id(&args)?;
                let plan = handler.get_plan(&plan_id).await?.ok_or_else(|| {
                    ToolError::ExecutionFailed(format!("Plan {} not found", plan_id))
                })?;

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
                lines.push(format!(
                    "Created: {}",
                    plan.created_at.format("%Y-%m-%d %H:%M")
                ));
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
                let default_session_key = format!("{}:{}", ctx.channel, ctx.chat_id);
                let session_key = args
                    .get("session_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&default_session_key);

                let plan = handler.get_active_plan(session_key).await?;
                match plan {
                    Some(p) => Ok(format!(
                        "Active plan: '{}' (id: {}, status: {:?}, progress: {}/{})",
                        p.title,
                        p.id,
                        p.status,
                        p.current_step_index,
                        p.steps.len()
                    )),
                    None => Ok("No active plan for this session.".to_string()),
                }
            }

            "execute" => {
                let plan_id = parse_plan_id(&args)?;
                let plan = handler.execute_plan(&plan_id).await?;
                Ok(format!(
                    "Executing plan '{}' (id: {}, status: {:?})",
                    plan.title, plan.id, plan.status
                ))
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
        assert!(action_enum.contains(&serde_json::json!("execute")));
    }

    #[tokio::test]
    async fn test_execute_without_handler() {
        let tool = PlanTool::new(None);
        let args = serde_json::json!({"action": "status"});
        let ctx = RoutingContext::new(common::ChannelName::new("cli"), "test".into());
        assert!(tool.execute(args, &ctx).await.is_err());
    }
}
