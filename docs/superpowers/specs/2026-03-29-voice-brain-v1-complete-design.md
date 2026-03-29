# Voice Brain v1 Complete: The Delight-First Sprint

**Date:** 2026-03-29
**Status:** Approved
**Scope:** Wire the full emotional arc — orb alive with real data, Mirror-powered echo, spoken agent response, context-aware hotkey, zero-friction first-run
**Timeline:** 2 weeks (14 days)
**Approach:** Refined Inside-Out with 3-day "live data unlock"
**Prerequisite:** Voice Brain infrastructure (Weeks 1–2) — `crates/voice-engine/`, desktop commands, `VoiceBrainOrb` component, `useVoiceEvents` hook, `MessageKind::Voice`, `VoiceConfig`, `VOICE_ACTIVE` flag, tray coordination. All implemented.

## Vision

Turn the blank voice-orb window into the living face of the second brain. Users press `Cmd+Shift+V`, the orb appears, partials + routing chips + Mirror-powered memory echo flow *while they're still speaking*, and the brain speaks back with context. One cohesive moment that makes users default to voice within days.

**Success scenario:** User presses the hotkey while walking. The orb appears, live waveform pulses, "schedule dentist" triggers a "→ Task" routing chip, a faded "Mirror" line says "You mentioned dentist last Tuesday — still unscheduled," the brain speaks back "Dentist scheduled for next Thursday, added to your calendar." The user never opened an app. The second brain was *with* them the entire time.

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Sprint scope | "Delight-First" hybrid (Week 3 + soul of Week 4) | Orb without echo = dictation toy; echo without orb = invisible backend. The intersection is where the magic lives. |
| Approach | Refined Inside-Out (backend first, orb lights up day 4) | Hardest piece (response loop) tackled first; orb mounts with real data, no mock→real transition. |
| Memory echo tier | Tiers 2 + 3 (Mirror + conversation recall) | Tier 3 alone = "any RAG tool." Tier 2 = "the brain that's been quietly watching." Tier 1 (pronunciation history) deferred — needs FSRS bridge. |
| Echo matching | Embedding-based cosine similarity (not keyword overlap) | Reuses existing `EmbeddingEngine`. Makes echo feel like understanding, not search. |
| Push-to-talk | Defer if Tauri plugin doesn't support key-down/key-up | Tap-to-toggle is 80% of usage. Ship it, add hold-to-talk in v1.5 based on data. |
| Desktop chat suppress | `HashSet<String>` of active voice session IDs | Orb is the conversation surface for voice, not a sidecar to chat. |
| First-run copy | "Brain waking up" not "Downloading model" | Every background process should feel like the brain thinking, not infrastructure loading. |

## Architecture

### Agent Response Loop Closure

**The problem:** VoiceService publishes `InboundMessage { kind: Voice }` to the bus → AgentRuntime processes it → response goes to desktop chat handler → VoiceService never sees the response. No path back for TTS.

**The solution:** Dependency-inverted `VoiceResponseHandler` trait (same pattern as `SpawnHandler`, `CronHandler`):

```rust
// In voice-engine (L1) — trait definition
#[async_trait]
pub trait VoiceResponseHandler: Send + Sync {
    /// Called when the agent produces a response for a voice session.
    async fn on_agent_response(&self, session_id: &str, response_text: &str);
}
```

**AppCore implementation (L7):**

1. Subscribes to `AgentEvent::Complete` for messages with `kind: Voice`
2. Matches `session_id` from `InboundMessage` metadata
3. Calls `TtsEngine::synthesize()` on the response text
4. Base64-encodes the PCM audio
5. Emits `VoiceEvent::SpeakResponse { audio_base64, sample_rate, text }`
6. Transitions `VoiceSessionState` → `Complete`

**Desktop chat suppression:**

```rust
// In AppCore — HashSet<String> of active voice session IDs
// When AgentRuntime emits a response for a voice session:
//   - Route to VoiceResponseHandler (orb gets the response)
//   - Skip desktop chat render
// Cleared when session reaches Complete state
```

The `session_id` (UUID) links the original capture to the response. VoiceService stamps it on each session, passes through `InboundMessage` metadata, bridge matches on the response side.

### Memory Echo (Tiers 2 + 3)

**Trait (L1):**

```rust
// In voice-engine
#[async_trait]
pub trait MemoryEchoProvider: Send + Sync {
    /// Fetch a contextual echo for the partial transcript.
    /// Returns None if no relevant memory found or privacy mode is Strict.
    async fn fetch_echo(&self, partial_text: &str) -> Option<String>;
}
```

