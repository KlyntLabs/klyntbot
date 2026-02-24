//! GoalTool — Tool interface for goal management with dependency inversion.

use async_trait::async_trait;
use chrono::Utc;
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
    /// Decompose a goal into an execution plan via LLM. Creates a Draft plan linked to
    /// this goal and returns a summary string. Returns a graceful message (not an error)
    /// when a provider or plan repository is not configured.
    async fn decompose_goal(&self, goal_id: &Uuid) -> Result<String>;
    /// Show progress of all plans linked to this goal — plan statuses and step completion.
    /// Returns a graceful message when no plan repository is configured.
    async fn goal_progress(&self, goal_id: &Uuid) -> Result<String>;
    /// Return plan-completion metrics for a goal: completion rate, avg duration, last activity.
    async fn goal_metrics(&self, goal_id: &Uuid) -> Result<String>;
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

/// Parse goal_id from args, returning a UUID.
fn parse_goal_id(args: &Value) -> Result<Uuid> {
    let id_str = args
        .get("goal_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidParams("Missing goal_id".into()))?;
    Uuid::parse_str(id_str)
        .map_err(|_| ToolError::InvalidParams(format!("Invalid goal_id: {}", id_str)).into())
}

#[async_trait]
impl Tool for GoalTool {
    fn name(&self) -> &str {
        "goal"
    }

    fn description(&self) -> &str {
        "Manage strategic goals that span multiple projects. Actions: create, list, show, update, delete, progress, decompose, status, metrics."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "show", "update", "delete", "progress", "decompose", "status", "metrics"],
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
            "create" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParams("Missing title for create".into()))?;

                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let priority = args.get("priority").and_then(|v| v.as_u64()).unwrap_or(3) as u8;

                let now = Utc::now();
                let goal = Goal {
                    id: Uuid::new_v4(),
                    title: title.to_string(),
                    description: description.to_string(),
                    status: GoalStatus::Active,
                    priority,
                    target_date: None,
                    created_at: now,
                    updated_at: now,
                    plans_completed: 0,
                    plans_failed: 0,
                    avg_duration_ms: None,
                    last_plan_at: None,
                    linked_project_ids: vec![],
                };

                goal.validate_priority()?;
                let id = handler.create_goal(goal).await?;
                Ok(format!("Created goal '{}' (id: {})", title, id))
            }

            "list" => {
                let status = args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<GoalStatus>().ok());
                let goals = handler.list_goals(status).await?;

                if goals.is_empty() {
                    return Ok("No goals found.".to_string());
                }

                let mut lines = vec![format!("Found {} goal(s):", goals.len())];
                for g in &goals {
                    lines.push(format!(
                        "- [{}] {} (priority: {}, status: {})",
                        &g.id.to_string()[..8],
                        g.title,
                        g.priority,
                        g.status,
                    ));
                }
                Ok(lines.join("\n"))
            }

            "show" => {
                let id = parse_goal_id(&args)?;
                let goal = handler
                    .get_goal(&id)
                    .await?
                    .ok_or_else(|| ToolError::ExecutionFailed(format!("Goal {} not found", id)))?;

                let mut lines = vec![
                    format!("Goal: {}", goal.title),
                    format!("ID: {}", goal.id),
                    format!("Status: {}", goal.status),
                    format!("Priority: {}", goal.priority),
                ];
                if !goal.description.is_empty() {
                    lines.push(format!("Description: {}", goal.description));
                }
                if let Some(target) = goal.target_date {
                    lines.push(format!("Target: {}", target.format("%Y-%m-%d")));
                }
                let total_plans = goal.plans_completed + goal.plans_failed;
                if total_plans > 0 {
                    lines.push(format!(
                        "Plans: {} completed, {} failed",
                        goal.plans_completed, goal.plans_failed
                    ));
                }
                lines.push(format!(
                    "Created: {}",
                    goal.created_at.format("%Y-%m-%d %H:%M")
                ));
                Ok(lines.join("\n"))
            }

            "update" => {
                let id = parse_goal_id(&args)?;
                let mut goal = handler
                    .get_goal(&id)
                    .await?
                    .ok_or_else(|| ToolError::ExecutionFailed(format!("Goal {} not found", id)))?;

                if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
                    goal.title = title.to_string();
                }
                if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
                    goal.description = desc.to_string();
                }
                if let Some(p) = args.get("priority").and_then(|v| v.as_u64()) {
                    goal.priority = p as u8;
                }
                if let Some(status_str) = args.get("status").and_then(|v| v.as_str()) {
                    goal.status = status_str.parse::<GoalStatus>().map_err(|_| {
                        ToolError::InvalidParams(format!("Invalid status: {}", status_str))
                    })?;
                }

                goal.validate_priority()?;
                goal.updated_at = Utc::now();
                handler.update_goal(goal).await?;
                Ok(format!("Updated goal {}", id))
            }

            "delete" => {
                let id = parse_goal_id(&args)?;
                handler.delete_goal(&id).await?;
                Ok(format!("Deleted goal {}", id))
            }

            "progress" => {
                let id = parse_goal_id(&args)?;
                let progress = handler.calculate_progress(&id).await?;

                let mut lines = vec![format!(
                    "Goal progress: {:.1}%",
                    progress.completion_percentage
                )];
                if !progress.summary.is_empty() {
                    lines.push(format!("Summary: {}", progress.summary));
                }
                Ok(lines.join("\n"))
            }

            "decompose" => {
                let id = parse_goal_id(&args)?;
                let result = handler.decompose_goal(&id).await?;
                Ok(result)
            }

            "status" => {
                let id = parse_goal_id(&args)?;
                let result = handler.goal_progress(&id).await?;
                Ok(result)
            }

            "metrics" => {
                let id = parse_goal_id(&args)?;
                handler.goal_metrics(&id).await
            }

            other => Err(ToolError::InvalidParams(format!("Unknown action: {}", other)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goal::GoalProgress;
    use std::sync::Mutex;

    /// Mock GoalHandler for testing all actions.
    struct MockGoalHandler {
        goals: Mutex<Vec<Goal>>,
    }

    impl MockGoalHandler {
        fn new() -> Self {
            Self {
                goals: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl GoalHandler for MockGoalHandler {
        async fn create_goal(&self, goal: Goal) -> Result<Uuid> {
            let id = goal.id;
            self.goals.lock().unwrap().push(goal);
            Ok(id)
        }

        async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>> {
            Ok(self
                .goals
                .lock()
                .unwrap()
                .iter()
                .find(|g| g.id == *id)
                .cloned())
        }

        async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
            let goals = self.goals.lock().unwrap();
            Ok(match status {
                Some(s) => goals.iter().filter(|g| g.status == s).cloned().collect(),
                None => goals.clone(),
            })
        }

        async fn update_goal(&self, goal: Goal) -> Result<()> {
            let mut goals = self.goals.lock().unwrap();
            if let Some(existing) = goals.iter_mut().find(|g| g.id == goal.id) {
                *existing = goal;
            }
            Ok(())
        }

        async fn delete_goal(&self, id: &Uuid) -> Result<()> {
            self.goals.lock().unwrap().retain(|g| g.id != *id);
            Ok(())
        }

        async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
            Ok(GoalProgress {
                goal_id: *id,
                completion_percentage: 50.0,
                summary: "Half done".to_string(),
            })
        }

        async fn decompose_goal(&self, _goal_id: &Uuid) -> Result<String> {
            Ok("Decomposed goal into 3 steps (mock)".to_string())
        }

        async fn goal_progress(&self, _goal_id: &Uuid) -> Result<String> {
            Ok("No plans linked to this goal.".to_string())
        }

        async fn goal_metrics(&self, goal_id: &Uuid) -> Result<String> {
            let goals = self.goals.lock().unwrap();
            let goal = goals
                .iter()
                .find(|g| g.id == *goal_id)
                .ok_or_else(|| ToolError::ExecutionFailed(format!("Goal {} not found", goal_id)))?;
            Ok(format!(
                "Goal: \"{}\"\nCompletion rate: No plans executed yet\nAverage plan duration: N/A\nLast activity: Never\nActive plans: 0",
                goal.title
            ))
        }
    }

    fn make_tool() -> (GoalTool, Arc<MockGoalHandler>) {
        let handler = Arc::new(MockGoalHandler::new());
        let tool = GoalTool::new(Some(handler.clone() as Arc<dyn GoalHandler>));
        (tool, handler)
    }

    fn ctx() -> RoutingContext {
        RoutingContext::new(common::ChannelName::new("cli"), "test".into())
    }

    #[test]
    fn test_goal_tool_creation_without_handler() {
        let tool = GoalTool::new(None);
        assert!(tool.handler.is_none());
    }

    #[test]
    fn test_goal_tool_metadata() {
        use crate::Tool;

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

        assert!(props.get("action").is_some());
        let action_enum = props["action"]["enum"].as_array().unwrap();
        assert!(action_enum.contains(&serde_json::json!("create")));
        assert!(action_enum.contains(&serde_json::json!("list")));
        assert!(action_enum.contains(&serde_json::json!("show")));
        assert!(action_enum.contains(&serde_json::json!("update")));
        assert!(action_enum.contains(&serde_json::json!("delete")));
        assert!(action_enum.contains(&serde_json::json!("progress")));
    }

    #[tokio::test]
    async fn test_execute_without_handler() {
        use crate::Tool;

        let tool = GoalTool::new(None);
        let args = serde_json::json!({"action": "list"});
        assert!(tool.execute(args, &ctx()).await.is_err());
    }

    #[tokio::test]
    async fn test_create_action() {
        use crate::Tool;

        let (tool, handler) = make_tool();
        let result = tool.execute(
            serde_json::json!({"action": "create", "title": "Ship v2", "description": "Release 2.0", "priority": 2}),
            &ctx(),
        ).await.unwrap();

        assert!(result.contains("Created goal 'Ship v2'"));
        let goals = handler.goals.lock().unwrap();
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].priority, 2);
        assert_eq!(goals[0].description, "Release 2.0");
    }

    #[tokio::test]
    async fn test_create_action_defaults() {
        use crate::Tool;

        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Minimal"}),
            &ctx(),
        )
        .await
        .unwrap();

        let goals = handler.goals.lock().unwrap();
        assert_eq!(goals[0].priority, 3);
        assert!(goals[0].description.is_empty());
    }

    #[tokio::test]
    async fn test_create_action_missing_title() {
        use crate::Tool;
        let (tool, _) = make_tool();
        assert!(tool
            .execute(serde_json::json!({"action": "create"}), &ctx())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_list_empty() {
        use crate::Tool;
        let (tool, _) = make_tool();
        let result = tool
            .execute(serde_json::json!({"action": "list"}), &ctx())
            .await
            .unwrap();
        assert_eq!(result, "No goals found.");
    }

    #[tokio::test]
    async fn test_list_with_goals() {
        use crate::Tool;
        let (tool, _) = make_tool();

        tool.execute(
            serde_json::json!({"action": "create", "title": "A"}),
            &ctx(),
        )
        .await
        .unwrap();
        tool.execute(
            serde_json::json!({"action": "create", "title": "B"}),
            &ctx(),
        )
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({"action": "list"}), &ctx())
            .await
            .unwrap();
        assert!(result.contains("Found 2 goal(s)"));
        assert!(result.contains("A"));
        assert!(result.contains("B"));
    }

    #[tokio::test]
    async fn test_show_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Show Me", "description": "Desc", "priority": 1}),
            &ctx(),
        ).await.unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        let result = tool
            .execute(serde_json::json!({"action": "show", "goal_id": id}), &ctx())
            .await
            .unwrap();

        assert!(result.contains("Goal: Show Me"));
        assert!(result.contains("Priority: 1"));
        assert!(result.contains("Description: Desc"));
    }

    #[tokio::test]
    async fn test_show_not_found() {
        use crate::Tool;
        let (tool, _) = make_tool();
        let result = tool
            .execute(
                serde_json::json!({"action": "show", "goal_id": Uuid::new_v4().to_string()}),
                &ctx(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Original"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        tool.execute(
            serde_json::json!({"action": "update", "goal_id": id, "title": "Updated", "priority": 1, "status": "paused"}),
            &ctx(),
        ).await.unwrap();

        let goals = handler.goals.lock().unwrap();
        assert_eq!(goals[0].title, "Updated");
        assert_eq!(goals[0].priority, 1);
        assert_eq!(goals[0].status, GoalStatus::Paused);
    }

    #[tokio::test]
    async fn test_delete_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Bye"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        tool.execute(
            serde_json::json!({"action": "delete", "goal_id": id}),
            &ctx(),
        )
        .await
        .unwrap();
        assert!(handler.goals.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_progress_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Track"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        let result = tool
            .execute(
                serde_json::json!({"action": "progress", "goal_id": id}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("50.0%"));
        assert!(result.contains("Half done"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        use crate::Tool;
        let (tool, _) = make_tool();
        assert!(tool
            .execute(serde_json::json!({"action": "invalid"}), &ctx())
            .await
            .is_err());
    }

    #[test]
    fn test_parse_goal_id_valid() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_goal_id(&serde_json::json!({"goal_id": id.to_string()})).unwrap(),
            id
        );
    }

    #[test]
    fn test_parse_goal_id_missing() {
        assert!(parse_goal_id(&serde_json::json!({})).is_err());
    }

    #[test]
    fn test_parse_goal_id_invalid() {
        assert!(parse_goal_id(&serde_json::json!({"goal_id": "not-a-uuid"})).is_err());
    }

    #[tokio::test]
    async fn test_decompose_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Big Goal"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        let result = tool
            .execute(
                serde_json::json!({"action": "decompose", "goal_id": id}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("Decomposed"));
    }

    #[tokio::test]
    async fn test_status_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Track Goal"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        let result = tool
            .execute(
                serde_json::json!({"action": "status", "goal_id": id}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("No plans linked"));
    }

    #[test]
    fn test_goal_tool_parameters_schema_includes_new_actions() {
        use crate::Tool;
        let tool = GoalTool::new(None);
        let params = tool.parameters();
        let action_enum = params["properties"]["action"]["enum"].as_array().unwrap();
        assert!(action_enum.contains(&serde_json::json!("decompose")));
        assert!(action_enum.contains(&serde_json::json!("status")));
        assert!(action_enum.contains(&serde_json::json!("metrics")));
    }

    #[tokio::test]
    async fn test_metrics_action() {
        use crate::Tool;
        let (tool, handler) = make_tool();
        tool.execute(
            serde_json::json!({"action": "create", "title": "Health Check Goal"}),
            &ctx(),
        )
        .await
        .unwrap();

        let id = handler.goals.lock().unwrap()[0].id.to_string();
        let result = tool
            .execute(
                serde_json::json!({"action": "metrics", "goal_id": id}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(result.contains("Health Check Goal"));
        assert!(result.contains("Completion rate"));
        assert!(result.contains("Average plan duration"));
        assert!(result.contains("Last activity"));
        assert!(result.contains("Active plans"));
    }
}
