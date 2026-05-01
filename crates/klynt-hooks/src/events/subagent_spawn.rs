use crate::events::common::BaseEventInput;
use klynt_protocol::HookEventName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubagentSpawnInput {
    pub session_id: String,
    pub parent_session_id: Option<String>,
    pub profile: String,
    pub task_summary: String,
    #[serde(flatten)]
    pub base: BaseEventInput,
}

#[derive(Debug, Clone, Default)]
pub struct SubagentSpawnOutcome {
    pub should_block: bool,
    pub block_reason: Option<String>,
    pub hook_events: Vec<klynt_protocol::HookCompletedEvent>,
}

pub const EVENT_NAME: HookEventName = HookEventName::SubagentSpawn;
