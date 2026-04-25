//! Claude Code stdin payload shapes for the 7 hooks we listen on.

#![allow(dead_code)]

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(super) struct CommonEnvelope {
    pub session_id: String,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub transcript_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionStartBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionEndBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UserPromptBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub prompt: String,
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(super) struct StopBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub stop_hook_active: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct PreCompactBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct NotificationBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SubagentStopBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    #[serde(default)]
    pub stop_hook_active: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolUseBody {
    #[serde(flatten)]
    pub common: CommonEnvelope,
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: serde_json::Value,
    #[serde(default)]
    pub tool_response: serde_json::Value,
    #[serde(default)]
    pub duration_ms: u32,
}
