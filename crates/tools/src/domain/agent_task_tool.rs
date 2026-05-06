//! Agent task tool for subagent coordination.
//!
//! Provides the `AgentTaskHandler` trait (dependency inversion) and `AgentTaskTool`
//! for subagents to interact with the shared task board. Only registered in
//! subagent tool registries, not the parent agent's.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::params::ParamExtractor;
use crate::{approval_class::ApprovalClass, RoutingContext, Tool};
use common::{Result, ToolError};

/// Handler trait for agent task operations (dependency inversion).
/// Defined here in tools crate, implemented in agent crate.
#[async_trait]
pub trait AgentTaskHandler: Send + Sync {
    async fn list_tasks(&self, session_key: &str) -> Result<String>;
    async fn claim_task(&self, task_id: &str, agent_id: &str) -> Result<String>;
    async fn complete_task(&self, task_id: &str, result: &str) -> Result<String>;
    async fn fail_task(&self, task_id: &str, error: &str) -> Result<String>;
}

pub struct AgentTaskTool {
    handler: Arc<dyn AgentTaskHandler>,
    session_key: String,
    agent_id: String,
}

impl AgentTaskTool {
    pub fn new(handler: Arc<dyn AgentTaskHandler>, session_key: String, agent_id: String) -> Self {
        Self {
            handler,
            session_key,
            agent_id,
        }
    }
}

#[async_trait]
impl Tool for AgentTaskTool {
    fn name(&self) -> &str {
        "agent_task"
    }

    fn description(&self) -> &str {
        "Manage your assigned tasks from the task board. Use 'list' to see all tasks, \
         'claim' to take ownership of an unclaimed task, \
         'complete' to mark a task done with results, or 'fail' to report a failure."
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::TaskManagement,
            tags: vec!["task".into(), "todo".into(), "action".into(), "plan".into()],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "claim", "complete", "fail"],
                    "description": "Action to perform on the task board"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task ID (required for claim, complete, fail)"
                },
                "result": {
                    "type": "string",
                    "description": "Result text (for complete) or error message (for fail)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "list" => self.handler.list_tasks(&self.session_key).await,
            "claim" => {
                let task_id = p.required_str("task_id")?;
                self.handler.claim_task(task_id, &self.agent_id).await
            }
            "complete" => {
                let task_id = p.required_str("task_id")?;
                let result = p.str_or("result", "Task completed")?;
                self.handler.complete_task(task_id, result).await
            }
            "fail" => {
                let task_id = p.required_str("task_id")?;
                let error = p.str_or("result", "Task failed")?;
                self.handler.fail_task(task_id, error).await
            }
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }

    fn approval_class(&self, args: &Value) -> ApprovalClass {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("claim" | "complete" | "fail") => ApprovalClass::Sensitive,
            _ => ApprovalClass::Safe,
        }
    }
}
