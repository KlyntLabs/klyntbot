//! Local Whisper transcription engine using whisper-rs (whisper.cpp).
//!
//! Metal-accelerated on Apple Silicon. Supports both file transcription
//! and streaming via buffered chunking.
//!
//! The GGML model weights (~500MB for small, ~1.5GB for medium) are loaded
//! lazily on first transcription and automatically unloaded after an idle
//! period to reclaim memory when voice isn't in active use.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript, TranscriptSegment};

/// Idle timeout before the model is unloaded from memory.
const IDLE_UNLOAD_SECS: u64 = 300; // 5 minutes

/// Local Whisper transcription engine with lazy model loading.
///
/// The model is loaded on first use and unloaded after [`IDLE_UNLOAD_SECS`]
/// of inactivity to free ~500MB–1.5GB of RAM/VRAM.
pub struct WhisperLocalEngine {
    ctx: Arc<Mutex<Option<WhisperContext>>>,
    last_used: Arc<Mutex<Instant>>,
    model_path: PathBuf,
}

impl WhisperLocalEngine {
    /// Create a new engine pointing at a GGML model file.
    ///
    /// The model is NOT loaded into memory yet — it will be loaded lazily
    /// on the first transcription call. This keeps startup memory low.
    pub fn new(model_path: impl Into<PathBuf>) -> common::Result<Self> {
        let model_path = model_path.into();

        if !model_path.exists() {
            return Err(common::KlyntbotError::Config(
                common::ConfigError::NotFound(format!(
                    "Whisper model not found: {}",
                    model_path.display()
                )),
            ));
        }

        info!(
            "Whisper engine created (lazy) for model: {}",
            model_path.display()
        );

        Ok(Self {
            ctx: Arc::new(Mutex::new(None)),
            last_used: Arc::new(Mutex::new(Instant::now())),
            model_path,
        })
    }

    /// Ensure the model is loaded into memory (lazy loading from disk).
    fn ensure_model_loaded(&self) -> common::Result<()> {
        let mut ctx_guard = self.ctx.lock().unwrap();
        *self.last_used.lock().unwrap() = Instant::now();

        if ctx_guard.is_none() {
            info!("Loading Whisper model from: {}", self.model_path.display());
            let params = WhisperContextParameters::default();
            let ctx = WhisperContext::new_with_params(&self.model_path, params).map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Failed to load Whisper model: {}",
                    e
                )))
            })?;
            info!("Whisper model loaded successfully");
            *ctx_guard = Some(ctx);
        }

        Ok(())
    }

    /// Ensure the model is loaded and return a state handle for transcription.
    fn ensure_loaded(&self) -> common::Result<whisper_rs::WhisperState> {
        self.ensure_model_loaded()?;
        let ctx_guard = self.ctx.lock().unwrap();
        ctx_guard.as_ref().unwrap().create_state().map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Failed to create Whisper state: {}",
                e
            )))
        })
    }

    /// Unload the model from memory if it has been idle.
    ///
    /// Call this periodically (e.g., from a maintenance timer) to reclaim
    /// memory when voice is not in active use.
    pub fn unload_if_idle(&self) -> bool {
        let last = *self.last_used.lock().unwrap();
        if last.elapsed() >= Duration::from_secs(IDLE_UNLOAD_SECS) {
            let mut ctx_guard = self.ctx.lock().unwrap();
            if ctx_guard.is_some() {
                *ctx_guard = None;
                info!("Whisper model unloaded after idle timeout");
                return true;
            }
        }
        false
    }

    /// Build FullParams for transcription.
    fn build_params<'a>(lang_hint: Option<&'a Language>) -> FullParams<'a, 'static> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language if provided, otherwise auto-detect
        if let Some(lang) = lang_hint {
            params.set_language(Some(lang.as_str()));
        } else {
            params.set_language(None);
        }

        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_token_timestamps(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        params
    }

    /// Extract transcript from whisper state after transcription.
    fn extract_transcript(
        state: &whisper_rs::WhisperState,
        detected_language: Option<&str>,
    ) -> common::Result<Transcript> {
        let num_segments = state.full_n_segments();

        let mut full_text = String::new();
        let mut segments = Vec::new();

        debug!("Extracting transcript from {} segments", num_segments);

        for i in 0..num_segments {
            let Some(segment) = state.get_segment(i) else {
                debug!("Segment {i}: no segment returned");
                continue;
            };

            let segment_text = segment.to_str_lossy().map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Failed to get segment text: {}",
                    e
                )))
            })?;

            debug!(
                "Segment {i}: text='{}', tokens={}, t0={}, t1={}",
                segment_text,
                segment.n_tokens(),
                segment.start_timestamp(),
                segment.end_timestamp()
            );

            let t0 = segment.start_timestamp();
            let t1 = segment.end_timestamp();
            let num_tokens = segment.n_tokens();

            // Get token-level details for word-level confidence
            for j in 0..num_tokens {
                let Some(token) = segment.get_token(j) else {
                    continue;
                };

                let token_text = match token.to_str_lossy() {
                    Ok(t) => t.into_owned(),
                    Err(_) => continue,
                };
                let token_prob = token.token_probability();

                let trimmed = token_text.trim();
                if trimmed.is_empty() || trimmed.starts_with('[') {
                    continue; // Skip special tokens like [_BEG_], etc.
                }

                // Token timestamps: use segment start + proportional offset
                let token_start_cs = t0 + (j as i64 * (t1 - t0)) / num_tokens.max(1) as i64;
                let token_end_cs = t0 + ((j as i64 + 1) * (t1 - t0)) / num_tokens.max(1) as i64;

                segments.push(TranscriptSegment {
                    text: trimmed.to_string(),
                    start: Duration::from_millis(token_start_cs as u64 * 10),
                    end: Duration::from_millis(token_end_cs as u64 * 10),
                    confidence: token_prob,
                });
            }

            if !full_text.is_empty() && !segment_text.starts_with(' ') {
                full_text.push(' ');
            }
            full_text.push_str(segment_text.trim());
        }

        let language = detected_language.map(Language::new).unwrap_or_default();

        let mut transcript = Transcript {
            text: full_text,
            language,
            segments,
            overall_confidence: 0.0,
        };
        transcript.recompute_confidence();

        Ok(transcript)
    }

    /// Read audio from a WAV file and convert to f32 samples at 16kHz.
    fn read_audio_file(path: &Path) -> common::Result<Vec<f32>> {
        let reader = hound::WavReader::open(path).map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Failed to open audio file: {}",
                e
            )))
        })?;

        let spec = reader.spec();
        debug!(
            "Audio file: {}Hz, {} ch, {:?}",
            spec.sample_rate, spec.channels, spec.sample_format
        );

        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect(),
            hound::SampleFormat::Int => {
                let bit_depth = spec.bits_per_sample;
                let max_val = (1 << (bit_depth - 1)) as f32;
                reader
                    .into_samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / max_val)
                    .collect()
            }
        };

        // If stereo, convert to mono by averaging channels
        let samples = if spec.channels == 2 {
            samples
                .chunks(2)
                .map(|pair| {
                    if pair.len() == 2 {
                        (pair[0] + pair[1]) / 2.0
                    } else {
                        pair[0]
                    }
                })
                .collect()
        } else {
            samples
        };

        // Note: Resampling to 16kHz is not implemented here.
        // For v1, we require 16kHz input. cpal captures at 16kHz by default.
        if spec.sample_rate != 16000 {
            warn!(
                "Audio file is {}Hz, expected 16000Hz. Transcription quality may be affected.",
                spec.sample_rate
            );
        }

        Ok(samples)
    }
}

