use crate::{
    channel::ApprovalChannel,
    class::{ApprovalClass, ApprovalDecision, ApprovalLifetime, ApprovalScope},
    grants::{ApprovalGrantsRepo, GrantRow},
    preview::SuggestedGrant,
    request::ApprovalRequest,
};
use common::Result;
use jiff::Timestamp;
use std::sync::Arc;

/// Thin trait for Mirror-driven grant suggestion. The cognitive crate implements
/// this for `MirrorFacade`; the approval crate stays free of cognitive dependencies.
#[async_trait::async_trait]
pub trait ApprovalSuggester: Send + Sync {
    async fn suggest(&self, tool_name: &str, path: Option<&str>) -> Option<SuggestedGrant>;
}

#[derive(Debug, Clone)]
pub enum GateOutcome {
    Allow,
    Deny { reason: String },
    Cancel,
}

pub struct ApprovalGate {
    grants: ApprovalGrantsRepo,
    channel: Arc<dyn ApprovalChannel>,
    hooks: Vec<Arc<dyn crate::policy::ClassifyHook>>,
    suggester: std::sync::Mutex<Option<Arc<dyn ApprovalSuggester>>>,
}

impl ApprovalGate {
    pub fn new(grants: ApprovalGrantsRepo, channel: Arc<dyn ApprovalChannel>) -> Self {
        Self {
            grants,
            channel,
            hooks: Vec::new(),
            suggester: std::sync::Mutex::new(None),
        }
    }

    pub fn with_classify_hooks(mut self, hooks: Vec<Arc<dyn crate::policy::ClassifyHook>>) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn with_suggester(mut self, suggester: Arc<dyn ApprovalSuggester>) -> Self {
        self.suggester = std::sync::Mutex::new(Some(suggester));
        self
    }

    /// Set the suggester after construction (used when the Mirror facade is
    /// created after the agent loop).
    pub fn set_suggester(&self, suggester: Arc<dyn ApprovalSuggester>) {
        *self.suggester.lock().unwrap() = Some(suggester);
    }

    /// Purge session-scoped grants for a given session.
    pub async fn purge_session(&self, session_id: &str) -> Result<u64> {
        self.grants.purge_session(session_id).await
    }

