use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PreToolUseInput {
    pub session_id: String,
    pub tool: String,
    pub args: serde_json::Value,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PreToolUseOutcome {
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub modified_args: Option<serde_json::Value>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PreToolUse;
