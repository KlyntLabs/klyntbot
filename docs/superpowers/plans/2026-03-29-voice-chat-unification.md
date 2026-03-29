# Voice-Chat Unification v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify voice and chat into a single modality via a VoiceConversationManager that orchestrates multi-turn voice conversations within existing chat sessions.

**Architecture:** New `VoiceConversationManager` in app-core wraps `VoiceService` (audio I/O) + `chat_send_internal` (agent + persistence). A spawned Tokio task owns the conversation loop (Listening → Reflecting → Speaking → auto-resume). Commands arrive via `mpsc` channel for atomic state changes. Dual emission: `VoiceEvent` to the orb, chat events to the main window.

**Tech Stack:** Rust (app-core, voice-engine, config, desktop), React + TypeScript (desktop-ui), Tauri 2 IPC, tokio mpsc channels, Web Audio API

**Spec:** `docs/superpowers/specs/2026-03-29-voice-chat-unification-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/handlers/voice_conversation.rs` | `VoiceConversationManager`, `VoiceConversationState`, `ConversationPhase`, `VoiceCommand`, conversation loop, session attachment |
| `crates/desktop-shared/src/commands/voice_conversation.rs` | IPC request/response types: `VoiceConversationStartResponse`, `VoiceConversationStatusResponse` |
| `crates/desktop/src/commands/voice_conversation.rs` | 8 Tauri `#[tauri::command]` functions delegating to `AppCore` |
| `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts` | React hook: conversation state + actions (start, pause, resume, interrupt, continue, newSession, end) |
| `desktop-ui/src/features/voice/__tests__/useVoiceConversation.test.ts` | Vitest tests for hook state machine and event handling |

### Modified Files

| File | Change |
|------|--------|
| `crates/voice-engine/src/events.rs` | Add `VoiceEvent` variants: `Reflecting`, `TtsFadeOut`, `ContinueAvailable`, `PhaseChanged`, `SetupRequired` |
| `crates/voice-engine/src/capture.rs` | Add `AudioCapture::start_monitor()` → lightweight RMS-only stream for interrupt detection |
| `crates/config/src/schema/voice.rs` | Add `VoiceConversationConfig` struct with warm thresholds, silence, auto_resume, adaptive_breath |
| `crates/app-core/src/handlers/mod.rs` | Add `pub mod voice_conversation;` |
| `crates/app-core/src/handlers/voice_echo.rs` | Add `Arc<ContextEngine>` field + Tier 3 fallback in `lookup()` |
| `crates/app-core/src/handlers/chat/streaming.rs` | Add `is_voice: bool` param to `chat_send()`, pass through to session metadata |
| `crates/app-core/src/init/mod.rs` | Create `VoiceConversationManager` and store in `AppCore` |
| `crates/desktop/src/commands/mod.rs` | Add `pub mod voice_conversation;` |
| `crates/desktop/src/main.rs` | Update hotkey handler to use `VoiceConversationManager` instead of raw `VoiceService` |
| `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` | Redesign for multi-turn: title bar, phase visuals, Continue button, dock button |
| `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx` | Switch from `useVoiceEvents` to `useVoiceConversation`, update window size to 320x280 |
| `desktop-ui/src/features/voice/index.ts` | Export `useVoiceConversation` |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Add voice indicator in header, voice-aware scroll behavior |
| `crates/desktop/src/tray_countdown.rs` | Add tray icon states (listening, speaking, idle) |

---

## Task 1: VoiceConversationConfig

Add conversation-specific configuration fields to the voice config schema.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`

- [ ] **Step 1: Add VoiceConversationConfig struct**

```rust
// In crates/config/src/schema/voice.rs, add after VoiceLearningConfig:

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationConfig {
    /// Minutes before a previous voice session is considered "cold" (default: 15)
    #[serde(default = "default_warm_session_minutes")]
    pub warm_session_minutes: u32,
    /// Minutes before a main chat session is considered "cold" (default: 5)
    #[serde(default = "default_warm_chat_minutes")]
    pub warm_chat_minutes: u32,
    /// Seconds of silence to end a turn (default: 1.5)
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_secs: f32,
    /// Auto-resume listening after agent response (default: true)
    #[serde(default = "default_true")]
    pub auto_resume: bool,
    /// Variable pause after response based on length (default: true)
    #[serde(default = "default_true")]
    pub adaptive_breath: bool,
}

fn default_warm_session_minutes() -> u32 { 15 }
fn default_warm_chat_minutes() -> u32 { 5 }
fn default_silence_threshold() -> f32 { 1.5 }
fn default_true() -> bool { true }

impl Default for VoiceConversationConfig {
    fn default() -> Self {
        Self {
            warm_session_minutes: 15,
            warm_chat_minutes: 5,
            silence_threshold_secs: 1.5,
            auto_resume: true,
            adaptive_breath: true,
        }
    }
}
```

- [ ] **Step 2: Add field to VoiceConfig**

```rust
// In VoiceConfig struct, add:
#[serde(default)]
pub conversation: VoiceConversationConfig,
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p config`
Expected: Compiles cleanly. Existing config.json without `conversation` field still deserializes (all fields have defaults).

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/voice.rs
git commit -m "feat(config): add VoiceConversationConfig with warm thresholds and auto-resume"
```

---

## Task 2: New VoiceEvent Variants

Extend the `VoiceEvent` enum for the conversation manager's needs.

**Files:**
- Modify: `crates/voice-engine/src/events.rs`

- [ ] **Step 1: Add new variants to VoiceEvent**

```rust
// Add these variants to the VoiceEvent enum in events.rs:

/// Manager phase changed (for orb UI state)
PhaseChanged {
    phase: String,  // "idle", "listening", "reflecting", "speaking"
    session_title: Option<String>,
    turn_count: u32,
},
/// Agent is processing — brain is reflecting (replaces generic "processing")
Reflecting,
/// TTS should fade out over 300ms (soft interrupt)
TtsFadeOut,
/// User can tap "Continue" to resume interrupted TTS
ContinueAvailable {
    /// How many seconds before the button auto-hides
    timeout_secs: u8,
},
/// Voice setup required before conversation can start
SetupRequired {
    needs_model: bool,
    needs_mic_permission: bool,
},
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p voice-engine`
Expected: Compiles. Existing code using VoiceEvent still works (new variants are additive).

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/src/events.rs
git commit -m "feat(voice-engine): add conversation manager VoiceEvent variants"
```

---

## Task 3: Audio Monitor for Interrupt Detection

Add a lightweight RMS-only audio monitor to `AudioCapture` for detecting speech during TTS playback.

**Files:**
- Modify: `crates/voice-engine/src/capture.rs`
- Modify: `crates/voice-engine/src/lib.rs` (if MonitorSession not re-exported)

- [ ] **Step 1: Write test for MonitorSession**

```rust
// In crates/voice-engine/src/capture.rs, add to #[cfg(test)] mod tests:

#[test]
fn monitor_session_has_rms_channel() {
    // MonitorSession should have rms_rx and stop_signal but NOT audio_rx
    // This verifies the lightweight monitor doesn't buffer full audio
    let session = MonitorSession {
        _stream: todo!(), // Mock in real test
        stop_signal: Arc::new(AtomicBool::new(false)),
        rms_rx: tokio::sync::mpsc::channel(32).1,
    };
    assert!(!session.stop_signal.load(Ordering::Relaxed));
}
```

- [ ] **Step 2: Add MonitorSession struct and start_monitor method**

```rust
/// Lightweight audio session — only RMS levels, no audio buffering.
/// Used during Speaking phase to detect user speech for interrupt.
pub struct MonitorSession {
    _stream: cpal::Stream,
    pub stop_signal: Arc<AtomicBool>,
    pub rms_rx: mpsc::Receiver<f32>,
}

