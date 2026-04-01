//! Pronunciation analysis trait — phoneme alignment and tone extraction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::{AudioClip, Language};

/// A single phoneme with timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignedPhoneme {
    pub phoneme: String,
    pub word: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

/// Result of phoneme alignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhonemeAlignment {
    pub phonemes: Vec<AlignedPhoneme>,
    pub language: Language,
}

/// F0 pitch contour for a syllable (Chinese tone analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyllableTone {
    pub syllable: String,
    pub expected_tone: u8,
    pub detected_tone: u8,
    pub f0_contour: Vec<f32>,
    pub correct: bool,
}

/// Tone contour analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToneContour {
    pub syllables: Vec<SyllableTone>,
}

#[async_trait]
pub trait PronunciationAnalyzer: Send + Sync {
    /// Align transcript to audio, returning phoneme-level timestamps.
    async fn align(
        &self,
        audio: &AudioClip,
        transcript: &str,
        lang: &Language,
    ) -> common::Result<PhonemeAlignment>;

    /// Extract F0 pitch contour for tone analysis (Chinese).
    async fn extract_tones(
        &self,
        audio: &AudioClip,
        alignment: &PhonemeAlignment,
    ) -> common::Result<ToneContour>;
}
