# Voice-Chat Unification v2: The Companion Sprint

**Date:** 2026-03-29
**Status:** Approved
**Scope:** Unify voice and chat into a single modality — the orb becomes the live voice surface for chat sessions, with multi-turn hybrid conversation, smart session attachment, soft interrupt, and persistent history
**Timeline:** 2 weeks
**Prerequisite:** Voice Brain infrastructure (Weeks 1–2) + Voice Brain v1 Complete sprint — `VoiceService`, `VoiceBrainOrb`, `useVoiceEvents`, `AppMemoryEchoProvider`, engine hot-swap, context-aware hotkey, all implemented.

## Vision

Voice and chat are not two systems. They are two interfaces to the same second brain. Press Alt+Shift+V, the orb appears, and you're in a real-time spoken conversation with the same session you were just typing in. The full transcript and agent responses appear in the main chat window as persistent history. Close the orb, continue typing. Reopen it, keep speaking. One brain, one conversation, two surfaces.

**Success scenario:** You're walking. You press the hotkey. The orb opens, attaching to your "Morning thoughts" session from 8 minutes ago. You speak: "reschedule the dentist to Thursday." The routing chip shows "→ Task," a Mirror echo says "You mentioned dentist last Tuesday — still unscheduled." The agent responds with voice: "Done — moved to Thursday at 2pm." The orb auto-resumes listening. You say "also remind me to buy toothpaste." The agent handles it. You close the orb. Back at your desk, you open the main chat — both turns are there, fully logged. You type a follow-up. Same session. Same brain.

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Orb model | Hybrid — live voice surface + persistent chat log | Orb is the real-time conversation; chat is the long-term memory log. Neither is "primary." |
| Turn flow | Hybrid turns — auto-listen + pause + interrupt + auto-resume | Feels like a patient companion. Continuous by default, controllable when needed. |
| Architecture | VoiceConversationManager (new orchestrator in app-core) | Clean separation — manager owns conversation cycle, chat pipeline stays untouched. |
| Interrupt behavior | Soft — 300ms TTS fade + Continue button | Polite and reversible. Full response text always persisted in chat (nothing lost). |
| Session attachment | Smart default — warm attach if recent, otherwise new | Respects thinking flow. Configurable thresholds. |
| Channel name | `desktop` (same as text chat) + `is_voice_session` metadata flag | True unification — no parallel "voice channel." Mic badge for visual distinction. |
| Phase model | 4 phases: Idle / Listening / Thinking / Speaking | Maps 1:1 to what the user sees and feels. Internal transitions stay internal. |
| Conversation loop | Spawned Tokio task + command channel | Atomic state changes, no race conditions between frontend and backend. |
| Scope sequence | Voice-chat unification first → Language config → Practice mode | Foundation first. Learning features layer on top cleanly. |

## Architecture

### VoiceConversationManager — The Orchestrator

Lives in `crates/app-core/src/handlers/voice_conversation.rs`. Stored as `Option<Arc<VoiceConversationManager>>` in `AppCore` (same pattern as `MirrorFacade`).

```rust
pub struct VoiceConversationManager {
    // Audio I/O
    voice_service: Arc<VoiceService>,

    // Chat integration (calls chat_send_internal)
    agent: Arc<AgentLoop>,
    repos: Repos,
    emitter: Arc<dyn AppEventEmitter>,  // For main chat window updates

    // Conversation state
    state: Arc<Mutex<VoiceConversationState>>,

    // Memory echo
    echo_provider: Arc<dyn MemoryEchoProvider>,

    // Config
    config: Arc<RwLock<VoiceConfig>>,
}

pub struct VoiceConversationState {
    pub session_key: Option<SessionKey>,
    pub phase: ConversationPhase,
    pub paused: bool,
    pub turn_count: u32,
    pub last_activity: DateTime<Utc>,
    pub interrupted: bool,
    pub pending_response_text: Option<String>,
    pub tts_position: usize,  // Char offset for Continue after interrupt
}

pub enum ConversationPhase {
    Idle,       // Orb closed or no active conversation
    Listening,  // Mic open, user speaking (waveform pulsing)
    Thinking,   // Agent processing (gentle pulse in orb)
    Speaking,   // TTS playing response (waveform synced to audio)
}
```