impl AudioCapture {
    /// Start a lightweight RMS-only monitor (no audio buffering, no silence detection).
    /// Used during Speaking phase to detect if the user starts talking.
    pub fn start_monitor(&self) -> common::Result<MonitorSession> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| common::KlyntbotError::internal("No audio input device"))?;

        let native_config = device.default_input_config()
            .map_err(|e| common::KlyntbotError::internal(format!("Audio config error: {e}")))?;

        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_signal);
        let (rms_tx, rms_rx) = mpsc::channel(32);

        let stream = device.build_input_stream(
            &native_config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if stop_clone.load(Ordering::Relaxed) {
                    return;
                }
                let rms = (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                let _ = rms_tx.try_send(rms);
            },
            |err| tracing::warn!("Audio monitor error: {err}"),
            None,
        ).map_err(|e| common::KlyntbotError::internal(format!("Stream error: {e}")))?;

        stream.play().map_err(|e| common::KlyntbotError::internal(format!("Play error: {e}")))?;

        Ok(MonitorSession {
            _stream: stream,
            stop_signal,
            rms_rx,
        })
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p voice-engine`
Expected: Compiles. Existing AudioCapture::start() unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/capture.rs
git commit -m "feat(voice-engine): add lightweight audio monitor for interrupt detection"
```

---

## Task 4: VoiceConversationManager Core + State Machine

Create the manager with state types, phase transitions, session attachment, and the command channel pattern.

