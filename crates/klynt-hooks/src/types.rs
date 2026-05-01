use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    pub cwd: PathBuf,
    pub event: HookEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HookEvent {
    #[serde(rename = "after_agent")]
    AfterAgent { thread_id: String },
    #[serde(rename = "after_tool_use")]
    AfterToolUse { tool: HookToolInput },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookToolInput {
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    #[serde(default)]
    pub r#continue: bool,
    #[serde(default)]
    pub block: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub modify_args: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
