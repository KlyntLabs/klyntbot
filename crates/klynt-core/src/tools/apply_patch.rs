use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, FileEditEvent};
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
pub struct ApplyPatchArgs {
    /// File path inside session cwd.
    #[param(required)] pub path: String,
    /// Unified-diff patch text. Headers `---`/`+++` are accepted but the file
    /// is identified by the `path` field, not the diff headers.
    #[param(required)] pub patch: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "apply_patch",
    description = "Apply a unified-diff patch to a single file. Errors if the patch \
                   does not cleanly apply to the current file content.",
    params = "ApplyPatchArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,patch,coding",
    allowed_channels = "coding_only"
)]
pub struct ApplyPatchTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::tool_channel::NonUiPolicy,
}

impl ApplyPatchTool {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, event_tx, bus, non_ui_policy }
    }
}

#[async_trait]
impl ToolExecute for ApplyPatchTool {
    type Params = ApplyPatchArgs;
    async fn execute(&self, args: ApplyPatchArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: ApplyPatchArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
    channel: common::tool_channel::Channel,
    non_ui_policy: common::tool_channel::NonUiPolicy,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel, request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
        channel,
        non_ui_policy,
    };
    let decision = evaluate(guard_ctx, "apply_patch", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
    let patch = diffy::Patch::from_str(&args.patch)
        .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("malformed patch: {e}"))))?;
    let after = diffy::apply(&before, &patch)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("apply: {e}"))))?;
    tokio::fs::write(&resolved, after.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

    emit_file_edit(&Some(event_tx), &bus, FileEditEvent {
        op: "apply_patch", path: &path_str, bytes: after.len() as u64, diff_full: args.patch.clone(),
    }).await;
    Ok(format!("applied patch to {} ({} bytes)", path_str, after.len()))
}
