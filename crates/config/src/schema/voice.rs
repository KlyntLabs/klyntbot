//! Voice input/output configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// STT engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SttEngineKind {
    /// Local whisper.cpp via whisper-rs (default).
    #[default]
    WhisperLocal,
    /// Placeholder for future cloud STT (Deepgram, etc.)
    Cloud,
}

/// TTS engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsEngineKind {
    /// macOS system TTS via AVSpeechSynthesizer (default).
    #[default]
    System,
    /// Kokoro-82M via ONNX Runtime.
    Kokoro,
    /// Piper VITS via ONNX Runtime.
    Piper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub input: VoiceInputConfig,
    #[serde(default)]
    pub output: VoiceOutputConfig,
    #[serde(default)]
    pub learning: VoiceLearningConfig,
    #[serde(default)]
    pub conversation: VoiceConversationConfig,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: VoiceInputConfig::default(),
            output: VoiceOutputConfig::default(),
            learning: VoiceLearningConfig::default(),
            conversation: VoiceConversationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInputConfig {
    #[serde(default = "default_voice_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_secs: f32,
    #[serde(default)]
    pub privacy_mode: VoicePrivacyMode,
    #[serde(default = "default_model_size")]
    pub model_size: String,
    /// STT engine to use.
    #[serde(default)]
    pub stt_engine: SttEngineKind,
    /// VAD threshold (0.0-1.0). Lower = more sensitive to speech.
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: f32,
    /// Whether to use neural/WebRTC VAD or simple RMS threshold.
    #[serde(default)]
    pub use_neural_vad: bool,
}

impl Default for VoiceInputConfig {
    fn default() -> Self {
        Self {
            hotkey: default_voice_hotkey(),
            silence_threshold_secs: default_silence_threshold(),
            privacy_mode: VoicePrivacyMode::default(),
            model_size: default_model_size(),
            stt_engine: SttEngineKind::default(),
            vad_threshold: default_vad_threshold(),
            use_neural_vad: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceOutputConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub voice_preferences: std::collections::HashMap<String, String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
    #[serde(default)]
    pub speak_during_focus: bool,
    /// TTS engine to use.
    #[serde(default)]
    pub tts_engine: TtsEngineKind,
}

impl Default for VoiceOutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_preferences: std::collections::HashMap::new(),
            speaking_rate: 1.0,
            speak_during_focus: false,
            tts_engine: TtsEngineKind::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceLearningConfig {
    #[serde(default)]
    pub target_language: Option<String>,
    #[serde(default = "default_true")]
    pub show_pronunciation_scores: bool,
    #[serde(default = "default_true")]
    pub auto_create_flashcards: bool,
}

impl Default for VoiceLearningConfig {
    fn default() -> Self {
        Self {
            target_language: None,
            show_pronunciation_scores: true,
            auto_create_flashcards: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationConfig {
    /// Minutes before a previous voice session is considered "cold" (default: 15)
    #[serde(default = "default_warm_session_minutes")]
    pub warm_session_minutes: u32,
    /// Minutes before a main chat session is considered "cold" (default: 5)
    #[serde(default = "default_warm_chat_minutes")]
    pub warm_chat_minutes: u32,
    /// Seconds of silence to end a turn (default: 1.5)
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_secs: f32,
    /// Auto-resume listening after agent response (default: true)
    #[serde(default = "default_true")]
    pub auto_resume: bool,
    /// Variable pause after response based on length (default: true)
    #[serde(default = "default_true")]
    pub adaptive_breath: bool,
}

impl Default for VoiceConversationConfig {
    fn default() -> Self {
        Self {
            warm_session_minutes: 15,
            warm_chat_minutes: 5,
            silence_threshold_secs: 1.5,
            auto_resume: true,
            adaptive_breath: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoicePrivacyMode {
    #[default]
    Standard,
    Strict,
    Off,
}

fn default_warm_session_minutes() -> u32 {
    15
}

fn default_warm_chat_minutes() -> u32 {
    5
}

fn default_voice_hotkey() -> String {
    "alt+shift+v".to_string()
}

fn default_silence_threshold() -> f32 {
    1.5
}

fn default_model_size() -> String {
    "small".to_string()
}

fn default_speaking_rate() -> f32 {
    1.0
}

fn default_vad_threshold() -> f32 {
    0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_kinds() {
        let input = VoiceInputConfig::default();
        assert_eq!(input.stt_engine, SttEngineKind::WhisperLocal);

        let output = VoiceOutputConfig::default();
        assert_eq!(output.tts_engine, TtsEngineKind::System);
    }

    #[test]
    fn deserialize_kokoro_engine() {
        let json = r#"{"ttsEngine": "kokoro"}"#;
        let output: VoiceOutputConfig = serde_json::from_str(json).unwrap();
        assert_eq!(output.tts_engine, TtsEngineKind::Kokoro);
    }

    #[test]
    fn deserialize_unknown_engine_rejects() {
        let json = r#"{"sttEngine": "nonexistent"}"#;
        let result: Result<VoiceInputConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
