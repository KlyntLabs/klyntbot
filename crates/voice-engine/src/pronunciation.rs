//! Pronunciation scoring — word-level confidence analysis.

use crate::types::{PronunciationRating, PronunciationReport, Transcript, WordScore};

/// Compute pronunciation scores from Whisper transcript confidence values.
pub fn compute_pronunciation_report(transcript: &Transcript) -> PronunciationReport {
    let word_scores: Vec<WordScore> = transcript
        .segments
        .iter()
        .map(|seg| WordScore {
            word: seg.text.clone(),
            confidence: seg.confidence,
            rating: match seg.confidence {
                c if c >= 0.85 => PronunciationRating::Good,
                c if c >= 0.60 => PronunciationRating::Fair,
                _ => PronunciationRating::Poor,
            },
        })
        .collect();

    let overall_score = if word_scores.is_empty() {
        0.0
    } else {
        word_scores.iter().map(|w| w.confidence).sum::<f32>() / word_scores.len() as f32
    };

    let weak_word_texts: Vec<&str> = word_scores
        .iter()
        .filter(|w| w.rating == PronunciationRating::Poor)
        .map(|w| w.word.as_str())
        .collect();

    let weak_words_count = weak_word_texts.len();

    let improvement_suggestion = if weak_word_texts.is_empty() {
        None
    } else {
        Some(format!("Focus on: {}", weak_word_texts.join(", ")))
    };

    PronunciationReport {
        overall_score,
        word_scores,
        weak_words_count,
        improvement_suggestion,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::types::{Language, TranscriptSegment};

    fn seg(text: &str, confidence: f32) -> TranscriptSegment {
        TranscriptSegment {
            text: text.to_string(),
            start: Duration::ZERO,
            end: Duration::from_millis(500),
            confidence,
        }
    }

    fn transcript(segments: Vec<TranscriptSegment>) -> Transcript {
        Transcript {
            text: segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            language: Language::new("fr"),
            segments,
            overall_confidence: 0.0,
        }
    }

    #[test]
    fn empty_transcript_gives_zero_score() {
        let t = transcript(vec![]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.overall_score, 0.0);
        assert_eq!(report.weak_words_count, 0);
        assert!(report.improvement_suggestion.is_none());
    }

    #[test]
    fn all_good_pronunciation() {
        let t = transcript(vec![seg("bonjour", 0.95), seg("monde", 0.90)]);
        let report = compute_pronunciation_report(&t);
        assert!(report.overall_score > 0.9);
        assert_eq!(report.weak_words_count, 0);
        assert!(report.improvement_suggestion.is_none());
        assert!(report
            .word_scores
            .iter()
            .all(|w| w.rating == PronunciationRating::Good));
    }

    #[test]
    fn mixed_pronunciation_highlights_weak_words() {
        let t = transcript(vec![
            seg("je", 0.92),
            seg("suis", 0.40),
            seg("content", 0.70),
        ]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.weak_words_count, 1);
        assert_eq!(report.word_scores[0].rating, PronunciationRating::Good);
        assert_eq!(report.word_scores[1].rating, PronunciationRating::Poor);
        assert_eq!(report.word_scores[2].rating, PronunciationRating::Fair);
        assert_eq!(
            report.improvement_suggestion.as_deref(),
            Some("Focus on: suis")
        );
    }

    #[test]
    fn single_word_transcript() {
        let t = transcript(vec![seg("merci", 0.55)]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.overall_score, 0.55);
        assert_eq!(report.weak_words_count, 1);
        assert!(report
            .improvement_suggestion
            .as_ref()
            .unwrap()
            .contains("merci"));
    }

    #[test]
    fn boundary_confidence_values() {
        let t = transcript(vec![seg("a", 0.85), seg("b", 0.60), seg("c", 0.59)]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.word_scores[0].rating, PronunciationRating::Good);
        assert_eq!(report.word_scores[1].rating, PronunciationRating::Fair);
        assert_eq!(report.word_scores[2].rating, PronunciationRating::Poor);
    }
}
