//! Channel categories and per-tool visibility masks.
//!
//! `Channel` discriminates the chat surface (coding / desktop / other). `ChannelMask`
//! is what `Tool::allowed_channels()` returns — a bitmask of channels in which
//! the tool is visible to the LLM.

use bitflags::bitflags;

// Re-export so `common::tool_channel::CODING_CHANNEL` works.
pub use crate::CODING_CHANNEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Coding,
    Desktop,
    Other,
}

impl Channel {
    pub fn from_name(s: &str) -> Self {
        if s == CODING_CHANNEL {
            Self::Coding
        } else if s == "desktop" {
            Self::Desktop
        } else {
            Self::Other
        }
    }

    /// True for channels that can render approval cards (`kind: "approval"`
    /// ConversationItem). Used by the approval evaluator to fall back to a
    /// configured policy in headless channels (Telegram/Discord/Slack/Email).
    pub fn supports_approval_ui(&self) -> bool {
        matches!(self, Self::Coding | Self::Desktop)
    }
}

bitflags! {
    /// A tool's visibility across channel categories.
    ///
    /// 95% of tools want `ALL` (default). Tools needing approval UI return
    /// `CODING_ONLY` so they don't appear in headless chats.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ChannelMask: u8 {
        const CODING  = 0b001;
        const DESKTOP = 0b010;
        const OTHER   = 0b100;

        const ALL          = Self::CODING.bits() | Self::DESKTOP.bits() | Self::OTHER.bits();
        const CODING_ONLY  = Self::CODING.bits();
        const DESKTOP_ONLY = Self::DESKTOP.bits();
        const NON_CODING   = Self::DESKTOP.bits() | Self::OTHER.bits();
    }
}

impl ChannelMask {
    #[inline]
    pub fn allows(self, ch: Channel) -> bool {
        let bit = match ch {
            Channel::Coding  => Self::CODING,
            Channel::Desktop => Self::DESKTOP,
            Channel::Other   => Self::OTHER,
        };
        self.contains(bit)
    }
}

/// Look up channel visibility for a named tool, falling back to ALL if the
/// registry doesn't know the tool. Replaces the old `available_for_channel`
/// signature for compatibility during the migration; once all callers use
/// `Tool::allowed_channels()` directly, this helper can be deleted.
pub fn available_for_channel(_tool_name: &str, _channel: Channel) -> bool {
    // Compatibility shim retained ONLY for code that hasn't migrated yet.
    // Returns true unconditionally — the real per-tool gate now lives on the
    // Tool trait. Callers MUST migrate to ChannelMask::allows() in Task 1.
    //
    // After Task 1 completes (filter site updated), this fn has no remaining
    // callers and can be deleted in Task 9 alongside the rest of the cleanup.
    true
}
