//! Qwen3-ForcedAligner wrapper for phoneme-level alignment.

use std::path::PathBuf;

use async_trait::async_trait;
use tracing::{debug, info};

use crate::pronunciation_analyzer::*;
use crate::types::{AudioClip, Language};

pub struct Qwen3PhonemeAligner {
    _model_dir: PathBuf,
}

impl Qwen3PhonemeAligner {
    pub fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        if !model_dir.exists() {
            return Err(common::KlyntbotError::Config(
                common::ConfigError::NotFound(format!(
                    "Qwen3-ForcedAligner model not found: {}",
                    model_dir.display()
                )),
            ));
        }
        info!("Qwen3 phoneme aligner ready: {}", model_dir.display());
        Ok(Self {
            _model_dir: model_dir,
        })
    }
}

#[async_trait]
impl PronunciationAnalyzer for Qwen3PhonemeAligner {
    async fn align(
        &self,
        audio: &AudioClip,
        transcript: &str,
        lang: &Language,
    ) -> common::Result<PhonemeAlignment> {
        debug!(
            "Aligning {} samples against '{}' (lang={})",
            audio.samples.len(),
            &transcript[..transcript.len().min(50)],
            lang.as_str()
        );

        // TODO: Integrate qwen3_asr forced alignment API.
        Ok(PhonemeAlignment {
            phonemes: vec![],
            language: lang.clone(),
        })
    }

    async fn extract_tones(
        &self,
        _audio: &AudioClip,
        alignment: &PhonemeAlignment,
    ) -> common::Result<ToneContour> {
        if alignment.language.as_str() != "zh" {
            return Ok(ToneContour {
                syllables: vec![],
            });
        }

        // TODO: Use pitch-detection crate (YIN) to extract F0 contour per syllable.
        Ok(ToneContour {
            syllables: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_chinese_skips_tones() {
        let alignment = PhonemeAlignment {
            phonemes: vec![],
            language: Language::new("en"),
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        let aligner = Qwen3PhonemeAligner::new(tmp.path()).unwrap();
        let audio = AudioClip {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
            channels: 1,
        };
        let result = rt
            .block_on(aligner.extract_tones(&audio, &alignment))
            .unwrap();
        assert!(result.syllables.is_empty());
    }
}
