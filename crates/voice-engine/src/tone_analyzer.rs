//! Tone contour analyzer — F0 pitch extraction for Chinese tones.
//!
//! Extracts fundamental frequency (F0) contour per syllable using the YIN
//! algorithm via the `pitch-detection` crate, then classifies each syllable
//! as Mandarin tone 1-4 or neutral based on contour shape.

use crate::pronunciation_analyzer::{PhonemeAlignment, SyllableTone, ToneContour};
use crate::types::AudioClip;

/// Classify a tone contour into Mandarin tones 1-4 (or 5 for neutral).
///
/// Tone patterns:
/// - Tone 1 (high level): stable high F0
/// - Tone 2 (rising): F0 rises from mid to high
/// - Tone 3 (dipping): F0 falls then rises
/// - Tone 4 (falling): F0 falls from high to low
/// - Tone 5 (neutral): short, unstressed
fn classify_tone(f0_contour: &[f32]) -> u8 {
    let n = f0_contour.len();
    if n < 3 {
        return 5; // neutral — too short to classify
    }

    // Single-pass: accumulate third-segment sums + global min/max.
    let boundary1 = n / 3;
    let boundary2 = n * 2 / 3;
    let mut sum_first = 0.0f32;
    let mut sum_mid = 0.0f32;
    let mut sum_last = 0.0f32;
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    for (i, &v) in f0_contour.iter().enumerate() {
        if i < boundary1 {
            sum_first += v;
        } else if i < boundary2 {
            sum_mid += v;
        } else {
            sum_last += v;
        }
        if v < min_val {
            min_val = v;
        }
        if v > max_val {
            max_val = v;
        }
    }

    let first_third = sum_first / boundary1 as f32;
    let mid_third = sum_mid / (boundary2 - boundary1) as f32;
    let last_third = sum_last / (n - boundary2) as f32;
    let range = max_val - min_val;

    let rise = last_third - first_third;
    let dip = mid_third - first_third;

    if range < 20.0 {
        1 // level
    } else if rise > 30.0 && dip > -10.0 {
        2 // rising
    } else if dip < -20.0 && rise > -10.0 {
        3 // dipping
    } else if rise < -30.0 {
        4 // falling
    } else {
        5 // neutral
    }
}

/// Analyze tone contours from an audio clip using aligned phoneme data.
///
/// Only operates on Chinese (`zh`) audio. For other languages, returns an
/// empty contour.
pub fn analyze_tones(
    _audio: &AudioClip,
    alignment: &PhonemeAlignment,
) -> ToneContour {
    if alignment.language.as_str() != "zh" {
        return ToneContour {
            syllables: vec![],
        };
    }

    // TODO: Extract actual F0 contour per syllable using pitch-detection crate.
    // For now, return empty — will be populated when the aligner provides
    // real phoneme timing data.
    ToneContour {
        syllables: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_level_tone() {
        let contour = vec![200.0, 201.0, 199.0, 200.5, 200.0, 199.5];
        assert_eq!(classify_tone(&contour), 1);
    }

    #[test]
    fn classify_rising_tone() {
        let contour = vec![150.0, 155.0, 165.0, 175.0, 185.0, 195.0];
        assert_eq!(classify_tone(&contour), 2);
    }

    #[test]
    fn classify_falling_tone() {
        let contour = vec![250.0, 230.0, 210.0, 190.0, 175.0, 160.0];
        assert_eq!(classify_tone(&contour), 4);
    }

    #[test]
    fn short_contour_is_neutral() {
        assert_eq!(classify_tone(&[100.0, 110.0]), 5);
    }

    #[test]
    fn non_chinese_returns_empty() {
        let alignment = PhonemeAlignment {
            phonemes: vec![],
            language: crate::types::Language::new("en"),
        };
        let audio = AudioClip {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
            channels: 1,
        };
        let result = analyze_tones(&audio, &alignment);
        assert!(result.syllables.is_empty());
    }
}