**Chat integration — internal helper:**

The existing `chat_send` gains a thin internal variant:

```rust
pub async fn chat_send_internal(
    &self,
    content: String,
    session_key: SessionKey,
    is_voice: bool,
) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError>
```

When `is_voice: true`:
- The session is created/upserted with `{ "is_voice_session": true }` in metadata (for mic badge in chat list)
- The relay fires all normal chat events (ContentChunk, ToolStart, Done) to the main window — identical to text chat
- The manager intercepts `Done` from `StreamingHandle.event_rx` to trigger TTS

No voice-specific logic leaks into the core chat pipeline. The `is_voice` flag is purely a metadata marker.

### IPC Commands

| Command | Action |
|---------|--------|
| `voice_conversation_start` | Smart attach or create session → start listening |
| `voice_conversation_pause` | Mute mic, keep session alive |
| `voice_conversation_resume` | Unmute, start listening again |
| `voice_conversation_interrupt` | Stop TTS (300ms fade), switch to listening |
| `voice_conversation_continue` | Resume TTS from interrupted position |
| `voice_conversation_new_session` | Create fresh session, start listening |
| `voice_conversation_end` | Stop everything, close orb |
| `voice_conversation_status` | Return current state snapshot |

The existing `voice_start_capture` / `voice_stop_capture` remain for the raw VoiceService layer (used internally by the manager). The new commands are what the orb calls.

### Event Flow — Dual Emission

The manager emits to two channels:

1. **VoiceEvent** → orb (audio levels, partials, phase changes, TTS audio, memory echo, routing chips)
2. **Chat events** → main window via `AppEventEmitter` (user message, streaming response chunks, tool calls, done)

When you speak in the orb, the main chat window updates in real-time — your transcript appears as a user message, the agent's response streams in, exactly as if you'd typed it.

### Session Attachment Logic

When Alt+Shift+V is pressed:

```
Alt+Shift+V pressed
    │
    ├─ Manager already has active session (phase != Idle)?
    │   └─ YES → Resume (unpause if paused, refocus orb)
    │
    ├─ Manager has previous session_key from last conversation?
    │   └─ Check last_activity timestamp
    │       ├─ < warm_session_minutes (default 15) → Reattach (warm)
    │       │   Orb title: "Morning thoughts (continuing)" + green link icon
    │       └─ ≥ warm_session_minutes → Create new session
    │           Orb title: "New voice session"
    │
    ├─ Main chat window has an active session?
    │   └─ Check that session's updated_at
    │       ├─ < warm_chat_minutes (default 5) → Attach to it
    │       │   Orb title: "{session title} (voice)" + green link icon
    │       └─ ≥ warm_chat_minutes → Create new session
    │
    └─ No prior context → Create new session
```

**New session creation:**
- Channel: `ChannelName::Desktop` (same as text chat)
- Chat ID: `ChatId(Uuid::new_v4())`
- Session metadata: `{ "is_voice_session": true }`
- Title auto-set from first spoken transcript (first 60 chars)

**Warm thresholds configurable** in `VoiceConfig`:
```rust
pub warm_session_minutes: u32,  // Default: 15 (voice session reattach)
pub warm_chat_minutes: u32,     // Default: 5 (main chat attach)
```

Hot-reloadable via existing config file watcher.

**"New Session" button:**
- Creates fresh session key, resets `turn_count` to 0, clears `pending_response_text`
- Does NOT clear memory graph — episodic/semantic memories preserved
- Old session stays in chat history (navigable in main window)

**Session continuity badge:** Clicking the green link icon in the orb title bar focuses the main chat window and scrolls to the last message. Bridges orb ↔ chat visually.

### Multi-Turn Conversation Loop

The manager spawns a single `conversation_loop` Tokio task. This task owns a `tokio::select!` loop listening to:

1. **VoiceService events** (audio levels, partials, silence) → forwarded to orb
2. **Agent streaming events** (content chunks, tool calls, done) → forwarded to chat window
3. **Command channel** (`mpsc::Receiver<VoiceCommand>`) → interrupt, pause, resume, new session, end

