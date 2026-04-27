//! Voice conversation command request/response types for Tauri IPC.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationStartResponse {
    pub session_key: String,
    pub session_title: String,
    pub is_continuing: bool,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationStatusResponse {
    pub phase: String,
    pub session_key: Option<String>,
    pub session_title: Option<String>,
    pub turn_count: u32,
    pub paused: bool,
    pub continue_available: bool,
    pub engine_kind: Option<String>,
}
