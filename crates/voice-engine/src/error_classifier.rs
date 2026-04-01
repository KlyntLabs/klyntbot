//! Error classifier — compares expected vs actual phonemes from alignment.
//!
//! Produces per-phoneme scores by comparing aligned phonemes against
//! reference pronunciation. For Chinese, combines phoneme score with tone match.

use serde::{Deserialize, Serialize};

use crate::pronunciation_analyzer::{PhonemeAlignment, ToneContour};

/// Score for a single phoneme comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhonemeScore {
    pub expected: String,
    pub actual: String,
    pub word: String,
    pub confidence: f32,
    pub timestamp_ms: u64,
    pub correct: bool,
}

/// Combined pronunciation error report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReport {
    pub phoneme_scores: Vec<PhonemeScore>,
    pub overall_accuracy: f32,
    pub weak_phonemes: Vec<String>,
}

/// Classify pronunciation errors from aligned phonemes.
///
/// Compares each aligned phoneme's confidence against thresholds:
/// - >= 0.8: correct
/// - < 0.8: error (phoneme is "weak")
pub fn classify_errors(
    alignment: &PhonemeAlignment,
    _tones: Option<&ToneContour>,
) -> ErrorReport {
    let phoneme_scores: Vec<PhonemeScore> = alignment
        .phonemes
        .iter()
        .map(|p| {
            let correct = p.confidence >= 0.8;
            PhonemeScore {
                expected: p.phoneme.clone(),
                actual: p.phoneme.clone(), // TODO: actual vs expected comparison
                word: p.word.clone(),
                confidence: p.confidence,
                timestamp_ms: p.start_ms,
                correct,
            }
        })
        .collect();

    let total = phoneme_scores.len();
    let mut correct_count = 0usize;
    let mut weak_phonemes = Vec::new();
    for s in &phoneme_scores {
        if s.correct {
            correct_count += 1;
        } else {
            weak_phonemes.push(s.expected.clone());
        }
    }
    let overall_accuracy = if total == 0 {
        1.0
    } else {
        correct_count as f32 / total as f32
    };

    ErrorReport {
        phoneme_scores,
        overall_accuracy,
        weak_phonemes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pronunciation_analyzer::{AlignedPhoneme, PhonemeAlignment};
    use crate::types::Language;

    #[test]
    fn empty_alignment_gives_perfect_accuracy() {
        let alignment = PhonemeAlignment {
            phonemes: vec![],
            language: Language::new("en"),
        };
        let report = classify_errors(&alignment, None);
        assert_eq!(report.overall_accuracy, 1.0);
        assert!(report.weak_phonemes.is_empty());
    }

    #[test]
    fn all_correct_phonemes() {
        let alignment = PhonemeAlignment {
            phonemes: vec![
                AlignedPhoneme {
                    phoneme: "h".to_string(),
                    word: "hello".to_string(),
                    start_ms: 0,
                    end_ms: 50,
                    confidence: 0.95,
                },
                AlignedPhoneme {
                    phoneme: "ɛ".to_string(),
                    word: "hello".to_string(),
                    start_ms: 50,
                    end_ms: 100,
                    confidence: 0.88,
                },
            ],
            language: Language::new("en"),
        };
        let report = classify_errors(&alignment, None);
        assert_eq!(report.overall_accuracy, 1.0);
        assert!(report.weak_phonemes.is_empty());
    }

    #[test]
    fn weak_phonemes_detected() {
        let alignment = PhonemeAlignment {
            phonemes: vec![
                AlignedPhoneme {
                    phoneme: "θ".to_string(),
                    word: "think".to_string(),
                    start_ms: 0,
                    end_ms: 80,
                    confidence: 0.45,
                },
                AlignedPhoneme {
                    phoneme: "ɪ".to_string(),
                    word: "think".to_string(),
                    start_ms: 80,
                    end_ms: 150,
                    confidence: 0.90,
                },
            ],
            language: Language::new("en"),
        };
        let report = classify_errors(&alignment, None);
        assert_eq!(report.overall_accuracy, 0.5);
        assert_eq!(report.weak_phonemes, vec!["θ"]);
    }
}
