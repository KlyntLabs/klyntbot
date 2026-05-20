use crate::privacy::PrivacyGuard;
use crate::tools::shared::file_edit_event::{emit_file_edit, FileEditEvent};
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::hook_emit::{
    fire_post_file_edit, fire_post_tool_use, fire_pre_file_edit, fire_pre_tool_use,
};
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ApplyPatchArgs {
    /// File path inside session cwd.
    #[param(required)]
    pub path: String,
    /// Unified-diff patch text. Headers `---`/`+++` are accepted but the file
    /// is identified by the `path` field, not the diff headers.
    #[param(required)]
    pub patch: String,
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
    allowed_channels = "all",
    approval_class = "destructive",
    approval_scope = "path"
)]
pub struct ApplyPatchTool {
    cwd: PathBuf,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    pub snapshot_repo: Option<std::sync::Arc<crate::snapshots::SnapshotRepo>>,
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub repo_id: String,
}

impl ApplyPatchTool {
    pub fn new(
        cwd: PathBuf,
        policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        Self {
            cwd,
            policy,
            privacy,
            bus,
            non_ui_policy,
            snapshot_repo: None,
            mirror_learning_enabled: false,
            mirror_min_approvals: 5,
            mirror_cooldown_seconds: 86400,
            repo_id: String::new(),
        }
    }
}

#[async_trait]
impl ToolExecute for ApplyPatchTool {
    type Params = ApplyPatchArgs;
    async fn execute(&self, args: ApplyPatchArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        run_for_test(
            args,
            self.cwd.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            ctx.event_tx.clone(),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_default(),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
            ctx.hook_engine.clone(),
            session_id,
            self.snapshot_repo.clone(),
            self.mirror_learning_enabled,
            self.mirror_min_approvals,
            self.mirror_cooldown_seconds,
            self.repo_id.clone(),
            ctx.message_id.clone(),
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: ApplyPatchArgs,
    cwd: PathBuf,
    _policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
    bus: Arc<DomainEventBus>,
    _cancel: CancellationToken,
    _channel: common::tool_channel::Channel,
    _non_ui_policy: common::tool_channel::NonUiPolicy,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    session_id: String,
    snapshot_repo: Option<std::sync::Arc<crate::snapshots::SnapshotRepo>>,
    _mirror_learning_enabled: bool,
    _mirror_min_approvals: u32,
    _mirror_cooldown_seconds: i64,
    _repo_id: String,
    message_id: Option<String>,
) -> Result<String> {
    let resolved = resolve_under_cwd(&args.path, &cwd, &privacy)
        .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;
    let path_str = resolved.to_string_lossy().into_owned();

    if let Some(repo) = snapshot_repo.as_ref() {
        let (content, existed) = match tokio::fs::read(&resolved).await {
            Ok(bytes) => (bytes, true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
            Err(e) => return Err(e.into()),
        };
        let _ = repo
            .try_record_with_ghost(
                &session_id,
                message_id.as_deref(),
                &resolved.to_string_lossy(),
                &content,
                existed,
            )
            .await;
    }

    if let Err(reason) = fire_pre_tool_use(
        hook_engine.as_ref(),
        session_id.clone(),
        "apply_patch",
        &args,
        None,
    )
    .await
    {
        return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
    }
    let start = std::time::Instant::now();
    let result: Result<String> = (async {
        let before = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read: {e}"))))?;
        // Reject obvious non-patches up front. `diffy::Patch::from_str` is too
        // lenient — it accepts arbitrary text as a zero-hunk patch.
        if !args.patch.contains("@@") && !args.patch.contains("---") {
            return Err(KlyntbotError::Tool(ToolError::InvalidParams(
                "malformed patch: missing unified-diff markers".into(),
            )));
        }
        let patch = diffy::Patch::from_str(&args.patch).map_err(|e| {
            KlyntbotError::Tool(ToolError::InvalidParams(format!("malformed patch: {e}")))
        })?;
        let after = diffy::apply(&before, &patch)
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("apply: {e}"))))?;
        let bytes_before = before.len() as u64;
        let bytes_after = after.len() as u64;
        let diff_preview = args.patch.clone();
        let pre_file_result = fire_pre_file_edit(
            hook_engine.as_ref(),
            session_id.clone(),
            "apply_patch",
            &path_str,
            "patch",
            diff_preview.clone(),
            bytes_before,
            bytes_after,
        )
        .await;
        let mut final_content = after;
        match pre_file_result {
            Ok(None) => {}
            Ok(Some(modified)) => {
                if let Some(new_content) = modified.get("content").and_then(|v| v.as_str()) {
                    final_content = new_content.to_string();
                }
            }
            Err(reason) => return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason))),
        }
        crate::tools::shared::fs_resolve::ensure_parent_dir(&resolved).await?;
        let write_result = tokio::fs::write(&resolved, final_content.as_bytes()).await;
        let write_ok = write_result.is_ok();
        fire_post_file_edit(
            hook_engine.as_ref(),
            session_id.clone(),
            "apply_patch",
            &path_str,
            "patch",
            (final_content.len() as i64) - (bytes_before as i64),
            write_ok,
        )
        .await;
        write_result
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("write: {e}"))))?;

        emit_file_edit(
            &event_tx,
            &bus,
            FileEditEvent {
                op: "apply_patch",
                path: &path_str,
                bytes: final_content.len() as u64,
                diff_full: args.patch.clone(),
            },
        )
        .await;
        Ok(format!(
            "applied patch to {} ({} bytes)",
            path_str,
            final_content.len()
        ))
    })
    .await;
    fire_post_tool_use(
        hook_engine.as_ref(),
        session_id,
        "apply_patch",
        result.is_ok(),
        start.elapsed().as_millis() as u64,
    )
    .await;
    result
}
