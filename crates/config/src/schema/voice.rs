//! Voice input/output configuration.

use serde::{Deserialize, Serialize};

use super::core::{default_true, Secret};

/// STT engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SttEngineKind {
    /// Qwen3-ASR local or cloud (default, replaces Whisper).
    #[default]
    Qwen3,
}

/// TTS engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsEngineKind {
    /// Qwen3-TTS local or cloud (default).
    #[default]
    Qwen3,
    /// macOS system TTS via AVSpeechSynthesizer.
    System,
}

/// Deployment mode — local model or cloud API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum EngineDeployment {
    /// Run model locally on device (MLX/Metal).
    Local,
    /// Call a cloud API (OpenAI-compatible endpoint).
    Cloud {
        #[serde(rename = "apiUrl")]
        api_url: String,
        #[serde(rename = "apiKey")]
        api_key: Secret<String>,
    },
}

impl Default for EngineDeployment {
    fn default() -> Self {
        Self::Local
    }
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
    /// STT engine to use.
    #[serde(default)]
    pub stt_engine: SttEngineKind,
    /// VAD threshold (0.0-1.0). Lower = more sensitive to speech.
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: f32,
    /// Whether to use neural/WebRTC VAD or simple RMS threshold.
    #[serde(default)]
    pub use_neural_vad: bool,
    /// Deployment mode: local model or cloud API.
    #[serde(default)]
    pub deployment: EngineDeployment,
    /// Restrict ASR language detection to these languages only.
    /// Prevents mispronunciation from triggering wrong-language transcripts.
    #[serde(default = "default_allowed_languages")]
    pub allowed_languages: Vec<String>,
}

impl Default for VoiceInputConfig {
    fn default() -> Self {
        Self {
            hotkey: default_voice_hotkey(),
            silence_threshold_secs: default_silence_threshold(),
            privacy_mode: VoicePrivacyMode::default(),
            stt_engine: SttEngineKind::default(),
            vad_threshold: default_vad_threshold(),
            use_neural_vad: false,
            deployment: EngineDeployment::default(),
            allowed_languages: default_allowed_languages(),
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
    /// Deployment mode: local model or cloud API.
    #[serde(default)]
    pub deployment: EngineDeployment,
    /// Active voice persona key.
    #[serde(default = "default_persona_name")]
    pub default_persona: String,
    /// Named voice persona configurations.
    #[serde(default = "default_personas")]
    pub personas: std::collections::HashMap<String, VoicePersona>,
}

impl Default for VoiceOutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_preferences: std::collections::HashMap::new(),
            speaking_rate: 1.0,
            speak_during_focus: false,
            tts_engine: TtsEngineKind::default(),
            deployment: EngineDeployment::default(),
            default_persona: default_persona_name(),
            personas: default_personas(),
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

fn default_speaking_rate() -> f32 {
    1.0
}

fn default_vad_threshold() -> f32 {
    0.5
}

fn default_temperature() -> f32 {
    0.9
}

fn default_persona_name() -> String {
    "neutral".into()
}

fn default_allowed_languages() -> Vec<String> {
    vec!["en".into(), "zh".into(), "vi".into()]
}

fn default_personas() -> std::collections::HashMap<String, VoicePersona> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "neutral".into(),
        VoicePersona::Preset {
            speaker: "alloy".into(),
            speed: 1.0,
            temperature: 0.85,
        },
    );
    m.insert(
        "professional".into(),
        VoicePersona::Preset {
            speaker: "onyx".into(),
            speed: 0.95,
            temperature: 0.8,
        },
    );
    m.insert(
        "friendly".into(),
        VoicePersona::Preset {
            speaker: "nova".into(),
            speed: 1.0,
            temperature: 0.9,
        },
    );
    m.insert(
        "calm".into(),
        VoicePersona::Preset {
            speaker: "shimmer".into(),
            speed: 0.9,
            temperature: 0.7,
        },
    );
    m.insert(
        "energetic".into(),
        VoicePersona::Preset {
            speaker: "echo".into(),
            speed: 1.1,
            temperature: 0.95,
        },
    );
    m.insert(
        "storyteller".into(),
        VoicePersona::Preset {
            speaker: "fable".into(),
            speed: 0.92,
            temperature: 0.8,
        },
    );
    m
}

/// A named voice persona for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoicePersona {
    Preset {
        speaker: String,
        #[serde(default = "default_speaking_rate")]
        speed: f32,
        #[serde(default = "default_temperature")]
        temperature: f32,
    },
    Custom {
        description: String,
        #[serde(default = "default_speaking_rate")]
        speed: f32,
        #[serde(default = "default_temperature")]
        temperature: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_is_qwen3() {
        let input = VoiceInputConfig::default();
        assert_eq!(input.stt_engine, SttEngineKind::Qwen3);

        let output = VoiceOutputConfig::default();
        assert_eq!(output.tts_engine, TtsEngineKind::Qwen3);
    }

    #[test]
    fn deserialize_cloud_deployment() {
        let json =
            r#"{"mode": "cloud", "apiUrl": "https://api.example.com/v1", "apiKey": "sk-test"}"#;
        let deployment: EngineDeployment = serde_json::from_str(json).unwrap();
        match deployment {
            EngineDeployment::Cloud { api_url, api_key } => {
                assert_eq!(api_url, "https://api.example.com/v1");
                assert_eq!(api_key.expose(), "sk-test");
            }
            _ => panic!("Expected Cloud deployment"),
        }
    }

    #[test]
    fn default_deployment_is_local() {
        let json = r#"{}"#;
        let input: VoiceInputConfig = serde_json::from_str(json).unwrap();
        assert!(matches!(input.deployment, EngineDeployment::Local));
    }

    #[test]
    fn deserialize_system_engine() {
        let json = r#"{"ttsEngine": "system"}"#;
        let output: VoiceOutputConfig = serde_json::from_str(json).unwrap();
        assert_eq!(output.tts_engine, TtsEngineKind::System);
    }

    #[test]
    fn deserialize_unknown_engine_rejects() {
        let json = r#"{"sttEngine": "nonexistent"}"#;
        let result: Result<VoiceInputConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn default_persona_is_neutral() {
        let config = VoiceOutputConfig::default();
        assert_eq!(config.default_persona, "neutral");
        assert!(!config.personas.is_empty());
        assert!(config.personas.contains_key("neutral"));
    }

    #[test]
    fn deserialize_preset_persona() {
        let json = r#"{"type": "preset", "speaker": "onyx", "speed": 0.95, "temperature": 0.8}"#;
        let persona: VoicePersona = serde_json::from_str(json).unwrap();
        match persona {
            VoicePersona::Preset { speaker, speed, .. } => {
                assert_eq!(speaker, "onyx");
                assert!((speed - 0.95).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Preset"),
        }
    }

    #[test]
    fn deserialize_custom_persona() {
        let json = r#"{"type": "custom", "description": "deep calm voice", "speed": 0.9, "temperature": 0.7}"#;
        let persona: VoicePersona = serde_json::from_str(json).unwrap();
        match persona {
            VoicePersona::Custom { description, .. } => {
                assert_eq!(description, "deep calm voice");
            }
            _ => panic!("Expected Custom"),
        }
    }

    #[test]
    fn default_allowed_languages() {
        let config = VoiceInputConfig::default();
        assert_eq!(config.allowed_languages, vec!["en", "zh", "vi"]);
    }
}
