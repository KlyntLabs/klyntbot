//! Voice command request/response types for Tauri IPC.

use serde::{Deserialize, Serialize};
use voice_engine::{EngineKind, ModelState, VoiceSessionState, WhisperModelSize};

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
    pub model_state: ModelState,
    pub engine: EngineKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDownloadModelRequest {
    pub model_size: WhisperModelSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelInfo {
    pub size: WhisperModelSize,
    pub display_name: String,
    pub size_bytes: u64,
    pub available: bool,
}
