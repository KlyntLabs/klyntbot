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
    tags = "web,fetch,coding",
    approval_class = "sensitive",
    approval_scope = "url"
)]
pub struct WebFetchTool {
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    bus: Arc<DomainEventBus>,
    client: reqwest::Client,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    pub history_repo: Option<std::sync::Arc<storage::repos::CodingApprovalHistoryRepo>>,
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub repo_id: String,
}

impl WebFetchTool {
    pub fn new(
        policy: Arc<Policy>,
        privacy: Arc<PrivacyGuard>,
        bus: Arc<DomainEventBus>,
        non_ui_policy: common::tool_channel::NonUiPolicy,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client construction");
        Self {
            policy,
            privacy,
            bus,
            client,
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
            self.policy.clone(),
            self.privacy.clone(),
            ctx.event_tx.clone(),
            self.bus.clone(),
            ctx.cancel_token.clone().unwrap_or_default(),
            self.client.clone(),
            common::tool_channel::Channel::from_name(ctx.channel.as_str()),
            self.non_ui_policy,
            ctx.hook_engine.clone(),
            session_id,
            self.history_repo.clone(),
            self.mirror_learning_enabled,
            self.mirror_min_approvals,
            self.mirror_cooldown_seconds,
            self.repo_id.clone(),
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_for_test(
    args: WebFetchArgs,
    policy: Arc<Policy>,
    privacy: Arc<PrivacyGuard>,
    event_tx: Option<mpsc::Sender<ToolEvent>>,
    bus: Arc<DomainEventBus>,
    cancel: CancellationToken,
    client: reqwest::Client,
    channel: common::tool_channel::Channel,
    non_ui_policy: common::tool_channel::NonUiPolicy,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    session_id: String,
    history_repo: Option<std::sync::Arc<storage::repos::CodingApprovalHistoryRepo>>,
    mirror_learning_enabled: bool,
    mirror_min_approvals: u32,
    mirror_cooldown_seconds: i64,
    repo_id: String,
) -> Result<String> {
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
        let mut resp =
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
        let format = args.format.as_deref().unwrap_or("text");
        let mut body = Vec::with_capacity(max_bytes.min(8192));
        while let Some(chunk) = resp.chunk().await.map_err(|e| {
            KlyntbotError::Tool(ToolError::ExecutionFailed(format!("download: {e}")))
        })? {
            body.extend_from_slice(&chunk);
            if body.len() >= max_bytes {
                body.truncate(max_bytes);
                break;
            }
        }
        let out = if format == "text" {
            html2text::from_read(&body[..], 80)
                .unwrap_or_else(|_| String::from_utf8_lossy(&body).into_owned())
        } else {
            String::from_utf8_lossy(&body).into_owned()
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
