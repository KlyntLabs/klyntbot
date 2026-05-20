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

    /// When true, returns immediately with a job_id; output read via coding_task_output.
    pub run_in_background: Option<bool>,

    /// Required when run_in_background=true. Short human-readable label.
    pub description: Option<String>,

    /// When true, skip the auto-injected completion notification.
    pub silent_completion: Option<bool>,

    /// Allocate a PTY for the command. Only valid with run_in_background=true.
    /// Enables ANSI/colour passthrough and accepts stdin via coding_task_stdin.
    pub tty: Option<bool>,

    /// PTY rows. Default 24. Only valid when tty=true.
    pub tty_rows: Option<u16>,

    /// PTY cols. Default 80. Only valid when tty=true.
    pub tty_cols: Option<u16>,
}

#[derive(tools_core::Tool)]
#[tool(
    name = "bash",
    description = "Run a shell command in a sandboxed bash session. \
                   Approval and sandbox rules apply. Output is captured and \
                   truncated to 50KB.",
    params = "BashArgs",
    allowed_channels = "all",
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
            if args.tty.unwrap_or(false) && !args.run_in_background.unwrap_or(false) {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::ExecutionFailed(
                        "tty=true requires run_in_background=true".into(),
                    ),
                ));
            }
            if args.run_in_background.unwrap_or(false) {
                return self.execute_background(args, ctx).await;
            }
            self.execute_foreground(args, ctx).await
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

impl BashTool {
    async fn execute_background(
        &self,
        args: BashArgs,
        ctx: &RoutingContext,
    ) -> common::Result<String> {
        let supervisor = ctx.job_supervisor.as_ref().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "background jobs disabled".into(),
            ))
        })?;
        let description = args.description.clone().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "description required when run_in_background=true".into(),
            ))
        })?;
        if description.is_empty() || description.len() > 120 {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed("description must be 1-120 chars".into()),
            ));
        }

        // ---------- 2.3c PTY validation ----------
        let tty = args.tty.unwrap_or(false);
        if (args.tty_rows.is_some() || args.tty_cols.is_some()) && !tty {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(
                    "tty_rows/tty_cols require tty=true".into(),
                ),
            ));
        }
        let mut warnings: Vec<String> = Vec::new();
        let clamp = |v: u16, lo: u16, hi: u16, name: &str, warnings: &mut Vec<String>| -> u16 {
            if v < lo {
                warnings.push(format!("{name} clamped from {v} to {lo}"));
                lo
            } else if v > hi {
                warnings.push(format!("{name} clamped from {v} to {hi}"));
                hi
            } else {
                v
            }
        };
        let tty_rows = if tty {
            Some(clamp(
                args.tty_rows.unwrap_or(24),
                tools_core::PTY_ROWS_MIN,
                tools_core::PTY_ROWS_MAX,
                "tty_rows",
                &mut warnings,
            ))
        } else {
            None
        };
        let tty_cols = if tty {
            Some(clamp(
                args.tty_cols.unwrap_or(80),
                tools_core::PTY_COLS_MIN,
                tools_core::PTY_COLS_MAX,
                "tty_cols",
                &mut warnings,
            ))
        } else {
            None
        };
        // ----------------------------------------

        let cwd = args
            .cwd
            .as_deref()
            .map(|p| resolve_path(p, &self.cwd))
            .unwrap_or_else(|| self.cwd.clone());
        let spec = tools_core::JobSpec {
            session_id: ctx.chat_id.as_str().to_string(),
            agent_id: ctx.agent_id.clone(),
            agent_chain: ctx.agent_chain.clone(),
            description,
            command: args.command,
            cwd,
            timeout_ms: args.timeout_ms.unwrap_or(600_000),
            silent_completion: args.silent_completion.unwrap_or(false),
            tty,
            tty_rows,
            tty_cols,
        };
        let view = supervisor.spawn(spec).await.map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "spawn failed: {e}"
            )))
        })?;
        let banner = if tty {
            format!(
                "Started background PTY job {}.\nDescription: {}\nTTY: {} rows × {} cols\nSend stdin:    coding_task_stdin(\"{}\", \"y\\n\")\nResize:        coding_task_resize(\"{}\", rows, cols)\nInspect output: coding_task_output(\"{}\")\nCancel:        coding_task_stop(\"{}\")\n\nThe user may attach via JobsPanel. While attached, defer stdin to them.",
                view.id.as_str(),
                view.description,
                tty_rows.unwrap_or(24),
                tty_cols.unwrap_or(80),
                view.id.as_str(),
                view.id.as_str(),
                view.id.as_str(),
                view.id.as_str(),
            )
        } else {
            format!(
                "Started background job {}.\nDescription: {}\nInspect:    coding_task_output(\"{}\")\nCancel:     coding_task_stop(\"{}\")\n\nThis job will auto-notify on completion.",
                view.id.as_str(),
                view.description,
                view.id.as_str(),
                view.id.as_str(),
            )
        };
        let mut out = banner;
        if !warnings.is_empty() {
            out.push_str("\n\nWarnings:\n- ");
            out.push_str(&warnings.join("\n- "));
        }
        Ok(out)
    }

    async fn execute_foreground(
        &self,
        args: BashArgs,
        ctx: &RoutingContext,
    ) -> common::Result<String> {
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
                    common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(e.to_string()))
                })?;
            Ok(out.stdout)
        }
    }
}
