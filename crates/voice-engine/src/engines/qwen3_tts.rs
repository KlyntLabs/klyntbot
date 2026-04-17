//! Qwen3-TTS speech synthesis engine.
//!
//! Local Qwen3-TTS via MLX backend. Supports multi-language synthesis
//! with natural prosody and speaker control. Uses lazy model loading
//! with automatic idle unloading to manage memory.

use std::path::PathBuf;
#[cfg(feature = "qwen3")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "qwen3")]
use std::time::Instant;

use async_trait::async_trait;
use tracing::info;
#[cfg(feature = "qwen3")]
use tracing::{debug, warn};

use crate::tts::TtsEngine;
use crate::types::*;

#[allow(dead_code)]
const QWEN3_TTS_SAMPLE_RATE: u32 = 24_000;
#[cfg(feature = "qwen3")]
const IDLE_UNLOAD_SECS: u64 = 300;
/// Max characters per text chunk — keeps generation within the 2048 max_codes budget.
#[cfg(feature = "qwen3")]
const MAX_CHUNK_CHARS: usize = 400;

#[cfg(feature = "qwen3")]
struct InnerState {
    last_used: Instant,
    model: Option<qwen3_tts_rs::inference::TTSInference>,
}

pub struct Qwen3TtsEngine {
    #[cfg_attr(not(feature = "qwen3"), allow(dead_code))]
    model_dir: PathBuf,
    #[cfg(feature = "qwen3")]
    state: Arc<Mutex<InnerState>>,
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
            #[cfg(feature = "qwen3")]
            state: Arc::new(Mutex::new(InnerState {
                last_used: Instant::now(),
                model: None,
            })),
        })
    }

    #[cfg(feature = "qwen3")]
    fn load_model(
        model_dir: &std::path::Path,
    ) -> Result<qwen3_tts_rs::inference::TTSInference, String> {
        qwen3_tts_rs::backend::mlx::stream::init_mlx(true);
        let device = qwen3_tts_rs::tensor::Device::Gpu(0);
        qwen3_tts_rs::inference::TTSInference::new(model_dir, device).map_err(|e| format!("{e}"))
    }

    /// Unload model if idle for more than `IDLE_UNLOAD_SECS`.
    pub fn unload_if_idle(&self) -> bool {
        #[cfg(feature = "qwen3")]
        {
            let mut state = self.state.lock().unwrap();
            if state.last_used.elapsed().as_secs() >= IDLE_UNLOAD_SECS && state.model.is_some() {
                state.model = None;
                info!("Qwen3-TTS model unloaded after idle");
                return true;
            }
        }
        false
    }

    /// Eagerly load the model in a blocking thread.
    pub async fn preload(&self) -> common::Result<()> {
        #[cfg(feature = "qwen3")]
        {
            let state = self.state.clone();
            let model_dir = self.model_dir.clone();
            tokio::task::spawn_blocking(move || {
                {
                    let guard = state.lock().unwrap();
                    if guard.model.is_some() {
                        return Ok::<(), common::KlyntbotError>(());
                    }
                }
                info!("Preloading Qwen3-TTS from {}...", model_dir.display());
                let start = Instant::now();
                let model = Self::load_model(&model_dir).map_err(|e| {
                    common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(e))
                })?;
                info!(
                    "Qwen3-TTS preloaded in {:.1}s",
                    start.elapsed().as_secs_f32()
                );
                let mut guard = state.lock().unwrap();
                guard.model = Some(model);
                Ok(())
            })
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                    e.to_string(),
                ))
            })??;
        }
        Ok(())
    }
}

