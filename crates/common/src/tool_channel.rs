//! Channel categories and per-tool visibility masks.
//!
//! `Channel` discriminates the chat surface (desktop / other). `ChannelMask`
//! is what `Tool::allowed_channels()` returns — a bitmask of channels in which
//! the tool is visible to the LLM.

use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Desktop,
    Other,
}

impl Channel {
    pub fn from_name(s: &str) -> Self {
        if s == "desktop" {
            Self::Desktop
        } else {
            Self::Other
        }
    }

    /// True for channels that can render approval cards (`kind: "approval"`
    /// ConversationItem). Used by the approval evaluator to fall back to a
    /// configured policy in headless channels (Telegram/Discord/Slack/Email).
    pub fn supports_approval_ui(&self) -> bool {
        matches!(self, Self::Desktop)
    }
}

bitflags! {
    /// A tool's visibility across channel categories.
    ///
    /// 95% of tools want `ALL` (default).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ChannelMask: u8 {
        const DESKTOP = 0b010;
        const OTHER   = 0b100;

        const ALL          = Self::DESKTOP.bits() | Self::OTHER.bits();
        const DESKTOP_ONLY = Self::DESKTOP.bits();
    }
}

impl ChannelMask {
    #[inline]
    pub fn allows(self, ch: Channel) -> bool {
        let bit = match ch {
            Channel::Desktop => Self::DESKTOP,
            Channel::Other => Self::OTHER,
        };
        self.contains(bit)
    }
}

/// Policy for headless channels that cannot render approval UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NonUiPolicy {
    /// Automatically allow the tool (default).
    #[default]
    Allow,
    /// Deny with an error explaining the configuration option.
    DenyWithError,
}
