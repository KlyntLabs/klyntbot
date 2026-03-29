//! Voice input/output configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

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
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: VoiceInputConfig::default(),
            output: VoiceOutputConfig::default(),
            learning: VoiceLearningConfig::default(),
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
    #[serde(default = "default_true")]
    pub prefer_local: bool,
    #[serde(default = "default_model_size")]
    pub model_size: String,
}

impl Default for VoiceInputConfig {
    fn default() -> Self {
        Self {
            hotkey: default_voice_hotkey(),
            silence_threshold_secs: default_silence_threshold(),
            privacy_mode: VoicePrivacyMode::default(),
            prefer_local: true,
            model_size: default_model_size(),
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
}

impl Default for VoiceOutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_preferences: std::collections::HashMap::new(),
            speaking_rate: 1.0,
            speak_during_focus: false,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoicePrivacyMode {
    #[default]
    Standard,
    Strict,
    Off,
}

fn default_voice_hotkey() -> String {
    "super+shift+v".to_string()
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
