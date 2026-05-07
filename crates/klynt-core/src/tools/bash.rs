use crate::privacy::PrivacyGuard;
use crate::tools::shared::file_edit_event::fan_out_tool_event;
use crate::tools::shared::fs_resolve::resolve_path;
use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use klynt_sandbox::SandboxRunner;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tools_core::events::ToolEvent;
use tools_core::{RoutingContext, ToolExecute, ToolParams};

#[derive(Debug, Clone, serde::Serialize, ToolParams)]
pub struct BashArgs {
    /// Shell command to run via /bin/bash -c.
    #[param(required)]
    pub command: String,

    /// Optional working directory; defaults to session cwd.
    pub cwd: Option<String>,

    /// Optional timeout in milliseconds; defaults to 60_000.
    pub timeout_ms: Option<u64>,
}

#[derive(tools_core::Tool)]
#[tool(
    name = "bash",
    description = "Run a shell command in a sandboxed bash session. \
                   Approval and sandbox rules apply. Output is captured and \
                   truncated to 50KB.",
    params = "BashArgs",
    allowed_channels = "coding_only",
    custom_timeout_secs = "600",
    approval_class = "destructive",
    approval_scope = "command"
)]
#[allow(dead_code)]
pub struct BashTool {
    /// Workspace root that all relative `args.cwd` values resolve against and
    /// that is used as the cwd when `args.cwd` is absent. Symmetric with
    /// `WriteTool`/`EditTool`/`ApplyPatchTool` — the per-turn rebind in
    /// `coding/turn_handler.rs` swaps this in by re-registering the tool.
    /// Never falls back to `std::env::current_dir()`: a process-cwd fallback
    /// previously caused new coding threads to scaffold files inside the
    /// running binary's launch directory instead of the workspace.
    cwd: PathBuf,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    pub history_repo: Option<std::sync::Arc<storage::repos::CodingApprovalHistoryRepo>>,
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub repo_id: String,
}

impl BashTool {
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
            history_repo: None,
            mirror_learning_enabled: false,
            mirror_min_approvals: 5,
            mirror_cooldown_seconds: 86400,
            repo_id: String::new(),
        }
    }
}

#[async_trait]
impl ToolExecute for BashTool {
    type Params = BashArgs;

    async fn execute(&self, args: BashArgs, ctx: &RoutingContext) -> common::Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "bash",
            &args,
            args.cwd.clone(),
        )
        .await
        {
            return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(
                reason,
            )));
        }
        let start = std::time::Instant::now();

        let result: common::Result<String> = (async {
            #[cfg(not(target_os = "macos"))]
            {
                return Err(common::KlyntbotError::NotImplemented(
                    "bash on non-macOS lands in Plan 3".into(),
                ));
            }

            #[cfg(target_os = "macos")]
            {
                let cwd = args
                    .cwd
                    .as_deref()
                    .map(|p| resolve_path(p, &self.cwd))
                    .unwrap_or_else(|| self.cwd.clone());
                let sandbox_policy = klynt_sandbox::SandboxPolicy::cwd_writes_only(cwd.clone());
                fan_out_tool_event(
                    ctx.event_tx.as_ref(),
                    Some(&self.bus),
                    ToolEvent::SandboxPolicyApplied {
                        tool: "bash".into(),
                        policy_summary: sandbox_policy.summary(),
                        policy_hash: sandbox_policy.policy_hash(),
                        fallback_unsandboxed: false,
                        fs_constraints: vec![format!("{:?}", sandbox_policy.fs)],
                        network_constraints: vec![format!("{:?}", sandbox_policy.network)],
                    },
                )
                .await;
                let runner = klynt_sandbox::MacOsSeatbeltRunner::new();
                let timeout = Duration::from_millis(args.timeout_ms.unwrap_or(60_000));
                let out = runner
                    .run_command(
                        &sandbox_policy,
                        "/bin/bash",
                        &["-c", &args.command],
                        Some(&cwd),
                        timeout,
                    )
                    .await
                    .map_err(|e| {
                        common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                            e.to_string(),
                        ))
                    })?;
                Ok(out.stdout)
            }
        })
        .await;
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "bash",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}
