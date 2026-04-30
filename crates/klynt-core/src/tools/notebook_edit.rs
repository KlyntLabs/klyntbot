use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::file_edit_event::{emit_file_edit, unified_diff, FileEditEvent};
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
use tools_core::events::ToolEvent;
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct NotebookEditArgs {
    /// .ipynb path.
    #[param(required)] pub path: String,
    /// 0-indexed cell to modify.
    #[param(required)] pub cell_index: u64,
    /// New cell source (replaces existing source array with a single string).
    #[param(required)] pub new_source: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "notebook_edit",
    description = "Replace the `source` of a cell in a Jupyter notebook (.ipynb). \
                   Preserves outputs; updates only the targeted cell.",
    params = "NotebookEditArgs",
    permission = "elevated",
    category = "FileSystem",
    cost = "Free",
    tags = "notebook,jupyter,coding",
    allowed_channels = "coding_only"
)]
pub struct NotebookEditTool {
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::tool_channel::NonUiPolicy,
}

impl NotebookEditTool {
    pub fn new(
        cwd: PathBuf, layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        Self { cwd, layer1, policy, privacy, pending, bus, non_ui_policy }
    }
}

#[async_trait]
impl ToolExecute for NotebookEditTool {
    type Params = NotebookEditArgs;
    async fn execute(&self, args: NotebookEditArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.cwd.clone(), self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            ctx.event_tx.clone(),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: NotebookEditArgs,
    cwd: PathBuf,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
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
        pending: &pending, event_tx: event_tx.as_ref(), domain_bus: &bus,
        cancel, request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: Some(cwd.to_string_lossy().into_owned()),
        channel,
        non_ui_policy,
    };
    let decision = evaluate(guard_ctx, "notebook_edit", &path_str).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let before = tokio::fs::read_to_string(&resolved).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
    let mut nb: serde_json::Value = serde_json::from_str(&before)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("parse ipynb: {e}"))))?;
    let cells = nb.get_mut("cells").and_then(|v| v.as_array_mut())
        .ok_or_else(|| KlyntbotError::Tool(ToolError::ExecutionFailed("ipynb missing cells array".into())))?;
    let idx = args.cell_index as usize;
    if idx >= cells.len() {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            format!("cell_index {idx} out of range (notebook has {} cells)", cells.len()))));
    }
    let old_source = cells[idx].get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if old_source == args.new_source {
        return Ok(format!("no change needed for cell {} in {}", idx, path_str));
    }
    cells[idx]["source"] = serde_json::Value::String(args.new_source.clone());
    let after = serde_json::to_string_pretty(&nb)
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;

    tokio::fs::write(&resolved, after.as_bytes()).await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;
    let diff = unified_diff(&path_str, &before, &after);
    emit_file_edit(&event_tx, &bus, FileEditEvent {
        op: "notebook_edit", path: &path_str, bytes: after.len() as u64, diff_full: diff,
    }).await;
    Ok(format!("edited cell {} in {}", idx, path_str))
}
