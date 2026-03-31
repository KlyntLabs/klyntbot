# Voice System Bugfixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 8 known bugs in the voice conversation system — dual audio playback, dead segment UI, unwired config, dropped loop handle, missing thread events, unused conversation_type, TTS file collision, and stop-TTS-on-Web-Audio for interrupt.

**Architecture:** Each fix is isolated to 1-3 files. No new crates or feature packages needed. Fixes touch voice-engine (Rust), app-core (Rust), storage (SQL), desktop-shared (types), and desktop-ui (TypeScript). All fixes preserve existing APIs and behavior — no breaking changes.

**Tech Stack:** Rust (tokio, serde, sqlx), TypeScript (React, Web Audio API), SQLite

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `desktop-ui/src/shared/lib/audio.ts` | Modify | Add `stopTtsAudio()` export + track active source node |
| `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts` | Modify | Call `stopTtsAudio()` on `ttsFadeOut`/`end`, remove dead `segments` state |
| `crates/voice-engine/src/events.rs` | Modify | Add `segments` field to `PartialTranscript` |
| `crates/voice-engine/src/service.rs` | Modify | Pass segments in `PartialTranscript` event, use config for TTS params, unique WAV filename |
| `crates/voice-engine/src/types.rs` | Check | Ensure `TranscriptSegment` is serializable |
| `crates/app-core/src/handlers/voice_conversation.rs` | Modify | Store loop handle, emit `chat:thread_created`/`chat:thread_updated`, set `conversation_type` |
| `crates/app-core/src/init/mod.rs` | Modify | Store loop handle in `AppCore`, wire privacy_mode from config |
| `crates/storage/src/repos/session.rs` | Modify | Add `upsert_session_with_type()` that sets `conversation_type` |
| `crates/app-core/src/lib.rs` or `core.rs` | Modify | Add `voice_loop_handle` field to `AppCore` |

---

### Task 1: Fix dual audio playback — add `stopTtsAudio()` to frontend

The backend plays TTS via `afplay` (native macOS) AND sends base64 audio to the frontend which plays via Web Audio API. Both play simultaneously. On interrupt, only `afplay` is killed — Web Audio keeps playing.

**Files:**
- Modify: `desktop-ui/src/shared/lib/audio.ts`
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts`

- [ ] **Step 1: Add source tracking and `stopTtsAudio()` to `audio.ts`**

```typescript
/**
 * Shared AudioContext — reused across calls to avoid autoplay policy issues.
 * Creating a new AudioContext per call can leave it in "suspended" state when
 * there hasn't been a direct user gesture in the WebView (e.g. the voice orb
 * is opened via a global hotkey, not a button click).
 */
let sharedCtx: AudioContext | null = null;
let activeSource: AudioBufferSourceNode | null = null;

function getAudioContext(): AudioContext {
  if (!sharedCtx || sharedCtx.state === "closed") {
    sharedCtx = new AudioContext();
  }
  return sharedCtx;
}

/**
 * Stop any currently playing TTS audio.
 */
export function stopTtsAudio(): void {
  if (activeSource) {
    try {
      activeSource.stop();
    } catch {
      // Already stopped — ignore
    }
    activeSource = null;
  }
}

/**
 * Decode base64-encoded PCM float32 samples and play via Web Audio API.
 * Uses fetch with a data URL instead of atob() for robust binary decoding.
 */
export async function playTtsAudio(base64: string, sampleRate: number): Promise<void> {
  if (!base64) return;

  // Stop any previous playback
  stopTtsAudio();

  const response = await fetch(`data:application/octet-stream;base64,${base64}`);
  const arrayBuffer = await response.arrayBuffer();
  const float32 = new Float32Array(arrayBuffer);

  if (float32.length === 0) return;

  const ctx = getAudioContext();
  // Resume if suspended (autoplay policy blocks audio without user gesture)
  if (ctx.state === "suspended") {
    console.log("[TTS] AudioContext suspended, resuming...");
    await ctx.resume();
  }

  console.log(`[TTS] Playing ${float32.length} samples at ${sampleRate}Hz (ctx.state=${ctx.state})`);

  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.onended = () => {
    if (activeSource === source) {
      activeSource = null;
    }
  };
  activeSource = source;
  source.start();
}
```

- [ ] **Step 2: Call `stopTtsAudio()` on fade-out and end in `useVoiceConversation.ts`**

Add import at line 3:

```typescript
import { playTtsAudio, stopTtsAudio } from "@shared/lib/audio";
```

In the `handleEvent` callback, update the `ttsFadeOut` case (around line 113):

```typescript
      case "ttsFadeOut":
        stopTtsAudio();
        // Delay clearing so CSS can animate a 300ms fade-out on the speaking visual
        setTimeout(() => setTtsAudio(null), 300);
        break;
