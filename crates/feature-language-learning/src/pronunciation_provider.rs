//! PronunciationProvider implementation — bridges voice-engine's pipeline hook
//! to the language learning feature's analysis and storage.

use async_trait::async_trait;
use tracing::debug;

use voice_engine::types::Language;
use voice_engine::PronunciationProvider;

/// Bridges the voice engine's pronunciation hook to the language learning pipeline.
///
/// When injected into `VoiceService`, this runs after each transcription to:
/// 1. Run phoneme alignment (when the aligner is available)
/// 2. Classify errors and decide feedback level
/// 3. Emit pronunciation events for the frontend
pub struct AppPronunciationProvider;

impl AppPronunciationProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PronunciationProvider for AppPronunciationProvider {
    async fn analyze(&self, transcript: &str, language: &Language, duration_ms: u64) {
        debug!(
            "Pronunciation analysis: '{}' (lang={}, {:.1}s)",
            &transcript[..transcript.len().min(50)],
            language.as_str(),
            duration_ms as f32 / 1000.0
        );

        // TODO: Wire the full pipeline when phoneme aligner produces real data:
        // 1. phoneme_aligner.align(audio, transcript, language)
        // 2. tone_analyzer::analyze_tones(audio, &alignment) [Chinese only]
        // 3. error_classifier::classify_errors(&alignment, tones)
        // 4. feedback_decider::decide_feedback_level(mastery, config)
        // 5. Store results in pronunciation_logs table
        // 6. Update phoneme_mastery with FSRS
        // 7. Emit VoiceEvent::PronunciationReport

        if transcript.trim().is_empty() {
            return;
        }

        // Log that analysis was requested — placeholder until aligner is integrated
        debug!(
            "Pronunciation analysis complete for {} ({} lang)",
            &transcript[..transcript.len().min(30)],
            language.as_str()
        );
    }
}