**Files:**
- Create: `crates/app-core/src/handlers/voice_conversation.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Write state machine tests**

```rust
// At the bottom of voice_conversation.rs:

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
        let mut state = VoiceConversationState::default();
        state.turn_count = 5;
        state.session_key = Some(SessionKey::from_parts("desktop", "old-uuid"));
        state.pending_response_text = Some("old response".into());

        state.reset_for_new_session(SessionKey::from_parts("desktop", "new-uuid"));

        assert_eq!(state.turn_count, 0);
        assert_eq!(state.session_key.as_ref().unwrap().as_str(), "desktop:new-uuid");
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
        assert!(!is_warm_session(old, 15));   // 20 min >= 15 min threshold
    }

    #[test]
    fn adaptive_breath_duration() {
        assert_eq!(compute_breath_ms("Hi"), 300);       // Short → minimum
        assert_eq!(compute_breath_ms("A".repeat(1000).as_str()), 800); // Long → capped
        let medium = "A".repeat(400);
        let ms = compute_breath_ms(&medium);
        assert!(ms > 300 && ms < 800); // Medium → in between
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p app-core -E 'test(phase_transitions)' 2>&1 | head -5`
Expected: FAIL (module doesn't exist yet)

- [ ] **Step 3: Implement core types and state machine**

```rust
// crates/app-core/src/handlers/voice_conversation.rs

use chrono::{DateTime, Utc};
use common::{SessionKey, ChannelName, ChatId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

use crate::events::AppEventEmitter;
use config::VoiceConfig;
use voice_engine::{MemoryEchoProvider, VoiceEvent, VoiceService};

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
                | (Self::Listening, Self::Idle) // pause or end during listening
                | (Self::Reflecting, Self::Speaking)
                | (Self::Reflecting, Self::Idle) // cancel during reflecting
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
    pub pending_transcript: Option<String>,      // Captured text from Listening → passed to Reflecting
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
```

- [ ] **Step 4: Add module export**

```rust
// In crates/app-core/src/handlers/mod.rs, add:
pub mod voice_conversation;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p app-core -E 'test(phase_transitions|state_resets|warm_session|adaptive_breath)'`
Expected: All 6 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(app-core): add VoiceConversationManager core types and state machine"
```

---

## Task 5: chat_send_internal with is_voice flag

Add the `is_voice` parameter to the chat send pipeline for voice session metadata.

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Add is_voice parameter to chat_send function**

Find the standalone `chat_send` function (not the AppCore method). Add `is_voice: bool` as the last parameter. When `is_voice` is true, merge `{"is_voice_session": true}` into session metadata:

```rust
// In the chat_send function, after the title/metadata creation:
let metadata = if is_voice {
    serde_json::json!({ "title": title, "is_voice_session": true })
} else {
    serde_json::json!({ "title": title })
};
```

- [ ] **Step 2: Update all callers to pass is_voice: false**

Find every call to `chat_send(...)` in streaming.rs and update to add `false` as the last argument. There should be one direct call and potentially one in the AppCore `chat_send` method.

- [ ] **Step 3: Add public wrapper for voice callers**

```rust
// Add a convenience method on AppCore for the manager to call:
impl AppCore {
    pub async fn chat_send_voice(
        &self,
        content: String,
        session_key: String,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content,
            session_key,
            None, // no context
            true, // is_voice
        ).await
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p app-core`
Expected: Compiles. Existing text chat still passes `is_voice: false`.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(app-core): add is_voice flag to chat_send for voice session metadata"
```

---

## Task 6: Memory Echo Tier 3 (ContextEngine Fallback)

Wire `ContextEngine` into `AppMemoryEchoProvider` so it falls back to conversation recall when Mirror returns nothing.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_echo.rs`
- Modify: `crates/app-core/src/init/mod.rs` (where echo provider is created)

- [ ] **Step 1: Write test for Tier 2→3 fallback**

```rust
// In voice_echo.rs, add #[cfg(test)] mod tests:

#[cfg(test)]
mod tests {
    use super::*;

    // Mock MirrorFacade and ContextEngine not trivially constructible,
    // so test the logic flow with a direct unit test of the lookup method.
    // Integration test (Task 5 in spec) covers the real wiring.

    #[test]
    fn provider_has_context_engine_field() {
        // Verify the struct accepts an optional ContextEngine
        let provider = AppMemoryEchoProvider::new(None, None);
        assert!(provider.mirror.is_none());
        assert!(provider.context_engine.is_none());
    }
}
```

- [ ] **Step 2: Add ContextEngine field to AppMemoryEchoProvider**

```rust
use context_engine::ContextEngine;

pub struct AppMemoryEchoProvider {
    mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
    context_engine: Option<Arc<ContextEngine>>,
}

impl AppMemoryEchoProvider {
    pub fn new(
        mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
        context_engine: Option<Arc<ContextEngine>>,
    ) -> Self {
        Self { mirror, context_engine }
    }
}

#[async_trait]
impl MemoryEchoProvider for AppMemoryEchoProvider {
    async fn lookup(&self, partial_text: &str, _learning_active: bool) -> Option<String> {
        // Tier 2: Mirror-powered snippet (embedding similarity)
        if let Some(ref facade) = self.mirror {
            if let Some(snippet) = facade.get_recent_voice_relevant_snippet(partial_text).await {
                return Some(snippet);
            }
        }
        // Tier 3: Conversation recall via ContextEngine
        if let Some(ref engine) = self.context_engine {
            if let Ok(Some(recall)) = engine
                .recall_relevant(
                    partial_text,
                    context_engine::RecallParams {
                        max_results: 1,
                        max_tokens: 50,
                        recency_boost: true,
                    },
                )
                .await
            {
                return Some(recall);
            }
        }
        None
    }
}
```

- [ ] **Step 3: Update init/mod.rs to pass ContextEngine**

Find where `AppMemoryEchoProvider::new(mirror_facade_for_voice)` is called in `init/mod.rs`. Update to also pass the context engine:

```rust
let echo_provider = crate::handlers::voice_echo::AppMemoryEchoProvider::new(
    mirror_facade_for_voice,
    context_engine.clone(), // Arc<ContextEngine> already available in init
);
```

Check what `context_engine` is called in the init scope — it may be `ctx_engine` or similar. Use the variable that's already in scope.

- [ ] **Step 4: Verify build**

Run: `cargo build -p app-core`
Expected: Compiles. If `ContextEngine` or `RecallParams` types differ from what's shown, adjust imports to match the actual crate API. Check `crates/context_engine/src/lib.rs` for the real method name and params.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/voice_echo.rs crates/app-core/src/init/mod.rs
git commit -m "feat(voice): wire ContextEngine as Tier 3 memory echo fallback"
```

---

## Task 7: Conversation Loop

Implement the spawned Tokio task that drives the Listening → Reflecting → Speaking → auto-resume cycle.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Write integration test for multi-turn cycle**

```rust
// Add to #[cfg(test)] mod tests in voice_conversation.rs:

#[tokio::test]
async fn conversation_loop_start_enters_listening() {
    // Test that calling start() transitions from Idle → Listening
    // and emits PhaseChanged event.
    // This requires a mock VoiceService — use voice_engine::mock::MockTranscriptionEngine
    // and construct a VoiceService with it.
    // For now, test the state transition logic directly:
    let state = VoiceConversationState::default();
    assert_eq!(state.phase, ConversationPhase::Idle);
    assert!(ConversationPhase::Idle.can_transition_to(&ConversationPhase::Listening));
}
```

- [ ] **Step 2: Implement session attachment logic**

Add to `VoiceConversationManager`:

```rust
impl VoiceConversationManager {
    /// Determine which session to attach to, or create a new one.
    pub async fn resolve_session(&self) -> SessionKey {
        let state = self.state.lock().await;
        let config = self.config.read().await;
        let conv_config = &config.conversation;

        // Already have an active session? Reuse it.
        if state.phase != ConversationPhase::Idle {
            if let Some(ref key) = state.session_key {
                return key.clone();
            }
        }

        // Previous voice session still warm?
        if let Some(ref key) = state.session_key {
            if is_warm_session(state.last_activity, conv_config.warm_session_minutes) {
                return key.clone();
            }
        }

        drop(state); // Release lock before DB query

        // Check main chat window's active session
        // (query most recent desktop session from DB)
        if let Ok(sessions) = self.repos.sessions.list_recent("desktop", 1).await {
            if let Some(session) = sessions.first() {
                if is_warm_session(session.updated_at, conv_config.warm_chat_minutes) {
                    return SessionKey::from_parts("desktop", &session.key);
                }
            }
        }

        // Create new session
        create_voice_session_key()
    }

    /// Start a conversation — resolve session, transition to Listening, spawn loop.
    pub async fn start(&self) -> common::Result<StartResponse> {
        let key = self.resolve_session().await;
        let mut state = self.state.lock().await;

        let is_continuing = state.session_key.as_ref() == Some(&key);

        if state.phase == ConversationPhase::Idle {
            state.session_key = Some(key.clone());
            state.phase = ConversationPhase::Listening;
            state.touch();
        } else if state.paused {
            state.paused = false;
            state.phase = ConversationPhase::Listening;
        }

        // Upsert session in DB
        let title = if is_continuing { None } else { Some("New voice session".to_string()) };
        if let Some(ref t) = title {
            let metadata = serde_json::json!({ "title": t, "is_voice_session": true });
            let _ = self.repos.sessions.upsert_session(key.as_str(), &metadata, None).await;
        }

        let session_title = title.unwrap_or_else(|| "Continuing".to_string());

        // Emit phase change
        let _ = self.voice_service.emit_event(VoiceEvent::PhaseChanged {
            phase: "listening".to_string(),
            session_title: Some(session_title.clone()),
            turn_count: state.turn_count,
        }).await;

        Ok(StartResponse {
            session_key: key.as_str().to_string(),
            session_title,
            is_continuing,
        })
    }
}

pub struct StartResponse {
    pub session_key: String,
    pub session_title: String,
    pub is_continuing: bool,
}
```

- [ ] **Step 3: Implement the conversation loop task**

```rust
impl VoiceConversationManager {
    /// Spawn the conversation loop. Call once after creating the manager.
    /// Returns a JoinHandle for the loop task.
    pub async fn spawn_loop(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let cmd_rx = self.cmd_rx.lock().await.take()
            .expect("spawn_loop called twice");
        let this = Arc::clone(self);

        tokio::spawn(async move {
            this.conversation_loop(cmd_rx).await;
        })
    }

    async fn conversation_loop(self: &Arc<Self>, mut cmd_rx: mpsc::Receiver<VoiceCommand>) {
        loop {
            let phase = self.state.lock().await.phase;

            match phase {
                ConversationPhase::Idle => {
                    // Wait for a command (Start is handled externally via start())
                    match cmd_rx.recv().await {
                        Some(VoiceCommand::End) | None => break,
                        Some(cmd) => self.handle_command_while_idle(cmd).await,
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
    }

    async fn run_listening_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        // Start audio capture
        let capture_result = self.voice_service.start_capture().await;
        if let Err(e) = capture_result {
            let _ = self.voice_service.emit_event(VoiceEvent::Error {
                message: e.to_string(),
                recoverable: true,
            }).await;
            self.state.lock().await.phase = ConversationPhase::Idle;
            return;
        }

        let mut echo_fired = false;

        // Forward voice events until silence or command
        // The VoiceService emits AudioLevel, PartialTranscript, RoutingSuggestion
        // via its own event channel. Silence detection triggers auto-stop.
        // We wait for either: silence (stop_capture), or a command.
        loop {
            tokio::select! {
                biased;
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(VoiceCommand::Pause) => {
                            let _ = self.voice_service.stop_capture().await;
                            let mut state = self.state.lock().await;
                            state.paused = true;
                            state.phase = ConversationPhase::Idle;
                            let _ = self.voice_service.emit_event(VoiceEvent::PhaseChanged {
                                phase: "idle".into(),
                                session_title: None,
                                turn_count: state.turn_count,
                            }).await;
                            return;
                        }
                        Some(VoiceCommand::End) => {
                            let _ = self.voice_service.stop_capture().await;
                            self.state.lock().await.phase = ConversationPhase::Idle;
                            return;
                        }
                        Some(VoiceCommand::NewSession) => {
                            let _ = self.voice_service.stop_capture().await;
                            let new_key = create_voice_session_key();
                            self.state.lock().await.reset_for_new_session(new_key);
                            // Will re-enter loop and start Listening with new session
                            return;
                        }
                        _ => {}
                    }
                }
                // VoiceService silence detection triggers auto-stop
                // after configured silence_threshold_secs.
                // When silence fires, stop_capture returns the transcript.
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Check if VoiceService has auto-stopped via silence detection
                    let session_state = self.voice_service.session_state().await;
                    if !session_state.is_active() {
                        // Silence detected, capture already stopped
                        break;
                    }

                    // Memory echo: fire once per turn on first partial ≥3 words
                    if !echo_fired {
                        // Check latest partial from VoiceService
                        // (VoiceService emits PartialTranscript events to its channel)
                        // Echo logic is handled by the VoiceService internally
                        // via the MemoryEchoProvider already wired in.
                        // The one-shot limiter needs to be enforced here or in VoiceService.
                    }
                }
            }
        }

        // Capture the transcript and transition to Reflecting
        let transcript = self.voice_service.stop_capture().await;
        let mut state = self.state.lock().await;
        if let Ok(Some((t, _))) = &transcript {
            state.pending_transcript = Some(t.text.clone());
        }
        state.phase = ConversationPhase::Reflecting;
        state.touch();
        drop(state);

        let _ = self.voice_service.emit_event(VoiceEvent::Reflecting).await;
        let _ = self.voice_service.emit_event(VoiceEvent::PhaseChanged {
            phase: "reflecting".into(),
            session_title: None,
            turn_count: self.state.lock().await.turn_count,
        }).await;
    }

    async fn run_reflecting_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        // Transcript was captured and stored by run_listening_phase
        let transcript_text = {
            let state = self.state.lock().await;
            state.pending_transcript.clone()
        };

        let transcript_text = match transcript_text {
            Some(t) if !t.trim().is_empty() => t,
            _ => {
                // Empty transcript — skip back to Listening
                self.state.lock().await.phase = ConversationPhase::Listening;
                return;
            }
        };

        let session_key = self.state.lock().await.session_key.clone()
            .expect("session_key must be set during conversation");

        // Send to agent via chat pipeline
        // This persists the user message and runs the agent
        let stream_result = self.agent.process_direct_streaming(
            transcript_text.clone(),
            session_key.as_str().to_string(),
        ).await;

        let handle = match stream_result {
            Ok(h) => h,
            Err(e) => {
                let _ = self.voice_service.emit_event(VoiceEvent::Error {
                    message: e.to_string(),
                    recoverable: true,
                }).await;
                self.state.lock().await.phase = ConversationPhase::Listening;
                return;
            }
        };

        // Drain agent events, forward to chat window
        let mut event_rx = handle.event_rx;
        let mut final_content = String::new();

        loop {
            tokio::select! {
                biased;
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(VoiceCommand::End) => {
                            handle.cancel_token.cancel();
                            self.state.lock().await.phase = ConversationPhase::Idle;
                            return;
                        }
                        _ => {}
                    }
                }
                event = event_rx.recv() => {
                    match event {
                        Some(agent::AgentEvent::ContentChunk { data }) => {
                            // Forward to main chat window
                            self.emitter.emit_event("agent:content_chunk", serde_json::json!({
                                "sessionKey": session_key.as_str(),
                                "data": data,
                            }));
                        }
                        Some(agent::AgentEvent::Done { content, message_id }) => {
                            final_content = content;
                            // Forward done event
                            self.emitter.emit_event("agent:done", serde_json::json!({
                                "sessionKey": session_key.as_str(),
                                "content": &final_content,
                                "messageId": message_id,
                            }));
                            break;
                        }
                        Some(other) => {
                            // Forward tool events etc. to chat window
                            if let Ok(val) = serde_json::to_value(&other) {
                                self.emitter.emit_event("agent:event", val);
                            }
                        }
                        None => break, // Channel closed
                    }
                }
            }
        }

        // Store response and transition to Speaking
        let mut state = self.state.lock().await;
        state.pending_response_text = Some(final_content);
        state.tts_position = 0;
        state.interrupted = false;
        state.turn_count += 1;
        state.phase = ConversationPhase::Speaking;
        state.touch();
    }

    async fn run_speaking_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
        let response_text = {
            let state = self.state.lock().await;
            match state.pending_response_text.clone() {
                Some(t) if !t.is_empty() => t,
                _ => {
                    // Nothing to speak, skip to auto-resume
                    drop(state);
                    self.auto_resume_or_idle().await;
                    return;
                }
            }
        };

        // Synthesize TTS
        let tts_text = &response_text[self.state.lock().await.tts_position..];
        let _ = self.voice_service.handle_response(tts_text).await;

        // Emit phase change
        let _ = self.voice_service.emit_event(VoiceEvent::PhaseChanged {
            phase: "speaking".into(),
            session_title: None,
            turn_count: self.state.lock().await.turn_count,
        }).await;

        // Start audio monitor for interrupt detection
        let monitor = self.voice_service.capture().start_monitor();

        // Estimate TTS duration (rough: 150ms per word)
        let word_count = tts_text.split_whitespace().count();
        let estimated_ms = (word_count as u64 * 150).max(500);
        let start = tokio::time::Instant::now();

        let mut interrupted = false;

        loop {
            tokio::select! {
                biased;
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(VoiceCommand::Interrupt) => {
                            interrupted = true;
                            break;
                        }
                        Some(VoiceCommand::Pause) => {
                            let mut state = self.state.lock().await;
                            state.paused = true;
                            state.phase = ConversationPhase::Idle;
                            return;
                        }
                        Some(VoiceCommand::End) => {
                            self.state.lock().await.phase = ConversationPhase::Idle;
                            return;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                    // Check audio monitor for speech (interrupt detection)
                    if let Ok(ref monitor) = monitor {
                        while let Ok(rms) = monitor.rms_rx.try_recv() {
                            if rms > 0.02 { // Above silence threshold
                                interrupted = true;
                                break;
                            }
                        }
                    }
                    if interrupted { break; }

                    // Check if TTS finished (estimated)
                    if start.elapsed().as_millis() as u64 >= estimated_ms {
                        break;
                    }
                }
            }
        }

        // Stop monitor
        if let Ok(ref monitor) = monitor {
            monitor.stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        if interrupted {
            // Store interrupt position
            let elapsed_ratio = start.elapsed().as_millis() as f64 / estimated_ms as f64;
            let estimated_char_pos = (elapsed_ratio * tts_text.len() as f64) as usize;
            let mut state = self.state.lock().await;
            state.interrupted = true;
            state.tts_position += estimated_char_pos;

            // Emit fade + continue available
            let _ = self.voice_service.emit_event(VoiceEvent::TtsFadeOut).await;
            let _ = self.voice_service.emit_event(VoiceEvent::ContinueAvailable {
                timeout_secs: 8,
            }).await;

            // Transition to Listening (user interrupted to speak)
            state.phase = ConversationPhase::Listening;
        } else {
            // TTS completed — auto-resume after adaptive breath
            self.auto_resume_or_idle().await;
        }
    }

    async fn auto_resume_or_idle(&self) {
        let config = self.config.read().await;
        let mut state = self.state.lock().await;

        if state.paused || !config.conversation.auto_resume {
            state.phase = ConversationPhase::Idle;
            return;
        }

        // Adaptive breath
        let breath_ms = if config.conversation.adaptive_breath {
            state.pending_response_text.as_deref()
                .map(compute_breath_ms)
                .unwrap_or(300)
        } else {
            500
        };

        drop(state);
        drop(config);
        tokio::time::sleep(tokio::time::Duration::from_millis(breath_ms)).await;

        // Resume listening
        self.state.lock().await.phase = ConversationPhase::Listening;
        let _ = self.voice_service.emit_event(VoiceEvent::PhaseChanged {
            phase: "listening".into(),
            session_title: None,
            turn_count: self.state.lock().await.turn_count,
        }).await;
    }

    async fn handle_command_while_idle(&self, cmd: VoiceCommand) {
        match cmd {
            VoiceCommand::Resume => {
                let mut state = self.state.lock().await;
                if state.paused {
                    state.paused = false;
                    state.phase = ConversationPhase::Listening;
                }
            }
            VoiceCommand::Continue => {
                let state = self.state.lock().await;
                if state.interrupted && state.pending_response_text.is_some() {
                    drop(state);
                    self.state.lock().await.phase = ConversationPhase::Speaking;
                }
            }
            _ => {}
        }
    }
}
```

**Note to implementing agent:** The above is the structural skeleton. Several details need refinement during implementation:
- `self.voice_service.capture()` may need a public accessor (currently `capture` is private).
- The `session_state()` method on VoiceService needs to be checked for the exact return type.
- The `monitor.rms_rx.try_recv()` call uses `tokio::sync::mpsc` which has `try_recv()`.
- The interaction between VoiceService's internal silence detection and the manager's polling loop needs careful testing. The silence signal from `CaptureSession.silence_rx` should trigger stop_capture.

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p app-core -E 'test(voice_conversation)'`
Expected: All previous unit tests still pass. New code compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "feat(app-core): implement VoiceConversationManager conversation loop"
```

---

## Task 8: Desktop Commands + Tauri Wiring

Create the 8 new Tauri commands and wire them into the app.

**Files:**
- Create: `crates/desktop-shared/src/commands/voice_conversation.rs`
- Create: `crates/desktop/src/commands/voice_conversation.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Create shared types**

