//! VoiceConversationManager — core types, state machine, command channel,
//! and the conversation loop (listening -> reflecting -> speaking -> auto-resume).

use chrono::{DateTime, Duration, Utc};
use common::{ChannelName, ChatId, SessionKey};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::events::AppEventEmitter;
use config::schema::VoiceConfig;
use voice_engine::{MemoryEchoProvider, MonitorSession, VoiceEvent, VoiceService};

/// Wrapper that makes `MonitorSession` moveable to a blocking thread.
///
/// `cpal::Stream` is `!Send` for Android AAudio compatibility, but on macOS
/// (CoreAudio) the underlying types are thread-safe. We only move the monitor
/// to a blocking thread to keep the audio stream alive.
struct SendableMonitorSession(#[allow(dead_code)] MonitorSession);

// SAFETY: Same justification as `SendableCaptureSession` in voice-engine.
// On macOS, CoreAudio streams are safe to move between threads.
#[cfg(target_os = "macos")]
unsafe impl Send for SendableMonitorSession {}

// ── Phase ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationPhase {
    Idle,
    Listening,
    Reflecting,
    Speaking,
}

impl ConversationPhase {
    pub fn can_transition_to(&self, next: &ConversationPhase) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Listening)
                | (Self::Listening, Self::Reflecting)
                | (Self::Listening, Self::Idle)   // pause or end during listening
                | (Self::Reflecting, Self::Speaking)
                | (Self::Reflecting, Self::Idle)  // cancel during reflecting
                | (Self::Speaking, Self::Listening) // auto-resume or interrupt
                | (Self::Speaking, Self::Idle) // end during speaking
        )
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Reflecting => "reflecting",
            Self::Speaking => "speaking",
        }
    }
}

// ── State ────────────────────────────────────────────────────

#[derive(Debug)]
pub struct VoiceConversationState {
    pub session_key: Option<SessionKey>,
    pub phase: ConversationPhase,
    pub paused: bool,
    pub turn_count: u32,
    pub last_activity: DateTime<Utc>,
    pub interrupted: bool,
    pub pending_transcript: Option<String>,
    pub pending_response_text: Option<String>,
    pub tts_position: usize,
    pub session_title: String,
}

impl Default for VoiceConversationState {
    fn default() -> Self {
        Self {
            session_key: None,
            phase: ConversationPhase::Idle,
            paused: false,
            turn_count: 0,
            last_activity: Utc::now(),
            interrupted: false,
            pending_transcript: None,
            pending_response_text: None,
            tts_position: 0,
            session_title: "Voice session".to_string(),
        }
    }
}

impl VoiceConversationState {
    pub fn reset_for_new_session(&mut self, new_key: SessionKey) {
        self.session_key = Some(new_key);
        self.phase = ConversationPhase::Idle;
        self.paused = false;
        self.turn_count = 0;
        self.last_activity = Utc::now();
        self.interrupted = false;
        self.pending_transcript = None;
        self.pending_response_text = None;
        self.tts_position = 0;
        self.session_title = "New voice session".to_string();
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

// ── Commands ─────────────────────────────────────────────────

#[derive(Debug)]
pub enum VoiceCommand {
    /// Wake up the conversation loop after start() sets phase to Listening.
    Start,
    Pause,
    Resume,
    Interrupt,
    Continue,
    NewSession,
    End,
}

// ── Helpers ──────────────────────────────────────────────────

pub fn is_warm_session(last_activity: DateTime<Utc>, threshold_minutes: u32) -> bool {
    let elapsed = Utc::now() - last_activity;
    elapsed.num_minutes() < threshold_minutes as i64
}

pub fn compute_breath_ms(response_text: &str) -> u64 {
    let base = 300u64;
    let extra = response_text.len() as u64 / 2;
    (base + extra).min(800)
}

pub fn create_voice_session_key() -> SessionKey {
    SessionKey::new(
        &ChannelName::new("desktop"),
        &ChatId::new(Uuid::new_v4().to_string()),
    )
}

// ── Start response ──────────────────────────────────────────

/// Result of starting a voice conversation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResponse {
    pub session_key: String,
    pub session_title: String,
    pub is_continuing: bool,
}

// ── Manager ──────────────────────────────────────────────────

pub struct VoiceConversationManager {
    pub(crate) voice_service: Arc<VoiceService>,
    pub(crate) repos: storage::Repos,
    pub(crate) agent: Arc<agent::AgentLoop>,
    pub(crate) emitter: Arc<dyn AppEventEmitter>,
    pub(crate) state: Arc<Mutex<VoiceConversationState>>,
    #[allow(dead_code)]
    pub(crate) echo_provider: Arc<dyn MemoryEchoProvider>,
    pub(crate) config: Arc<RwLock<VoiceConfig>>,
    pub(crate) cmd_tx: mpsc::Sender<VoiceCommand>,
    cmd_rx: Mutex<Option<mpsc::Receiver<VoiceCommand>>>,
}

impl VoiceConversationManager {
    pub fn new(
        voice_service: Arc<VoiceService>,
        repos: storage::Repos,
        agent: Arc<agent::AgentLoop>,
        emitter: Arc<dyn AppEventEmitter>,
        echo_provider: Arc<dyn MemoryEchoProvider>,
        config: Arc<RwLock<VoiceConfig>>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(16);
        Self {
            voice_service,
            repos,
            agent,
            emitter,
            state: Arc::new(Mutex::new(VoiceConversationState::default())),
            echo_provider,
            config,
            cmd_tx,
            cmd_rx: Mutex::new(Some(cmd_rx)),
        }
    }

