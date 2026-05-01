use crate::approval::host_cache::{HostApprovalCache, HostCheckResult, HostDecision, HostKey};
use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct WebFetchArgs {
    /// http(s) URL to fetch.
    #[param(required)]
    pub url: String,
    /// "text" (default — strip HTML to plain text via html2text) or "raw".
    pub format: Option<String>,
    /// Hard cap on response body bytes (default 200_000).
    pub max_bytes: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "web_fetch",
    description = "Fetch a URL via HTTP GET and return the body. \
                   Default `format=\"text\"` strips HTML tags to readable text. \
                   Approval gated — every fetch consults the approval layers.",
    params = "WebFetchArgs",
    permission = "standard",
    category = "Web",
    cost = "Low",
    tags = "web,fetch,coding"
)]
pub struct WebFetchTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    bus: Arc<DomainEventBus>,
    client: reqwest::Client,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    host_cache: Arc<HostApprovalCache>,
}

impl WebFetchTool {
    pub fn new(
        layer1: Arc<Layer1>,
        policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
        host_cache: Arc<HostApprovalCache>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client construction");
        Self {
            layer1,
            policy,
            privacy,
            pending,
            bus,
            client,
            non_ui_policy,
            host_cache,
        }
    }
}

#[async_trait]
impl ToolExecute for WebFetchTool {
    type Params = WebFetchArgs;
    async fn execute(&self, args: WebFetchArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        run_for_test(
            args,
            self.layer1.clone(),
            self.policy.clone(),
            self.privacy.clone(),
            self.pending.clone(),
            ctx.event_tx.clone(),
            self.bus.clone(),
            ctx.cancel_token
                .clone()
                .unwrap_or_else(CancellationToken::new),
            self.client.clone(),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
            self.host_cache.clone(),
            ctx.hook_engine.clone(),
            session_id,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WebFetchArgs,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
    client: reqwest::Client,
    channel: common::tool_channel::Channel,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    host_cache: Arc<HostApprovalCache>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    session_id: String,
) -> Result<String> {
    let host_key = HostKey::from_url(&args.url)?;
    let host_decision = match host_cache.check_or_register(host_key.clone()) {
        HostCheckResult::Cached(d) => d,
        HostCheckResult::AwaitPending(mut rx) => {
            rx.changed().await.map_err(|_| {
                KlyntbotError::Tool(ToolError::ExecutionFailed("host approval cancelled".into()))
            })?;
            rx.borrow().expect("decision set on resolution")
        }
        HostCheckResult::NewlyRegistered { tx } => {
            let request_id = Uuid::new_v4().to_string();
            let guard_ctx = GuardCtx {
                layer1: &layer1,
                policy: &policy,
                privacy: &privacy,
                pending: &pending,
                event_tx: event_tx.as_ref(),
                domain_bus: &bus,
                cancel: cancel.clone(),
                request_id,
                args: Some(serde_json::to_value(&args).unwrap_or_default()),
                cwd: None,
                channel,
                non_ui_policy,
                history_repo: None,
                repo_id: String::new(),
                mirror_learning_enabled: false,
                mirror_min_approvals: 5,
                mirror_cooldown_seconds: 86400,
                now_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
            };
            let approval = evaluate(guard_ctx, "web_fetch", &args.url).await;
            let host_decision = if approval.allowed() {
                HostDecision::AllowForSession
            } else {
                HostDecision::Deny
            };
            let _ = tx.send(Some(host_decision));
            host_cache.resolve(host_key.clone(), host_decision);
            host_decision
        }
    };

    if host_decision == HostDecision::Deny {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!(
            "host {} previously denied",
            host_key.host
        ))));
    }

    if let Err(reason) = fire_pre_tool_use(
        hook_engine.as_ref(),
        session_id.clone(),
        "web_fetch",
        &args,
        None,
    )
    .await
    {
        return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
    }
    let start = std::time::Instant::now();
    let result: Result<String> = (async {
        let max_bytes = args.max_bytes.unwrap_or(200_000) as usize;
        let resp =
            client.get(&args.url).send().await.map_err(|e| {
                KlyntbotError::Tool(ToolError::ExecutionFailed(format!("http: {e}")))
            })?;
        if !resp.status().is_success() {
            return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                "http {} from {}",
                resp.status(),
                args.url
            ))));
        }
        let body = resp
            .bytes()
            .await
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
        let truncated = &body[..body.len().min(max_bytes)];
        let format = args.format.as_deref().unwrap_or("text");
        let out = if format == "text" {
            html2text::from_read(truncated, 80)
                .unwrap_or_else(|_| String::from_utf8_lossy(truncated).into_owned())
        } else {
            String::from_utf8_lossy(truncated).into_owned()
        };
        Ok(out)
    })
    .await;
    fire_post_tool_use(
        hook_engine.as_ref(),
        session_id,
        "web_fetch",
        result.is_ok(),
        start.elapsed().as_millis() as u64,
    )
    .await;
    result
}
