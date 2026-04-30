use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage::Repos;
use tokio::sync::mpsc;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct EnterPlanModeArgs {
    /// Optional rationale string the agent provides; logged but not enforced.
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ExitPlanModeArgs {
    pub rationale: Option<String>,
}

#[derive(ToolDerive)]
#[tool(
    name = "enter_plan_mode",
    description = "Switch the current session into plan mode (writes/exec denied). \
                   Persists to sessions.approval_mode='plan'. Emits PlanModeChanged.",
    params = "EnterPlanModeArgs",
    permission = "standard",
    category = "System",
    cost = "Free",
    tags = "plan,coding"
)]
pub struct EnterPlanModeTool {
    repos: Repos,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

#[derive(ToolDerive)]
#[tool(
    name = "exit_plan_mode",
    description = "Leave plan mode and resume normal approval evaluation. \
                   Persists sessions.approval_mode='default'. Emits PlanModeChanged.",
    params = "ExitPlanModeArgs",
    permission = "standard",
    category = "System",
    cost = "Free",
    tags = "plan,coding"
)]
pub struct ExitPlanModeTool {
    repos: Repos,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl EnterPlanModeTool {
    pub fn new(repos: Repos, event_tx: Option<mpsc::Sender<AgentEvent>>, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, event_tx, bus }
    }
}
impl ExitPlanModeTool {
    pub fn new(repos: Repos, event_tx: Option<mpsc::Sender<AgentEvent>>, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for EnterPlanModeTool {
    type Params = EnterPlanModeArgs;
    async fn execute(&self, _args: EnterPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let key = ctx.chat_id.as_str().to_string();
        run_enter_for_test(&self.repos, &key,
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone()).await
    }
}
#[async_trait]
impl ToolExecute for ExitPlanModeTool {
    type Params = ExitPlanModeArgs;
    async fn execute(&self, _args: ExitPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let key = ctx.chat_id.as_str().to_string();
        run_exit_for_test(&self.repos, &key,
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone()).await
    }
}

pub async fn run_enter_for_test(
    repos: &Repos, session_key: &str,
    event_tx: mpsc::Sender<AgentEvent>, bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos.sessions.update_approval_mode(session_key, "plan").await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = AgentEvent::PlanModeChanged {
        session_key: session_key.into(), active: true, requested_by: "tool".into(),
    };
    agent::execution::core::fan_out_event(Some(&event_tx), Some(&bus), evt).await;
    Ok("entered plan mode (writes and exec are now denied)".into())
}

pub async fn run_exit_for_test(
    repos: &Repos, session_key: &str,
    event_tx: mpsc::Sender<AgentEvent>, bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos.sessions.update_approval_mode(session_key, "default").await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = AgentEvent::PlanModeChanged {
        session_key: session_key.into(), active: false, requested_by: "tool".into(),
    };
    agent::execution::core::fan_out_event(Some(&event_tx), Some(&bus), evt).await;
    Ok("exited plan mode".into())
}
