// Re-export so `common::coding_channel::CODING_CHANNEL` works.
pub use crate::CODING_CHANNEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

pub const CODING_ONLY: &[&str] = &[
    "bash",
    "read",
    "glob",
    "grep",
    "edit",
    "write",
    "apply_patch",
    "web_fetch",
    "ask_user",
    "enter_plan_mode",
    "exit_plan_mode",
    "notebook_edit",
    "tool_search",
];

pub fn available_for_channel(tool_name: &str, channel: Channel) -> bool {
    let is_coding_only = CODING_ONLY.contains(&tool_name);
    match channel {
        Channel::Coding => true,
        Channel::Desktop | Channel::Other => !is_coding_only,
    }
}