```rust
enum VoiceCommand {
    Pause,
    Resume,
    Interrupt,
    Continue,     // Resume TTS from interrupted position
    NewSession,
    End,
}
```

**The cycle:**

```
voice_conversation_start called
    │
    ▼
┌─► LISTENING
│   ├─ VoiceService.start_capture()
│   ├─ Emit VoiceEvent::AudioLevel (RMS ~30fps → orb waveform)
│   ├─ Emit VoiceEvent::PartialTranscript (real-time words)
│   ├─ Memory echo: fires once per turn (first partial ≥3 words)
│   ├─ VoiceRouter emits RoutingSuggestion
│   ├─ Silence detected (configurable, default 1.5s) → auto-stop
│   │
│   ▼
│ THINKING
│   ├─ VoiceService.stop_capture() → final Transcript
│   ├─ Skip if transcript is empty/noise
│   ├─ chat_send_internal(transcript.text, session_key, is_voice: true)
│   │   └─ Persists user message + runs agent pipeline
│   ├─ Drain StreamingHandle.event_rx:
│   │   ├─ ContentChunk → emit to main chat window (real-time)
│   │   ├─ ToolStart/ToolEnd → emit to main chat (transparency)
│   │   └─ Done { content } → store as pending_response_text
│   ├─ Memory echo visible in orb during Thinking (Mirror badge + cognitive pulse)
│   ├─ Emit VoiceEvent::Thinking to orb
│   │
│   ▼
│ SPEAKING
│   ├─ TTS synthesize(pending_response_text)
│   ├─ Emit VoiceEvent::SpeakResponse { audio_base64, sample_rate, text }
│   ├─ Track tts_position (char offset for Continue)
│   ├─ Wait for TTS duration
│   │   ├─ Interrupted? → 300ms fade, → LISTENING
│   │   ├─ Paused? → stop TTS, wait for resume
│   │   └─ Completed? → adaptive breath pause → auto-resume
│   │
│   ▼
│ AUTO-RESUME (internal transition, not a visible phase)
│   ├─ Adaptive pause: ~300ms (short response) to ~800ms (long response)
│   │   Heuristic: min(300 + pending_response_text.len() / 2, 800) ms
│   ├─ If paused → stay idle until resume
│   └─ Otherwise → back to LISTENING
│
└───────────────────────────────────────────────────┘
```

**Interrupt detection during Speaking:**

During the Speaking phase, the manager starts a lightweight audio monitor via `AudioCapture` (same cpal stream, but only checking RMS levels — no transcription). When RMS exceeds `silence_threshold` for >100ms (debounced to avoid coughs/clicks), the manager triggers the interrupt flow. This monitor is separate from the full `start_capture()` used during Listening — it's purely a voice-activity detector.

**Interrupt flow (soft, 300ms fade):**

1. Audio monitor detects user speech during Speaking phase
2. Manager sets `interrupted = true`, estimates `tts_position` (see below)
3. Emits `VoiceEvent::TtsFadeOut` → frontend fades audio over 300ms
4. Stops audio monitor, starts full `start_capture()` → transitions to Listening
5. User's speech becomes the next turn
6. Emits `VoiceEvent::ContinueAvailable` → orb shows "Continue" button briefly
7. Full response text already persisted in chat (nothing lost)

**`tts_position` estimation:**

TTS speaking rate doesn't map 1:1 to character offsets. The manager estimates position using: `elapsed_tts_ms / total_tts_duration_ms * text.len()`. This gives a rough char offset — good enough for Continue (re-synthesizing from roughly where the user interrupted). The full text is always available in the chat log, so precision isn't critical.

**Continue flow:**

1. User taps "Continue" (or sends `VoiceCommand::Continue`)
2. Manager calls TTS with `pending_response_text[tts_position..]` (re-synthesizes remaining text)
3. Transitions back to Speaking
4. `continueAvailable` clears in orb

**Echo one-shot limiter** (managed per turn in the loop):

