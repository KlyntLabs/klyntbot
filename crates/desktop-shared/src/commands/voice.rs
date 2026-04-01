//! Voice command request/response types for Tauri IPC.

use serde::{Deserialize, Serialize};
use voice_engine::{EngineKind, VoiceSessionState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCaptureInfo {
    pub session_id: String,
    pub engine: EngineKind,
    pub state: VoiceSessionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatusResponse {
    pub state: VoiceSessionState,
    pub engine: EngineKind,
    pub enabled: bool,
}
