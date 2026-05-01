use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PostCompactInput {
    pub session_id: String,
    pub messages_compacted: u64,
    pub tokens_before: u64,
    pub tokens_after: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PostCompactOutcome {
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PostCompact;