```rust
if !echo_fired_this_turn && word_count >= 3 {
    if let Some(echo) = echo_provider.lookup(partial_text).await {
        emit(VoiceEvent::MemoryEcho { text: echo });
        echo_fired_this_turn = true;
    }
}
// Reset at start of each new LISTENING phase
```

### Memory Echo — Tier 2 + Tier 3 Fallback

`AppMemoryEchoProvider` gains Tier 3 (ContextEngine recall) as a fallback:

```rust
impl MemoryEchoProvider for AppMemoryEchoProvider {
    async fn lookup(&self, partial_text: &str) -> Option<String> {
        // Tier 2: Mirror snippets (embedding cosine similarity, threshold 0.45)
        if let Some(echo) = self.mirror.get_recent_voice_relevant_snippet(partial_text).await {
            return Some(echo);
        }
        // Tier 3: Conversation recall (existing ContextEngine)
        if let Some(recall) = self.context_engine.recall_relevant(partial_text, RecallParams {
            max_results: 1, max_tokens: 50, recency_boost: true,
        }).await.ok().flatten() {
            return Some(recall);
        }
        None
    }
}
```

Requires injecting `Arc<ContextEngine>` into `AppMemoryEchoProvider` at init time (alongside existing `MirrorFacade`).

Privacy: `Strict` mode → returns `None` immediately (skip both tiers). `Standard` and `Off` → tries both.

## Frontend

### Orb UI Redesign

The orb evolves from one-shot display to persistent conversation surface:

```
┌─────────────────────────────────┐
│ ● Morning thoughts (continuing) │  ← Title bar: session name + phase dot
│                    ⊕ New  ⏸ Pause│  ← Action buttons
├─────────────────────────────────┤
│                                 │
│   [Waveform / Thinking pulse]   │  ← Central visual (phase-dependent)
│                                 │
│   "schedule dentist tomorrow"   │  ← Current turn transcript
│   → Task  · Mirror: "You        │  ← Routing chip + memory echo
│     mentioned dentist Tuesday"  │
│                                 │
├─────────────────────────────────┤
│   ▸ Continue                    │  ← Only visible after interrupt
│                                 │
│   ⌥⇧V to close · tap to pause  │  ← Hint bar
└─────────────────────────────────┘
```

**Window:** 320x280, transparent, always-on-top, no decorations, HUD window effect. Top-center of active monitor, 80px from top.

**Phase-dependent visuals:**

| Phase | Central visual | Title dot color |
|-------|---------------|-----------------|
| Listening | Red pulsing waveform (12 bars, RMS-driven) | Red |
| Thinking | Gentle cognitive pulse + memory echo with Mirror badge | Amber |
| Speaking | TTS-synced waveform (blue) | Blue |
| Idle (paused) | Static muted mic icon | Gray |

**Dockable orb:** A small paperclip icon in the title bar. Click → orb snaps to the right edge of the main chat window (320px wide, full chat height). Stays docked until undocked or closed. Uses existing Tauri window positioning APIs.

### `useVoiceConversation` Hook

Replaces `useVoiceEvents` as the orb's primary hook:

```typescript
export function useVoiceConversation() {
  // State from VoiceEvent stream
  const phase: ConversationPhase;       // "idle" | "listening" | "thinking" | "speaking"
  const transcript: string;              // Current turn's partial/final text
  const segments: TranscriptSegment[];   // Word-level confidence
  const routingChips: RoutingChip[];
  const memoryEcho: string | null;
  const audioLevel: number;              // RMS for waveform
  const ttsAudio: TtsAudioData | null;
  const sessionInfo: { key: string; title: string; turnCount: number } | null;
  const continueAvailable: boolean;
  const engineKind: "local" | "cloud";

  // Actions (IPC calls to VoiceConversationManager)
  const start: () => Promise<SessionInfo>;
  const pause: () => Promise<void>;
  const resume: () => Promise<void>;
  const interrupt: () => Promise<void>;
  const continueTts: () => Promise<void>;
  const newSession: () => Promise<SessionInfo>;
  const end: () => Promise<void>;

  return { phase, transcript, segments, routingChips, memoryEcho,
           audioLevel, ttsAudio, sessionInfo, continueAvailable, engineKind,
           start, pause, resume, interrupt, continueTts, newSession, end };
}
```