    pub async fn phase(&self) -> ConversationPhase {
        self.state.lock().await.phase
    }

    pub async fn session_key(&self) -> Option<SessionKey> {
        self.state.lock().await.session_key.clone()
    }

    pub async fn send_command(&self, cmd: VoiceCommand) {
        let _ = self.cmd_tx.send(cmd).await;
    }

    // ── 1. resolve_session ──────────────────────────────────

    /// Smart session attachment: reuse warm sessions, or create a new one.
    pub async fn resolve_session(&self) -> SessionKey {
        let config = self.config.read().await;
        let warm_session_min = config.conversation.warm_session_minutes;
        let warm_chat_min = config.conversation.warm_chat_minutes;
        drop(config);

        // 1. If manager already has an active session (phase != Idle), reuse it
        {
            let state = self.state.lock().await;
            if state.phase != ConversationPhase::Idle {
                if let Some(ref key) = state.session_key {
                    return key.clone();
                }
            }

            // 2. If previous session_key exists and last_activity is warm, reattach
            if let Some(ref key) = state.session_key {
                if is_warm_session(state.last_activity, warm_session_min) {
                    return key.clone();
                }
            }
        }

        // 3. Query DB for most recent "desktop" session — if updated recently, attach
        if let Ok(sessions) = self
            .repos
            .sessions
            .list_sessions_since(Utc::now() - Duration::minutes(warm_chat_min as i64))
            .await
        {
            if let Some(session) = sessions.iter().find(|s| s.key.starts_with("desktop:")) {
                let key = SessionKey::from_parts(
                    "desktop",
                    session.key.strip_prefix("desktop:").unwrap_or(&session.key),
                );
                return key;
            }
        }

        // 4. Otherwise create new
        create_voice_session_key()
    }

    // ── 2. start ────────────────────────────────────────────

    /// Begin (or resume) a voice conversation.
    pub async fn start(&self) -> common::Result<StartResponse> {
        if !self.voice_service.is_available() {
            let _ = self
                .voice_service
                .emit_event(VoiceEvent::SetupRequired {
                    needs_model: true,
                    needs_mic_permission: false,
                })
                .await;

            return Err(common::KlyntbotError::Bus(
                "No STT engine configured — voice not available".to_string(),
            ));
        }

        let session_key = self.resolve_session().await;

        // Check if we're continuing an existing session
        let is_continuing = {
            let state = self.state.lock().await;
            state
                .session_key
                .as_ref()
                .is_some_and(|k| k.as_str() == session_key.as_str())
                && state.turn_count > 0
        };

        // Determine session title
        let session_title = if is_continuing {
            // Try to read the existing title from DB
            match self.repos.sessions.get_session(session_key.as_str()).await {
                Ok(row) => row
                    .metadata
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Voice session")
                    .to_string(),
                Err(_) => "Voice session".to_string(),
            }
        } else {
            "New voice session".to_string()
        };

        // Update state
        let turn_count = {
            let mut state = self.state.lock().await;
            state.session_key = Some(session_key.clone());
            state.phase = ConversationPhase::Listening;
            state.paused = false;
            state.interrupted = false;
            state.session_title = session_title.clone();
            state.touch();
            state.turn_count
        };

        // Upsert session in DB
        let metadata = serde_json::json!({
            "title": session_title,
            "is_voice_session": true,
        });
        let _ = self
            .repos
            .sessions
            .upsert_voice_session(session_key.as_str(), &metadata)
            .await;

        // Emit phase changed
        let _ = self
            .voice_service
            .emit_event(VoiceEvent::PhaseChanged {
                phase: ConversationPhase::Listening.as_str().to_string(),
                session_title: Some(session_title.clone()),
                turn_count,
            })
            .await;

        info!(
            "Voice conversation started: {} (continuing={})",
            session_key, is_continuing
        );

        // Wake up the conversation loop — it's blocked on cmd_rx.recv()
        // while idle. The Start command unblocks it so it re-checks the
        // phase (now Listening) and enters run_listening_phase.
        let _ = self.cmd_tx.send(VoiceCommand::Start).await;

        Ok(StartResponse {
            session_key: session_key.to_string(),
            session_title,
            is_continuing,
        })
    }