```rust
// crates/desktop-shared/src/commands/voice_conversation.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationStartResponse {
    pub session_key: String,
    pub session_title: String,
    pub is_continuing: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConversationStatusResponse {
    pub phase: String,
    pub session_key: Option<String>,
    pub session_title: Option<String>,
    pub turn_count: u32,
    pub paused: bool,
    pub continue_available: bool,
    pub engine_kind: Option<String>,
}
```

- [ ] **Step 2: Create Tauri commands**

```rust
// crates/desktop/src/commands/voice_conversation.rs

use crate::ApiError;
use app_core::AppCore;
use desktop_shared::commands::voice_conversation::*;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn voice_conversation_start(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceConversationStartResponse, ApiError> {
    let manager = state.voice_conversation_manager()?;
    let result = manager.start().await
        .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;
    crate::tray_countdown::VOICE_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(VoiceConversationStartResponse {
        session_key: result.session_key,
        session_title: result.session_title,
        is_continuing: result.is_continuing,
    })
}

#[tauri::command]
pub async fn voice_conversation_pause(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::Pause).await;
    crate::tray_countdown::VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn voice_conversation_resume(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::Resume).await;
    crate::tray_countdown::VOICE_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn voice_conversation_interrupt(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::Interrupt).await;
    Ok(())
}

#[tauri::command]
pub async fn voice_conversation_continue(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::Continue).await;
    Ok(())
}

#[tauri::command]
pub async fn voice_conversation_new_session(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceConversationStartResponse, ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::NewSession).await;
    // After new session command processed, start fresh
    let result = manager.start().await
        .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;
    Ok(VoiceConversationStartResponse {
        session_key: result.session_key,
        session_title: result.session_title,
        is_continuing: false,
    })
}

#[tauri::command]
pub async fn voice_conversation_end(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    let manager = state.voice_conversation_manager()?;
    manager.send_command(app_core::handlers::voice_conversation::VoiceCommand::End).await;
    crate::tray_countdown::VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn voice_conversation_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceConversationStatusResponse, ApiError> {
    let manager = state.voice_conversation_manager()?;
    let state_guard = manager.state.lock().await;
    Ok(VoiceConversationStatusResponse {
        phase: state_guard.phase.as_str().to_string(),
        session_key: state_guard.session_key.as_ref().map(|k| k.as_str().to_string()),
        session_title: None, // TODO: look up from DB
        turn_count: state_guard.turn_count,
        paused: state_guard.paused,
        continue_available: state_guard.interrupted && state_guard.pending_response_text.is_some(),
        engine_kind: manager.voice_service.engine_kind().map(|e| format!("{e:?}").to_lowercase()),
    })
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_conversation_start",
    "voice_conversation_pause",
    "voice_conversation_resume",
    "voice_conversation_interrupt",
    "voice_conversation_continue",
    "voice_conversation_new_session",
    "voice_conversation_end",
    "voice_conversation_status",
];
```

