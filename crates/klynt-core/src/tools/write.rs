use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, unified_diff, FileEditEvent};
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct WriteArgs {
    /// File path (relative to session cwd or absolute inside cwd).
    #[param(required)] pub path: String,
    /// New file contents (UTF-8). Replaces existing content if file exists.
    #[param(required)] pub content: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "write",
    description = "Write a UTF-8 text file inside the session cwd. \
                   Replaces existing content. Approval and privacy guard apply.",
    params = "WriteArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,write,coding"
)]
pub struct WriteTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl WriteTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus }
    }
}

#[async_trait]
impl ToolExecute for WriteTool {
    type Params = WriteArgs;

    async fn execute(&self, args: WriteArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(
            args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
        ).await
    }
}

/// Test-friendly runner with fully-explicit deps (mirrors BashTool's pattern).
#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WriteArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();

    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel: cancel.clone(), request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
    };
    let decision = evaluate(guard_ctx, "write", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await.unwrap_or_default();
    if before == args.content {
        return Ok(format!("no change needed for {}", path_str));
    }
    tokio::fs::write(&resolved, args.content.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

    let bytes = args.content.len() as u64;
    let diff = unified_diff(&path_str, &before, &args.content);
    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "write", path: &path_str, bytes, diff_full: diff,
    }).await;

    Ok(format!("wrote {} bytes to {}", bytes, path_str))
}