    // ── 2b. new_session ─────────────────────────────────────

    /// Atomically end any active capture, reset state, and start a fresh session.
    /// Avoids the race of sending `NewSession` through the command channel and
    /// sleeping before calling `start()`.
    pub async fn new_session(&self) -> common::Result<StartResponse> {
        // Stop any active capture
        let _ = self.cmd_tx.send(VoiceCommand::End).await;

        // Reset state directly under the lock
        {
            let mut state = self.state.lock().await;
            let new_key = create_voice_session_key();
            state.reset_for_new_session(new_key);
        }

        // Now start fresh (resolve_session will see the new key)
        self.start().await
    }

    // ── 3. spawn_supervised_loop ──────────────────────────────

    /// Spawn the conversation loop with automatic panic detection.
    ///
    /// If the inner loop panics, the error is logged clearly instead of being
    /// silently dropped. A full auto-restart would require a recreatable command
    /// channel — deferred to a future task.
    pub async fn spawn_supervised_loop(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let cmd_rx = self
            .cmd_rx
            .lock()
            .await
            .take()
            .expect("spawn_supervised_loop called more than once");
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let result = tokio::spawn(async move {
                manager.conversation_loop(cmd_rx).await;
            })
            .await;

            match result {
                Ok(()) => {
                    info!("Voice conversation loop exited normally");
                }
                Err(e) => {
                    error!(
                        "Voice conversation loop panicked: {e}. \
                         Voice will not work until app restart."
                    );
                }
            }
        })
    }

    // ── 4. conversation_loop ────────────────────────────────

    async fn conversation_loop(self: &Arc<Self>, mut cmd_rx: mpsc::Receiver<VoiceCommand>) {
        info!("Voice conversation loop started");
        loop {
            let phase = self.state.lock().await.phase;
            match phase {
                ConversationPhase::Idle => {
                    // Wait for a command
                    match cmd_rx.recv().await {
                        Some(cmd) => self.handle_command_while_idle(cmd).await,
                        None => {
                            info!("Voice command channel closed, exiting loop");
                            break;
                        }
                    }
                }
                ConversationPhase::Listening => {
                    self.run_listening_phase(&mut cmd_rx).await;
                }
                ConversationPhase::Reflecting => {
                    self.run_reflecting_phase(&mut cmd_rx).await;
                }
                ConversationPhase::Speaking => {
                    self.run_speaking_phase(&mut cmd_rx).await;
                }
            }
        }
        info!("Voice conversation loop ended");
    }

    // ── 5. run_listening_phase ──────────────────────────────

    async fn run_listening_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        info!("Listening phase: starting capture");

        // Start audio capture
        let capture_result = self.voice_service.start_capture().await;
        let (_session_id, _engine_kind) = match capture_result {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to start audio capture: {e}");
                let _ = self
                    .voice_service
                    .emit_event(VoiceEvent::Error {
                        message: format!("Capture failed: {e}"),
                        recoverable: true,
                    })
                    .await;
                self.transition_to(ConversationPhase::Idle).await;
                return;
            }
        };

        // Poll for silence detection or commands.
        // VoiceService emits CaptureEnded when silence is detected, but the
        // manager can also stop capture via stop_capture(). We poll session_state
        // periodically and check for commands.
        let mut silence_detected = false;
        loop {
            tokio::select! {
                biased;
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        VoiceCommand::Pause => {
                            info!("Listening phase: Pause received, stopping capture");
                            let _ = self.voice_service.stop_capture().await;
                            self.state.lock().await.paused = true;
                            self.transition_to(ConversationPhase::Idle).await;
                            return;
                        }
                        VoiceCommand::End => {
                            // End during listening: finalize the transcript so
                            // the captured audio is processed by the agent.
                            // Mark paused so auto_resume_or_idle goes idle
                            // instead of re-entering listening after speaking.
                            info!("Listening phase: End received, finalizing capture");
                            self.state.lock().await.paused = true;
                            silence_detected = true;
                        }
                        VoiceCommand::NewSession => {
                            info!("Listening phase: NewSession, stopping capture");
                            let _ = self.voice_service.stop_capture().await;
                            let new_key = create_voice_session_key();
                            self.state.lock().await.reset_for_new_session(new_key);
                            self.transition_to(ConversationPhase::Idle).await;
                            return;
                        }
                        VoiceCommand::Interrupt => {
                            // During listening, interrupt means stop capture and finalize
                            silence_detected = true;
                        }
                        _ => {} // Resume/Continue ignored during listening
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                    // Check if the capture module detected silence
                    if self.voice_service.is_silence_detected() {
                        silence_detected = true;
                    }
                }
            }

            if silence_detected {
                break;
            }
        }

        // Finalize: stop capture and get transcript
        info!("Listening phase: finalizing capture");
        match self.voice_service.stop_capture().await {
            Ok(Some((transcript, _metadata))) => {
                let text = transcript.text.trim().to_string();
                if text.is_empty() {
                    info!("Listening phase: empty transcript, returning to listening");
                    // Re-enter listening (don't transition to reflecting with empty text)
                    return;
                }
                info!("Listening phase: transcript = \"{}\"", text);
                self.state.lock().await.pending_transcript = Some(text);
                self.transition_to(ConversationPhase::Reflecting).await;

                // Emit Reflecting event
                let _ = self.voice_service.emit_event(VoiceEvent::Reflecting).await;
            }
            Ok(None) => {
                info!("Listening phase: no transcript (session was already stopped)");
                // Empty capture — stay in listening or go idle
                // Check if we were in the middle of a conversation
                let turn_count = self.state.lock().await.turn_count;
                if turn_count > 0 {
                    // Re-listen
                    return;
                }
                self.transition_to(ConversationPhase::Idle).await;
            }
            Err(e) => {
                warn!("Listening phase: transcription error: {e}");
                let _ = self
                    .voice_service
                    .emit_event(VoiceEvent::Error {
                        message: format!("Transcription failed: {e}"),
                        recoverable: true,
                    })
                    .await;
                self.transition_to(ConversationPhase::Idle).await;
            }
        }
    }

    // ── 6. run_reflecting_phase ─────────────────────────────

    async fn run_reflecting_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        let (transcript_text, session_key_str) = {
            let state = self.state.lock().await;
            let text = state.pending_transcript.clone().unwrap_or_default();
            let sk = state
                .session_key
                .as_ref()
                .map(|k| k.to_string())
                .unwrap_or_default();
            (text, sk)
        };

        if transcript_text.is_empty() {
            info!("Reflecting phase: empty transcript, back to listening");
            self.transition_to(ConversationPhase::Listening).await;
            return;
        }

        info!(
            "Reflecting phase: sending to agent (session={})",
            session_key_str
        );

        // Emit PhaseChanged for reflecting
        let (turn_count, session_title) = {
            let state = self.state.lock().await;
            (state.turn_count, state.session_title.clone())
        };
        let _ = self
            .voice_service
            .emit_event(VoiceEvent::PhaseChanged {
                phase: ConversationPhase::Reflecting.as_str().to_string(),
                session_title: Some(session_title),
                turn_count,
            })
            .await;

        // Call agent
        let streaming_handle = match self
            .agent
            .process_direct_streaming(transcript_text, session_key_str.clone())
            .await
        {
            Ok(handle) => handle,
            Err(e) => {
                warn!("Reflecting phase: agent error: {e}");
                let _ = self
                    .voice_service
                    .emit_event(VoiceEvent::Error {
                        message: format!("Agent processing failed: {e}"),
                        recoverable: true,
                    })
                    .await;
                self.transition_to(ConversationPhase::Idle).await;
                return;
            }
        };

        let mut event_rx = streaming_handle.event_rx;
        let cancel_token = streaming_handle.cancel_token;
        let mut response_content = String::new();

        // Forward agent events to the emitter, collect the final content
        loop {
            tokio::select! {
                biased;
                Some(cmd) = cmd_rx.recv() => {
                    if let VoiceCommand::End = cmd {
                        info!("Reflecting phase: End received, cancelling agent");
                        cancel_token.cancel();
                        self.transition_to(ConversationPhase::Idle).await;
                        return;
                    }
                }
                event = event_rx.recv() => {
                    match event {
                        Some(agent::AgentEvent::ContentChunk { data }) => {
                            // Forward to main chat UI
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::ContentChunkPayload {
                                    session_key: session_key_str.clone(),
                                    data: data.clone(),
                                },
                            ) {
                                self.emitter.emit_event(
                                    desktop_shared::events::AGENT_CONTENT_CHUNK,
                                    val,
                                );
                            }
                            response_content.push_str(&data);
                        }
                        Some(agent::AgentEvent::Done { content, message_id }) => {
                            response_content = content.clone();
                            // Forward done event
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::DonePayload {
                                    session_key: session_key_str.clone(),
                                    content,
                                },
                            ) {
                                self.emitter.emit_event(
                                    desktop_shared::events::AGENT_DONE,
                                    val,
                                );
                            }
                            let _ = message_id; // consumed by DonePayload via session
                            break;
                        }
                        Some(agent::AgentEvent::Error { message }) => {
                            warn!("Reflecting phase: agent event error: {message}");
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::AgentErrorPayload {
                                    session_key: session_key_str.clone(),
                                    message,
                                },
                            ) {
                                self.emitter.emit_event(
                                    desktop_shared::events::AGENT_ERROR,
                                    val,
                                );
                            }
                            break;
                        }
                        Some(agent::AgentEvent::ToolStart { name, args, agent }) => {
                            let action = args.get("action").and_then(|v| v.as_str()).map(String::from);
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::ToolStartPayload {
                                    session_key: session_key_str.clone(),
                                    name,
                                    action,
                                    agent,
                                },
                            ) {
                                self.emitter.emit_event(
                                    desktop_shared::events::AGENT_TOOL_START,
                                    val,
                                );
                            }
                        }
                        Some(agent::AgentEvent::ToolEnd { name, success, duration_ms, result, agent }) => {
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::ToolEndPayload {
                                    session_key: session_key_str.clone(),
                                    name,
                                    action: None,
                                    success,
                                    duration_ms,
                                    result,
                                    estimated_tokens: None,
                                    agent,
                                },
                            ) {
                                self.emitter.emit_event(
                                    desktop_shared::events::AGENT_TOOL_END,
                                    val,
                                );
                            }
                        }
                        Some(other_event) => {
                            // Forward remaining events generically
                            if let Ok(val) = serde_json::to_value(&other_event) {
                                self.emitter.emit_event("agent:event", val);
                            }
                        }
                        None => {
                            // Event channel closed — agent finished
                            break;
                        }
                    }
                }
            }
        }

        // Store response and advance
        {
            let mut state = self.state.lock().await;
            state.pending_response_text = Some(response_content);
            state.pending_transcript = None;
            state.turn_count += 1;
            state.tts_position = 0;
            state.interrupted = false;
            state.touch();
        }

        // Emit chat thread event so the sidebar auto-refreshes
        {
            let state = self.state.lock().await;
            if let Some(ref sk) = state.session_key {
                let is_new = state.turn_count == 1;
                self.emitter.emit_chat_thread(is_new, sk.as_str());
            }
        }

        self.transition_to(ConversationPhase::Speaking).await;
    }

    // ── 7. run_speaking_phase ───────────────────────────────

    async fn run_speaking_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        let (response_text, tts_position) = {
            let state = self.state.lock().await;
            (
                state.pending_response_text.clone().unwrap_or_default(),
                state.tts_position,
            )
        };

        if response_text.is_empty() {
            info!("Speaking phase: no response text, auto-resume or idle");
            self.auto_resume_or_idle().await;
            return;
        }

        // Get the portion to speak (from tts_position if continuing after interrupt)
        let tts_text = if tts_position > 0 && tts_position < response_text.len() {
            // Find the next word boundary after tts_position
            let remaining = &response_text[tts_position..];
            remaining
                .find(' ')
                .map(|i| &remaining[i + 1..])
                .unwrap_or(remaining)
                .to_string()
        } else {
            response_text.clone()
        };

        if tts_text.is_empty() {
            self.auto_resume_or_idle().await;
            return;
        }

        info!(
            "Speaking phase: synthesizing TTS ({} chars)",
            tts_text.len()
        );

        // Emit PhaseChanged
        let (turn_count, title) = {
            let state = self.state.lock().await;
            (state.turn_count, state.session_title.clone())
        };
        let _ = self
            .voice_service
            .emit_event(VoiceEvent::PhaseChanged {
                phase: ConversationPhase::Speaking.as_str().to_string(),
                session_title: Some(title),
                turn_count,
            })
            .await;

        // Call TTS via voice service (emits SpeakResponse event)
        let tts_params = {
            let config = self.config.read().await;
            voice_engine::TtsParams {
                speaking_rate: config.output.speaking_rate,
                voice_name: config.output.voice_preferences.get("default").cloned(),
                ..Default::default()
            }
        };
        if let Err(e) = self
            .voice_service
            .handle_response(&tts_text, &tts_params)
            .await
        {
            warn!("Speaking phase: TTS failed: {e}");
        }

        // Start audio monitor for interrupt detection.
        // MonitorSession contains a cpal::Stream which is !Send, so we extract
        // the Send-safe rms_rx + stop_signal and keep the stream alive on a
        // blocking thread (same pattern as VoiceService::begin_capture).
        let mut monitor = start_monitor_safe(&self.voice_service);

        // Estimate TTS duration: ~150ms per word
        let word_count = tts_text.split_whitespace().count();
        let estimated_duration_ms = (word_count as u64) * 150;
        let speak_start = tokio::time::Instant::now();
        let speak_deadline =
            speak_start + std::time::Duration::from_millis(estimated_duration_ms.max(1000));

        // Speech interrupt threshold: RMS above 0.02 indicates the user is talking
        const INTERRUPT_RMS_THRESHOLD: f32 = 0.02;
        // Require multiple consecutive samples above threshold to avoid false triggers
        let mut consecutive_speech_samples = 0u32;
        const SPEECH_SAMPLE_THRESHOLD: u32 = 3;

        let mut interrupted = false;

        loop {
            tokio::select! {
                biased;
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        VoiceCommand::End => {
                            info!("Speaking phase: End received");
                            self.voice_service.stop_tts_playback().await;
                            if let Some(ref stop) = monitor.stop_signal {
                                stop.store(true, Ordering::Relaxed);
                            }
                            drop(monitor);
                            self.transition_to(ConversationPhase::Idle).await;
                            return;
                        }
                        VoiceCommand::Pause => {
                            info!("Speaking phase: Pause received");
                            self.voice_service.stop_tts_playback().await;
                            if let Some(ref stop) = monitor.stop_signal {
                                stop.store(true, Ordering::Relaxed);
                            }
                            drop(monitor);
                            self.state.lock().await.paused = true;
                            self.transition_to(ConversationPhase::Idle).await;
                            return;
                        }
                        VoiceCommand::Interrupt => {
                            self.voice_service.stop_tts_playback().await;
                            interrupted = true;
                        }
                        _ => {} // Resume/Continue/NewSession handled elsewhere
                    }
                }
                Some(rms) = monitor.rms_rx.recv() => {
                    if rms > INTERRUPT_RMS_THRESHOLD {
                        consecutive_speech_samples += 1;
                        if consecutive_speech_samples >= SPEECH_SAMPLE_THRESHOLD {
                            info!("Speaking phase: speech detected (rms={rms:.3}), interrupt");
                            self.voice_service.stop_tts_playback().await;
                            interrupted = true;
                        }
                    } else {
                        consecutive_speech_samples = 0;
                    }
                }
                _ = tokio::time::sleep_until(speak_deadline) => {
                    // TTS playback estimated complete
                    info!("Speaking phase: TTS playback estimated complete");
                    break;
                }
            }

            if interrupted {
                break;
            }
        }

        // Stop monitor
        if let Some(ref stop) = monitor.stop_signal {
            stop.store(true, Ordering::Relaxed);
        }
        drop(monitor);

        if interrupted {
            // Estimate how far into the text we got
            let elapsed_ms = speak_start.elapsed().as_millis() as u64;
            let fraction = if estimated_duration_ms > 0 {
                (elapsed_ms as f64 / estimated_duration_ms as f64).min(1.0)
            } else {
                1.0
            };
            let estimated_chars = (tts_text.len() as f64 * fraction) as usize;
            let new_tts_position = tts_position + estimated_chars;

            {
                let mut state = self.state.lock().await;
                state.interrupted = true;
                state.tts_position = new_tts_position;
            }

            // Emit TTS fade out and continue available
            let _ = self.voice_service.emit_event(VoiceEvent::TtsFadeOut).await;
            let _ = self
                .voice_service
                .emit_event(VoiceEvent::ContinueAvailable { timeout_secs: 8 })
                .await;

            // Go to listening to capture what the user is saying
            self.transition_to(ConversationPhase::Listening).await;
        } else {
            // Completed normally
            self.state.lock().await.interrupted = false;
            self.auto_resume_or_idle().await;
        }
    }

    // ── 8. auto_resume_or_idle ──────────────────────────────

    async fn auto_resume_or_idle(&self) {
        let config = self.config.read().await;
        let auto_resume = config.conversation.auto_resume;
        let adaptive_breath = config.conversation.adaptive_breath;
        drop(config);

        let (paused, response_text) = {
            let state = self.state.lock().await;
            (
                state.paused,
                state.pending_response_text.clone().unwrap_or_default(),
            )
        };

        if paused || !auto_resume {
            info!("Auto-resume disabled or paused, going idle");
            self.transition_to(ConversationPhase::Idle).await;
            return;
        }

        // Adaptive breath pause
        let breath_ms = if adaptive_breath {
            compute_breath_ms(&response_text)
        } else {
            300 // fixed minimum
        };

        info!("Auto-resume: breath pause {}ms", breath_ms);
        tokio::time::sleep(std::time::Duration::from_millis(breath_ms)).await;

        // Transition to listening
        self.transition_to(ConversationPhase::Listening).await;

        let (turn_count, title) = {
            let state = self.state.lock().await;
            (state.turn_count, state.session_title.clone())
        };
        let _ = self
            .voice_service
            .emit_event(VoiceEvent::PhaseChanged {
                phase: ConversationPhase::Listening.as_str().to_string(),
                session_title: Some(title),
                turn_count,
            })
            .await;
    }

    // ── 9. handle_command_while_idle ────────────────────────

    async fn handle_command_while_idle(&self, cmd: VoiceCommand) {
        match cmd {
            VoiceCommand::Start => {
                // start() already set phase to Listening — the loop will
                // pick it up on the next iteration. Nothing else needed.
                info!("Idle: Start received, entering listening");
            }
            VoiceCommand::Resume => {
                let (was_paused, turn_count, title) = {
                    let mut state = self.state.lock().await;
                    if state.paused {
                        state.paused = false;
                        (true, state.turn_count, state.session_title.clone())
                    } else {
                        (false, 0, String::new())
                    }
                };
                if was_paused {
                    info!("Idle: Resume received, transitioning to Listening");
                    self.transition_to(ConversationPhase::Listening).await;

                    let _ = self
                        .voice_service
                        .emit_event(VoiceEvent::PhaseChanged {
                            phase: ConversationPhase::Listening.as_str().to_string(),
                            session_title: Some(title),
                            turn_count,
                        })
                        .await;
                }
            }
            VoiceCommand::Continue => {
                let (interrupted, has_response) = {
                    let state = self.state.lock().await;
                    (state.interrupted, state.pending_response_text.is_some())
                };
                if interrupted && has_response {
                    info!("Idle: Continue received, resuming TTS from position");
                    self.transition_to(ConversationPhase::Speaking).await;
                }
            }
            VoiceCommand::End => {
                info!("Idle: End received (already idle, no-op)");
                // Emit final idle phase change in case UI needs it
                let _ = self
                    .voice_service
                    .emit_event(VoiceEvent::PhaseChanged {
                        phase: ConversationPhase::Idle.as_str().to_string(),
                        session_title: None,
                        turn_count: 0,
                    })
                    .await;
            }
            _ => {
                // Pause, Interrupt, NewSession while idle — ignore
            }
        }
    }

    // ── Utilities ───────────────────────────────────────────

    /// Transition to a new phase with logging and event emission.
    async fn transition_to(&self, next: ConversationPhase) {
        let mut state = self.state.lock().await;
        let current = state.phase;
        if current == next {
            return;
        }
        if !current.can_transition_to(&next) {
            warn!(
                "Invalid phase transition: {} -> {}",
                current.as_str(),
                next.as_str()
            );
            return;
        }
        info!(
            "Phase transition: {} -> {}",
            current.as_str(),
            next.as_str()
        );
        state.phase = next;
        state.touch();
    }
}

