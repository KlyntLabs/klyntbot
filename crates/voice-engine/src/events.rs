//! Voice events emitted by VoiceService, consumed by the frontend orb.

use serde::{Deserialize, Serialize};

use crate::types::EngineKind;

/// Events streamed from VoiceService to the frontend Voice Brain orb.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoiceEvent {
    CaptureStarted {
        session_id: String,
        engine: EngineKind,
    },
    AudioLevel {
        rms: f32,
    },
    PartialTranscript {
        text: String,
        language: String,
        is_final: bool,
    },
    RoutingSuggestion {
        skill: String,
        confidence: f32,
        label: String,
    },
    MemoryEcho {
        text: String,
    },
    CaptureEnded {
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
    ProcessingInBackground,
    Finalized {
        text: String,
        routed_to: String,
        response_preview: String,
    },
    SpeakResponse {
        audio_base64: String,
        sample_rate: u32,
        text: String,
    },
    Error {
        message: String,
        recoverable: bool,
    },
    /// Manager phase changed (for orb UI state)
    PhaseChanged {
        phase: String, // "idle", "listening", "reflecting", "speaking"
        session_title: Option<String>,
        turn_count: u32,
    },
    /// Agent is processing — brain is reflecting
    Reflecting,
    /// TTS should fade out over 300ms (soft interrupt)
    TtsFadeOut,
    /// User can tap "Continue" to resume interrupted TTS
    ContinueAvailable {
        /// How many seconds before the button auto-hides
        timeout_secs: u8,
    },
    /// Voice setup required before conversation can start
    SetupRequired {
        needs_model: bool,
        needs_mic_permission: bool,
    },
}

pub const VOICE_EVENT: &str = "voice:event";