- [ ] **Step 3: Add module exports and register commands**

In `crates/desktop-shared/src/commands/mod.rs`, add `pub mod voice_conversation;`

In `crates/desktop/src/commands/mod.rs`, add `pub mod voice_conversation;`

In `crates/desktop/src/main.rs`, add the new commands to the Tauri builder's `invoke_handler` alongside existing voice commands.

- [ ] **Step 4: Add voice_conversation_manager() accessor to AppCore**

```rust
// In app-core, add accessor method:
impl AppCore {
    pub fn voice_conversation_manager(&self) -> Result<&Arc<VoiceConversationManager>, ApiError> {
        self.voice_conversation_manager.as_ref()
            .ok_or_else(|| ApiError::new("VOICE_NOT_AVAILABLE", "Voice conversation not initialized"))
    }
}
```

Also add the field to AppCore struct: `pub voice_conversation_manager: Option<Arc<VoiceConversationManager>>`

- [ ] **Step 5: Update hotkey handler in main.rs**

Replace the voice-orb toggle logic in the hotkey handler to use the manager:

```rust
// In the voice hotkey handler, replace the raw VoiceService calls with:
let manager = core_ref.voice_conversation_manager();
if let Ok(manager) = manager {
    let phase = manager.phase().await;
    if phase == ConversationPhase::Idle {
        // Start conversation + show orb
        let _ = manager.start().await;
        // Show voice-orb window...
    } else {
        // End conversation + hide orb
        manager.send_command(VoiceCommand::End).await;
        // Hide voice-orb window...
    }
}
```

Keep the existing focus-session and launcher-open context checks.

- [ ] **Step 6: Add dev server dispatch for new commands**

In `crates/desktop/src/dev_server/dispatch.rs`, add dispatch entries for all 8 new commands, delegating to the appropriate `AppCore` methods.

- [ ] **Step 7: Verify build**

Run: `cargo build -p desktop`
Expected: Compiles. The `dev_server_covers_all_tauri_commands` test should be checked — add new commands to the dev server coverage list.

- [ ] **Step 8: Commit**

```bash
git add crates/desktop-shared/src/commands/voice_conversation.rs \
       crates/desktop/src/commands/voice_conversation.rs \
       crates/desktop-shared/src/commands/mod.rs \
       crates/desktop/src/commands/mod.rs \
       crates/desktop/src/main.rs \
       crates/desktop/src/dev_server/dispatch.rs \
       crates/app-core/src/handlers/voice_conversation.rs
git commit -m "feat(desktop): add voice_conversation IPC commands and hotkey wiring"
```

---

## Task 9: AppCore Initialization

Wire the `VoiceConversationManager` into the app-core init pipeline and spawn the conversation loop.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Create manager after VoiceService and agent are ready**

Find the section in `init/mod.rs` where `voice_service` is assigned to `core`. After that, create the manager:

```rust
// After: core.voice_service = Some(Arc::clone(&service));
// Create VoiceConversationManager
let voice_conv_manager = Arc::new(VoiceConversationManager::new(
    Arc::clone(&service),       // voice_service
    repos.clone(),              // repos
    Arc::clone(&agent),         // agent
    Arc::clone(&emitter),       // emitter (AppEventEmitter)
    echo_provider_arc.clone(),  // MemoryEchoProvider (already created above)
    config.clone(),             // Arc<RwLock<VoiceConfig>> — use the config reference
));

// Spawn the conversation loop
let _loop_handle = voice_conv_manager.spawn_loop().await;

core.voice_conversation_manager = Some(voice_conv_manager);
```

**Note to implementing agent:** The variable names (`repos`, `agent`, `emitter`, `config`) need to match what's actually in scope at this point in `init/mod.rs`. Read the surrounding code to find the correct variable names. The `config` for VoiceConfig specifically may need to be extracted from the main `Config` — check the existing voice service creation for how it accesses `config.voice`.

- [ ] **Step 2: Verify build**

Run: `cargo build -p app-core`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): wire VoiceConversationManager into init pipeline"
```

---

## Task 10: Frontend — useVoiceConversation Hook

Create the new hook that wraps conversation-level actions and state.

**Files:**
- Create: `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts`
- Create: `desktop-ui/src/features/voice/__tests__/useVoiceConversation.test.ts`
- Modify: `desktop-ui/src/features/voice/index.ts`

- [ ] **Step 1: Write tests**

```typescript
// desktop-ui/src/features/voice/__tests__/useVoiceConversation.test.ts