**AppCore implementation (L7) — tries Tier 2 first, falls back to Tier 3:**

**Tier 2 — Mirror snippets (new facade method):**

```rust
// In crates/cognitive/src/mirror/facade.rs
pub async fn get_recent_voice_relevant_snippet(
    &self,
    query: &str,
) -> Option<String> {
    let query_embedding = self.embedding_engine.embed(query).await.ok()?;
    let snippets = self.repo.get_recent_snippets(3).await.ok()?;
    snippets
        .into_iter()
        .max_by(|a, b| {
            cosine_similarity(&query_embedding, &a.embedding)
                .partial_cmp(&cosine_similarity(&query_embedding, &b.embedding))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|s| cosine_similarity(&query_embedding, &s.embedding) >= 0.65)
        .map(|s| s.content)
}
```

**Tier 3 — Conversation recall (existing):**

```rust
// Uses existing ContextEngine::recall_relevant()
let recall = context_engine
    .recall_relevant(partial_text, RecallParams {
        max_results: 1,
        max_tokens: 50,
        recency_boost: true,
    })
    .await?;
```

**Timing:** One-shot prefetch — fires once on the first `PartialTranscript` with ≥3 words. No repeat calls on subsequent partials. Best effort (non-blocking).

**Privacy:** `Strict` mode → returns `None` immediately (skips both tiers). `Standard` and `Off` → tries both.

### Orb Mounting + Frontend Wiring

**Route:**

```tsx
// New route in app router (alongside /launcher, /overlay)
{ path: "/voice-orb", element: <VoiceOrbPage /> }
```

`VoiceOrbPage` is a thin wrapper — transparent background, no chrome, just `VoiceBrainOrb` filling the 320x200 window. Same pattern as distraction overlay and launcher pages.

**Three states with real data:**

| State | Elements |
|-------|----------|
| **Listening** | Live waveform (from `AudioLevel.rms` at ~30fps), scrolling transcript with word-level highlights (green ≥0.85 / amber 0.60–0.84 / red <0.60), routing chips (glass-panel pills with skill icon + label), memory echo faded line with "Mirror" badge + cognitive pulse animation (300ms fade-in + single waveform heartbeat, once per session), engine badge (cloud icon if Groq), hint bar "⌘⇧V to finish · tap to close" |
| **Processing** | Pulsing dot replacing waveform, final transcript (static) with highlights, routing chips show progress checkmarks, "Cancel & discard" button, background-processing toast if orb was dismissed |
| **Response** | TTS playback waveform (synced via Web Audio API), agent response text synced with audio, pronunciation summary (overall score + weak words suggestion), replay button (re-triggers stored base64 audio), session summary chips, auto-dismiss after TTS completes + 2s, "tap anywhere to close" |

**Web Audio TTS playback:**

```tsx
const playTtsAudio = (base64: string, sampleRate: number) => {
  const ctx = new AudioContext();
  const samples = base64ToFloat32(base64);
  const buffer = ctx.createBuffer(1, samples.length, sampleRate);
  buffer.copyToChannel(samples, 0);
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
};
```

**Word-level CSS highlights (using existing theme tokens):**

```css
.word-good    { color: var(--color-success); }
.word-fair    { color: var(--color-warning); }
.word-poor    { color: var(--color-destructive); }
```

**Dismiss behavior:**

- Tap orb or hotkey during Listening → `voice:dismiss` → VoiceService transitions to background → `ProcessingInBackground` event → orb closes, transient toast
- "Cancel & discard" in Processing → `voice:cancel` → session dropped, no InboundMessage
- Response state auto-dismisses after TTS + 2s

### Hotkey, Push-to-Talk, Menu-Bar Mic

**Hotkey behavior:**

| Gesture | Action |
|---------|--------|
| First press | Start capture → voice-orb window opens (scale 0.9→1.0 + fade, 200ms) |
| Second press (while capturing) | Stop capture → finalize → Processing state |
| Hold ≥500ms + release | Push-to-talk (if Tauri API supports key events; otherwise defer to v1.5) |

**Context-aware hotkey (new):**

```rust
// In hotkey handler — one-line checks on existing flags
match context {
    FocusSessionActive => quick_voice_journal_capture(),
    // No orb window. Capture audio, transcribe, create InboundMessage,
    // speak a short confirmation ("Got it, noted."). Minimal interruption
    // to deep work. Transcript still flows through full pipeline.

    LauncherOpen => hands_free_launcher_search(),
    // Treat spoken text as launcher search input. Transcript populates
    // the launcher's search field. No separate orb window.

    _ => full_orb_capture(),
    // Normal voice brain orb — full three-state experience.
}
```

