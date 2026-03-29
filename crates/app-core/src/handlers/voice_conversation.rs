//! VoiceConversationManager — core types, state machine, and command channel.
//!
//! This module defines the conversation phase state machine, voice session state,
//! command enum, helper functions, and the manager struct. The actual conversation
//! loop (listening → reflecting → speaking) is implemented in a later task.

use chrono::{DateTime, Utc};
use common::{ChannelName, ChatId, SessionKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::events::AppEventEmitter;
use config::schema::VoiceConfig;
use voice_engine::{MemoryEchoProvider, VoiceService};

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
                | (Self::Speaking, Self::Idle)     // end during speaking
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
    }

    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

// ── Commands ─────────────────────────────────────────────────

#[derive(Debug)]
pub enum VoiceCommand {
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

// ── Manager ──────────────────────────────────────────────────

#[allow(dead_code)] // Fields used by conversation loop (Task 7)
pub struct VoiceConversationManager {
    pub(crate) voice_service: Arc<VoiceService>,
    pub(crate) repos: storage::Repos,
    pub(crate) agent: Arc<agent::AgentLoop>,
    pub(crate) emitter: Arc<dyn AppEventEmitter>,
    pub(crate) state: Arc<Mutex<VoiceConversationState>>,
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
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_transitions_from_idle() {
        assert!(ConversationPhase::Idle.can_transition_to(&ConversationPhase::Listening));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Speaking));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Reflecting));
    }

    #[test]
    fn phase_transitions_full_cycle() {
        assert!(ConversationPhase::Listening.can_transition_to(&ConversationPhase::Reflecting));
        assert!(ConversationPhase::Reflecting.can_transition_to(&ConversationPhase::Speaking));
        assert!(ConversationPhase::Speaking.can_transition_to(&ConversationPhase::Listening)); // auto-resume
        assert!(ConversationPhase::Speaking.can_transition_to(&ConversationPhase::Idle)); // end
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
        assert_eq!(compute_breath_ms("Hi"), 301); // Short → near minimum (300 + 2/2)
        assert_eq!(compute_breath_ms(&"A".repeat(1000)), 800); // Long → capped
        let medium = "A".repeat(400);
        let ms = compute_breath_ms(&medium);
        assert!(ms > 300 && ms < 800); // Medium → in between
    }
}
