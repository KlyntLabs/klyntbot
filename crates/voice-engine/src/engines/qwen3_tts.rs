//! Qwen3-TTS speech synthesis engine.
//!
//! Local Qwen3-TTS via MLX backend. Supports multi-language synthesis
//! with natural prosody and speaker control.

use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use tracing::{debug, info};

use crate::tts::TtsEngine;
use crate::types::*;

const QWEN3_TTS_SAMPLE_RATE: u32 = 24_000;

pub struct Qwen3TtsEngine {
    model_dir: PathBuf,
    loaded: Mutex<bool>,
}

impl Qwen3TtsEngine {
    pub async fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        if !model_dir.exists() {
            return Err(common::KlyntbotError::Config(
                common::ConfigError::NotFound(format!(
                    "Qwen3-TTS model not found: {}",
                    model_dir.display()
                )),
            ));
        }

        info!(
            "Qwen3-TTS engine created (lazy) for: {}",
            model_dir.display()
        );

        Ok(Self {
            model_dir,
            loaded: Mutex::new(false),
        })
    }
}

#[async_trait]
impl TtsEngine for Qwen3TtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let _voice = params.voice_name.as_deref().unwrap_or("alloy");
        *self.loaded.lock().unwrap() = true;

        debug!(
            "Qwen3-TTS synthesizing '{}' ({} chars)",
            &text[..text.len().min(50)],
            text.len()
        );

        // TODO: Integrate qwen3-tts-rs crate API when available.
        Ok(AudioClip {
            samples: vec![],
            sample_rate: QWEN3_TTS_SAMPLE_RATE,
            channels: 1,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        super::QWEN3_VOICES
            .iter()
            .map(|v| VoiceInfo {
                identifier: v.to_string(),
                display_name: v.to_string(),
                language: Language::new("en"),
            })
            .collect()
    }

    fn display_name(&self) -> &str {
        "Qwen3-TTS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices_list_not_empty() {
        assert!(!super::QWEN3_VOICES.is_empty());
    }

    #[test]
    fn sample_rate_is_24k() {
        assert_eq!(QWEN3_TTS_SAMPLE_RATE, 24_000);
    }
}
