use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use klynt_execpolicy::Policy;
use klynt_sandbox::SandboxRunner;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolExecute, ToolParams};
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
    params = "BashArgs"
)]
pub struct BashTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
}

impl BashTool {
    pub fn new(
        layer1: Arc<Layer1>,
        policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
    ) -> Self {
        Self {
            layer1,
            policy,
            privacy,
            pending,
            event_tx,
            bus,
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
            event_tx: self.event_tx.as_ref(),
            domain_bus: &self.bus,
            cancel: ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
            request_id,
            args: Some(serde_json::to_value(&args).unwrap_or(serde_json::Value::Null)),
            cwd: args.cwd.clone(),
        };
        let decision = evaluate(guard_ctx, "bash", &args.command).await;
        if !decision.allowed() {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::PermissionDenied(format!("bash denied: {:?}", decision)),
            ));
        }

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
            agent::execution::core::fan_out_event(
                self.event_tx.as_ref(),
                Some(&self.bus),
                AgentEvent::SandboxPolicyApplied {
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
