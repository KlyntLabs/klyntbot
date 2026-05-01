use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::hook_emit::{fire_pre_tool_use, fire_post_tool_use};
use async_trait::async_trait;
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use klynt_sandbox::SandboxRunner;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolExecute, ToolParams};
use tools_core::events::ToolEvent;
use uuid::Uuid;

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
    allowed_channels = "coding_only"
)]
pub struct BashTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    bus: Arc<DomainEventBus>,
    non_ui_policy: common::tool_channel::NonUiPolicy,
}

impl BashTool {
    pub fn new(
        layer1: Arc<Layer1>,
        policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        Self {
            layer1,
            policy,
            privacy,
            pending,
            bus,
            non_ui_policy,
        }
    }
}

#[async_trait]
impl ToolExecute for BashTool {
    type Params = BashArgs;

    async fn execute(&self, args: BashArgs, ctx: &RoutingContext) -> common::Result<String> {
        let request_id = Uuid::new_v4().to_string();
        let guard_ctx = GuardCtx {
            layer1: &self.layer1,
            policy: &self.policy,
            privacy: &self.privacy,
            pending: &self.pending,
            event_tx: ctx.event_tx.as_ref(),
            domain_bus: &self.bus,
            cancel: ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
            request_id,
            args: Some(serde_json::to_value(&args).unwrap_or(serde_json::Value::Null)),
            cwd: args.cwd.clone(),
            channel: common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            non_ui_policy: self.non_ui_policy,
        };
        let decision = evaluate(guard_ctx, "bash", &args.command).await;
        if !decision.allowed() {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::PermissionDenied(format!("bash denied: {:?}", decision)),
            ));
        }

        let session_id = ctx.session_key.clone().map(|s| s.to_string()).unwrap_or_default();
        let args_json = serde_json::to_value(&args).unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(ctx.hook_engine.as_ref(), session_id.clone(), "bash", args_json, args.cwd.clone()).await {
            return Err(common::KlyntbotError::Tool(common::ToolError::HookBlocked(reason)));
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
                    .map(PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap());
                let sandbox_policy = klynt_sandbox::SandboxPolicy::cwd_writes_only(cwd.clone());
                if let Some(ref tx) = ctx.event_tx {
                    let _ = tx.send(ToolEvent::SandboxPolicyApplied {
                        tool: "bash".into(),
                        policy_summary: sandbox_policy.summary(),
                        policy_hash: sandbox_policy.policy_hash(),
                        fallback_unsandboxed: false,
                        fs_constraints: vec![format!("{:?}", sandbox_policy.fs)],
                        network_constraints: vec![format!("{:?}", sandbox_policy.network)],
                    }).await;
                }
                if let Some(ref bus) = Some(&self.bus) {
                    let payload = serde_json::json!({
                        "type": "sandboxPolicyApplied",
                        "tool": "bash",
                        "policySummary": sandbox_policy.summary(),
                        "policyHash": sandbox_policy.policy_hash(),
                        "fallbackUnsandboxed": false,
                        "fsConstraints": vec![format!("{:?}", sandbox_policy.fs)],
                        "networkConstraints": vec![format!("{:?}", sandbox_policy.network)],
                    });
                    bus.publish(bus::DomainEvent::Generic {
                        kind: "agent_event".into(),
                        payload,
                    });
                }
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
        }).await;
        fire_post_tool_use(ctx.hook_engine.as_ref(), session_id, "bash", result.is_ok(), start.elapsed().as_millis() as u64).await;
        result
    }
}
