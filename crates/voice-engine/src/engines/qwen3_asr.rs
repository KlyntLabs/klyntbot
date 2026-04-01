//! Qwen3-ASR speech recognition engine.
//!
//! Uses the `qwen3-asr` crate with Metal (Apple Silicon) backend for
//! local speech-to-text. Supports 52 languages with auto-detection.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript};

const IDLE_UNLOAD_SECS: u64 = 300;
pub(crate) const QWEN3_ASR_SAMPLE_RATE: u32 = 16_000;
const EXPECTED_SAMPLES: usize = QWEN3_ASR_SAMPLE_RATE as usize * 30;

struct InnerState {
    last_used: Instant,
    model: Option<qwen3_asr::AsrInference>,
}

pub struct Qwen3AsrEngine {
    /// Parent models directory (e.g., `~/.klyntbot/models/`).
    /// `from_pretrained` will create `{models_dir}/Qwen--Qwen3-ASR-0.6B/` inside.
    models_dir: PathBuf,
    state: Arc<Mutex<InnerState>>,
    allowed_languages: Vec<String>,
}

/// Map model-returned language names to ISO 639-1 codes.
fn normalize_language(lang: &str) -> &str {
    match lang.to_lowercase().as_str() {
        "english" | "en" => "en",
        "chinese" | "mandarin chinese" | "zh" => "zh",
        "vietnamese" | "vi" => "vi",
        "japanese" | "ja" => "ja",
        "korean" | "ko" => "ko",
        "french" | "fr" => "fr",
        "german" | "de" => "de",
        "spanish" | "es" => "es",
        _ => lang.split_whitespace().next().unwrap_or("en"),
    }
}

impl Qwen3AsrEngine {
    /// Create a new Qwen3-ASR engine. `models_dir` is the parent models directory
    /// (e.g., `~/.klyntbot/models/`). The model will be auto-downloaded on first use
    /// via `from_pretrained` if not already cached.
    pub fn new(
        models_dir: impl Into<PathBuf>,
        allowed_languages: Vec<String>,
    ) -> common::Result<Self> {
        let models_dir = models_dir.into();
        info!(
            "Qwen3-ASR engine created for cache: {} (languages: {:?})",
            models_dir.display(),
            allowed_languages
        );
        Ok(Self {
            models_dir,
            state: Arc::new(Mutex::new(InnerState {
                last_used: Instant::now(),
                model: None,
            })),
            allowed_languages,
        })
    }

    /// Load model via `from_pretrained` which handles tokenizer reconstruction
    /// and model download if needed.
    fn load_model(models_dir: &std::path::Path) -> Result<qwen3_asr::AsrInference, String> {
        let device = qwen3_asr::best_device();
        qwen3_asr::AsrInference::from_pretrained("Qwen/Qwen3-ASR-0.6B", models_dir, device)
            .map_err(|e| format!("{e}"))
    }
}

