//! Voice events emitted by VoiceService, consumed by the frontend orb.

use serde::{Deserialize, Serialize};

use crate::error_classifier::PhonemeScore;
use crate::feedback_decider::FeedbackLevel;
use crate::pronunciation_analyzer::SyllableTone;
use crate::types::EngineKind;

/// Lightweight segment data for frontend word-level confidence display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentEvent {
    pub text: String,
    pub confidence: f32,
}

/// Events streamed from VoiceService to the frontend Voice Brain orb.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoiceEvent {
    CaptureStarted {
        #[serde(rename = "sessionId")]
        session_id: String,
        engine: EngineKind,
    },
    AudioLevel {
        rms: f32,
    },
    PartialTranscript {
        text: String,
        language: String,
        #[serde(rename = "isFinal")]
        is_final: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        segments: Vec<TranscriptSegmentEvent>,
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
        #[serde(rename = "routedTo")]
        routed_to: String,
        #[serde(rename = "responsePreview")]
        response_preview: String,
    },
    SpeakResponse {
        #[serde(rename = "audioBase64")]
        audio_base64: String,
        #[serde(rename = "sampleRate")]
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
        #[serde(rename = "sessionTitle")]
        session_title: Option<String>,
        #[serde(rename = "turnCount")]
        turn_count: u32,
    },
    /// Agent is processing — brain is reflecting
    Reflecting,
    /// TTS should fade out over 300ms (soft interrupt)
    TtsFadeOut,
    /// User can tap "Continue" to resume interrupted TTS
    ContinueAvailable {
        /// How many seconds before the button auto-hides
        #[serde(rename = "timeoutSecs")]
        timeout_secs: u8,
    },
    /// Voice setup required before conversation can start
    SetupRequired {
        #[serde(rename = "needsModel")]
        needs_model: bool,
        #[serde(rename = "needsMicPermission")]
        needs_mic_permission: bool,
    },
    /// Detailed pronunciation report after a scored turn.
    PronunciationReport {
        #[serde(rename = "overallScore")]
        overall_score: f32,
        #[serde(rename = "phonemeScores")]
        phoneme_scores: Vec<PhonemeScore>,
        #[serde(rename = "toneScores")]
        tone_scores: Vec<SyllableTone>,
        #[serde(rename = "feedbackLevel")]
        feedback_level: FeedbackLevel,
    },
    /// Adaptive feedback level escalated for a phoneme.
    FeedbackEscalated {
        phoneme: String,
        #[serde(rename = "fromLevel")]
        from_level: FeedbackLevel,
        #[serde(rename = "toLevel")]
        to_level: FeedbackLevel,
    },
    /// Chinese tone contour data for visualization.
    ToneContour {
        syllables: Vec<SyllableTone>,
    },
    /// Speech model is loading (downloading or initializing).
    ModelLoading {
        engine: String,
    },
    /// Speech model is ready for use.
    ModelReady {
        engine: String,
    },
    /// Model download progress update.
    DownloadProgress {
        model: String,
        downloaded: u64,
        total: u64,
    },
}

pub const VOICE_EVENT: &str = "voice:event";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speak_response_serialization() {
        let event = VoiceEvent::SpeakResponse {
            audio_base64: "dGVzdA==".to_string(),
            sample_rate: 16000,
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        println!("Serialized: {json}");
        assert!(
            json.contains("\"audioBase64\""),
            "Expected audioBase64 in: {json}"
        );
        assert!(
            json.contains("\"sampleRate\""),
            "Expected sampleRate in: {json}"
        );
    }
}