// ── Monitor helper ──────────────────────────────────────────

/// Result of starting a monitor: the rms receiver, the stop signal, and a
/// oneshot sender whose drop will unpark the blocking keepalive thread.
struct MonitorHandle {
    rms_rx: mpsc::Receiver<f32>,
    stop_signal: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// Drop this to release the blocking keepalive thread (and the cpal stream).
    _keepalive_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

/// Start a lightweight audio monitor, extracting Send-safe parts and keeping
/// the !Send cpal::Stream alive on a blocking thread.
fn start_monitor_safe(voice_service: &VoiceService) -> MonitorHandle {
    match voice_service.start_monitor() {
        Ok(mut session) => {
            let stop = session.stop_signal.clone();

            // Extract the Send-safe rms_rx before wrapping in SendableMonitorSession.
            let rms_rx = {
                let (_, dummy) = mpsc::channel(1);
                std::mem::replace(&mut session.rms_rx, dummy)
            };

            // Keep the !Send MonitorSession alive on a blocking thread.
            // The thread parks until keepalive_tx is dropped (or sends).
            let sendable = SendableMonitorSession(session);
            let (keepalive_tx, keepalive_rx) = tokio::sync::oneshot::channel::<()>();
            tokio::task::spawn_blocking(move || {
                let _ = keepalive_rx.blocking_recv();
                drop(sendable);
            });

            MonitorHandle {
                rms_rx,
                stop_signal: Some(stop),
                _keepalive_tx: Some(keepalive_tx),
            }
        }
        Err(e) => {
            warn!("Failed to start audio monitor: {e}");
            let (_tx, rx) = mpsc::channel::<f32>(1);
            MonitorHandle {
                rms_rx: rx,
                stop_signal: None,
                _keepalive_tx: None,
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_session_creates_unique_keys() {
        let key1 = create_voice_session_key();
        let key2 = create_voice_session_key();
        assert_ne!(key1.as_str(), key2.as_str());
        assert!(key1.as_str().starts_with("desktop:"));
    }

    #[test]
    fn phase_transitions_from_idle() {
        assert!(ConversationPhase::Idle.can_transition_to(&ConversationPhase::Listening));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Speaking));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Reflecting));
    }