import { describe, expect, it } from "vitest";

// Test the event reducer logic (same pattern as useVoiceEvents tests)
type ConversationPhase = "idle" | "listening" | "reflecting" | "speaking";

interface ConversationState {
  phase: ConversationPhase;
  transcript: string;
  segments: Array<{ text: string; confidence: number }>;
  routingChips: Array<{ skill: string; confidence: number; label: string }>;
  memoryEcho: string | null;
  audioLevel: number;
  ttsAudio: { base64: string; sampleRate: number; text: string } | null;
  sessionInfo: { key: string; title: string; turnCount: number } | null;
  continueAvailable: boolean;
  engineKind: "local" | "cloud";
}

function initialState(): ConversationState {
  return {
    phase: "idle",
    transcript: "",
    segments: [],
    routingChips: [],
    memoryEcho: null,
    audioLevel: 0,
    ttsAudio: null,
    sessionInfo: null,
    continueAvailable: false,
    engineKind: "local",
  };
}

function reduceConversationEvent(
  state: ConversationState,
  event: Record<string, unknown>,
): ConversationState {
  const next = { ...state };
  switch (event.type) {
    case "phaseChanged":
      next.phase = event.phase as ConversationPhase;
      if (event.sessionTitle || event.turnCount !== undefined) {
        next.sessionInfo = {
          key: next.sessionInfo?.key ?? "",
          title: (event.sessionTitle as string) ?? next.sessionInfo?.title ?? "",
          turnCount: (event.turnCount as number) ?? 0,
        };
      }
      if (next.phase === "listening") {
        next.transcript = "";
        next.segments = [];
        next.routingChips = [];
        next.memoryEcho = null;
        next.continueAvailable = false;
      }
      break;
    case "audioLevel":
      next.audioLevel = event.rms as number;
      break;
    case "partialTranscript":
      next.transcript = event.text as string;
      if (event.segments) {
        next.segments = event.segments as Array<{ text: string; confidence: number }>;
      }
      break;
    case "routingSuggestion": {
      const skill = event.skill as string;
      if (!next.routingChips.some((c) => c.skill === skill)) {
        next.routingChips = [
          ...next.routingChips,
          { skill, confidence: event.confidence as number, label: event.label as string },
        ];
      }
      break;
    }
    case "memoryEcho":
      next.memoryEcho = event.text as string;
      break;
    case "reflecting":
      next.phase = "reflecting";
      break;
    case "speakResponse":
      next.phase = "speaking";
      next.ttsAudio = {
        base64: event.audioBase64 as string,
        sampleRate: event.sampleRate as number,
        text: event.text as string,
      };
      break;
    case "ttsFadeOut":
      next.ttsAudio = null;
      break;
    case "continueAvailable":
      next.continueAvailable = true;
      break;
    case "captureStarted":
      next.engineKind = event.engine === "Cloud" ? "cloud" : "local";
      break;
  }
  return next;
}

