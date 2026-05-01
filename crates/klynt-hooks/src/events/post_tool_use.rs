use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PostToolUseInput {
    pub session_id: String,
    pub tool: String,
    pub success: bool,
    pub duration_ms: u64,
    pub output_summary: Option<String>,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct PostToolUseOutcome {
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::PostToolUse;