    pub async fn check(
        &self,
        mut req: ApprovalRequest,
        cancel_token: &tokio_util::sync::CancellationToken,
    ) -> Result<GateOutcome> {
        // Run classify hooks — last non-None override wins.
        for hook in &self.hooks {
            if let Some(class) = hook.classify(&req.tool_name, req.action.as_deref(), &req.args) {
                req.class = class;
            }
            if let Some(scope) = hook.scope(&req.tool_name, req.action.as_deref(), &req.args) {
                req.scope = scope;
            }
        }

        let resource = match &req.scope {
            ApprovalScope::ToolAction => None,
            ApprovalScope::ToolActionResource(k) => Some(k.clone()),
        };

        // Safe class always auto-allows (no prompt needed).
        if req.class == ApprovalClass::Safe {
            return Ok(GateOutcome::Allow);
        }

        // Remote-channel auto-allow for non-prompted classes.
        let caps = self.channel.capabilities();
        if req.ctx.is_remote() && !caps.supports_classes.contains(&req.class) {
            tracing::debug!(
                tool = %req.tool_name, class = ?req.class, channel = ?req.ctx.channel,
                "approval: remote auto-allow (class not in channel capabilities)"
            );
            return Ok(GateOutcome::Allow);
        }

        // Existing-grant lookup.
        if self
            .grants
            .find(
                req.class,
                &req.tool_name,
                req.action.as_deref(),
                resource.as_deref(),
                Some(&req.ctx.session_id),
            )
            .await?
            .is_some()
        {
            return Ok(GateOutcome::Allow);
        }

        // Consult Mirror suggester for grant pattern (if wired).
        if req.suggested_grant.is_none() {
            let suggester = self.suggester.lock().unwrap().clone();
            if let Some(s) = suggester {
                let path = crate::extract_path_str_from_args(&req.args);
                if let Some(grant) = s.suggest(&req.tool_name, path.as_deref()).await {
                    req.suggested_grant = Some(grant);
                }
            }
        }

        // Prompt the channel — race against cancellation so a hung approval
        // modal cannot pin the agent loop forever.
        let decision = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                tracing::info!(
                    tool = %req.tool_name,
                    "approval: cancellation observed while awaiting channel decision"
                );
                return Ok(GateOutcome::Cancel);
            }
            d = self.channel.request(req.clone()) => d,
        };
        match decision {
            ApprovalDecision::Once => Ok(GateOutcome::Allow),
            ApprovalDecision::Session => {
                self.persist(&req, resource.as_deref(), ApprovalLifetime::Session)
                    .await?;
                Ok(GateOutcome::Allow)
            }
            ApprovalDecision::Forever => {
                self.persist(&req, resource.as_deref(), ApprovalLifetime::Forever)
                    .await?;
                Ok(GateOutcome::Allow)
            }
            ApprovalDecision::Decline { reason } => Ok(GateOutcome::Deny { reason }),
            ApprovalDecision::Cancel => Ok(GateOutcome::Cancel),
        }
    }

    async fn persist(
        &self,
        req: &ApprovalRequest,
        resource: Option<&str>,
        lifetime: ApprovalLifetime,
    ) -> Result<()> {
        let now = Timestamp::now().as_second();
        let session_id = match lifetime {
            ApprovalLifetime::Session => Some(req.ctx.session_id.clone()),
            ApprovalLifetime::Forever => None,
            ApprovalLifetime::Once => return Ok(()),
        };
        self.grants
            .insert(&GrantRow {
                class: req.class,
                tool_name: req.tool_name.clone(),
                action: req.action.clone(),
                resource_key: resource.map(str::to_string),
                lifetime,
                session_id,
                granted_at: now,
                expires_at: None,
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::ApprovalCapabilities;
    use crate::request::{ApprovalContext, ChannelKind};
    use std::collections::HashSet;
    use std::sync::Mutex;
    use storage::StoragePool;
    use tokio_util::sync::CancellationToken;

    #[derive(Default, Clone)]
    struct StubChannel {
        decisions: Arc<Mutex<Vec<ApprovalDecision>>>,
        requested: Arc<Mutex<u32>>,
    }

    #[async_trait::async_trait]
    impl ApprovalChannel for StubChannel {
        async fn request(&self, _r: ApprovalRequest) -> ApprovalDecision {
            *self.requested.lock().unwrap() += 1;
            self.decisions
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(ApprovalDecision::Once)
        }
        fn capabilities(&self) -> ApprovalCapabilities {
            ApprovalCapabilities {
                supports_inline: true,
                supports_classes: HashSet::from([
                    ApprovalClass::Sensitive,
                    ApprovalClass::Destructive,
                    ApprovalClass::Admin,
                ]),
            }
        }
    }

    fn ctx() -> ApprovalContext {
        ApprovalContext {
            mode: common::SessionMode::Coding,
            channel: ChannelKind::Desktop,
            session_id: "sess-1".into(),
            user_id: None,
            cwd: std::path::PathBuf::from("."),
        }
    }

    #[tokio::test]
    async fn safe_class_auto_allows_without_prompt() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let chan = StubChannel::default();
        let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

        let req = ApprovalRequest {
            tool_name: "notes".into(),
            action: Some("read".into()),
            args: serde_json::json!({}),
            class: ApprovalClass::Safe,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };
        let token = tokio_util::sync::CancellationToken::new();
        let out = gate.check(req, &token).await.unwrap();
        assert!(matches!(out, GateOutcome::Allow));
        assert_eq!(*chan.requested.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn destructive_session_grant_persists_for_session() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let chan = StubChannel::default();
        chan.decisions
            .lock()
            .unwrap()
            .push(ApprovalDecision::Session);
        let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

        let make_req = || ApprovalRequest {
            tool_name: "notes".into(),
            action: Some("delete".into()),
            args: serde_json::json!({"id":"n1"}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };

        let token = tokio_util::sync::CancellationToken::new();
        let out1 = gate.check(make_req(), &token).await.unwrap();
        assert!(matches!(out1, GateOutcome::Allow));
        assert_eq!(*chan.requested.lock().unwrap(), 1);

        let out2 = gate.check(make_req(), &token).await.unwrap();
        assert!(matches!(out2, GateOutcome::Allow));
        assert_eq!(*chan.requested.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn decline_returns_deny() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let chan = StubChannel::default();
        chan.decisions
            .lock()
            .unwrap()
            .push(ApprovalDecision::Decline {
                reason: "no".into(),
            });
        let gate = ApprovalGate::new(repo, Arc::new(chan));

        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: serde_json::json!({"cmd":"rm"}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };
        let token = tokio_util::sync::CancellationToken::new();
        let out = gate.check(req, &token).await.unwrap();
        assert!(matches!(out, GateOutcome::Deny { .. }));
    }

    #[tokio::test]
    async fn cancel_propagates() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let chan = StubChannel::default();
        chan.decisions
            .lock()
            .unwrap()
            .push(ApprovalDecision::Cancel);
        let gate = ApprovalGate::new(repo, Arc::new(chan));

        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: serde_json::json!({}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };
        let token = tokio_util::sync::CancellationToken::new();
        let out = gate.check(req, &token).await.unwrap();
        assert!(matches!(out, GateOutcome::Cancel));
    }

    #[tokio::test]
    async fn remote_channel_auto_allows_safe_when_capabilities_omit_it() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let chan = StubChannel::default();
        let gate = ApprovalGate::new(repo, Arc::new(chan.clone()));

        let mut c = ctx();
        c.channel = ChannelKind::Telegram;
        let req = ApprovalRequest {
            tool_name: "notes".into(),
            action: Some("update".into()),
            args: serde_json::json!({}),
            class: ApprovalClass::Safe,
            scope: ApprovalScope::ToolAction,
            ctx: c,
            preview: None,
            suggested_grant: None,
        };
        let token = tokio_util::sync::CancellationToken::new();
        let out = gate.check(req, &token).await.unwrap();
        assert!(matches!(out, GateOutcome::Allow));
        assert_eq!(*chan.requested.lock().unwrap(), 0);
    }

    use std::time::Duration;

    /// Channel whose `request` future never resolves until dropped.
    struct HungChannel;

    #[async_trait::async_trait]
    impl ApprovalChannel for HungChannel {
        async fn request(&self, _r: ApprovalRequest) -> ApprovalDecision {
            std::future::pending().await
        }
        fn capabilities(&self) -> ApprovalCapabilities {
            ApprovalCapabilities {
                supports_inline: true,
                supports_classes: HashSet::from([ApprovalClass::Destructive]),
            }
        }
    }

    #[tokio::test]
    async fn cancel_during_channel_request_returns_cancel_outcome() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let gate = Arc::new(ApprovalGate::new(repo, Arc::new(HungChannel)));

        let token = CancellationToken::new();
        let token_for_task = token.clone();
        let gate_for_task = gate.clone();

        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: serde_json::json!({"cmd":"rm"}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };

        let handle = tokio::spawn(async move { gate_for_task.check(req, &token_for_task).await });

        tokio::time::sleep(Duration::from_millis(20)).await;
        token.cancel();

        let outcome = tokio::time::timeout(Duration::from_millis(200), handle)
            .await
            .expect("gate.check should return within 200ms of cancel")
            .expect("task should not panic")
            .expect("gate.check should return Ok");

        assert!(matches!(outcome, GateOutcome::Cancel));
    }
}
