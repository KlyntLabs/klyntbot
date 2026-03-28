//! Integration tests for the voice engine pipeline.

use std::time::Duration;

use voice_engine::events::VoiceEvent;
use voice_engine::mock::MockTranscriptionEngine;
use voice_engine::pronunciation::compute_pronunciation_report;
use voice_engine::router::VoiceRouter;
use voice_engine::session::VoiceSessionState;
use voice_engine::types::*;

#[test]
fn full_pipeline_capture_to_routing() {
    let transcript = Transcript {
        text: "schedule dentist and practice french vocab".to_string(),
        language: Language::new("en"),
        segments: vec![
            seg("schedule", 0.92),
            seg("dentist", 0.88),
            seg("and", 0.95),
            seg("practice", 0.90),
            seg("french", 0.85),
            seg("vocab", 0.87),
        ],
        overall_confidence: 0.90,
    };

    let router = VoiceRouter::new();
    let intents = router.detect_intents(&transcript.text);
    assert!(intents.len() >= 2, "Should detect task + learning intents, got {}", intents.len());
    assert!(VoiceRouter::is_multi_intent(&intents));

    let events = router.to_events(&intents);
    assert!(events.iter().all(|e| matches!(e, VoiceEvent::RoutingSuggestion { .. })));

    let report = compute_pronunciation_report(&transcript);
    assert!(report.overall_score > 0.85);
    assert_eq!(report.weak_words_count, 0);
}

#[test]
fn pronunciation_report_feeds_voice_metadata() {
    let transcript = Transcript {
        text: "je suis content".to_string(),
        language: Language::new("fr"),
        segments: vec![
            seg("je", 0.92),
            seg("suis", 0.40),
            seg("content", 0.70),
        ],
        overall_confidence: 0.67,
    };

    let report = compute_pronunciation_report(&transcript);

    let metadata = VoiceMetadata {
        language: transcript.language.clone(),
        overall_confidence: transcript.overall_confidence,
        pronunciation_scores: report
            .word_scores
            .iter()
            .map(|w| (w.word.clone(), w.confidence))
            .collect(),
        audio_ref: Some("/tmp/test.wav".to_string()),
        duration: Duration::from_secs(3),
        engine: EngineKind::Local,
        privacy_mode: PrivacyLevel::Standard,
    };

    assert_eq!(metadata.pronunciation_scores.len(), 3);
    assert_eq!(metadata.pronunciation_scores[1].0, "suis");
    assert!(metadata.pronunciation_scores[1].1 < 0.5);

    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: VoiceMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.pronunciation_scores.len(), 3);
}

#[test]
fn session_state_full_lifecycle() {
    let mut state = VoiceSessionState::Idle;

    assert!(state.can_transition_to(VoiceSessionState::Capturing));
    state = VoiceSessionState::Capturing;
    assert!(state.is_active());

    assert!(state.can_transition_to(VoiceSessionState::Finalizing));
    state = VoiceSessionState::Finalizing;
    assert!(state.is_active());

    assert!(state.can_transition_to(VoiceSessionState::WaitingForResponse));
    state = VoiceSessionState::WaitingForResponse;
    assert!(state.is_active());

    assert!(state.can_transition_to(VoiceSessionState::Complete));
    state = VoiceSessionState::Complete;
    assert!(!state.is_active());

    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[test]
fn dismiss_during_capture_allows_cancel() {
    let state = VoiceSessionState::Capturing;
    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[test]
fn dismiss_during_finalizing_allows_cancel() {
    let state = VoiceSessionState::Finalizing;
    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[tokio::test]
async fn mock_engine_produces_transcript() {
    use voice_engine::stt::TranscriptionEngine;

    let engine = MockTranscriptionEngine::new("hello world");
    let transcript = engine
        .transcribe_file(std::path::Path::new("/dev/null"), None)
        .await
        .unwrap();

    assert_eq!(transcript.text, "hello world");
    assert_eq!(transcript.segments.len(), 2);
    assert_eq!(transcript.segments[0].text, "hello");
}

fn seg(text: &str, confidence: f32) -> TranscriptSegment {
    TranscriptSegment {
        text: text.to_string(),
        start: Duration::ZERO,
        end: Duration::from_millis(300),
        confidence,
    }
}