```

In the `end` callback (around line 180):

```typescript
  const end = useCallback(async () => {
    stopTtsAudio();
    await ipc("voice_conversation_end");
    setPhase("idle");
    // Hide orb window if in Tauri
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().hide();
    }
  }, []);
```

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: PASS — no new lint errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/lib/audio.ts desktop-ui/src/features/voice/hooks/useVoiceConversation.ts
git commit -m "fix(voice): stop Web Audio playback on TTS interrupt and conversation end

Both afplay (native) and Web Audio API were playing simultaneously.
On interrupt, only afplay was killed. Now stopTtsAudio() is called
on ttsFadeOut and end to stop the Web Audio source node too.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Fix dead `segments` field — add segments to `PartialTranscript` event

The frontend reads `payload.segments` from `PartialTranscript` events, but the Rust `VoiceEvent::PartialTranscript` has no `segments` field. Word-level confidence highlighting is dead code.

**Files:**
- Modify: `crates/voice-engine/src/events.rs`
- Modify: `crates/voice-engine/src/service.rs`
- Test: existing tests in `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Add `segments` field to `PartialTranscript` in `events.rs`**

Replace the `PartialTranscript` variant (lines 18-22):

```rust
    PartialTranscript {
        text: String,
        language: String,
        is_final: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        segments: Vec<TranscriptSegmentEvent>,
    },
```

Add the segment event struct after the enum (before the `VOICE_EVENT` constant):

```rust
/// Lightweight segment data for frontend word-level confidence display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentEvent {
    pub text: String,
    pub confidence: f32,
}
```

Add the `crate::types::TranscriptSegment` import is NOT needed — we define a separate lightweight struct to avoid leaking internal types.

- [ ] **Step 2: Update `stop_capture()` in `service.rs` to pass segments**

In `stop_capture()`, find the `PartialTranscript` emission (around line 392). Replace:

```rust
                .send(VoiceEvent::PartialTranscript {
                    text: partial.text.clone(),
                    language: partial.language.as_str().to_string(),
                    is_final: partial.is_final,
                })
```

With:

```rust
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
```

- [ ] **Step 3: Fix all other `VoiceEvent::PartialTranscript` constructions in the codebase**

Search for any other places constructing `PartialTranscript` and add `segments: vec![]`:

Run: `cd /Users/jayden/Projects/Klynt/bot && grep -rn 'PartialTranscript {' --include='*.rs'`

For each match that constructs the event, add `segments: vec![]` if there are no segments to pass.

- [ ] **Step 4: Build and test**

Run: `cargo build -p voice-engine`
Expected: PASS

Run: `cargo nextest run -p voice-engine`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/events.rs crates/voice-engine/src/service.rs
git commit -m "fix(voice): add segments to PartialTranscript event for word-level confidence

The frontend expected payload.segments for word-level confidence
highlighting, but the Rust event had no segments field. Now segments
are passed through from the Whisper transcription engine.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Fix dropped loop handle — store the conversation loop JoinHandle

`spawn_loop()` returns a `JoinHandle<()>` but `init/mod.rs` drops it immediately (`let _loop_handle = ...`). A panic inside the conversation loop is silently lost with no restart.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:664`

- [ ] **Step 1: Find the AppCore struct definition**

Run: `grep -n 'voice_loop_handle\|voice_conversation_manager\|pub struct AppCore' crates/app-core/src/lib.rs crates/app-core/src/core.rs crates/app-core/src/init/mod.rs`

Look for where `AppCore` fields are defined to add the new field.

- [ ] **Step 2: Add `voice_loop_handle` field to AppCore**

Find the struct definition and add after `voice_conversation_manager`:

```rust
    voice_loop_handle: Option<tokio::task::JoinHandle<()>>,
