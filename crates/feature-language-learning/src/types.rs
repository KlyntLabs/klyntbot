//! Types for the language learning feature.
//!
//! Pronunciation pipeline types (`PhonemeScore`, `SyllableTone`) are canonical
//! in `voice_engine::error_classifier` and `voice_engine::pronunciation_analyzer`.
//! This module re-exports them and adds feature-specific report types.

use ai_core_macros::AiEntity;
use serde::{Deserialize, Serialize};

// Re-export canonical pipeline types.
pub use voice_engine::error_classifier::PhonemeScore;
pub use voice_engine::pronunciation_analyzer::SyllableTone;

/// Detailed pronunciation report for a single turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedPronunciationReport {
    pub overall_score: f32,
    pub phoneme_scores: Vec<PhonemeScore>,
    pub tone_scores: Vec<SyllableTone>,
    pub fluency: FluencyMetrics,
    pub weak_phonemes: Vec<WeakPhoneme>,
    pub feedback_level: config::schema::FeedbackLevel,
}

/// Fluency metrics across an utterance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluencyMetrics {
    pub words_per_minute: f32,
    pub pause_count: u32,
    pub filler_count: u32,
}

/// A phoneme that the learner consistently struggles with.
#[derive(Debug, Clone, Serialize, Deserialize, AiEntity)]
#[serde(rename_all = "camelCase")]
#[ai(entity_type = "weak_phoneme", embed_on = ["phoneme", "language"])]
pub struct WeakPhoneme {
    pub phoneme: String,
    pub language: String,
    pub stability: f32,
    pub encounters: u32,
    pub error_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::AiEntity;

    #[test]
    fn weak_phoneme_ai_entity() {
        let w = WeakPhoneme {
            phoneme: "θ".into(),
            language: "en".into(),
            stability: 0.3,
            encounters: 12,
            error_rate: 0.7,
        };
        assert_eq!(WeakPhoneme::entity_type(), "weak_phoneme");
        assert_eq!(w.embed_text(), "θ\nen");
    }
}
