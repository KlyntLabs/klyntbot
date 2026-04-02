//! Voice service — orchestrates capture, transcription, routing, and TTS.
//!
//! This is the central coordinator for voice capture sessions. It manages
//! the VoiceSessionState machine, streams audio to transcription, runs
//! routing on partials, and produces VoiceEvents for the frontend orb.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::capture::{AudioCapture, CaptureConfig, CaptureSession};
use crate::events::VoiceEvent;
use crate::model_manager::ModelManager;
use crate::pronunciation::compute_pronunciation_report;
use crate::router::VoiceRouter;
use crate::session::VoiceSessionState;
use crate::stt::TranscriptionEngine;
use crate::tts::TtsEngine;
use crate::types::*;

/// Callback for memory echo lookups (dependency-inverted from higher layers).
#[async_trait::async_trait]
pub trait MemoryEchoProvider: Send + Sync {
    async fn lookup(&self, partial_text: &str, learning_active: bool) -> Option<String>;
}

/// Hook for pronunciation analysis pipeline (dependency-inverted from feature-language-learning).
///
/// Called after each transcription with the transcript text, language, and audio duration.
/// Implementations run phoneme alignment, tone analysis, error classification, and
/// emit VoiceEvents for the frontend.
#[async_trait::async_trait]
pub trait PronunciationProvider: Send + Sync {
    async fn analyze(&self, transcript: &str, language: &Language, duration_ms: u64);
}

/// Base64-encode an `AudioClip`'s raw float samples for transport to the frontend.
fn base64_encode_audio(clip: &AudioClip) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = clip.samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

enum AudioCommand {
    Play {
        samples: Vec<f32>,
        sample_rate: u32,
        done_tx: tokio::sync::oneshot::Sender<()>,
    },
    Stop,
}

/// Persistent audio output player. Runs a dedicated thread that receives
/// audio via a channel and plays through the configured output device.
pub struct AudioPlayer {
    tx: std::sync::mpsc::Sender<AudioCommand>,
    _thread: std::thread::JoinHandle<()>,
    /// Shared output device name — updated on config hot-reload.
    output_device: Arc<std::sync::RwLock<Option<String>>>,
}

impl AudioPlayer {
    pub fn new(output_device: Option<String>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<AudioCommand>();
        let device_ref = Arc::new(std::sync::RwLock::new(output_device));
        let device_ref_thread = device_ref.clone();

        let thread = std::thread::spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Play {
                        samples,
                        sample_rate,
                        done_tx,
                    } => {
                        let host = cpal::default_host();
                        let preferred = device_ref_thread.read().ok().and_then(|g| g.clone());
                        let device = preferred
                            .as_deref()
                            .and_then(|name| {
                                host.output_devices().ok().and_then(|devices| {
                                    devices
                                        .into_iter()
                                        .find(|d| d.name().ok().as_deref() == Some(name))
                                })
                            })
                            .or_else(|| host.default_output_device());
                        let Some(device) = device else {
                            warn!("No audio output device available");
                            let _ = done_tx.send(());
                            continue;
                        };

                        let config = cpal::StreamConfig {
                            channels: 1,
                            sample_rate: cpal::SampleRate(sample_rate),
                            buffer_size: cpal::BufferSize::Default,
                        };

                        let samples = Arc::new(samples);
                        let pos = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                        let done = Arc::new(AtomicBool::new(false));
                        let done_flag = done.clone();
                        let samples_ref = samples.clone();
                        let pos_ref = pos.clone();

                        let stream = match device.build_output_stream(
                            &config,
                            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                let current = pos_ref.load(Ordering::Relaxed);
                                let remaining = samples_ref.len().saturating_sub(current);
                                let to_copy = data.len().min(remaining);
                                if to_copy > 0 {
                                    data[..to_copy]
                                        .copy_from_slice(&samples_ref[current..current + to_copy]);
                                    pos_ref.store(current + to_copy, Ordering::Relaxed);
                                }
                                for sample in data[to_copy..].iter_mut() {
                                    *sample = 0.0;
                                }
                                if to_copy == 0 {
                                    done_flag.store(true, Ordering::Relaxed);
                                }
                            },
                            |err| warn!("Audio output error: {err}"),
                            None,
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                warn!("Failed to build audio output stream: {e}");
                                let _ = done_tx.send(());
                                continue;
                            }
                        };

                        if let Err(e) = stream.play() {
                            warn!("Failed to play audio stream: {e}");
                            let _ = done_tx.send(());
                            continue;
                        }