```

Initialize it as `None` in the constructor, then update `init/mod.rs:664`:

Replace:
```rust
                let _loop_handle = voice_conv_manager.spawn_loop().await;
```

With:
```rust
                let loop_handle = voice_conv_manager.spawn_loop().await;
                core.voice_loop_handle = Some(loop_handle);
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/
git commit -m "fix(voice): store conversation loop JoinHandle instead of dropping it

The spawn_loop() JoinHandle was immediately dropped, silently losing
any panics inside the conversation loop. Now stored in AppCore so
it can be monitored or awaited during shutdown.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Fix missing thread events — emit `chat:thread_created`/`chat:thread_updated` after voice turns

`VoiceConversationManager::run_reflecting_phase` calls `process_direct_streaming` directly, bypassing `chat_send_voice` which calls `emit_chat_thread`. The chat sidebar never auto-refreshes for voice turns.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs:748-758` (after reflecting phase stores response)

- [ ] **Step 1: Add thread event emission after reflecting phase completes**

In `run_reflecting_phase()`, after the block that stores `pending_response_text` and increments `turn_count` (around line 748-758), add:

```rust
        // Emit chat thread event so the sidebar auto-refreshes
        {
            let state = self.state.lock().await;
            if let Some(ref sk) = state.session_key {
                let is_new = state.turn_count == 1; // first turn = new thread
                self.emitter.emit_chat_thread(is_new, sk.as_str());
            }
        }
```

This goes right before `self.transition_to(ConversationPhase::Speaking).await;` (line 758).

- [ ] **Step 2: Build**

Run: `cargo build -p app-core`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "fix(voice): emit chat thread events after voice turns

Voice conversations bypassed emit_chat_thread, so the chat sidebar
never auto-refreshed for voice sessions. Now emits thread_created
on first turn and thread_updated on subsequent turns.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Fix `conversation_type` — set it to `'voice'` for voice sessions

The `sessions` table has a `conversation_type` column (default `'general'`) but voice sessions never set it. The distinction is buried in an unqueryable JSON metadata blob.

**Files:**
- Modify: `crates/storage/src/repos/session.rs` — add `upsert_voice_session` method
- Modify: `crates/app-core/src/handlers/voice_conversation.rs:347-356` — use new method

- [ ] **Step 1: Add `upsert_voice_session` to SessionRepo**

In `crates/storage/src/repos/session.rs`, after `upsert_session` (around line 47), add:

```rust
    /// Upsert a voice session — same as `upsert_session` but also sets
    /// `conversation_type = 'voice'` on insert (preserved on conflict).
    pub async fn upsert_voice_session(
        &self,
        key: &str,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now = Utc::now();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, metadata, conversation_type, created_at, updated_at)
             VALUES (?1, ?2, 'voice', ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET
               updated_at = ?4,
               conversation_type = 'voice'
             RETURNING *",
        )
        .bind(key)
        .bind(metadata)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
```

- [ ] **Step 2: Use `upsert_voice_session` in `voice_conversation.rs`**

In `start()` (around line 352-356), replace:

```rust
        let _ = self
            .repos
            .sessions
            .upsert_session(session_key.as_str(), &metadata, None)
            .await;
```

With:

```rust
        let _ = self
            .repos
            .sessions
            .upsert_voice_session(session_key.as_str(), &metadata)
            .await;
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p storage -p app-core`
Expected: PASS

Run: `cargo nextest run -p storage`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/repos/session.rs crates/app-core/src/handlers/voice_conversation.rs
git commit -m "fix(voice): set conversation_type='voice' for voice sessions

Voice sessions used only a JSON metadata flag (is_voice_session) which
is unqueryable in SQLite. Now the conversation_type column is properly
set to 'voice' via upsert_voice_session.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Fix TTS WAV file collision — use unique filenames per turn

`handle_response()` always writes to `tts_output.wav`. Rapid turns can overwrite the file while `afplay` is still reading the previous one.

**Files:**
- Modify: `crates/voice-engine/src/service.rs:551-554`

- [ ] **Step 1: Generate unique WAV filename per TTS call**

In `handle_response()` (around line 551), replace:

```rust
                    let wav_path = self.config.data_dir.join("tts_output.wav");