#[async_trait]
impl TtsEngine for Qwen3TtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        #[cfg(not(feature = "qwen3"))]
        {
            let _ = (text, params);
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(
                    "Qwen3-TTS requires the 'qwen3' feature".to_string(),
                ),
            ));
        }

        #[cfg(feature = "qwen3")]
        {
            let voice = params.voice_name.as_deref().unwrap_or("alloy");
            let lang = map_language(&params.language);
            let temperature = params.temperature.unwrap_or(0.9) as f64;
            let instruct = params.instruct.clone();

            debug!(
                "Qwen3-TTS synthesizing '{}' ({} chars, voice={}, lang={}, instruct={})",
                &text[..text.len().min(50)],
                text.len(),
                voice,
                lang,
                instruct.is_some()
            );

            let text_owned = text.to_string();
            let voice_owned = voice.to_string();
            let state = self.state.clone();
            let model_dir = self.model_dir.clone();

            let result = tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
                let mut guard = state.lock().unwrap();
                if guard.model.is_none() {
                    info!("Lazy-loading Qwen3-TTS from {}...", model_dir.display());
                    let start = Instant::now();
                    let model = Qwen3TtsEngine::load_model(&model_dir)?;
                    info!("Qwen3-TTS loaded in {:.1}s", start.elapsed().as_secs_f32());
                    guard.model = Some(model);
                }
                guard.last_used = Instant::now();
                let model = guard.model.as_ref().ok_or("Model not loaded")?;

                // Chunk long texts to stay within the model's max_codes budget.
                let chunks = qwen3_tts_rs::api::chunking::chunk_text(&text_owned, MAX_CHUNK_CHARS);
                let mut all_samples = Vec::new();

                for (i, chunk) in chunks.iter().enumerate() {
                    debug!(
                        "Qwen3-TTS chunk {}/{}: '{}' ({} chars)",
                        i + 1,
                        chunks.len(),
                        &chunk[..chunk.len().min(40)],
                        chunk.len()
                    );

                    let (samples, _sr) = if let Some(ref desc) = instruct {
                        model
                            .generate_with_instruct(
                                chunk,
                                &voice_owned,
                                &lang,
                                desc,
                                temperature,
                                50,
                                2048,
                            )
                            .map_err(|e| format!("Qwen3-TTS instruct failed: {e}"))?
                    } else {
                        model
                            .generate_with_params(chunk, &voice_owned, &lang, temperature, 50, 2048)
                            .map_err(|e| format!("Qwen3-TTS generation failed: {e}"))?
                    };
                    all_samples.extend(samples);
                }

                Ok(all_samples)
            })
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Qwen3-TTS task failed: {e}"
                )))
            })?;

            match result {
                Ok(samples) => {
                    debug!("Qwen3-TTS produced {} samples", samples.len());
                    Ok(AudioClip {
                        samples,
                        sample_rate: QWEN3_TTS_SAMPLE_RATE,
                        channels: 1,
                    })
                }
                Err(e) => {
                    warn!("Qwen3-TTS inference error: {e}");
                    Err(common::KlyntbotError::Provider(
                        common::ProviderError::InvalidResponse(e),
                    ))
                }
            }
        }
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

    fn unload_if_idle(&self) -> bool {
        Qwen3TtsEngine::unload_if_idle(self)
    }

    async fn preload(&self) -> common::Result<()> {
        Qwen3TtsEngine::preload(self).await
    }
}

/// Map our Language type to the string the Qwen3 model expects.
#[cfg(feature = "qwen3")]
fn map_language(lang: &Language) -> String {
    match lang.as_str() {
        "en" | "eng" => "english".to_string(),
        "zh" | "cmn" | "zho" => "chinese".to_string(),
        "ja" | "jpn" => "japanese".to_string(),
        "ko" | "kor" => "korean".to_string(),
        other => other.to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices_list_not_empty() {
        assert!(!crate::engines::QWEN3_VOICES.is_empty());
    }

    #[test]
    fn sample_rate_is_24k() {
        assert_eq!(QWEN3_TTS_SAMPLE_RATE, 24_000);
    }

    #[cfg(feature = "qwen3")]
    #[test]
    fn language_mapping() {
        assert_eq!(map_language(&Language::new("en")), "english");
        assert_eq!(map_language(&Language::new("zh")), "chinese");
        assert_eq!(map_language(&Language::new("ja")), "japanese");
        assert_eq!(map_language(&Language::new("ko")), "korean");
        assert_eq!(map_language(&Language::new("fr")), "fr");
    }
}