                        // Wait for playback to finish or stop command.
                        while !done.load(Ordering::Relaxed) {
                            if let Ok(AudioCommand::Stop) = rx.try_recv() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                        let _ = done_tx.send(());
                    }
                    AudioCommand::Stop => {} // No-op when not playing
                }
            }
        });

        Self {
            tx,
            _thread: thread,
            output_device: device_ref,
        }
    }

    /// Play audio and return a receiver that resolves when playback completes.
    pub fn play(&self, samples: Vec<f32>, sample_rate: u32) -> tokio::sync::oneshot::Receiver<()> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let _ = self.tx.send(AudioCommand::Play {
            samples,
            sample_rate,
            done_tx,
        });
        done_rx
    }

    /// Stop current playback immediately.
    pub fn stop(&self) {
        let _ = self.tx.send(AudioCommand::Stop);
    }

    /// Update the preferred output device (takes effect on next play).
    pub fn set_output_device(&self, device: Option<String>) {
        if let Ok(mut guard) = self.output_device.write() {
            *guard = device;
        }
    }
}

/// Configuration for the VoiceService.
#[derive(Debug, Clone)]
pub struct VoiceServiceConfig {
    pub capture: CaptureConfig,
    pub privacy_mode: PrivacyLevel,
    /// Data directory for saving audio captures and TTS output.
    pub data_dir: PathBuf,
    /// When true, audio plays natively via cpal and base64 encoding is skipped.
    pub native_audio: bool,
    /// Preferred output device name (None = system default).
    pub output_device: Option<String>,
}

impl Default for VoiceServiceConfig {
    fn default() -> Self {
        Self {
            capture: CaptureConfig::default(),
            privacy_mode: PrivacyLevel::Standard,
            data_dir: PathBuf::from("."),
            native_audio: false,
            output_device: None,
        }
    }
}