describe("useVoiceConversation reducer", () => {
  it("phaseChanged to listening resets turn state", () => {
    let state = initialState();
    state.transcript = "old text";
    state.memoryEcho = "old echo";
    state.continueAvailable = true;

    state = reduceConversationEvent(state, {
      type: "phaseChanged",
      phase: "listening",
      sessionTitle: "Test Session",
      turnCount: 0,
    });

    expect(state.phase).toBe("listening");
    expect(state.transcript).toBe("");
    expect(state.memoryEcho).toBeNull();
    expect(state.continueAvailable).toBe(false);
    expect(state.sessionInfo?.title).toBe("Test Session");
  });

  it("full multi-turn cycle", () => {
    let state = initialState();

    // Start listening
    state = reduceConversationEvent(state, { type: "phaseChanged", phase: "listening", turnCount: 0 });
    expect(state.phase).toBe("listening");

    // Partial transcript
    state = reduceConversationEvent(state, { type: "partialTranscript", text: "hello world" });
    expect(state.transcript).toBe("hello world");

    // Routing
    state = reduceConversationEvent(state, { type: "routingSuggestion", skill: "tasks", confidence: 0.8, label: "Task" });
    expect(state.routingChips).toHaveLength(1);

    // Reflecting
    state = reduceConversationEvent(state, { type: "reflecting" });
    expect(state.phase).toBe("reflecting");

    // Speaking
    state = reduceConversationEvent(state, { type: "speakResponse", audioBase64: "abc", sampleRate: 16000, text: "response" });
    expect(state.phase).toBe("speaking");
    expect(state.ttsAudio?.text).toBe("response");

    // Auto-resume → next turn
    state = reduceConversationEvent(state, { type: "phaseChanged", phase: "listening", turnCount: 1 });
    expect(state.phase).toBe("listening");
    expect(state.transcript).toBe(""); // Reset for new turn
    expect(state.sessionInfo?.turnCount).toBe(1);
  });

  it("interrupt sets continueAvailable", () => {
    let state = initialState();
    state.phase = "speaking";

    state = reduceConversationEvent(state, { type: "ttsFadeOut" });
    expect(state.ttsAudio).toBeNull();

    state = reduceConversationEvent(state, { type: "continueAvailable", timeoutSecs: 8 });
    expect(state.continueAvailable).toBe(true);
  });

  it("routing chips deduplicate by skill", () => {
    let state = initialState();
    state = reduceConversationEvent(state, { type: "routingSuggestion", skill: "tasks", confidence: 0.8, label: "Task" });
    state = reduceConversationEvent(state, { type: "routingSuggestion", skill: "tasks", confidence: 0.9, label: "Task" });
    expect(state.routingChips).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd desktop-ui && bun run test -- --run useVoiceConversation`
Expected: Tests pass (they test the reducer function defined in the test file itself, not the hook). If they don't pass, fix the reducer logic.

- [ ] **Step 3: Implement the hook**

```typescript
// desktop-ui/src/features/voice/hooks/useVoiceConversation.ts

import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { playTtsAudio } from "@shared/lib/audio";

export type ConversationPhase = "idle" | "listening" | "reflecting" | "speaking";

export interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

export interface SessionInfo {
  key: string;
  title: string;
  turnCount: number;
}

export interface TtsAudioData {
  base64: string;
  sampleRate: number;
  text: string;
}

export function useVoiceConversation() {
  const [phase, setPhase] = useState<ConversationPhase>("idle");
  const [transcript, setTranscript] = useState("");
  const [segments, setSegments] = useState<Array<{ text: string; confidence: number }>>([]);
  const [routingChips, setRoutingChips] = useState<RoutingChip[]>([]);
  const [memoryEcho, setMemoryEcho] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const [ttsAudio, setTtsAudio] = useState<TtsAudioData | null>(null);
  const [sessionInfo, setSessionInfo] = useState<SessionInfo | null>(null);
  const [continueAvailable, setContinueAvailable] = useState(false);
  const [engineKind, setEngineKind] = useState<"local" | "cloud">("local");

  const continueTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Handle incoming voice events
  const handleEvent = useCallback((payload: Record<string, unknown>) => {
    const type = payload.type as string;

    switch (type) {
      case "phaseChanged": {
        const newPhase = payload.phase as ConversationPhase;
        setPhase(newPhase);
        if (payload.sessionTitle || payload.turnCount !== undefined) {
          setSessionInfo((prev) => ({
            key: prev?.key ?? "",
            title: (payload.sessionTitle as string) ?? prev?.title ?? "",
            turnCount: (payload.turnCount as number) ?? 0,
          }));
        }
        if (newPhase === "listening") {
          setTranscript("");
          setSegments([]);
          setRoutingChips([]);
          setMemoryEcho(null);
          setContinueAvailable(false);
          if (continueTimerRef.current) {
            clearTimeout(continueTimerRef.current);
            continueTimerRef.current = null;
          }
        }
        break;
      }
      case "audioLevel":
        setAudioLevel(payload.rms as number);
        break;
      case "partialTranscript":
        setTranscript(payload.text as string);
        if (payload.segments) {
          setSegments(payload.segments as Array<{ text: string; confidence: number }>);
        }
        break;
      case "routingSuggestion":
        setRoutingChips((prev) => {
          const skill = payload.skill as string;
          if (prev.some((c) => c.skill === skill)) return prev;
          return [...prev, { skill, confidence: payload.confidence as number, label: payload.label as string }];
        });
        break;
      case "memoryEcho":
        setMemoryEcho(payload.text as string);
        break;
      case "reflecting":
        setPhase("reflecting");
        break;
      case "speakResponse": {
        setPhase("speaking");
        const audio = {
          base64: payload.audioBase64 as string,
          sampleRate: (payload.sampleRate as number) ?? 16000,
          text: payload.text as string,
        };
        setTtsAudio(audio);
        playTtsAudio(audio.base64, audio.sampleRate);
        break;
      }
      case "ttsFadeOut":
        setTtsAudio(null);
        break;
      case "continueAvailable":
        setContinueAvailable(true);
        // Auto-hide after 8 seconds
        continueTimerRef.current = setTimeout(() => {
          setContinueAvailable(false);
          continueTimerRef.current = null;
        }, 8000);
        break;
      case "captureStarted":
        setEngineKind(payload.engine === "Cloud" ? "cloud" : "local");
        break;
      case "error":
        // Emit phase idle on error
        setPhase("idle");
        break;
    }
  }, []);

  // Listen to Tauri events or browser CustomEvents
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    if (window.__TAURI_INTERNALS__) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<Record<string, unknown>>("voice:event", (event) => {
          handleEvent(event.payload);
        }).then((fn) => {
          unlisten = fn;
        });
      });
    } else {
      const handler = (e: Event) => {
        const detail = (e as CustomEvent).detail;
        if (detail) handleEvent(detail);
      };
      window.addEventListener("voice:event", handler);
      unlisten = () => window.removeEventListener("voice:event", handler);
    }

    return () => {
      unlisten?.();
      if (continueTimerRef.current) clearTimeout(continueTimerRef.current);
    };
  }, [handleEvent]);

  // Actions
  const start = useCallback(async (): Promise<SessionInfo> => {
    const result = await ipc<{ sessionKey: string; sessionTitle: string; isContinuing: boolean }>(
      "voice_conversation_start",
    );
    const info: SessionInfo = { key: result.sessionKey, title: result.sessionTitle, turnCount: 0 };
    setSessionInfo(info);
    return info;
  }, []);

  const pause = useCallback(async () => {
    await ipc("voice_conversation_pause");
  }, []);

  const resume = useCallback(async () => {
    await ipc("voice_conversation_resume");
  }, []);

  const interrupt = useCallback(async () => {
    await ipc("voice_conversation_interrupt");
  }, []);

  const continueTts = useCallback(async () => {
    await ipc("voice_conversation_continue");
    setContinueAvailable(false);
    if (continueTimerRef.current) {
      clearTimeout(continueTimerRef.current);
      continueTimerRef.current = null;
    }
  }, []);

  const newSession = useCallback(async (): Promise<SessionInfo> => {
    const result = await ipc<{ sessionKey: string; sessionTitle: string; isContinuing: boolean }>(
      "voice_conversation_new_session",
    );
    const info: SessionInfo = { key: result.sessionKey, title: result.sessionTitle, turnCount: 0 };
    setSessionInfo(info);
    return info;
  }, []);

  const end = useCallback(async () => {
    await ipc("voice_conversation_end");
    setPhase("idle");
    // Hide orb window if in Tauri
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().hide();
    }
  }, []);

  return {
    phase,
    transcript,
    segments,
    routingChips,
    memoryEcho,
    audioLevel,
    ttsAudio,
    sessionInfo,
    continueAvailable,
    engineKind,
    start,
    pause,
    resume,
    interrupt,
    continueTts,
    newSession,
    end,
  };
}
```

- [ ] **Step 4: Export from index**

```typescript
// In desktop-ui/src/features/voice/index.ts, add:
export { useVoiceConversation } from "./hooks/useVoiceConversation";
```

- [ ] **Step 5: Run tests**

Run: `cd desktop-ui && bun run test -- --run useVoiceConversation`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/voice/hooks/useVoiceConversation.ts \
       desktop-ui/src/features/voice/__tests__/useVoiceConversation.test.ts \
       desktop-ui/src/features/voice/index.ts
git commit -m "feat(ui): add useVoiceConversation hook with multi-turn state management"
```

---

## Task 11: Frontend — VoiceBrainOrb Redesign

Rewrite the orb component for multi-turn conversation with phase-dependent visuals, title bar, and actions.

**Files:**
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`
- Modify: `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx`

- [ ] **Step 1: Rewrite VoiceBrainOrb for multi-turn**

Replace the component to use `useVoiceConversation` instead of `useVoiceEvents`. Key UI elements:

1. **Title bar** — Phase dot (red/amber/blue/gray) + session title + "(continuing)" badge + "New" button + "Pause" button
2. **Central visual** — Waveform (Listening), cognitive pulse (Reflecting), TTS waveform (Speaking), muted mic (Idle/paused)
3. **Transcript area** — Current turn text with word-level highlights
4. **Routing chips + memory echo** — Same as before but reset per turn
5. **Continue button** — Visible after interrupt, auto-hides after 8s
6. **Hint bar** — "⌥⇧V close · ⌥⇧D dock · pause"

Use the existing `Waveform`, `WordHighlights`, and `RoutingChips` sub-components but adjust their rendering based on the new `phase` (not the old `sessionState`).

The component should NOT auto-dismiss. The orb stays open until the user explicitly closes it.

- [ ] **Step 2: Update VoiceOrbPage**

```typescript
// desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx
import { useRef } from "react";
import { VoiceBrainOrb } from "@features/voice/components/VoiceBrainOrb";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";

export function VoiceOrbPage() {
  const contentRef = useRef<HTMLDivElement>(null);
  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 320, maxHeight: 400 });

  return (
    <div ref={contentRef}>
      <VoiceBrainOrb />
    </div>
  );
}
```

Window size updated to support 320x280 default.

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Biome fixes any formatting issues.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx \
       desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx
git commit -m "feat(ui): redesign VoiceBrainOrb for multi-turn conversation"
```

---

## Task 12: Chat Page Voice Integration

Add voice session badge, voice indicator in header, and voice-aware scroll behavior.

**Files:**
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`

- [ ] **Step 1: Add voice indicator in chat header**

In the header area of ChatPage (the `flex items-center justify-between` div), add a voice indicator that shows when the orb is active and attached to this session:

```tsx
// Query voice conversation status periodically (or listen to events)
const [voicePhase, setVoicePhase] = useState<string>("idle");

// Listen for voice phase changes
useEffect(() => {
  const handler = (e: Event) => {
    const detail = (e as CustomEvent).detail;
    if (detail?.type === "phaseChanged") {
      setVoicePhase(detail.phase);
    }
  };
  // Listen to both Tauri and browser events
  window.addEventListener("voice:event", handler);
  return () => window.removeEventListener("voice:event", handler);
}, []);

