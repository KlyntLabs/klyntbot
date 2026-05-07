use crate::class::{ApprovalClass, ApprovalScope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    Desktop,
    Telegram,
    Discord,
    Slack,
    Email,
    Mcp,
}

impl ChannelKind {
    pub fn is_remote(&self) -> bool {
        !matches!(self, Self::Desktop)
    }
}

impl From<&str> for ChannelKind {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "desktop" | "coding" => Self::Desktop,
            "telegram" => Self::Telegram,
            "discord" => Self::Discord,
            "slack" => Self::Slack,
            "email" => Self::Email,
            "mcp" => Self::Mcp,
            other => {
                tracing::warn!(channel = %other, "unknown channel kind, falling back to Desktop");
                Self::Desktop
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalContext {
    pub mode: common::SessionMode,
    pub channel: ChannelKind,
    pub session_id: String,
    pub user_id: Option<String>,
    pub cwd: std::path::PathBuf,
}

impl ApprovalContext {
    pub fn is_remote(&self) -> bool {
        self.channel.is_remote()
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub action: Option<String>,
    pub args: Value,
    pub class: ApprovalClass,
    pub scope: ApprovalScope,
    pub ctx: ApprovalContext,
    /// Preview metadata for the approval card. Populated by the channel boundary
    /// at request emission time. None for internal flows that don't need UI.
    pub preview: Option<crate::preview::ApprovalPreview>,
    /// Mirror-suggested grant pattern. None if Mirror has no signal yet
    /// or no Mirror facade is wired.
    pub suggested_grant: Option<crate::preview::SuggestedGrant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_remote_predicate() {
        let local = ApprovalContext {
            mode: common::SessionMode::Coding,
            channel: ChannelKind::Desktop,
            session_id: "s1".into(),
            user_id: None,
            cwd: std::path::PathBuf::from("."),
        };
        assert!(!local.is_remote());
        let remote = ApprovalContext {
            channel: ChannelKind::Telegram,
            ..local
        };
        assert!(remote.is_remote());
    }
}
