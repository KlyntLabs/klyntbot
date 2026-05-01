use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UserPromptSubmitInput {
    pub session_id: String,
    pub prompt: String,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct UserPromptSubmitOutcome {
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::UserPromptSubmit;