`useVoiceEvents` remains available for raw VoiceService access (used internally by `useVoiceConversation` and for the existing VoiceRecorder in the launcher).

### Main Chat Window Integration

Minimal changes to the existing chat page:

1. **Voice session badge** — In the session list sidebar, sessions with `is_voice_session` metadata get a small mic icon. Active voice sessions show a pulsing dot.

2. **Real-time message sync** — The manager emits chat events through `AppEventEmitter`. The existing `useAgentStream` / SSE infrastructure handles this. Messages appear in real-time when speaking in the orb.

3. **Voice indicator in chat header** — When the orb is open and attached to the displayed session, the header shows a pulsing green mic dot + "Listening" label. Clicking it focuses the orb window or toggles pause.

4. **Scroll behavior** — When `voice_active` is true for the displayed session, the chat auto-scrolls only if the user is already at the bottom. Prevents jarring scroll jumps during multi-turn voice.

5. **No changes to ChatInput, message rendering, or session persistence** — voice messages look identical to typed messages (with an optional small mic icon on user message bubbles).

## Menu-Bar Tray

Three tray states coordinated via the existing `VOICE_ACTIVE` flag:

| State | Tray title | Tray icon | Click action |
|-------|-----------|-----------|-------------|
| Idle (voice available, model loaded) | Normal countdown | Normal icon + tooltip "Voice ready — ⌥⇧V" | `voice_conversation_start` |
| Listening / Thinking | "Listening..." | Filled red mic | Toggle pause |
| Speaking | "Speaking..." | Filled blue mic | Toggle pause |

Voice-ready badge: Faint green dot on tray icon when `ModelState::Ready` + phase is `Idle`. Indicates the brain is ready for voice without opening the orb.

Icon changes via `tray.set_icon()` — three small PNGs bundled as resources (idle, listening, speaking).

Focus timer coordination: `FOCUS_ACTIVE` still takes tray priority. When focus is active and voice starts, the tray shows focus title but the voice-ready badge disappears (voice runs headlessly in focus mode per the context-aware hotkey).

## First-Run Flow

On the very first `voice_conversation_start`:

```
Manager.start() called
    │
    ├─ macOS mic permission denied?
    │   └─ Emit VoiceEvent::Error { message: "Enable mic in System Settings", recoverable: true }
    │       Orb shows error state with "Open Settings" button
    │
    ├─ Groq API key configured?
    │   ├─ YES → Start immediately (cloud badge in orb)
    │   │        Background: ModelManager downloads whisper-small (488 MB)
    │   │        On complete: hot-swap to local, toast "Now fully offline"
    │   │
    │   └─ NO, local model available?
    │       ├─ YES → Start with local engine
    │       └─ NO → Emit VoiceEvent::SetupRequired { needs_model: true }
    │               Orb shows: "Waking up your second brain..." + progress bar
    │               Download starts automatically
    │               On complete: conversation begins
    │
    └─ Welcome echo (one-time, first successful capture ever):
        "Welcome to your second brain. I'm listening. Everything you say
         here becomes memory, learning, and reflection — just like your
         thoughts. Press ⌥⇧V anytime."
        Delivered via MemoryEcho event path. Config flag prevents repeat.
```

**"Speak anyway" button:** If Groq key exists but local model is downloading, the orb shows "Speak anyway (cloud)" — one tap switches to Groq immediately. Disappears when local model is ready.

## Configuration

Additions to `VoiceConfig`:

```rust
pub struct VoiceConversationConfig {
    pub warm_session_minutes: u32,   // Default: 15 (reattach previous voice session)
    pub warm_chat_minutes: u32,      // Default: 5 (attach to active chat session)
    pub silence_threshold_secs: f32, // Default: 1.5 (end-of-turn detection)
    pub auto_resume: bool,           // Default: true (listen again after response)
    pub adaptive_breath: bool,       // Default: true (variable pause after response)
}
```

All hot-reloadable via existing config file watcher.

## Testing Strategy

