//! Core types for the voice engine.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Detected or configured language for voice capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language(pub String);

impl Language {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Language {
    fn default() -> Self {
        Self("en".to_string())
    }
}

/// Which transcription engine produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Local,
    Cloud,
}

/// Privacy level for voice capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyLevel {
    /// Normal: full cognitive pipeline integration.
    #[default]
    Standard,
    /// Strict: skip mirror lookups and pronunciation history.
    Strict,
    /// Off: no privacy restrictions.
    Off,
}

/// A chunk of raw audio samples from the microphone.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// A complete transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Language,
    pub segments: Vec<TranscriptSegment>,
    pub overall_confidence: f32,
}

impl Transcript {
    /// Recompute `overall_confidence` from segment averages.
    pub fn recompute_confidence(&mut self) {
        self.overall_confidence = if self.segments.is_empty() {
            0.0
        } else {
            self.segments.iter().map(|s| s.confidence).sum::<f32>() / self.segments.len() as f32
        };
    }
}

/// A word-level segment within a transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub text: String,
    #[serde(with = "duration_millis")]
    pub start: Duration,
    #[serde(with = "duration_millis")]
    pub end: Duration,
    /// 0.0-1.0 confidence score. Powers green/red word highlights.
    pub confidence: f32,
}

/// Metadata attached to a voice capture for downstream enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMetadata {
    pub language: Language,
    pub overall_confidence: f32,
    /// (word, score) pairs for pronunciation feedback.
    pub pronunciation_scores: Vec<(String, f32)>,
    /// Path to stored audio file for later playback/FSRS.
    pub audio_ref: Option<String>,
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    pub engine: EngineKind,
    pub privacy_mode: PrivacyLevel,
}

/// Synthesized audio clip ready for playback.
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Parameters for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsParams {
    pub language: Language,
    pub voice_name: Option<String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
    /// Natural language voice description for instruct-mode TTS (1.7B model).
    #[serde(default)]
    pub instruct: Option<String>,
    /// Override generation temperature (0.1-1.0). None uses engine default (0.9).
    #[serde(default)]
    pub temperature: Option<f32>,
}

fn default_speaking_rate() -> f32 {
    1.0
}

impl Default for TtsParams {
    fn default() -> Self {
        Self {
            language: Language::default(),
            voice_name: None,
            speaking_rate: default_speaking_rate(),
            instruct: None,
            temperature: None,
        }
    }
}

/// Info about an available TTS voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub identifier: String,
    pub display_name: String,
    pub language: Language,
}

/// Pronunciation rating derived from confidence thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PronunciationRating {
    Good,
    Fair,
    Poor,
}

/// Per-word pronunciation score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordScore {
    pub word: String,
    pub confidence: f32,
    pub rating: PronunciationRating,
}

/// Pronunciation analysis report for a transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PronunciationReport {
    pub overall_score: f32,
    pub word_scores: Vec<WordScore>,
    pub weak_words_count: usize,
    pub improvement_suggestion: Option<String>,
}

#[cfg(test)]
mod tts_params_tests {
    use super::*;

    #[test]
    fn default_tts_params_has_no_instruct() {
        let params = TtsParams::default();
        assert!(params.instruct.is_none());
        assert!(params.temperature.is_none());
        assert!((params.speaking_rate - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tts_params_with_instruct() {
        let params = TtsParams {
            instruct: Some("deep calm male voice".into()),
            temperature: Some(0.75),
            ..Default::default()
        };
        assert_eq!(params.instruct.as_deref(), Some("deep calm male voice"));
        assert_eq!(params.temperature, Some(0.75));
    }
}

/// Serde helper for Duration as milliseconds.
mod duration_millis {
    use std::time::Duration;

    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}