#[async_trait]
impl TranscriptionEngine for Qwen3AsrEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        let state = self.state.clone();
        let models_dir = self.models_dir.clone();
        let allowed_languages = self.allowed_languages.clone();

        tokio::spawn(async move {
            let mut all_samples: Vec<f32> = Vec::with_capacity(EXPECTED_SAMPLES);
            while let Some(chunk) = audio.recv().await {
                all_samples.extend_from_slice(&chunk.samples);
            }

            if all_samples.is_empty() {
                warn!("Empty audio stream, skipping transcription");
                return;
            }

            debug!(
                "Qwen3-ASR transcribing {} samples ({:.1}s)",
                all_samples.len(),
                all_samples.len() as f32 / QWEN3_ASR_SAMPLE_RATE as f32
            );

            // Run inference in a blocking thread (CPU/GPU-bound).
            // AsrInference holds Metal tensors which may not be Send,
            // so we load + infer within the same spawn_blocking call.
            let result =
                tokio::task::spawn_blocking(move || -> Result<(String, String), String> {
                    let mut guard = state.lock().unwrap();
                    if guard.model.is_none() {
                        info!("Lazy-loading Qwen3-ASR from {}...", models_dir.display());
                        let start = Instant::now();
                        let model = Qwen3AsrEngine::load_model(&models_dir)?;
                        info!("Qwen3-ASR loaded in {:.1}s", start.elapsed().as_secs_f32());
                        guard.model = Some(model);
                    }
                    guard.last_used = Instant::now();
                    let model = guard.model.as_ref().ok_or("Model not loaded")?;
                    let opts = qwen3_asr::TranscribeOptions::default();
                    let result = model
                        .transcribe_samples(&all_samples, opts)
                        .map_err(|e| format!("Inference failed: {e}"))?;
                    Ok((result.text, result.language))
                })
                .await;

            match result {
                Ok(Ok((text, lang))) => {
                    let text = text.trim().to_string();
                    let normalized = normalize_language(&lang);
                    let lang = if lang.is_empty()
                        || (!allowed_languages.is_empty()
                            && !allowed_languages.iter().any(|a| a == normalized))
                    {
                        "en".to_string()
                    } else {
                        normalized.to_string()
                    };
                    debug!(
                        "Qwen3-ASR result: '{}' (lang={})",
                        &text[..text.len().min(80)],
                        lang
                    );
                    let _ = tx
                        .send(PartialTranscript {
                            text,
                            segments: vec![],
                            language: Language::new(lang),
                            is_final: true,
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    warn!("Qwen3-ASR inference error: {e}");
                    let _ = tx
                        .send(PartialTranscript {
                            text: String::new(),
                            segments: vec![],
                            language: Language::default(),
                            is_final: true,
                        })
                        .await;
                }
                Err(e) => {
                    warn!("Qwen3-ASR inference task failed: {e}");
                }
            }
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        path: &std::path::Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        let path_str = path.to_string_lossy().to_string();
        let state = self.state.clone();
        let models_dir = self.models_dir.clone();
        let allowed_languages = self.allowed_languages.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<(String, String), String> {
            let mut guard = state.lock().unwrap();
            if guard.model.is_none() {
                let model = Qwen3AsrEngine::load_model(&models_dir)?;
                guard.model = Some(model);
            }
            guard.last_used = Instant::now();
            let model = guard.model.as_ref().ok_or("Model not loaded")?;
            let opts = qwen3_asr::TranscribeOptions::default();
            let r = model
                .transcribe(&path_str, opts)
                .map_err(|e| format!("{e}"))?;
            Ok((r.text, r.language))
        })
        .await
        .map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Qwen3-ASR file transcription failed: {e}"
            )))
        })?;

        match result {
            Ok((text, lang)) => {
                let normalized = normalize_language(&lang);
                let final_lang = if lang.is_empty()
                    || (!allowed_languages.is_empty()
                        && !allowed_languages.iter().any(|a| a == normalized))
                {
                    "en"
                } else {
                    normalized
                };
                Ok(Transcript {
                    text: text.trim().to_string(),
                    language: Language::new(final_lang),
                    segments: vec![],
                    overall_confidence: 0.9,
                })
            }
            Err(e) => {
                warn!("Qwen3-ASR file transcription error: {e}");
                Ok(Transcript {
                    text: String::new(),
                    language: Language::default(),
                    segments: vec![],
                    overall_confidence: 0.0,
                })
            }
        }
    }

    fn display_name(&self) -> &str {
        "Qwen3-ASR"
    }

    fn unload_if_idle(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.last_used.elapsed().as_secs() >= IDLE_UNLOAD_SECS && state.model.is_some() {
            state.model = None;
            info!("Qwen3-ASR model unloaded after idle");
            return true;
        }
        false
    }

    async fn preload(&self) -> common::Result<()> {
        let state = self.state.clone();
        let models_dir = self.models_dir.clone();
        tokio::task::spawn_blocking(move || {
            // Check if already loaded without holding the lock during load_model.
            {
                let guard = state.lock().unwrap();
                if guard.model.is_some() {
                    return Ok::<(), common::KlyntbotError>(());
                }
            }
            info!("Preloading Qwen3-ASR from {}...", models_dir.display());
            let start = Instant::now();
            let model = Qwen3AsrEngine::load_model(&models_dir).map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(e))
            })?;
            info!(
                "Qwen3-ASR preloaded in {:.1}s",
                start.elapsed().as_secs_f32()
            );
            let mut guard = state.lock().unwrap();
            guard.model = Some(model);
            Ok(())
        })
        .await
        .map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(e.to_string()))
        })??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_constant() {
        assert_eq!(QWEN3_ASR_SAMPLE_RATE, 16_000);
    }
}