```

With:

```rust
                    let wav_path = self.config.data_dir.join(format!(
                        "tts_{}.wav",
                        chrono::Utc::now().timestamp_millis()
                    ));
```

And add cleanup after `afplay` finishes. Replace the `spawn_blocking` block (lines 557-560):

```rust
                    if let Ok(child) = child {
                        *self.tts_playback_pid.lock().await = Some(child.id());
                        tokio::task::spawn_blocking(move || {
                            let mut child = child;
                            let _ = child.wait();
                            // Clean up the temp WAV file after playback
                            let _ = std::fs::remove_file(&wav_path);
                        });
                    }
```

- [ ] **Step 2: Update `AvSpeechTtsEngine::synthesize()` to accept the output path**

Check how `synthesize()` writes the WAV. If it uses a hardcoded path, it also needs updating. Read `crates/voice-engine/src/engines/avspeech.rs` to verify.

The `synthesize()` method in `avspeech.rs` writes to `self.data_dir.join("tts_output.wav")`. Update it to accept an optional output path, OR change the `handle_response` in `service.rs` to create the unique path and pass it to the `say` command directly.

The simpler fix: change the `data_dir` field on `AvSpeechTtsEngine` to have `synthesize` return the wav path along with the clip. Since `synthesize()` already writes to a fixed path, change it to use a unique temp file:

In `crates/voice-engine/src/engines/avspeech.rs`, find where `tts_output.wav` is constructed and replace with:

```rust
let wav_path = self.data_dir.join(format!("tts_{}.wav", chrono::Utc::now().timestamp_millis()));
```

Then in `service.rs:handle_response`, after `tts.synthesize()` returns, read the WAV path from the clip metadata or use the same timestamp approach. Since both use `Utc::now()`, generate the filename once and pass it through.

**Alternative simpler approach:** Keep the pattern as-is but just change the filename in `avspeech.rs` to use a timestamp. The `service.rs` already reads `data_dir.join("tts_output.wav")` for `afplay` — so both files need the same name. The cleanest fix is to add the path to `AudioClip`:

Actually, the simplest fix: `handle_response` doesn't need to know the path because `afplay` reads the file that `synthesize` wrote. So just change the path in `avspeech.rs` and have `synthesize` return the path in `AudioClip`. But that's a bigger refactor.

**Simplest safe fix:** Pre-generate the filename in `handle_response`, pass it to `synthesize` via `TtsParams`, then use the same path for `afplay`. This requires adding a field to `TtsParams`:

In `crates/voice-engine/src/tts.rs`, add to `TtsParams`:

```rust
    /// Output file path for WAV synthesis (used by file-based TTS engines).
    pub output_path: Option<std::path::PathBuf>,
```

In `service.rs:handle_response`, set:

```rust
                let wav_path = self.config.data_dir.join(format!(
                    "tts_{}.wav",
                    chrono::Utc::now().timestamp_millis()
                ));
                let params = TtsParams {
                    output_path: Some(wav_path.clone()),
                    ..TtsParams::default()
                };
```

In `avspeech.rs:synthesize`, use `params.output_path` if provided:

```rust
let wav_path = params.output_path.clone().unwrap_or_else(|| self.data_dir.join("tts_output.wav"));
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p voice-engine`
Expected: PASS

Run: `cargo nextest run -p voice-engine`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/
git commit -m "fix(voice): use unique WAV filename per TTS call to prevent file collision

handle_response() always wrote to tts_output.wav, which could be
overwritten while afplay was still reading the previous turn's audio.
Now uses timestamped filenames and cleans up after playback.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Wire `VoiceConfig` values — privacy_mode, speaking_rate, voice_preferences

Config values for `privacy_mode`, `speaking_rate`, and `voice_preferences` exist in the schema but are never read at runtime. `PrivacyLevel::Standard` and `TtsParams::default()` are hardcoded.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:619-622` — wire privacy_mode from config
- Modify: `crates/voice-engine/src/service.rs:535` — read TTS config for params
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` — pass config to `handle_response`

- [ ] **Step 1: Wire privacy_mode from config in init**

In `init/mod.rs`, find where `VoiceServiceConfig` is created (around line 612-622). Replace:

```rust
                    privacy_mode: PrivacyLevel::Standard,