**Menu-bar mic:**

- Click: same toggle as hotkey (start/stop capture)
- Icon states: outline mic (idle) → filled red + pulse (listening) → spinner (processing)
- Voice-ready badge: faint green dot when VoiceService is ready + model loaded (idle). Tooltip: "Voice Brain ready — ⌘⇧V to think out loud"
- `VOICE_ACTIVE` flag coordinates with focus timer (focus timer takes tray title priority)

**Window positioning:** Top-center of active monitor, 80px from top. `window.set_position()` with monitor detection on capture start.

### First-Run Flow

```
User presses ⌘⇧V or taps tray mic for the first time
  │
  ├─ macOS mic permission dialog
  │   └─ Denied? → VoiceEvent::Error → orb shows "Enable mic in System Settings"
  │
  ├─ Groq API key configured?
  │   ├─ YES → Voice works immediately (cloud badge in orb)
  │   │        → Background: ModelManager downloads whisper-small (488 MB)
  │   │        → Settings tab shows progress
  │   │        → Complete → silent hot-swap to local → toast: "Now fully offline"
  │   │
  │   └─ NO → Orb shows "Waking up your second brain…" with progress bar
  │           → "Speak anyway (cloud mode)" button for instant Groq fallback
  │           → Download complete → voice works, first capture starts
```

**Copy refinements:**
- Download progress: "Waking up your second brain… (local voice model)" with gentle pulse on progress bar
- No "Downloading voice model" — every process feels like the brain thinking
- "Speak anyway" button: one-tap Groq fallback during download (disappears when local ready)

**Welcome echo (one-time, first successful capture ever):**

After the first `Finalized` event (triggered via config flag, never shown again):

> "Welcome to your second brain. I'm listening. Everything you say here becomes memory, learning, and reflection — just like your thoughts."

Delivered via the existing `MemoryEcho` event path. Zero new infrastructure.

**Hot-swap mechanism:** `VoiceService` holds `Arc<RwLock<Arc<dyn TranscriptionEngine>>>`. When `ModelManager` transitions to `Ready`, AppCore creates `WhisperLocalEngine` and swaps the inner Arc. Next capture uses local. No restart needed.

## Testing Strategy

### Unit Tests (voice-engine crate)

| Test | What it verifies |
|------|-----------------|
| Existing: PronunciationReport (5 tests) | Boundary values, empty, mixed, single word |
| Existing: VoiceSessionState (6 tests) | Valid/invalid transitions, active states |
| Existing: VoiceRouter (multi-intent, single, empty) | Keyword scoring, threshold, multi-intent detection |
| Existing: ModelManager (4 tests) | State machine, file detection |
| **New:** VoiceResponseHandler contract | Mock handler records calls, session_id linking |
| **New:** MemoryEchoProvider contract | Mock returns echo, one-shot behavior, privacy skip |

### Integration Tests (app-core + voice-engine)

| Test | What it verifies |
|------|-----------------|
| Full capture→response loop | start → mock partials → stop → InboundMessage → response handler → SpeakResponse |
| Dismiss during capture | ProcessingInBackground → pipeline completes → InboundMessage created |
| Dismiss during finalize | Same, from Finalizing state |
| Cancel discards | No InboundMessage, session → Idle |
| Memory echo fires once | First partial ≥3 words → provider called once, subsequent → no calls |
| Privacy Strict skips echo | privacy_mode: Strict → provider never called |
| Engine hot-swap | ModelManager → Ready → VoiceService uses new engine |
| Suppress desktop chat | Voice session → response only emits VoiceEvent, not chat render |

### Frontend Tests (Vitest)

| Test | What it verifies |
|------|-----------------|
| Orb state machine | All transitions: idle → listening → processing → response → idle |
| VoiceEvent rendering | Partials → transcript, chips appear, echo with badge |
| Word-level highlights | Confidence 0.90/0.70/0.40 → correct CSS classes |
| TTS playback | SpeakResponse → Web Audio buffer (mock AudioContext) |
| Replay button | Click → same audio replays from stored base64 |
| Cognitive pulse | MemoryEcho event → fade-in once |
| Dismiss/cancel | Click handlers → correct IPC commands |
| **End-to-end delight assertion** | CaptureStarted → partial + chip <400ms → echo ~500ms → SpeakResponse with synced waveform |