#[async_trait]
impl TranscriptionEngine for WhisperLocalEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        let ctx = Arc::clone(&self.ctx);
        let last_used = Arc::clone(&self.last_used);

        self.ensure_model_loaded()?;

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();

            // Collect audio chunks, emitting intermediate partials every ~3s
            let chunk_threshold = 16000 * 3; // 3 seconds of 16kHz audio
            let mut all_samples: Vec<f32> = Vec::new();
            let mut since_last_partial = 0usize;

            while let Some(chunk) = rt.block_on(audio.recv()) {
                all_samples.extend_from_slice(&chunk.samples);
                since_last_partial += chunk.samples.len();

                // Emit an intermediate partial every ~3 seconds
                if since_last_partial >= chunk_threshold && all_samples.len() > 16000 {
                    since_last_partial = 0;
                    let ctx_guard = ctx.lock().unwrap();
                    if let Some(ref whisper_ctx) = *ctx_guard {
                        if let Ok(mut state) = whisper_ctx.create_state() {
                            let params = WhisperLocalEngine::build_params(None);
                            if state.full(params, &all_samples).is_ok() {
                                if let Ok(transcript) =
                                    WhisperLocalEngine::extract_transcript(&state, None)
                                {
                                    if !transcript.text.trim().is_empty() {
                                        let _ = rt.block_on(tx.send(PartialTranscript {
                                            text: transcript.text,
                                            segments: transcript.segments,
                                            language: transcript.language,
                                            is_final: false,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if all_samples.is_empty() {
                warn!("Empty audio stream, skipping transcription");
                return;
            }

            debug!(
                "Transcribing {} samples ({:.1}s of audio)",
                all_samples.len(),
                all_samples.len() as f32 / 16000.0
            );

            // Final transcription with all audio
            *last_used.lock().unwrap() = std::time::Instant::now();
            let ctx_guard = ctx.lock().unwrap();
            let Some(ref whisper_ctx) = *ctx_guard else {
                warn!("Whisper model unloaded during transcription");
                return;
            };

            let mut state = match whisper_ctx.create_state() {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to create Whisper state: {e}");
                    return;
                }
            };

            let full_params = WhisperLocalEngine::build_params(None);
            if let Err(e) = state.full(full_params, &all_samples) {
                warn!("Whisper transcription failed: {e}");
                return;
            }

            match WhisperLocalEngine::extract_transcript(&state, None) {
                Ok(transcript) => {
                    let _ = rt.block_on(tx.send(PartialTranscript {
                        text: transcript.text,
                        segments: transcript.segments,
                        language: transcript.language,
                        is_final: true,
                    }));
                }
                Err(e) => {
                    warn!("Failed to extract transcript: {e}");
                }
            }
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        path: &Path,
        lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        if !path.exists() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!(
                    "Audio file not found: {}",
                    path.display()
                )),
            ));
        }

        let audio = Self::read_audio_file(path)?;

        debug!(
            "Transcribing file: {} ({:.1}s of audio)",
            path.display(),
            audio.len() as f32 / 16000.0
        );

        let mut state = self.ensure_loaded()?;

        let params = Self::build_params(lang_hint);

        state.full(params, &audio).map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Transcription failed: {}",
                e
            )))
        })?;

        Self::extract_transcript(&state, lang_hint.map(|l| l.as_str()))
    }

    fn display_name(&self) -> &str {
        "Local (Whisper)"
    }

    fn unload_if_idle(&self) -> bool {
        Self::unload_if_idle(self)
    }
}
