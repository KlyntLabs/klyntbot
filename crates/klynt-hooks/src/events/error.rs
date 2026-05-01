use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ErrorInput {
    pub session_id: String,
    pub kind: String,
    pub message: String,
    pub recoverable: bool,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct ErrorOutcome {
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::Error;
