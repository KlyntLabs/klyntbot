//! Subagents multi-action tool — spawn, resume, list, kill.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use common::Result;
use tools_core::{tool_actions, ActionParams, RoutingContext, Tool};

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct SpawnAction {
    /// Short human-readable label for this subagent run (3-8 words).
    pub description: String,
    /// The full task description / prompt the subagent should execute.
    pub prompt: String,
    /// Optional model override (defaults to the parent's effective model).
    #[serde(default)]
    pub model: Option<String>,
    /// Optional per-call turn cap (default 500).
    #[serde(default)]
    pub max_turns: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct ResumeAction {
    pub agent_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct ListAction {
    #[serde(default)]
    pub parent_agent_id: Option<String>,
    /// Optional status filter: 'running' | 'idle' | 'stopped_turn' | 'failed' | 'killed' | 'completed'
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, ActionParams)]
pub struct KillAction {
    pub agent_id: String,
}

/// Trait for subagent operations (dependency inversion to avoid circular dependencies).
/// Implemented by klyntbot-agent's SubagentManager.
#[async_trait]
pub trait SubagentsHandler: Send + Sync {
    async fn spawn(
        &self,
        action: SpawnAction,
        ctx: &RoutingContext,
    ) -> Result<String>;
    async fn resume(
        &self,
        action: ResumeAction,
        ctx: &RoutingContext,
    ) -> Result<String>;
    async fn list(
        &self,
        action: ListAction,
        ctx: &RoutingContext,
    ) -> Result<String>;
    async fn kill(
        &self,
        action: KillAction,
        ctx: &RoutingContext,
    ) -> Result<String>;
}

pub struct SubagentsTool {
    handler: Option<Arc<dyn SubagentsHandler>>,
}

impl SubagentsTool {
    pub fn new() -> Self {
        Self { handler: None }
    }

    pub fn with_handler(handler: Arc<dyn SubagentsHandler>) -> Self {
        Self {
            handler: Some(handler),
        }
    }
}

impl Default for SubagentsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_actions(
    name = "subagents",
    description = "Manage persistent background subagents. spawn: launch a new one (returns immediately, runs in background). list: poll status/progress of all subagents (use this for progress checks). resume: send a follow-up prompt to a subagent that has reached idle or stopped_turn (NOT for status checks). kill: cancel a running subagent.",
    category = "System",
    tags = "agent,delegate,subagent,spawn",
    cost = "Variable"
)]
impl SubagentsTool {
    /// Launch a new subagent. Returns `{agent_id, session_id, status:"running", message}`
    /// as soon as the persistent row is inserted; the subagent's LLM loop runs
    /// detached in the background. Use `subagents list` afterwards to poll
    /// status.
    #[action(name = "spawn")]
    async fn handle_spawn(
        &self,
        params: SpawnAction,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        debug!("Spawning subagent: {}", params.description);
        handler.spawn(params, ctx).await
    }

    /// Send a follow-up prompt to a subagent that has reached `idle` or
    /// `stopped_turn`. NOT for checking on a running subagent — use
    /// `subagents list` for status checks. Calling resume on a running
    /// subagent returns a snapshot instead of resuming.
    #[action(name = "resume")]
    async fn handle_resume(
        &self,
        params: ResumeAction,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.resume(params, ctx).await
    }

    /// List subagents and their current status. This is the right action for
    /// progress polling. Optional filters: `parent_agent_id`, `status`.
    #[action(name = "list")]
    async fn handle_list(
        &self,
        params: ListAction,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.list(params, ctx).await
    }

    /// Cancel a running subagent. Terminal — the row transitions to `killed`.
    #[action(name = "kill")]
    async fn handle_kill(
        &self,
        params: KillAction,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let handler = self.handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("SubagentsHandler not available".to_string())
        })?;
        handler.kill(params, ctx).await
    }
}