### Rust Unit Tests

| Test | Verifies |
|------|----------|
| `VoiceConversationState` transitions | All valid phase transitions, invalid transitions rejected |
| `VoiceCommand` processing | Each command produces correct phase change |
| Session attachment: warm voice | Last activity < 15 min → same session_key |
| Session attachment: cold voice | Last activity ≥ 15 min → new session_key |
| Session attachment: warm chat | Main chat < 5 min → attach to chat session |
| Session attachment: cold chat | Main chat ≥ 5 min → create new |
| Echo one-shot per turn | First partial ≥3 words triggers, subsequent don't |
| Echo Tier 2→3 fallback | Mirror None → ContextEngine called; Mirror Some → skip Tier 3 |
| Interrupt stores position | `tts_position` and `pending_response_text` saved |
| Continue resumes from position | TTS called with `text[tts_position..]` |
| Adaptive breath duration | Short text → ~300ms, long text → ~800ms, capped |
| New session resets state | `turn_count` = 0, new key, old session preserved |
| Turn count increment | Each completed user+agent turn increments `turn_count` |
| Privacy Strict skips echo | Both tiers skipped, returns None |

### Integration Tests (app-core, mock agent + mock VoiceService)

| Test | Verifies |
|------|----------|
| Full multi-turn cycle | Start → transcript → chat_send_internal → response → TTS → auto-resume → second turn |
| Chat persistence | Voice turns in `session_messages` with correct session_key and roles |
| `is_voice_session` metadata | Session queryable by metadata flag |
| Warm reattach | End → start within 15 min → same session_key |
| Cold creates new | End → start after 15 min → different session_key |
| Dual emission | Voice turn emits both VoiceEvent and chat events |
| First-run gate | No engine → SetupRequired event, no crash |
| Interrupt + continue | Interrupt stores position, continue resumes correctly |

### Frontend Tests (Vitest)

| Test | Verifies |
|------|----------|
| `useVoiceConversation` phase cycle | idle → listening → thinking → speaking → listening |
| Interrupt flow | Speaking + interrupt → listening, continueAvailable = true |
| Continue flow | continueTts → speaking, continueAvailable = false |
| Pause/resume | pause → idle, resume → listening |
| Session info display | Warm → "(continuing)", new → "New voice session" |
| Orb multi-turn rendering | Transcript resets per turn, routing chips refresh |
| Docked mode | Dock button → orb repositions to chat window edge |
| Chat header voice dot | Voice active → pulsing green dot visible |
| TTS fade on interrupt | 300ms CSS fade-out on interrupt |
| Memory echo once per turn | Only one echo event per listening phase |

## Deliberately Deferred

| Feature | Why deferred |
|---------|-------------|
| Language learning configuration | Next spec — foundation must be solid first |
| Practice mode (shadowing, dictation, etc.) | Third spec — needs language config + solid voice-chat |
| Push-to-talk (hold ≥500ms) | v1.5 — tap-to-toggle + hybrid turns cover 95% of usage |
| FSRS dual-signal review sessions | v1.5 — needs learning↔voice bridge |
| Coaching SpokenNudge | v1.5 — needs real voice usage data |
| Voice journal audio_ref WAV recording | v1.5 — useful but not part of unification |
| Per-language voice selector + speaking rate slider | Settings polish after unification validated |
| Persistent HUD mode | Power-user feature, add after dogfood confirms demand |

## Relationship to Previous Specs

This design supersedes the voice interaction model from `2026-03-29-voice-brain-v1-complete-design.md`:

- **Voice Brain v1 Complete** delivered the infrastructure: VoiceService, orb component, engine hot-swap, context-aware hotkey, memory echo, TTS playback, pronunciation scoring.
- **This spec (Voice-Chat Unification v2)** replaces the one-shot capture→response→dismiss model with a multi-turn conversation loop, unifies voice into the chat session model, and adds the VoiceConversationManager orchestrator.

The v1 infrastructure (VoiceService, audio capture, STT engines, TTS, VoiceRouter, ModelManager, pronunciation) is fully reused. The manager wraps VoiceService — it doesn't replace it.