```

With:

```rust
                    privacy_mode: match voice_config.input.privacy_mode {
                        config::schema::VoicePrivacyMode::Standard => PrivacyLevel::Standard,
                        config::schema::VoicePrivacyMode::Strict => PrivacyLevel::Strict,
                        config::schema::VoicePrivacyMode::Off => PrivacyLevel::Off,
                    },
```

Check that the `VoicePrivacyMode` enum variants match `PrivacyLevel` variants. Read `crates/config/src/schema/voice.rs` and `crates/voice-engine/src/types.rs` to confirm.

- [ ] **Step 2: Pass speaking_rate and voice to TTS params in `handle_response`**

The voice config needs to be accessible in `VoiceConversationManager` during speaking phase. The manager already has `config: Arc<RwLock<VoiceConfig>>`.

In `run_speaking_phase()`, before calling `self.voice_service.handle_response(&tts_text)`, read the config:

```rust
        let tts_params = {
            let config = self.config.read().await;
            voice_engine::TtsParams {
                rate: config.output.speaking_rate,
                voice_name: config.output.voice_preferences.first().cloned(),
                ..Default::default()
            }
        };
```

But `handle_response` currently creates its own `TtsParams::default()`. Modify it to accept params:

In `service.rs`, change `handle_response` signature:

```rust
pub async fn handle_response(&self, response_text: &str, params: &TtsParams) -> common::Result<()> {
```

Remove the internal `let params = TtsParams::default();` line.

Update the call in `voice_conversation.rs:run_speaking_phase` to pass the params.

Also update the legacy `voice.rs` handler call to pass `TtsParams::default()`.

- [ ] **Step 3: Build and test**

Run: `cargo build --workspace`
Expected: PASS

Run: `cargo nextest run -p voice-engine -p app-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/ crates/voice-engine/src/service.rs
git commit -m "fix(voice): wire privacy_mode, speaking_rate, and voice_preferences from config

These config values existed in the schema but were never read at
runtime. Now privacy_mode controls memory echo behavior, and TTS
params (rate, voice) are read from VoiceConfig.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Remove dead `segments` state from frontend (cleanup after Task 2)

After Task 2 adds segments to the event, the frontend `segments` state will work correctly. But the `setSegments` in `useVoiceConversation.ts` needs no changes since Task 2 makes the backend send segments. This task just verifies the integration works end-to-end.

**Files:**
- Verify: `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts:78-80`
- Verify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` — confirm segments are used

- [ ] **Step 1: Verify the `partialTranscript` handler reads segments correctly**

The handler at line 78 already reads `payload.segments`. After Task 2, the backend now sends segments. Verify the shape matches:

Backend sends: `segments: [{ text: "hello", confidence: 0.95 }]`
Frontend reads: `payload.segments as Array<{ text: string; confidence: number }>`

These match. No frontend changes needed.

- [ ] **Step 2: Verify VoiceBrainOrb uses segments**

Check that `VoiceBrainOrb.tsx` consumes the `segments` prop from the hook and renders them. If it doesn't currently render segments, no changes needed — the data is now available for when the UI wants to use it.

- [ ] **Step 3: Run full lint + type check**

Run: `cd desktop-ui && bun run lint`
Expected: PASS

---

### Task 9: Full workspace build verification

- [ ] **Step 1: Format check**

Run: `cargo fmt --all --check`
Expected: PASS (no formatting issues)

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Test all affected crates**

Run: `cargo nextest run -p voice-engine -p storage -p app-core`
Expected: All tests PASS

- [ ] **Step 4: Frontend lint + test**

Run: `cd desktop-ui && bun run lint && bun run test`
Expected: PASS

- [ ] **Step 5: Full workspace build**

Run: `cargo build --workspace`
Expected: PASS
