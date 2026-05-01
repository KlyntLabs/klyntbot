pub mod error;
pub mod op;
pub mod submission;
pub mod trace;

pub use error::ProtocolError;
pub use op::Op;
pub use submission::{Submission, SubmissionResult};
pub use trace::CodingTraceEvent;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable session identifier — re-export from common::types.
pub use common::types::SessionKey;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventName {
    PreToolUse,
    PostToolUse,
    SessionStart,
    UserPromptSubmit,
    Stop,
    SessionEnd,
    PreCompact,
    PostCompact,
    PreFileEdit,
    PostFileEdit,
    Notification,
    SubagentSpawn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookRunStatus {
    Success,
    Blocked,
    Timeout,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct HookOutputEntry {
    pub kind: HookOutputEntryKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookOutputEntryKind {
    Stdout,
    Stderr,
    Block,
    ModifyArgs,
    AdditionalContext,
    StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct HookRunSummary {
    pub id: String,
    pub event_name: HookEventName,
    pub status: HookRunStatus,
    pub duration_ms: u64,
    pub output: Vec<HookOutputEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct HookCompletedEvent {
    pub session_id: String,
    pub run: HookRunSummary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookExecutionMode {
    Subprocess,
    InProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookHandlerType {
    Command,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookScope {
    User,
    Project,
}
