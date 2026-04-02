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

/// Audio device listing for settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevicesResponse {
    pub input: Vec<String>,
    pub output: Vec<String>,
    /// Name of the system default input device (if available).
    pub default_input: Option<String>,
    /// Name of the system default output device (if available).
    pub default_output: Option<String>,
}

/// Per-model download status for settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelStatusResponse {
    pub asr: ModelStatus,
    pub tts: ModelStatus,
    pub tts_instruct: ModelStatus,
    /// Whether the STT engine is loaded and ready.
    pub stt_loaded: bool,
    /// Whether the TTS engine is loaded and ready.
    pub tts_loaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub downloaded: bool,
    pub display_name: String,
    pub dir_name: String,
}
