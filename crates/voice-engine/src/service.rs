//! Voice service — orchestrates capture, transcription, routing, and TTS.
//!
//! This is the central coordinator for voice capture sessions. It manages
//! the VoiceSessionState machine, streams audio to transcription, runs
//! routing on partials, and produces VoiceEvents for the frontend orb.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::capture::{AudioCapture, CaptureConfig, CaptureSession};
use crate::events::VoiceEvent;
use crate::model_manager::{ModelManager, ModelState};
use crate::pronunciation::compute_pronunciation_report;
use crate::router::VoiceRouter;
use crate::session::VoiceSessionState;
use crate::stt::TranscriptionEngine;
use crate::tts::TtsEngine;
use crate::types::*;

/// Configuration for the VoiceService.
#[derive(Debug, Clone)]
pub struct VoiceServiceConfig {
    pub capture: CaptureConfig,
    pub prefer_local: bool,
    pub privacy_mode: PrivacyLevel,
}

impl Default for VoiceServiceConfig {
    fn default() -> Self {
        Self {
            capture: CaptureConfig::default(),
            prefer_local: true,
            privacy_mode: PrivacyLevel::Standard,
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
    stt_local: Option<Arc<dyn TranscriptionEngine>>,
    stt_cloud: Option<Arc<dyn TranscriptionEngine>>,
    /// TTS engine for spoken responses (used in SpeakResponse phase).
    #[allow(dead_code)]
    tts: Option<Arc<dyn TtsEngine>>,
    capture: AudioCapture,
    model_manager: ModelManager,
    router: VoiceRouter,
    config: VoiceServiceConfig,

    /// Current active session (at most one at a time).
    active_session: Arc<Mutex<Option<ActiveSession>>>,

    /// Channel for emitting VoiceEvents to the frontend.
    event_tx: mpsc::Sender<VoiceEvent>,
    /// Receiver side — consumed by the Tauri adapter to relay events.
    event_rx: Arc<Mutex<Option<mpsc::Receiver<VoiceEvent>>>>,
}

impl VoiceService {
    /// Create a new VoiceService.
    ///
    /// Pass `None` for engines that aren't available yet (e.g., local STT
    /// while the model is downloading).
    pub fn new(
        stt_local: Option<Arc<dyn TranscriptionEngine>>,
        stt_cloud: Option<Arc<dyn TranscriptionEngine>>,
        tts: Option<Arc<dyn TtsEngine>>,
        model_manager: ModelManager,
        config: VoiceServiceConfig,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(128);

        Self {
            stt_local,
            stt_cloud,
            tts,
            capture: AudioCapture::new(config.capture.clone()),
            model_manager,
            router: VoiceRouter::new(),
            config,
            active_session: Arc::new(Mutex::new(None)),
            event_tx,
            event_rx: Arc::new(Mutex::new(Some(event_rx))),
        }
    }

    /// Take the event receiver (can only be called once).
    /// The Tauri adapter consumes this to relay events to the frontend.
    pub async fn take_event_rx(&self) -> Option<mpsc::Receiver<VoiceEvent>> {
        self.event_rx.lock().await.take()
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

    /// Get the active transcription engine (local preferred, cloud fallback).
    fn active_engine(&self) -> Option<(&Arc<dyn TranscriptionEngine>, EngineKind)> {
        if self.config.prefer_local {
            if let Some(ref local) = self.stt_local {
                return Some((local, EngineKind::Local));
            }
        }
        if let Some(ref cloud) = self.stt_cloud {
            return Some((cloud, EngineKind::Cloud));
        }
        self.stt_local.as_ref().map(|l| (l, EngineKind::Local))
    }

    /// Start a voice capture session.
    ///
    /// Returns the session ID and engine kind, or an error if capture can't start.
    pub async fn start_capture(&self) -> common::Result<(String, EngineKind)> {
        let mut session_guard = self.active_session.lock().await;

        if session_guard.is_some() {
            return Err(common::ChannelError::ConnectionFailed(
                "Voice capture already in progress".to_string(),
            )
            .into());
        }

        let (_, engine_kind) = self.active_engine().ok_or_else(|| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                "No transcription engine available. Download a model or configure Groq API key."
                    .to_string(),
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

        let _ = self
            .event_tx
            .send(VoiceEvent::CaptureStarted {
                session_id: session_id.clone(),
                engine: engine_kind,
            })
            .await;

        *session_guard = Some(ActiveSession {
            id: session_id.clone(),
            stop_signal,
            audio_rx: Some(audio_rx),
            stream_keepalive,
            started_at: Instant::now(),
            state: VoiceSessionState::Capturing,
        });

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

        let mut rx = engine.transcribe_stream(audio_rx).await?;

        // Collect transcript partials and the final result.
        let mut final_transcript = None;
        while let Some(partial) = rx.recv().await {
            let _ = self
                .event_tx
                .send(VoiceEvent::PartialTranscript {
                    text: partial.text.clone(),
                    language: partial.language.as_str().to_string(),
                    is_final: partial.is_final,
                })
                .await;

            // Run routing on each partial.
            let intents = self.router.detect_intents(&partial.text);
            for event in self.router.to_events(&intents) {
                let _ = self.event_tx.send(event).await;
            }

            if partial.is_final {
                final_transcript = Some(Transcript {
                    text: partial.text,
                    language: partial.language,
                    segments: partial.segments,
                    overall_confidence: 0.0, // recomputed below
                });
            }
        }

        let transcript = match final_transcript {
            Some(mut t) => {
                if !t.segments.is_empty() {
                    t.overall_confidence = t.segments.iter().map(|s| s.confidence).sum::<f32>()
                        / t.segments.len() as f32;
                }
                t
            }
            None => {
                warn!("No transcript produced from voice capture");
                return Ok(None);
            }
        };

        let report = compute_pronunciation_report(&transcript);

        let metadata = VoiceMetadata {
            language: transcript.language.clone(),
            overall_confidence: transcript.overall_confidence,
            pronunciation_scores: report
                .word_scores
                .iter()
                .map(|w| (w.word.clone(), w.confidence))
                .collect(),
            audio_ref: None, // TODO: save audio to file for FSRS playback
            duration,
            engine: engine_kind,
            privacy_mode: self.config.privacy_mode,
        };

        let routed_to = self
            .router
            .detect_intents(&transcript.text)
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

    /// Get the current model state from ModelManager.
    pub fn model_state(&self) -> ModelState {
        self.model_manager.state()
    }

    /// Get the active engine kind (Local or Cloud).
    pub fn engine_kind(&self) -> Option<EngineKind> {
        self.active_engine().map(|(_, k)| k)
    }

    /// Check if voice capture is available (at least one engine configured).
    pub fn is_available(&self) -> bool {
        self.active_engine().is_some()
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
            model_manager,
            VoiceServiceConfig::default(),
        );
        (svc, tmp)
    }

    #[test]
    fn default_config() {
        let config = VoiceServiceConfig::default();
        assert!(config.prefer_local);
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
        let rx = svc.take_event_rx().await;
        assert!(rx.is_some());
        let rx2 = svc.take_event_rx().await;
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
        let mut rx = svc.take_event_rx().await.unwrap();
        svc.dismiss().await;
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, VoiceEvent::ProcessingInBackground));
    }
}
