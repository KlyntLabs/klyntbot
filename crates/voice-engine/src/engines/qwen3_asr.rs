//! Qwen3-ASR speech recognition engine.
//!
//! Uses the `qwen3-asr` crate with Metal (Apple Silicon) backend for
//! local speech-to-text. Supports 52 languages with auto-detection.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript};

const IDLE_UNLOAD_SECS: u64 = 300;
pub(crate) const QWEN3_ASR_SAMPLE_RATE: u32 = 16_000;

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

        // Channel to feed audio from async world to blocking thread.
        // Uses std::sync::mpsc because the blocking thread needs a sync receiver.
        // `None` signals end-of-audio.
        let (audio_in_tx, audio_in_rx) = std::sync::mpsc::channel::<Option<Vec<f32>>>();

        // Blocking thread owns the model lock and StreamingState (which is !Send
        // due to Metal tensors in EncoderCache). The MutexGuard is held for the
        // entire streaming session, serializing transcription — acceptable since
        // only one voice capture runs at a time.
        let transcript_tx = tx;
        std::thread::spawn(move || {
            let mut guard = state.lock().unwrap();
            if guard.model.is_none() {
                info!("Lazy-loading Qwen3-ASR from {}...", models_dir.display());
                let start_time = Instant::now();
                match Qwen3AsrEngine::load_model(&models_dir) {
                    Ok(model) => {
                        info!(
                            "Qwen3-ASR loaded in {:.1}s",
                            start_time.elapsed().as_secs_f32()
                        );
                        guard.model = Some(model);
                    }
                    Err(e) => {
                        warn!("Qwen3-ASR model load failed: {e}");
                        return;
                    }
                }
            }
            guard.last_used = Instant::now();

            // Scope the immutable borrow of `guard.model` so we can update
            // `guard.last_used` again after streaming completes.
            {
                let model = guard.model.as_ref().unwrap();

                let opts = qwen3_asr::StreamingOptions::default().with_chunk_size_sec(2.0);
                let mut streaming = model.init_streaming(opts);

                // Process incoming audio chunks incrementally.
                while let Ok(Some(samples)) = audio_in_rx.recv() {
                    match model.feed_audio(&mut streaming, &samples) {
                        Ok(Some(result)) => {
                            let normalized = normalize_language(&result.language);
                            let _ = transcript_tx.blocking_send(PartialTranscript {
                                text: result.text.trim().to_string(),
                                segments: vec![],
                                language: Language::new(normalized),
                                is_final: false,
                            });
                        }
                        Ok(None) => {} // Not enough audio accumulated for a chunk yet
                        Err(e) => {
                            warn!("Qwen3-ASR streaming feed error: {e}");
                        }
                    }
                }

                // Finalize: flush remaining buffered audio and run final inference.
                match model.finish_streaming(&mut streaming) {
                    Ok(result) => {
                        let normalized = normalize_language(&result.language);
                        let lang = if result.language.is_empty()
                            || (!allowed_languages.is_empty()
                                && !allowed_languages.iter().any(|a| a == normalized))
                        {
                            "en".to_string()
                        } else {
                            normalized.to_string()
                        };
                        let _ = transcript_tx.blocking_send(PartialTranscript {
                            text: result.text.trim().to_string(),
                            segments: vec![],
                            language: Language::new(lang),
                            is_final: true,
                        });
                    }
                    Err(e) => {
                        warn!("Qwen3-ASR streaming finish error: {e}");
                    }
                }
            }
            // Model borrow released; update last_used timestamp.
            guard.last_used = Instant::now();
            // guard drops here, releasing the model lock
        });

        // Async relay: forward audio chunks from the async AudioStream to the
        // blocking thread via the std::sync::mpsc channel.
        tokio::spawn(async move {
            while let Some(chunk) = audio.recv().await {
                if audio_in_tx.send(Some(chunk.samples)).is_err() {
                    break; // Blocking thread exited
                }
            }
            // Signal end of audio
            let _ = audio_in_tx.send(None);
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
