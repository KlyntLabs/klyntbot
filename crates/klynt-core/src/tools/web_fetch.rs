use crate::approval::{evaluate, GuardCtx, Layer1, PendingApprovalsMap};
use crate::privacy::PrivacyGuard;
use agent::events::AgentEvent;
use async_trait::async_trait;
use bus::DomainEventBus;
use common::{KlyntbotError, Result, ToolError};
use klynt_execpolicy::Policy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct WebFetchArgs {
    /// http(s) URL to fetch.
    #[param(required)] pub url: String,
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
    tags = "web,fetch,coding",
    allowed_channels = "coding_only"
)]
pub struct WebFetchTool {
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
    bus: Arc<DomainEventBus>,
    client: reqwest::Client,
    non_ui_policy: common::tool_channel::NonUiPolicy,
}

impl WebFetchTool {
    pub fn new(
        layer1: Arc<Layer1>, policy: Arc<Policy>, privacy: Arc<PrivacyGuard>,
        pending: Arc<PendingApprovalsMap>, event_tx: Option<mpsc::Sender<AgentEvent>>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client construction");
        Self { layer1, policy, privacy, pending, event_tx, bus, client, non_ui_policy }
    }
}

#[async_trait]
impl ToolExecute for WebFetchTool {
    type Params = WebFetchArgs;
    async fn execute(&self, args: WebFetchArgs, ctx: &RoutingContext) -> Result<String> {
        run_for_test(args, self.layer1.clone(), self.policy.clone(),
            self.privacy.clone(), self.pending.clone(),
            self.event_tx.clone().unwrap_or_else(|| mpsc::channel(1).0),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_else(CancellationToken::new),
            self.client.clone(),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WebFetchArgs,
    layer1: Arc<Layer1>,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    pending: Arc<PendingApprovalsMap>,
    event_tx: mpsc::Sender<AgentEvent>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
    client: reqwest::Client,
    channel: common::tool_channel::Channel,
    non_ui_policy: common::tool_channel::NonUiPolicy,
) -> Result<String> {
    let request_id = Uuid::new_v4().to_string();
    let guard_ctx = GuardCtx {
        layer1: &layer1, policy: &policy, privacy: &privacy,
        pending: &pending, event_tx: Some(&event_tx), domain_bus: &bus,
        cancel: cancel.clone(), request_id,
        args: Some(serde_json::to_value(&args).unwrap_or_default()),
        cwd: None,
        channel,
        non_ui_policy,
    };
    let decision = evaluate(guard_ctx, "web_fetch", &args.url).await;
    if !decision.allowed() {
        return Err(KlyntbotError::Tool(ToolError::PermissionDenied(format!("{decision:?}"))));
    }

    let max_bytes = args.max_bytes.unwrap_or(200_000) as usize;
    let resp = client.get(&args.url).send().await
        .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("http: {e}"))))?;
    if !resp.status().is_success() {
        return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(
            format!("http {} from {}", resp.status(), args.url))));
    }
    let body = resp.bytes().await
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
}
