use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PreFileEditInput {
    pub session_id: String,
    pub tool: String,
    pub path: String,
    pub op: String,
    pub diff_preview: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PreFileEditOutcome {
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub modified_args: Option<serde_json::Value>,
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PreFileEdit;
