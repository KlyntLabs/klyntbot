use crate::class::ApprovalClass;
use crate::request::ApprovalRequest;
use crate::ApprovalDecision;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ApprovalCapabilities {
    pub supports_inline: bool,
    pub supports_classes: HashSet<ApprovalClass>,
}

#[async_trait::async_trait]
pub trait ApprovalChannel: Send + Sync {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision;
    fn capabilities(&self) -> ApprovalCapabilities;
}

/// Fallback channel for surfaces without interactive approval UI.
/// Returns `Decline` with a message directing the user to approve on desktop.
pub struct BlockingFallbackChannel {
    message: String,
}

impl BlockingFallbackChannel {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn desktop_prompt() -> Self {
        Self::new("Action requires approval. Open Klynt on desktop to confirm.")
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for BlockingFallbackChannel {
    async fn request(&self, _req: ApprovalRequest) -> ApprovalDecision {
        ApprovalDecision::Decline {
            reason: self.message.clone(),
        }
    }
    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: false,
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::ApprovalClass;
    use std::collections::HashSet;

    struct DummyChannel;

    #[async_trait::async_trait]
    impl ApprovalChannel for DummyChannel {
        async fn request(&self, _r: crate::ApprovalRequest) -> crate::ApprovalDecision {
            crate::ApprovalDecision::Once
        }
        fn capabilities(&self) -> ApprovalCapabilities {
            ApprovalCapabilities {
                supports_inline: true,
                supports_classes: HashSet::from([ApprovalClass::Destructive]),
            }
        }
    }

    #[tokio::test]
    async fn dummy_channel_returns_once() {
        let c = DummyChannel;
        assert!(c.capabilities().supports_inline);
    }

    #[tokio::test]
    async fn fallback_channel_declines_with_message() {
        let c = BlockingFallbackChannel::desktop_prompt();
        let req = crate::ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: serde_json::json!({}),
            class: ApprovalClass::Destructive,
            scope: crate::ApprovalScope::ToolAction,
            ctx: crate::ApprovalContext {
                mode: common::SessionMode::Coding,
                channel: crate::ChannelKind::Telegram,
                session_id: "s1".into(),
                user_id: None,
                cwd: std::path::PathBuf::from("."),
            },
            preview: None,
            suggested_grant: None,
        };
        let decision = c.request(req).await;
        match decision {
            ApprovalDecision::Decline { reason } => {
                assert!(reason.contains("desktop"));
            }
            _ => panic!("expected Decline"),
        }
    }
}
