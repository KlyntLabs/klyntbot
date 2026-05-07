use crate::tools::shared::file_edit_event::fan_out_tool_event;
use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage::Repos;
use tokio::sync::mpsc;
use tools_core::events::ToolEvent;
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
    tags = "plan,coding",
    allowed_channels = "coding_only"
)]
pub struct EnterPlanModeTool {
    repos: Repos,
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
    tags = "plan,coding",
    allowed_channels = "coding_only"
)]
pub struct ExitPlanModeTool {
    repos: Repos,
    bus: Arc<DomainEventBus>,
}

impl EnterPlanModeTool {
    pub fn new(repos: Repos, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, bus }
    }
}
impl ExitPlanModeTool {
    pub fn new(repos: Repos, bus: Arc<DomainEventBus>) -> Self {
        Self { repos, bus }
    }
}

#[async_trait]
impl ToolExecute for EnterPlanModeTool {
    type Params = EnterPlanModeArgs;
    async fn execute(&self, args: EnterPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "enter_plan_mode",
            &args,
            None,
        )
        .await
        {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let key = ctx.chat_id.as_str().to_string();
        let result =
            run_enter_for_test(&self.repos, &key, ctx.event_tx.clone(), self.bus.clone()).await;
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "enter_plan_mode",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}
#[async_trait]
impl ToolExecute for ExitPlanModeTool {
    type Params = ExitPlanModeArgs;
    async fn execute(&self, args: ExitPlanModeArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "exit_plan_mode",
            &args,
            None,
        )
        .await
        {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let key = ctx.chat_id.as_str().to_string();
        let result =
            run_exit_for_test(&self.repos, &key, ctx.event_tx.clone(), self.bus.clone()).await;
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "exit_plan_mode",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}

pub async fn run_enter_for_test(
    repos: &Repos,
    session_key: &str,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
    bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos
        .sessions
        .update_approval_mode(session_key, "plan")
        .await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = ToolEvent::PlanModeChanged {
        in_plan_mode: true,
        plan_id: Some(session_key.into()),
    };
    fan_out_tool_event(event_tx.as_ref(), Some(&bus), evt).await;
    Ok("entered plan mode (writes and exec are now denied)".into())
}

pub async fn run_exit_for_test(
    repos: &Repos,
    session_key: &str,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
    bus: Arc<DomainEventBus>,
) -> Result<String> {
    repos
        .sessions
        .update_approval_mode(session_key, "default")
        .await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
    let evt = ToolEvent::PlanModeChanged {
        in_plan_mode: false,
        plan_id: Some(session_key.into()),
    };
    fan_out_tool_event(event_tx.as_ref(), Some(&bus), evt).await;
    Ok("exited plan mode".into())
}