    #[test]
    fn phase_invalid_transitions_rejected() {
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Speaking));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Reflecting));
        assert!(!ConversationPhase::Listening.can_transition_to(&ConversationPhase::Speaking));
    }

    #[test]
    fn phase_transitions_full_cycle() {
        assert!(ConversationPhase::Listening.can_transition_to(&ConversationPhase::Reflecting));
        assert!(ConversationPhase::Reflecting.can_transition_to(&ConversationPhase::Speaking));
        assert!(ConversationPhase::Speaking.can_transition_to(&ConversationPhase::Listening)); // auto-resume
        assert!(ConversationPhase::Speaking.can_transition_to(&ConversationPhase::Idle));
        // end
    }

    #[test]
    fn phase_interrupt_during_speaking() {
        // Interrupt: Speaking → Listening (skip Reflecting)
        assert!(ConversationPhase::Speaking.can_transition_to(&ConversationPhase::Listening));
    }

    #[test]
    fn state_resets_on_new_session() {
        let mut state = VoiceConversationState {
            turn_count: 5,
            session_key: Some(SessionKey::from_parts("desktop", "old-uuid")),
            pending_response_text: Some("old response".into()),
            ..Default::default()
        };

        state.reset_for_new_session(SessionKey::from_parts("desktop", "new-uuid"));

        assert_eq!(state.turn_count, 0);
        assert_eq!(
            state.session_key.as_ref().unwrap().as_str(),
            "desktop:new-uuid"
        );
        assert!(state.pending_response_text.is_none());
        assert!(!state.interrupted);
        assert!(!state.paused);
    }

    #[test]
    fn warm_session_detection() {
        let now = Utc::now();
        let recent = now - chrono::Duration::minutes(10);
        let old = now - chrono::Duration::minutes(20);

        assert!(is_warm_session(recent, 15)); // 10 min < 15 min threshold
        assert!(!is_warm_session(old, 15)); // 20 min >= 15 min threshold
    }

    #[test]
    fn adaptive_breath_duration() {
        assert_eq!(compute_breath_ms("Hi"), 301); // Short -> near minimum (300 + 2/2)
        assert_eq!(compute_breath_ms(&"A".repeat(1000)), 800); // Long -> capped
        let medium = "A".repeat(400);
        let ms = compute_breath_ms(&medium);
        assert!(ms > 300 && ms < 800); // Medium -> in between
    }
}
