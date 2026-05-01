use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub hook: Vec<Hook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub event: klynt_protocol::HookEventName,
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub fail_open: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookEvents {
    #[serde(default)]
    pub pre_tool_use: Vec<Hook>,
    #[serde(default)]
    pub post_tool_use: Vec<Hook>,
    #[serde(default)]
    pub session_start: Vec<Hook>,
    #[serde(default)]
    pub user_prompt_submit: Vec<Hook>,
    #[serde(default)]
    pub stop: Vec<Hook>,
    #[serde(default)]
    pub session_end: Vec<Hook>,
    #[serde(default)]
    pub pre_compact: Vec<Hook>,
    #[serde(default)]
    pub post_compact: Vec<Hook>,
    #[serde(default)]
    pub pre_file_edit: Vec<Hook>,
    #[serde(default)]
    pub post_file_edit: Vec<Hook>,
    #[serde(default)]
    pub notification: Vec<Hook>,
    #[serde(default)]
    pub subagent_spawn: Vec<Hook>,
    #[serde(default)]
    pub error: Vec<Hook>,
}