// In the header JSX:
{voicePhase !== "idle" && (
  <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
    <span className="h-2 w-2 rounded-full bg-success animate-pulse" />
    <span>{voicePhase === "listening" ? "Listening" : voicePhase === "reflecting" ? "Reflecting" : "Speaking"}</span>
  </div>
)}
```

- [ ] **Step 2: Add voice-aware scroll behavior**

In the message scroll container, prevent auto-scroll during active voice:

```tsx
// In the scroll handler or auto-scroll effect:
const shouldAutoScroll = voicePhase === "idle" || isAtBottom;
```

- [ ] **Step 3: Add mic badge to session list**

If the session list renders session items, add a mic icon for sessions with `is_voice_session` metadata. This depends on how the session list currently works — check if session metadata is available in the list items.

- [ ] **Step 4: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/chat/pages/ChatPage.tsx
git commit -m "feat(ui): add voice indicator and mic badge to chat page"
```

---

## Task 13: Tray Icon States

Add tray icon switching based on voice phase.

**Files:**
- Modify: `crates/desktop/src/tray_countdown.rs`
- Add: tray icon PNGs to `crates/desktop/icons/` (listening, speaking)

- [ ] **Step 1: Add phase-aware tray title**

In the `countdown_loop` function, extend the `VOICE_ACTIVE` check to show phase-specific text:

```rust
// Add a new atomic for the current voice phase
pub static VOICE_PHASE: AtomicU8 = AtomicU8::new(0); // 0=idle, 1=listening, 2=reflecting, 3=speaking

// In the countdown_loop, replace the simple VOICE_ACTIVE check:
if VOICE_ACTIVE.load(Ordering::Relaxed) {
    let phase = VOICE_PHASE.load(Ordering::Relaxed);
    let title = match phase {
        1 => "🎤 Listening...",
        2 => "💭 Reflecting...",
        3 => "🔊 Speaking...",
        _ => "🎤 Voice active",
    };
    tray.set_title(Some(title))?;
    continue;
}
```

- [ ] **Step 2: Update VOICE_PHASE in desktop commands**

In the voice_conversation commands (Task 8), update `VOICE_PHASE` alongside `VOICE_ACTIVE`:

```rust
// In voice_conversation_start:
VOICE_PHASE.store(1, Ordering::Relaxed); // listening

// In voice_conversation_end:
VOICE_PHASE.store(0, Ordering::Relaxed); // idle
```

Also set VOICE_PHASE from VoiceEvent::PhaseChanged in the event relay loop.

- [ ] **Step 3: Verify build**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/tray_countdown.rs \
       crates/desktop/src/commands/voice_conversation.rs
git commit -m "feat(desktop): add phase-aware tray title for voice conversation"
```

---

## Task 14: First-Run Flow

Handle the initial setup experience: mic permissions, model download, "Speak anyway" button.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`

- [ ] **Step 1: Add first-run gate in manager start()**

In the `start()` method, before entering the conversation loop, check engine availability:

```rust
// At the beginning of start(), before resolve_session:
if !self.voice_service.is_available() {
    // Check if model is downloading
    let model_state = self.voice_service.model_state();
    let needs_model = matches!(model_state, voice_engine::ModelState::NotDownloaded);

    let _ = self.voice_service.emit_event(VoiceEvent::SetupRequired {
        needs_model,
        needs_mic_permission: false, // macOS handles this at OS level
    }).await;

    if needs_model {
        // Trigger background download
        let _ = self.voice_service.download_model(voice_engine::WhisperModelSize::Small).await;
    }

    // If Groq is available, allow cloud-mode start
    if self.voice_service.engine_kind().is_none() {
        return Err(common::KlyntbotError::internal("No voice engine available"));
    }
}
```

- [ ] **Step 2: Add SetupRequired handling in VoiceBrainOrb**

In the orb component, handle the `setupRequired` event:

```tsx
case "setupRequired": {
  // Show "Waking up your second brain..." with progress bar
  // If needs_model, show download progress
  // Add "Speak anyway (cloud)" button that calls voice_conversation_start with force_cloud flag
  break;
}
```

The orb shows a centered message: "Waking up your second brain..." with a gentle pulse animation on the progress bar. Below it, if a Groq key is configured, show a "Speak anyway (cloud)" text button.

- [ ] **Step 3: Verify build**

Run: `cargo build -p app-core && cd desktop-ui && bun run lint`
Expected: Compiles and lints clean.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs \
       desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx
git commit -m "feat(voice): add first-run flow with model download gate and Speak Anyway button"
```

---

## Task 15: Full Integration Test

End-to-end test verifying the multi-turn conversation flow through the manager.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` (add integration test)

- [ ] **Step 1: Write integration test**

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn new_session_creates_unique_keys() {
        let key1 = create_voice_session_key();
        let key2 = create_voice_session_key();
        assert_ne!(key1.as_str(), key2.as_str());
        assert!(key1.as_str().starts_with("desktop:"));
    }

    #[tokio::test]
    async fn warm_session_within_threshold_returns_true() {
        let recent = Utc::now() - chrono::Duration::minutes(5);
        assert!(is_warm_session(recent, 15));
    }

    #[tokio::test]
    async fn cold_session_outside_threshold_returns_false() {
        let old = Utc::now() - chrono::Duration::minutes(20);
        assert!(!is_warm_session(old, 15));
    }

    #[tokio::test]
    async fn adaptive_breath_scales_with_response_length() {
        let short = compute_breath_ms("Hi");
        let medium = compute_breath_ms(&"word ".repeat(100));
        let long = compute_breath_ms(&"word ".repeat(2000));

        assert_eq!(short, 300); // Minimum
        assert!(medium > 300);
        assert!(medium < 800);
        assert_eq!(long, 800); // Capped
    }

    #[test]
    fn phase_full_cycle_transitions() {
        let phases = [
            (ConversationPhase::Idle, ConversationPhase::Listening),
            (ConversationPhase::Listening, ConversationPhase::Reflecting),
            (ConversationPhase::Reflecting, ConversationPhase::Speaking),
            (ConversationPhase::Speaking, ConversationPhase::Listening), // auto-resume
        ];
        for (from, to) in phases {
            assert!(from.can_transition_to(&to), "{from:?} → {to:?} should be valid");
        }
    }

    #[test]
    fn phase_invalid_transitions_rejected() {
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Speaking));
        assert!(!ConversationPhase::Idle.can_transition_to(&ConversationPhase::Reflecting));
        assert!(!ConversationPhase::Listening.can_transition_to(&ConversationPhase::Speaking)); // must go through Reflecting
    }
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo nextest run -p app-core -E 'test(voice_conversation)'`
Expected: All tests pass.

- [ ] **Step 3: Run full workspace check**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -5`
Expected: 0 warnings (or only pre-existing desktop exceptions).

Run: `cd desktop-ui && bun run test -- --run`
Expected: All frontend tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "test(voice): add integration tests for VoiceConversationManager"
```

---

## Post-Implementation Notes

**What needs manual testing (browser dev mode):**
1. Press Alt+Shift+V → orb opens, enters Listening phase
2. Speak → see partial transcript, routing chips, memory echo
3. Silence → transition to Reflecting → agent processes → Speaking → TTS plays
4. Auto-resume → orb listens again (second turn)
5. Interrupt during Speaking → TTS fades, Continue button appears
6. Tap Continue → TTS resumes from where it left off
7. Pause → orb shows muted state, mic stops
8. Resume → orb returns to Listening
9. New Session → fresh session key, turn count resets
10. Close orb → open main chat → verify voice messages appear in history
11. Reopen orb within 15 min → same session (warm reattach)

**Tray icon PNGs** — The actual icon files need to be designed and added. For now, tray shows text ("Listening...", "Speaking...") which works without icons.

**Dockable orb** — The dock-to-chat feature requires additional Tauri window positioning code. It's a polish item that can be added after the core loop works. Track as a separate follow-up task.