### Dev Server Mock Endpoints

- `POST /api/voice_simulate_event { event: VoiceEvent }` — inject any event into frontend
- `POST /api/voice_mock_session { text, language, duration_ms }` — simulate full session

### Delight Validation Protocol (Manual, Days 11–14)

Entire team (including non-engineers) uses voice as primary input for 7 days:
- Score on three questions (1–5): "Did it feel like the brain was listening?", "Did the memory echo make me feel known?", "Did the spoken response make me smile?"
- Note any moment where it felt like "just dictation" vs. "the brain is with me"
- Fill delight survey daily

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Delight score | ≥4.5/5 | Micro-survey in Response state (thumbs + "why?") |
| "Feels like it's with me" | ≥60% of free-text responses | Qualitative analysis of delight survey |
| Capture → first routing chip | <400ms | CaptureStarted → first RoutingSuggestion timestamp |
| TTS response latency | <300ms | Finalized → first audio byte in SpeakResponse |
| Session completion rate | >80% reach Finalized | VoiceSessionState transition logs |
| Memory echo hit rate | >50% of sessions show an echo | MemoryEcho events / total sessions |
| First-run success | >95% complete first capture | Error rate on first voice_start_capture |

## Sprint Structure (2 Weeks)

| Days | Phase | Deliverables |
|------|-------|-------------|
| **1–3** | Close the loop | `VoiceResponseHandler` trait + AppCore impl, agent response → TTS → `SpeakResponse`, desktop chat suppression, `MirrorFacade::get_recent_voice_relevant_snippet` (embedding-based), `MemoryEchoProvider` trait + AppCore impl, real `ModelManager::start_download` (HuggingFace streaming), unit + integration tests |
| **4–7** | Orb alive | Mount `VoiceBrainOrb` to `/#/voice-orb`, connect real `VoiceEvent` stream, word-level highlights, memory echo with "Mirror" badge + cognitive pulse, TTS playback via Web Audio + response waveform, replay button, routing chips, redirect hotkey to voice-orb, menu-bar mic toggle + icon states, voice-ready badge, context-aware hotkey |
| **8–10** | First-run + polish | Mic permission handling, Groq-instant + background download + hot-swap, "Brain waking up" progress, "Speak anyway" button, welcome echo, dismiss/cancel/background-processing toast, download progress in Settings, dev server mock endpoints, Vitest suite |
| **11–14** | Dogfood + tune | Delight Validation Protocol (full team, daily surveys), end-to-end delight assertion, tune echo timing + cognitive pulse, tune TTS latency + waveform sync, fix frictions, delight score micro-survey wiring |

## Deliberately Deferred (v1.5 Follow-Up)

| Feature | Why deferred |
|---------|-------------|
| FSRS dual-signal review sessions | Needs learning↔voice bridge (Tier 1 pronunciation history). First follow-up task once orb is live. |
| Coaching `SpokenNudge` interventions | Depends on real voice usage data to calibrate triggers. |
| Voice journal `audio_ref` WAV recording + playback | Needs audio tee during capture. Useful but not part of the emotional arc. |
| Multi-intent "Split into two turns?" pill UI | Auto-split works for v1. UI refinement based on usage patterns. |
| Per-language voice selector + speaking rate slider | Settings polish after core delight is validated. |
| Push-to-talk (hold ≥500ms) | Depends on Tauri plugin key event support. Tap-to-toggle is 80% of usage. |
| Phoneme-level pronunciation analysis | v1.5 with force-alignment library. Word-level confidence is an effective proxy. |
| Persistent HUD mode | Power-user feature, add after dogfood confirms demand. |

## Relationship to Original Spec

This design implements the "Delight-First" subset of `docs/superpowers/specs/2026-03-28-voice-brain-design.md`:

- **Week 3 (Orb UI):** Fully covered — mounting, three states, hotkey, menu-bar mic, dev mock endpoints
- **Week 4 (Learning + Cognitive):** Partially covered — memory echo (Tiers 2+3), pronunciation display in orb. Deferred: FSRS dual-signal, coaching nudges, voice journal audio_ref
- **Week 5 (Polish):** Partially covered — first-run flow, VOICE_ACTIVE coordination, dismiss behavior. Deferred: full settings polish, dogfood fixes beyond 2-week sprint

The infrastructure from Weeks 1–2 (voice-engine crate, desktop commands, VoiceBrainOrb component, useVoiceEvents hook, MessageKind::Voice, VoiceConfig) is the foundation this sprint builds on.
