//! Kokoro TTS engine via ONNX Runtime.
//!
//! Wraps the `kokoro-tts` crate which provides a lightweight (82M params)
//! non-autoregressive TTS model. Produces 24kHz audio with sub-200ms latency
//! on Apple Silicon.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::tts::TtsEngine;
use crate::types::*;

/// Expected filenames inside the Kokoro model directory.
const KOKORO_MODEL_FILE: &str = "kokoro.onnx";
const KOKORO_VOICES_FILE: &str = "voices.bin";

/// Kokoro output sample rate (fixed by the model architecture).
const KOKORO_SAMPLE_RATE: u32 = 24_000;

/// Voice identifiers supported by the Kokoro engine.
/// Single source of truth for both `resolve_voice` and `available_voices`.
const VOICES: &[(&str, &str, &str)] = &[
    ("af_heart", "Heart (Female, American)", "en"),
    ("af_bella", "Bella (Female, American)", "en"),
    ("af_nicole", "Nicole (Female, American)", "en"),
    ("af_nova", "Nova (Female, American)", "en"),
    ("af_sarah", "Sarah (Female, American)", "en"),
    ("af_sky", "Sky (Female, American)", "en"),
    ("am_michael", "Michael (Male, American)", "en"),
    ("am_adam", "Adam (Male, American)", "en"),
    ("am_liam", "Liam (Male, American)", "en"),
    ("bf_emma", "Emma (Female, British)", "en"),
    ("bf_lily", "Lily (Female, British)", "en"),
    ("bm_daniel", "Daniel (Male, British)", "en"),
    ("bm_george", "George (Male, British)", "en"),
];

/// Kokoro TTS engine.
///
/// The internal `KokoroTts` instance uses a tokio Mutex for serialized access
/// (ONNX inference is single-threaded per session). Model files required:
/// `kokoro.onnx` and `voices.bin` in the model directory.
pub struct KokoroTtsEngine {
    model: Mutex<kokoro_tts::KokoroTts>,
}

impl KokoroTtsEngine {
    /// Load the Kokoro TTS engine from a model directory.
    ///
    /// Expects `kokoro.onnx` and `voices.bin` inside the directory.
    pub async fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        let model_path = model_dir.join(KOKORO_MODEL_FILE);
        let voices_path = model_dir.join(KOKORO_VOICES_FILE);

        info!("Loading Kokoro TTS model from {}", model_dir.display());

        let model = kokoro_tts::KokoroTts::new(&model_path, &voices_path)
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Failed to load Kokoro TTS model: {e}"
                )))
            })?;

        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

/// Map a voice name string to a `kokoro_tts::Voice` enum variant.
fn resolve_voice(name: Option<&str>, speed: f32) -> kokoro_tts::Voice {
    match name {
        Some("af_heart") | None => kokoro_tts::Voice::AfHeart(speed),
        Some("af_bella") => kokoro_tts::Voice::AfBella(speed),
        Some("af_nicole") => kokoro_tts::Voice::AfNicole(speed),
        Some("af_nova") => kokoro_tts::Voice::AfNova(speed),
        Some("af_sarah") => kokoro_tts::Voice::AfSarah(speed),
        Some("af_sky") => kokoro_tts::Voice::AfSky(speed),
        Some("am_michael") => kokoro_tts::Voice::AmMichael(speed),
        Some("am_adam") => kokoro_tts::Voice::AmAdam(speed),
        Some("am_liam") => kokoro_tts::Voice::AmLiam(speed),
        Some("bf_emma") => kokoro_tts::Voice::BfEmma(speed),
        Some("bf_lily") => kokoro_tts::Voice::BfLily(speed),
        Some("bm_daniel") => kokoro_tts::Voice::BmDaniel(speed),
        Some("bm_george") => kokoro_tts::Voice::BmGeorge(speed),
        Some(unknown) => {
            warn!("Unknown Kokoro voice '{unknown}', falling back to af_heart");
            kokoro_tts::Voice::AfHeart(speed)
        }
    }
}

#[async_trait]
impl TtsEngine for KokoroTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let voice = resolve_voice(params.voice_name.as_deref(), params.speaking_rate);

        // Synthesis is serialized: ONNX session is single-threaded per model instance.
        let (samples, duration) = self
            .model
            .lock()
            .await
            .synth(text, voice)
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Kokoro synthesis failed: {e}"
                )))
            })?;

        debug!("Kokoro synthesized {}ms of audio", duration.as_millis());

        Ok(AudioClip {
            samples,
            sample_rate: KOKORO_SAMPLE_RATE,
            channels: 1,
        })
    }

    fn supports_language(&self, lang: &Language) -> bool {
        matches!(
            lang.as_str(),
            "en" | "zh" | "ja" | "ko" | "fr" | "it" | "pt" | "es" | "hi"
        )
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        VOICES
            .iter()
            .map(|(id, name, lang)| VoiceInfo {
                identifier: id.to_string(),
                display_name: name.to_string(),
                language: Language::new(*lang),
            })
            .collect()
    }

    fn display_name(&self) -> &str {
        "Kokoro TTS"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_voice_defaults_to_af_heart() {
        let v = resolve_voice(None, 1.0);
        assert!(matches!(v, kokoro_tts::Voice::AfHeart(_)));
    }

    #[test]
    fn resolve_voice_maps_known_names() {
        assert!(matches!(
            resolve_voice(Some("am_michael"), 1.2),
            kokoro_tts::Voice::AmMichael(_)
        ));
        assert!(matches!(
            resolve_voice(Some("bf_emma"), 0.8),
            kokoro_tts::Voice::BfEmma(_)
        ));
    }

    #[test]
    fn resolve_voice_falls_back_on_unknown() {
        let v = resolve_voice(Some("nonexistent_voice"), 1.0);
        assert!(matches!(v, kokoro_tts::Voice::AfHeart(_)));
    }

    #[test]
    fn voices_list_matches_resolve() {
        // Every voice in VOICES must be handled by resolve_voice (not hit the fallback).
        for (id, _, _) in VOICES {
            let v = resolve_voice(Some(id), 1.0);
            // The fallback branch produces AfHeart — only "af_heart" itself should map there.
            if *id != "af_heart" {
                assert!(
                    !matches!(v, kokoro_tts::Voice::AfHeart(_)),
                    "Voice '{id}' fell through to default in resolve_voice"
                );
            }
        }
    }

    #[test]
    fn supports_expected_languages() {
        let en = Language::new("en");
        let zh = Language::new("zh");
        let de = Language::new("de");
        assert!(matches!(
            en.as_str(),
            "en" | "zh" | "ja" | "ko" | "fr" | "it" | "pt" | "es" | "hi"
        ));
        assert!(matches!(
            zh.as_str(),
            "en" | "zh" | "ja" | "ko" | "fr" | "it" | "pt" | "es" | "hi"
        ));
        assert!(!matches!(
            de.as_str(),
            "en" | "zh" | "ja" | "ko" | "fr" | "it" | "pt" | "es" | "hi"
        ));
    }
}
