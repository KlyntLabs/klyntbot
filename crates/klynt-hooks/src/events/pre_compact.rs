use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PreCompactInput {
    pub session_id: String,
    pub message_count: u64,
    pub current_tokens: u64,
    pub context_window: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PreCompactOutcome {
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
    pub should_block: bool,
    pub block_reason: Option<String>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PreCompact;