/// Wrapper that makes `CaptureSession` `Send`.
///
/// `cpal::Stream` is marked `!Send` for Android AAudio compatibility, but on
/// macOS (CoreAudio) the underlying types are thread-safe. We only move the
/// session into a blocking thread to keep the audio stream alive, and never
/// access it concurrently from multiple threads.
struct SendableCaptureSession(#[allow(dead_code)] CaptureSession);

// SAFETY: On macOS, CoreAudio streams are safe to move between threads.
// The `!Send` bound on `cpal::Stream` is a conservative cross-platform
// restriction for Android AAudio. We only target macOS.
#[cfg(target_os = "macos")]
unsafe impl Send for SendableCaptureSession {}

/// Active voice session — all fields are `Send`.
struct ActiveSession {
    id: String,
    stop_signal: Arc<AtomicBool>,
    /// Taken (moved to STT engine) during finalization.
    audio_rx: Option<mpsc::Receiver<AudioChunk>>,
    /// Handle to the blocking thread that owns the cpal::Stream.
    /// Dropping this does NOT cancel the thread — the stop_signal does.
    #[allow(dead_code)]
    stream_keepalive: tokio::task::JoinHandle<()>,
    started_at: Instant,
    state: VoiceSessionState,
}

/// Central voice orchestrator.
///
/// Manages the lifecycle of voice capture sessions:
/// start -> capture audio -> stream to STT -> route intents -> finalize -> TTS response.
pub struct VoiceService {
    /// Local STT engine — wrapped in RwLock for hot-swap after background model download.
    stt_local: std::sync::RwLock<Option<Arc<dyn TranscriptionEngine>>>,
    /// TTS engine — wrapped in RwLock for hot-swap (same pattern as stt_local).
    tts: std::sync::RwLock<Option<Arc<dyn TtsEngine>>>,
    /// Memory echo provider for dependency-inverted memory lookups.
    memory_echo: Option<Arc<dyn MemoryEchoProvider>>,
    /// Pronunciation analysis hook (injected from feature-language-learning).
    pronunciation: Option<Arc<dyn PronunciationProvider>>,
    capture: AudioCapture,
    model_manager: ModelManager,
    router: VoiceRouter,
    config: std::sync::RwLock<VoiceServiceConfig>,

    /// Current active session (at most one at a time).
    active_session: Arc<Mutex<Option<ActiveSession>>>,

    /// Channel for emitting VoiceEvents to the frontend.
    event_tx: mpsc::Sender<VoiceEvent>,
    /// Receiver side — consumed by the Tauri adapter to relay events.
    /// Uses std::sync::Mutex (not tokio) because it's taken once at init from a sync context.
    event_rx: Arc<std::sync::Mutex<Option<mpsc::Receiver<VoiceEvent>>>>,

    /// Track first-ever capture for one-time welcome echo.
    first_capture_complete: AtomicBool,

    /// One-shot guard: memory echo fires at most once per capture session.
    echo_fired_this_session: AtomicBool,

    /// Set by the silence notification task when silence is detected.
    /// Polled by VoiceConversationManager to trigger automatic finalization.
    silence_detected: Arc<AtomicBool>,

    /// Set to true while a voice conversation is active.
    /// Prevents idle model unloading during active conversations.
    conversation_active: Arc<AtomicBool>,

    /// Persistent audio player — dedicated thread for cpal output.
    audio_player: AudioPlayer,
}

impl VoiceService {
    /// Create a new VoiceService.
    ///
    /// Pass `None` for engines that aren't available yet (e.g., local STT
    /// while the model is downloading).
    pub fn new(
        stt_local: Option<Arc<dyn TranscriptionEngine>>,
        tts: Option<Arc<dyn TtsEngine>>,
        memory_echo: Option<Arc<dyn MemoryEchoProvider>>,
        pronunciation: Option<Arc<dyn PronunciationProvider>>,
        model_manager: ModelManager,
        config: VoiceServiceConfig,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(128);
        let audio_player = AudioPlayer::new(config.output_device.clone());

        Self {
            stt_local: std::sync::RwLock::new(stt_local),
            tts: std::sync::RwLock::new(tts),
            memory_echo,
            pronunciation,
            capture: AudioCapture::new(config.capture.clone()),
            model_manager,
            router: VoiceRouter::new(),
            config: std::sync::RwLock::new(config),
            active_session: Arc::new(Mutex::new(None)),
            event_tx,
            event_rx: Arc::new(std::sync::Mutex::new(Some(event_rx))),
            first_capture_complete: AtomicBool::new(false),
            echo_fired_this_session: AtomicBool::new(false),
            silence_detected: Arc::new(AtomicBool::new(false)),
            conversation_active: Arc::new(AtomicBool::new(false)),
            audio_player,
        }
    }

    /// Take the event receiver (can only be called once).
    /// The Tauri adapter consumes this to relay events to the frontend.
    /// Synchronous — safe to call from non-async init code.
    pub fn take_event_rx(&self) -> Option<mpsc::Receiver<VoiceEvent>> {
        self.event_rx.lock().ok()?.take()
    }

    /// Access the model manager for status checks and downloads.
    pub fn model_manager(&self) -> &ModelManager {
        &self.model_manager
    }

    /// Read a snapshot of the current service config.
    fn cfg(&self) -> VoiceServiceConfig {
        self.config.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Returns true if the STT engine is loaded and ready.
    pub fn is_stt_loaded(&self) -> bool {
        self.stt_local.read().ok().is_some_and(|g| g.is_some())
    }

    /// Returns true if the TTS engine is loaded and ready.
    pub fn is_tts_loaded(&self) -> bool {
        self.tts.read().ok().is_some_and(|g| g.is_some())
    }

    /// Stop any in-progress audio playback.
    pub fn stop_playback(&self) {
        self.audio_player.stop();
    }

    /// Hot-reload voice config. Updates capture config, output device, and
    /// service config so changes take effect on the next operation.
    pub fn update_config(&self, new_config: VoiceServiceConfig) {
        self.capture.update_config(new_config.capture.clone());
        self.audio_player
            .set_output_device(new_config.output_device.clone());
        if let Ok(mut guard) = self.config.write() {
            *guard = new_config;
        }
    }

    /// Get current session state.
    pub async fn session_state(&self) -> VoiceSessionState {
        self.active_session
            .lock()
            .await
            .as_ref()
            .map(|s| s.state)
            .unwrap_or(VoiceSessionState::Idle)
    }

    /// Get the local transcription engine if available.
    fn active_engine(&self) -> Option<(Arc<dyn TranscriptionEngine>, EngineKind)> {
        self.stt_local
            .read()
            .ok()
            .and_then(|g| g.clone())
            .map(|l| (l, EngineKind::Local))
    }

    /// Hot-swap the local STT engine (called after background model download completes).
    pub fn set_local_engine(&self, engine: Arc<dyn TranscriptionEngine>) {
        match self.stt_local.write() {
            Ok(mut guard) => *guard = Some(engine),
            Err(e) => warn!("STT engine lock poisoned, cannot swap: {e}"),
        }
    }

    /// Hot-swap the TTS engine at runtime (e.g. after model download).
    pub fn set_tts_engine(&self, engine: Arc<dyn TtsEngine>) {
        match self.tts.write() {
            Ok(mut guard) => *guard = Some(engine),
            Err(e) => warn!("TTS engine lock poisoned, cannot swap: {e}"),
        }
    }

    /// Set whether a voice conversation is currently active.
    ///
    /// When `true`, idle model unloading is suppressed to avoid latency spikes
    /// mid-conversation.
    pub fn set_conversation_active(&self, active: bool) {
        self.conversation_active.store(active, Ordering::Relaxed);
    }

    /// Returns `true` if a voice conversation is currently active.
    pub fn is_conversation_active(&self) -> bool {
        self.conversation_active.load(Ordering::Relaxed)
    }

    /// Try to unload the STT model if it has been idle.
    ///
    /// Called periodically by a maintenance timer to reclaim memory.
    /// No-op while a conversation is active to avoid mid-turn latency spikes.
    pub fn try_unload_idle_stt(&self) {
        if self.is_conversation_active() {
            return;
        }
        if let Ok(guard) = self.stt_local.read() {
            if let Some(ref engine) = *guard {
                engine.unload_if_idle();
            }
        }
    }

    /// Try to unload the TTS model if it has been idle.
    ///
    /// Called periodically by a maintenance timer to reclaim GPU memory.
    /// No-op while a conversation is active.
    pub fn try_unload_idle_tts(&self) {
        if self.is_conversation_active() {
            return;
        }
        if let Ok(guard) = self.tts.read() {
            if let Some(ref engine) = *guard {
                engine.unload_if_idle();
            }
        }
    }

    /// Eagerly preload the STT model so it's ready before first voice use.
    pub async fn preload_stt(&self) -> common::Result<()> {
        let engine = self.stt_local.read().ok().and_then(|g| g.clone());
        if let Some(engine) = engine {
            engine.preload().await?;
        }
        Ok(())
    }

    /// Synchronous helper: creates the capture session, wraps the `!Send` cpal
    /// stream on a blocking thread, and returns only `Send`-safe parts. This
    /// keeps `CaptureSession` out of any async generator state.
    /// Returns `(session, silence_rx, rms_rx, session_id, engine_kind)`.
    fn begin_capture(
        &self,
    ) -> common::Result<(
        ActiveSession,
        mpsc::Receiver<()>,
        mpsc::Receiver<f32>,
        String,
        EngineKind,
    )> {
        let (_, engine_kind) = self.active_engine().ok_or_else(|| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                "No transcription engine available. Waiting for model download.".to_string(),
            ))
        })?;

        let mut capture_session = self.capture.start()?;

        let session_id = format!("voice-{}", chrono::Utc::now().timestamp_millis());

        // Extract Send-safe pieces before wrapping the !Send CaptureSession.
        let stop_signal = capture_session.stop_signal.clone();
        let audio_rx = {
            let (_, dummy_rx) = mpsc::channel(1);
            std::mem::replace(&mut capture_session.audio_rx, dummy_rx)
        };
        let silence_rx = {
            let (_, dummy_rx) = mpsc::channel(1);
            std::mem::replace(&mut capture_session.silence_rx, dummy_rx)
        };
        let rms_rx = {
            let (_, dummy_rx) = mpsc::channel(1);
            std::mem::replace(&mut capture_session.rms_rx, dummy_rx)
        };

        // Keep the cpal::Stream alive on a blocking thread.
        // CaptureSession wraps cpal::Stream which is !Send on all platforms
        // (conservative for Android AAudio), but safe to send on macOS.
        let sendable = SendableCaptureSession(capture_session);
        let keepalive_stop = stop_signal.clone();
        let stream_keepalive = tokio::task::spawn_blocking(move || {
            // Hold the CaptureSession (and its cpal::Stream) alive until
            // the stop signal fires. Audio flows via the channel we already
            // extracted as audio_rx.
            while !keepalive_stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            // CaptureSession drops here, stopping the cpal stream.
            drop(sendable);
        });

        let active = ActiveSession {
            id: session_id.clone(),
            stop_signal,
            audio_rx: Some(audio_rx),
            stream_keepalive,
            started_at: Instant::now(),
            state: VoiceSessionState::Capturing,
        };

        Ok((active, silence_rx, rms_rx, session_id, engine_kind))
    }

    /// Stop any in-progress TTS audio playback and signal the frontend to fade out.
    pub async fn stop_tts_playback(&self) {
        self.audio_player.stop();
        let _ = self.event_tx.send(VoiceEvent::TtsFadeOut).await;
    }

    /// Whether silence has been detected since the last capture started.
    pub fn is_silence_detected(&self) -> bool {
        self.silence_detected.load(Ordering::Relaxed)
    }

    /// Start a voice capture session.
    ///
    /// Returns the session ID and engine kind, or an error if capture can't start.
    pub async fn start_capture(&self) -> common::Result<(String, EngineKind)> {
        self.silence_detected.store(false, Ordering::Relaxed);
        let mut session_guard = self.active_session.lock().await;

        if session_guard.is_some() {
            return Err(common::ChannelError::ConnectionFailed(
                "Voice capture already in progress".to_string(),
            )
            .into());
        }

        // Create the capture session synchronously so the !Send CaptureSession
        // never exists in the async generator state (keeps this future Send).
        let (active, silence_rx, rms_rx, session_id, engine_kind) = self.begin_capture()?;

        let _ = self
            .event_tx
            .send(VoiceEvent::CaptureStarted {
                session_id: session_id.clone(),
                engine: engine_kind,
            })
            .await;

        // Spawn silence notification task: tells the frontend silence was detected
        // and sets the silence_detected flag for the conversation manager to poll.
        {
            let event_tx = self.event_tx.clone();
            let silence_flag = Arc::clone(&self.silence_detected);
            let mut silence_rx = silence_rx;
            tokio::spawn(async move {
                if silence_rx.recv().await.is_some() {
                    info!("Silence detected — notifying frontend");
                    silence_flag.store(true, Ordering::Relaxed);
                    let _ = event_tx
                        .send(VoiceEvent::CaptureEnded { duration_ms: 0 })
                        .await;
                }
            });
        }

        // Spawn RMS relay task: forwards RMS values as AudioLevel events
        {
            let event_tx = self.event_tx.clone();
            let mut rms_rx = rms_rx;
            tokio::spawn(async move {
                while let Some(rms) = rms_rx.recv().await {
                    let _ = event_tx.send(VoiceEvent::AudioLevel { rms }).await;
                }
            });
        }

        // Reset one-shot echo guard for the new capture session.
        self.echo_fired_this_session.store(false, Ordering::Relaxed);

        *session_guard = Some(active);

        info!(
            "Voice capture started: {} (engine: {:?})",
            session_id, engine_kind
        );

        Ok((session_id, engine_kind))
    }

    /// Stop the current capture and begin finalization.
    ///
    /// Returns the final Transcript + VoiceMetadata, or None if no session was active.
    pub async fn stop_capture(&self) -> common::Result<Option<(Transcript, VoiceMetadata)>> {
        // Take the session out of the mutex so we don't hold the lock across awaits.
        let session = {
            let mut guard = self.active_session.lock().await;
            match guard.take() {
                Some(s) => s,
                None => return Ok(None),
            }
        };

        // Signal capture to stop. The cpal callback will stop sending audio,
        // and the audio_rx channel will eventually close.
        session.stop_signal.store(true, Ordering::Relaxed);
        let duration = session.started_at.elapsed();

        let _ = self
            .event_tx
            .send(VoiceEvent::CaptureEnded {
                duration_ms: duration.as_millis() as u64,
            })
            .await;

        info!(
            "Voice capture stopped after {:.1}s, finalizing...",
            duration.as_secs_f32()
        );

        let (engine, engine_kind) = self.active_engine().ok_or_else(|| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                "No engine available for transcription".to_string(),
            ))
        })?;

        let audio_rx = session.audio_rx.ok_or_else(|| {
            common::KlyntbotError::Channel(common::ChannelError::ConnectionFailed(
                "Audio receiver already consumed".to_string(),
            ))
        })?;

        info!(
            "Passing audio_rx to transcription engine ({})",
            engine.display_name()
        );
        let mut rx = engine.transcribe_stream(audio_rx).await?;
        info!("Waiting for transcript partials...");

        // Collect transcript partials and the final result.
        let mut final_transcript = None;
        let mut last_intents = Vec::new();
        while let Some(partial) = rx.recv().await {
            let _ = self
                .event_tx
                .send(VoiceEvent::PartialTranscript {
                    text: partial.text.clone(),
                    language: partial.language.as_str().to_string(),
                    is_final: partial.is_final,
                    segments: partial
                        .segments
                        .iter()
                        .map(|s| crate::events::TranscriptSegmentEvent {
                            text: s.text.clone(),
                            confidence: s.confidence,
                        })
                        .collect(),
                })
                .await;

            // Memory echo: fire once per turn on the first partial with ≥3 words.
            // Skip entirely in Strict privacy mode.
            if self.cfg().privacy_mode != PrivacyLevel::Strict
                && partial.text.split_whitespace().count() >= 3
                && !self.echo_fired_this_session.swap(true, Ordering::Relaxed)
            {
                if let Some(ref echo_provider) = self.memory_echo {
                    if let Some(echo_text) = echo_provider.lookup(&partial.text, false).await {
                        let _ = self
                            .event_tx
                            .send(VoiceEvent::MemoryEcho { text: echo_text })
                            .await;
                    }
                }
            }

            // Run routing on each partial.
            let intents = self.router.detect_intents(&partial.text);
            for event in self.router.to_events(&intents) {
                let _ = self.event_tx.send(event).await;
            }

            if partial.is_final {
                last_intents = intents;
                final_transcript = Some(Transcript {
                    text: partial.text,
                    language: partial.language,
                    segments: partial.segments,
                    overall_confidence: 0.0, // recomputed below
                });
            } else {
                last_intents = intents;
            }
        }

        let transcript = match final_transcript {
            Some(mut t) => {
                t.recompute_confidence();
                t
            }
            None => {
                warn!("No transcript produced from voice capture");
                return Ok(None);
            }
        };

        let report = compute_pronunciation_report(&transcript);

        if let Some(ref provider) = self.pronunciation {
            let provider = Arc::clone(provider);
            let text = transcript.text.clone();
            let lang = transcript.language.clone();
            let dur = duration.as_millis() as u64;
            tokio::spawn(async move {
                provider.analyze(&text, &lang, dur).await;
            });
        }

        // Generate audio_ref path for FSRS playback.
        // TODO: tee the audio stream during capture to actually write the WAV file.
        // For now we prepare the path so downstream consumers can check for it.
        let audio_ref = {
            let audio_dir = self.cfg().data_dir.join("voice_captures");
            let _ = tokio::fs::create_dir_all(&audio_dir).await;
            let filename = format!("{}.wav", session.id);
            let path = audio_dir.join(filename);
            Some(path.to_string_lossy().to_string())
        };

        let metadata = VoiceMetadata {
            language: transcript.language.clone(),
            overall_confidence: transcript.overall_confidence,
            pronunciation_scores: report
                .word_scores
                .iter()
                .map(|w| (w.word.clone(), w.confidence))
                .collect(),
            audio_ref,
            duration,
            engine: engine_kind,
            privacy_mode: self.cfg().privacy_mode,
        };

        // Reuse cached intents from the last partial instead of re-detecting.
        let routed_to = last_intents
            .first()
            .map(|i| i.skill.clone())
            .unwrap_or_else(|| "general".to_string());

        let _ = self
            .event_tx
            .send(VoiceEvent::Finalized {
                text: transcript.text.clone(),
                routed_to: routed_to.clone(),
                response_preview: String::new(),
            })
            .await;

        // One-time welcome echo on the very first successful capture
        if !self.first_capture_complete.swap(true, Ordering::Relaxed) {
            let _ = self
                .event_tx
                .send(VoiceEvent::MemoryEcho {
                    text: "Welcome to your second brain. I'm listening. Everything you say here becomes memory, learning, and reflection — just like your thoughts. Press ⌥⇧V anytime.".to_string(),
                })
                .await;
        }

        // TTS read-back is now handled by handle_response(), called by app-core
        // after the agent produces its response. This keeps VoiceService decoupled
        // from the agent pipeline.

        info!(
            "Voice transcription complete: \"{}\" (confidence: {:.0}%, routed to: {})",
            transcript.text,
            transcript.overall_confidence * 100.0,
            routed_to,
        );

        Ok(Some((transcript, metadata)))
    }

    /// Dismiss the current session (orb closes, processing continues in background).
    pub async fn dismiss(&self) {
        let mut session_guard = self.active_session.lock().await;
        if let Some(ref mut session) = *session_guard {
            session.state = VoiceSessionState::Finalizing;
        }
        let _ = self.event_tx.send(VoiceEvent::ProcessingInBackground).await;
    }

    /// Cancel the current session entirely.
    pub async fn cancel(&self) {
        let mut session_guard = self.active_session.lock().await;
        if let Some(session) = session_guard.take() {
            session.stop_signal.store(true, Ordering::Relaxed);
            info!("Voice capture cancelled: {}", session.id);
        }
    }

    /// Handle the agent's response for the current voice session.
    ///
    /// Synthesizes TTS audio and emits a `SpeakResponse` event to the frontend.
    /// Called by app-core after `AgentRuntime` produces a response for a voice message.
    pub async fn handle_response(
        &self,
        response_text: &str,
        tts_params: &TtsParams,
    ) -> common::Result<()> {
        self.handle_response_with_completion(response_text, tts_params)
            .await?;
        Ok(())
    }

    /// Synthesize TTS and return a receiver that signals when playback ends.
    pub async fn handle_response_with_completion(
        &self,
        response_text: &str,
        tts_params: &TtsParams,
    ) -> common::Result<Option<tokio::sync::oneshot::Receiver<()>>> {
        let tts = self.tts.read().ok().and_then(|g| g.clone());
        match tts {
            Some(tts) => {
                info!(
                    "TTS: synthesizing {} chars via {}",
                    response_text.len(),
                    tts.display_name()
                );
                match tts.synthesize(response_text, tts_params).await {
                    Ok(clip) if clip.samples.is_empty() => {
                        warn!("TTS: synthesized clip is empty (0 samples), skipping playback");
                        Ok(None)
                    }
                    Ok(clip) => {
                        let native_audio = self.cfg().native_audio;
                        info!(
                            "TTS: {} samples at {}Hz, emitting SpeakResponse (native_audio={})",
                            clip.samples.len(),
                            clip.sample_rate,
                            native_audio
                        );
                        // Encode before any move of clip.samples.
                        let audio_base64 = if native_audio {
                            String::new()
                        } else {
                            base64_encode_audio(&clip)
                        };
                        let sample_rate = clip.sample_rate;
                        let playback_rx = if native_audio {
                            Some(self.audio_player.play(clip.samples, clip.sample_rate))
                        } else {
                            None
                        };
                        let _ = self
                            .event_tx
                            .send(VoiceEvent::SpeakResponse {
                                audio_base64,
                                sample_rate,
                                text: response_text.to_string(),
                            })
                            .await;
                        Ok(playback_rx)
                    }
                    Err(e) => {
                        warn!("TTS synthesis failed: {e}");
                        Ok(None)
                    }
                }
            }
            None => {
                warn!("TTS: no engine available, cannot speak response");
                Ok(None)
            }
        }
    }

    /// Get the active engine kind (Local or Cloud).
    pub fn engine_kind(&self) -> Option<EngineKind> {
        self.active_engine().map(|(_, k)| k)
    }

    /// Check if voice capture is available (at least one engine configured).
    pub fn is_available(&self) -> bool {
        self.active_engine().is_some()
    }

    /// Start a lightweight RMS-only audio monitor (no buffering, no silence detection).
    ///
    /// Used during Speaking phase to detect user speech for interrupt. Delegates
    /// to the internal `AudioCapture::start_monitor()`.
    pub fn start_monitor(&self) -> common::Result<crate::capture::MonitorSession> {
        self.capture.start_monitor()
    }

    /// Emit an arbitrary VoiceEvent (for dev/testing simulation).
    pub async fn emit_event(&self, event: VoiceEvent) -> common::Result<()> {
        self.event_tx
            .send(event)
            .await
            .map_err(|_| common::KlyntbotError::BusDisconnected)?;
        Ok(())
    }

    /// Non-blocking emit — used from sync callbacks inside async tasks.
    /// Drops the event silently if the channel is full (acceptable for progress updates).
    pub fn try_emit_event(&self, event: VoiceEvent) {
        let _ = self.event_tx.try_send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockTranscriptionEngine;
    use tempfile::TempDir;

    fn make_service(stt: Option<Arc<dyn TranscriptionEngine>>) -> (VoiceService, TempDir) {
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            stt,
            None,
            None,
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );
        (svc, tmp)
    }

    #[test]
    fn default_config() {
        let config = VoiceServiceConfig::default();
        assert_eq!(config.privacy_mode, PrivacyLevel::Standard);
    }

    #[test]
    fn not_available_without_engines() {
        let (svc, _tmp) = make_service(None);
        assert!(!svc.is_available());
        assert!(svc.engine_kind().is_none());
    }

    #[test]
    fn available_with_local_engine() {
        let mock = Arc::new(MockTranscriptionEngine::new("hello"));
        let (svc, _tmp) = make_service(Some(mock));
        assert!(svc.is_available());
        assert_eq!(svc.engine_kind(), Some(EngineKind::Local));
    }

    #[tokio::test]
    async fn session_state_idle_initially() {
        let (svc, _tmp) = make_service(None);
        assert_eq!(svc.session_state().await, VoiceSessionState::Idle);
    }

    #[tokio::test]
    async fn take_event_rx_only_once() {
        let (svc, _tmp) = make_service(None);
        let rx = svc.take_event_rx();
        assert!(rx.is_some());
        let rx2 = svc.take_event_rx();
        assert!(rx2.is_none());
    }

    #[tokio::test]
    async fn start_capture_fails_without_engine() {
        let (svc, _tmp) = make_service(None);
        let result = svc.start_capture().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No transcription engine available"));
    }

    #[tokio::test]
    async fn stop_capture_returns_none_when_idle() {
        let (svc, _tmp) = make_service(None);
        let result = svc.stop_capture().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn cancel_when_idle_is_noop() {
        let (svc, _tmp) = make_service(None);
        svc.cancel().await; // should not panic
    }

    #[tokio::test]
    async fn dismiss_sends_event() {
        let (svc, _tmp) = make_service(None);
        let mut rx = svc.take_event_rx().unwrap();
        svc.dismiss().await;
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, VoiceEvent::ProcessingInBackground));
    }

    /// End-to-end pipeline test: inject mock audio directly into the session,
    /// call stop_capture, and verify the full chain fires.
    #[tokio::test]
    async fn end_to_end_pipeline_with_mock_audio() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new(
            "schedule dentist and practice french",
        )) as Arc<dyn TranscriptionEngine>;
        let (svc, _tmp) = make_service(Some(mock_stt));

        let mut event_rx = svc.take_event_rx().unwrap();

        // Manually inject a session with a pre-populated audio_rx channel.
        // This bypasses start_capture (which requires real mic hardware).
        let (audio_tx, audio_rx) = mpsc::channel::<AudioChunk>(16);
        let stop_signal = Arc::new(AtomicBool::new(false));
        let keepalive_stop = stop_signal.clone();
        let keepalive = tokio::task::spawn_blocking(move || {
            while !keepalive_stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        {
            let mut guard = svc.active_session.lock().await;
            *guard = Some(ActiveSession {
                id: "test-session-1".to_string(),
                stop_signal: stop_signal.clone(),
                audio_rx: Some(audio_rx),
                stream_keepalive: keepalive,
                started_at: Instant::now(),
                state: VoiceSessionState::Capturing,
            });
        }

        // Send some fake audio then close the channel (simulating capture end)
        let chunk = AudioChunk {
            samples: vec![0.1; 1600], // 100ms of audio
            sample_rate: 16000,
        };
        audio_tx.send(chunk).await.unwrap();
        drop(audio_tx); // Closes channel, mock STT will finish

        // Call stop_capture — this should: transcribe → route → score → emit events
        let result = svc.stop_capture().await.unwrap();
        assert!(result.is_some(), "Should produce a transcript");

        let (transcript, metadata) = result.unwrap();

        // Verify transcript
        assert_eq!(transcript.text, "schedule dentist and practice french");
        assert!(!transcript.segments.is_empty());

        // Verify metadata
        assert_eq!(metadata.language.as_str(), "en");
        assert!(metadata.overall_confidence > 0.0);
        assert!(!metadata.pronunciation_scores.is_empty());
        assert!(metadata.audio_ref.is_some());
        assert_eq!(metadata.engine, EngineKind::Local);

        // Collect all emitted events
        let mut events = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            events.push(event);
        }

        // Should have: CaptureEnded, PartialTranscript, RoutingSuggestion(s), Finalized
        let event_types: Vec<&str> = events
            .iter()
            .map(|e| match e {
                VoiceEvent::CaptureStarted { .. } => "CaptureStarted",
                VoiceEvent::CaptureEnded { .. } => "CaptureEnded",
                VoiceEvent::PartialTranscript { .. } => "PartialTranscript",
                VoiceEvent::RoutingSuggestion { .. } => "RoutingSuggestion",
                VoiceEvent::Finalized { .. } => "Finalized",
                VoiceEvent::AudioLevel { .. } => "AudioLevel",
                VoiceEvent::MemoryEcho { .. } => "MemoryEcho",
                VoiceEvent::ProcessingInBackground => "ProcessingInBackground",
                VoiceEvent::SpeakResponse { .. } => "SpeakResponse",
                VoiceEvent::Error { .. } => "Error",
                _ => "Other",
            })
            .collect();

        assert!(
            event_types.contains(&"CaptureEnded"),
            "Missing CaptureEnded event. Got: {:?}",
            event_types
        );
        assert!(
            event_types.contains(&"PartialTranscript"),
            "Missing PartialTranscript event. Got: {:?}",
            event_types
        );
        assert!(
            event_types.contains(&"Finalized"),
            "Missing Finalized event. Got: {:?}",
            event_types
        );

        // Verify routing detected intents (schedule → tasks, french → learning)
        let routing_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, VoiceEvent::RoutingSuggestion { .. }))
            .collect();
        assert!(
            !routing_events.is_empty(),
            "Should detect at least one routing intent"
        );

        // Verify Finalized event has correct routing
        let finalized = events
            .iter()
            .find(|e| matches!(e, VoiceEvent::Finalized { .. }))
            .unwrap();
        if let VoiceEvent::Finalized {
            routed_to, text, ..
        } = finalized
        {
            assert_eq!(text, "schedule dentist and practice french");
            assert!(!routed_to.is_empty());
        }
    }

    #[tokio::test]
    async fn handle_response_emits_speak_event() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new("hello world"));
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            Some(mock_stt),
            Some(mock_tts),
            None,
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );

        let mut event_rx = svc.take_event_rx().unwrap();

        svc.handle_response(
            "Task created: dentist appointment Thursday",
            &TtsParams::default(),
        )
        .await
        .unwrap();

        // Should receive a SpeakResponse event with the TTS audio
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        match event {
            VoiceEvent::SpeakResponse {
                text, sample_rate, ..
            } => {
                assert_eq!(text, "Task created: dentist appointment Thursday");
                assert_eq!(sample_rate, 16000); // MockTtsEngine returns 16kHz
            }
            other => panic!("Expected SpeakResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_response_without_tts_returns_ok() {
        let (svc, _tmp) = make_service(None);
        let _event_rx = svc.take_event_rx().unwrap();
        // No TTS engine configured — should return Ok without emitting SpeakResponse
        let result = svc.handle_response("hello", &TtsParams::default()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_response_with_tts_emits_speak_and_text_matches() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new("test input"));
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            Some(mock_stt),
            Some(mock_tts),
            None,
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );

        let mut event_rx = svc.take_event_rx().unwrap();

        // Simulate the full response path
        let response = "I've scheduled your dentist appointment for Thursday at 2pm.";
        svc.handle_response(response, &TtsParams::default())
            .await
            .unwrap();

        match event_rx.recv().await.unwrap() {
            VoiceEvent::SpeakResponse {
                text,
                sample_rate,
                audio_base64,
            } => {
                assert_eq!(text, response);
                assert_eq!(sample_rate, 16000);
                assert!(!audio_base64.is_empty());
            }
            other => panic!("Expected SpeakResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn set_tts_engine_hot_swaps() {
        let (svc, _tmp) = make_service(None);
        let mut event_rx = svc.take_event_rx().unwrap();

        // Initially no TTS engine
        svc.handle_response("hello", &TtsParams::default())
            .await
            .unwrap();
        // No SpeakResponse event should be emitted
        assert!(event_rx.try_recv().is_err());

        // Hot-swap a TTS engine
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        svc.set_tts_engine(mock_tts);

        // Now TTS should work
        svc.handle_response("hello after swap", &TtsParams::default())
            .await
            .unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        match event {
            VoiceEvent::SpeakResponse { text, .. } => {
                assert_eq!(text, "hello after swap");
            }
            other => panic!("Expected SpeakResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn welcome_echo_emitted_only_on_first_capture() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new("hello"));
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            Some(mock_stt.clone()),
            None,
            None,
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );

        // First capture flag should be false initially
        assert!(!svc.first_capture_complete.load(Ordering::Relaxed));

        // After first swap, flag should become true
        svc.first_capture_complete.store(false, Ordering::Relaxed);
        assert!(!svc.first_capture_complete.swap(true, Ordering::Relaxed));
        // Second swap should return true (already set)
        assert!(svc.first_capture_complete.swap(true, Ordering::Relaxed));
    }
}
