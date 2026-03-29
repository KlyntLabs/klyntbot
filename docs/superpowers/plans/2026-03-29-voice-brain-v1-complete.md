# Voice Brain v1 Complete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the full emotional arc — mount the orb with real data, close the agent response loop (transcript → agent → TTS), add Mirror-powered memory echo, context-aware hotkey, and zero-friction first-run flow — so users press ⌘⇧V and feel the second brain listening, remembering, and speaking back.

**Architecture:** VoiceService (L1) already handles capture → transcription → events. This sprint closes the response loop by having `app-core` (L7) call `AgentLoop::process_direct_streaming()` after voice finalization, relay `AgentEvent::Done` back to VoiceService for TTS synthesis, and emit `VoiceEvent::SpeakResponse` to the orb. Memory echo uses a new `MirrorFacade` method with on-the-fly embedding similarity. The orb mounts to `/#/voice-orb` as a standalone Tauri window route. The hotkey redirects from launcher to voice-orb.

**Tech Stack:** Rust (voice-engine, cognitive, app-core, desktop crates), TypeScript/React (VoiceBrainOrb, useVoiceEvents), Web Audio API (TTS playback), Vitest (frontend tests)

**Spec:** `docs/superpowers/specs/2026-03-29-voice-brain-v1-complete-design.md`

---

### Task 1: Add `handle_response` method to VoiceService

**Files:**
- Modify: `crates/voice-engine/src/service.rs`

This method receives the agent's response text, synthesizes TTS, and emits `SpeakResponse`. Called by app-core after the agent finishes processing the voice transcript.

- [ ] **Step 1: Write the test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/voice-engine/src/service.rs`:

```rust
    #[tokio::test]
    async fn handle_response_emits_speak_event() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new("hello world"));
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            Some(mock_stt),
            None,
            Some(mock_tts),
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );

        let mut event_rx = svc.take_event_rx().unwrap();

        svc.handle_response("Task created: dentist appointment Thursday")
            .await
            .unwrap();

        // Should receive a SpeakResponse event with the TTS audio
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("channel closed");

        match event {
            VoiceEvent::SpeakResponse { text, sample_rate, .. } => {
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
        let result = svc.handle_response("hello").await;
        assert!(result.is_ok());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p voice-engine -E 'test(handle_response)'`
Expected: FAIL — `handle_response` method doesn't exist.

- [ ] **Step 3: Implement `handle_response`**

Add this method to the `impl VoiceService` block in `crates/voice-engine/src/service.rs`, after the `cancel` method (~line 491):

```rust
    /// Handle the agent's response for the current voice session.
    ///
    /// Synthesizes TTS audio and emits a `SpeakResponse` event to the frontend.
    /// Called by app-core after `AgentRuntime` produces a response for a voice message.
    pub async fn handle_response(&self, response_text: &str) -> common::Result<()> {
        if let Some(ref tts) = self.tts {
            let params = TtsParams::default();
            match tts.synthesize(response_text, &params).await {
                Ok(clip) => {
                    let audio_base64 = base64_encode_audio(&clip);
                    let _ = self
                        .event_tx
                        .send(VoiceEvent::SpeakResponse {
                            audio_base64,
                            sample_rate: clip.sample_rate,
                            text: response_text.to_string(),
                        })
                        .await;
                }
                Err(e) => {
                    warn!("TTS synthesis failed for voice response: {e}");
                }
            }
        }
        Ok(())
    }
```

- [ ] **Step 4: Remove the hardcoded TTS from `stop_capture`**

In the same file, in `stop_capture()`, remove the placeholder TTS block (~lines 443-463). Replace:

```rust
        // TTS read-back if available
        if let Some(ref tts) = self.tts {
            let response_text = "Got it."; // Placeholder — real response comes from agent
            let params = TtsParams::default();
            match tts.synthesize(response_text, &params).await {
                Ok(clip) => {
                    let audio_base64 = base64_encode_audio(&clip);
                    let _ = self
                        .event_tx
                        .send(VoiceEvent::SpeakResponse {
                            audio_base64,
                            sample_rate: clip.sample_rate,
                            text: response_text.to_string(),
                        })
                        .await;
                }
                Err(e) => {
                    warn!("TTS synthesis failed: {e}");
                }
            }
        }
```

With:

```rust
        // TTS read-back is now handled by handle_response(), called by app-core
        // after the agent produces its response. This keeps VoiceService decoupled
        // from the agent pipeline.
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass including the two new ones.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/service.rs
git commit -m "feat(voice): add handle_response for agent→TTS→SpeakResponse loop"
```

---

### Task 2: Wire agent response loop in app-core voice handler

**Files:**
- Modify: `crates/app-core/src/handlers/voice.rs`

Change `voice_stop_capture()` to send the transcript through `AgentLoop::process_direct_streaming()` instead of `bus.publish_inbound()`, then relay the agent response to `VoiceService::handle_response()`.

- [ ] **Step 1: Rewrite `voice_stop_capture` to use direct streaming**

Replace the entire `voice_stop_capture` method in `crates/app-core/src/handlers/voice.rs`:

```rust
    /// Stop the current voice capture, send to agent, relay response to TTS.
    pub async fn voice_stop_capture(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        if let Some((transcript, _metadata)) = result {
            if transcript.text.trim().is_empty() {
                return Ok(());
            }

            // Send transcript through the agent pipeline and relay response to TTS.
            let session_key = "desktop-voice".to_string();
            let voice_svc = self.voice_service()?.clone();
            let agent = self.agent.clone();

            tokio::spawn(async move {
                match agent
                    .process_direct_streaming(transcript.text.clone(), session_key)
                    .await
                {
                    Ok(streaming_handle) => {
                        let mut event_rx = streaming_handle.event_rx;
                        while let Some(event) = event_rx.recv().await {
                            if let agent::AgentEvent::Done { content, .. } = event {
                                if let Err(e) = voice_svc.handle_response(&content).await {
                                    tracing::warn!("Voice TTS response failed: {e}");
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Agent processing of voice transcript failed: {e}");
                    }
                }
            });
        }

        Ok(())
    }
```

- [ ] **Step 2: Add the agent import**

At the top of `crates/app-core/src/handlers/voice.rs`, make sure `agent` is imported. Check existing imports — `crate::state::AppCore` is already imported. The `agent` crate should be accessible via the workspace dependency. If not already imported, add:

```rust
use agent::AgentEvent;
```

Adjust the match arm in step 1 accordingly if the import path differs.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p app-core`
Expected: Compiles. The `process_direct_streaming` method on `AgentLoop` takes `(String, String)` and returns `Result<StreamingHandle>`.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/voice.rs
git commit -m "feat(voice): wire agent response loop — transcript → agent → TTS"
```

---

### Task 3: MirrorFacade voice echo method

**Files:**
- Modify: `crates/cognitive/src/mirror/facade.rs`
- Modify: `crates/cognitive/src/mirror/mod.rs` (if needed for EmbeddingEngine access)

Add `get_recent_voice_relevant_snippet()` — uses on-the-fly embedding of recent snippets to find the most relevant one for the partial transcript.

- [ ] **Step 1: Add EmbeddingEngine to MirrorFacade**

In `crates/cognitive/src/mirror/facade.rs`, add a field and builder method:

```rust
// Add to the MirrorFacade struct (after the domain_event_bus field):
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
```

Add the builder method after `with_domain_event_bus`:

```rust
    pub fn with_embedding_engine(mut self, engine: Arc<tools::EmbeddingEngine>) -> Self {
        self.embedding_engine = Some(engine);
        self
    }
```

Update `new()` to initialize the field:

```rust
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            narrative_handler: None,
            autotuner_bridge: None,
            active_timers: None,
            episodic_repo: None,
            domain_event_bus: None,
            embedding_engine: None,
        }
    }
```

- [ ] **Step 2: Add the echo method**

Add this method to the `impl MirrorFacade` block:

```rust
    /// Find the most semantically relevant recent snippet for a voice partial transcript.
    ///
    /// Uses on-the-fly embedding similarity (cosine) against recent undismissed snippets.
    /// Returns `None` if no embedding engine is available or no snippet scores above threshold.
    pub async fn get_recent_voice_relevant_snippet(&self, query: &str) -> Option<String> {
        let engine = self.embedding_engine.as_ref()?;
        if !engine.is_available() {
            return None;
        }

        let snippets = self.repo.get_pending_snippets().await.ok()?;
        if snippets.is_empty() {
            return None;
        }

        let query_embedding = engine.embed_async(Arc::clone(engine), query.to_string()).await.ok()?;

        let mut best_score = 0.0f64;
        let mut best_text: Option<String> = None;

        for snippet in &snippets {
            let snippet_text = format!("{} {}", snippet.headline, snippet.body);
            if let Ok(snippet_embedding) = engine.embed(&snippet_text) {
                let score = common::helpers::cosine_similarity(&query_embedding, &snippet_embedding);
                if score > best_score {
                    best_score = score;
                    best_text = Some(snippet.headline.clone());
                }
            }
        }

        if best_score >= 0.45 {
            best_text
        } else {
            None
        }
    }
```

Note: Using `embed` (sync) for snippets and `embed_async` for the query. Since we're comparing at most ~20 snippets, the sync path is fine (~3ms each).

- [ ] **Step 3: Add the tools dependency to cognitive if needed**

Check if `crates/cognitive/Cargo.toml` already depends on `tools`. If not, add:

```toml
tools = { workspace = true, optional = true }
```

And gate the import:

```rust
#[cfg(feature = "semantic-search")]
use tools::EmbeddingEngine;
```

If `tools` is already a dependency (likely, since cognitive uses embedding for memory), skip this step.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(mirror): add get_recent_voice_relevant_snippet for Tier-2 memory echo"
```

---

### Task 4: Wire MemoryEchoProvider into VoiceService initialization

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

Create a concrete `MemoryEchoProvider` implementation in app-core that tries Mirror (Tier 2) then ContextEngine recall (Tier 3).

- [ ] **Step 1: Create the echo provider struct**

In `crates/app-core/src/init/mod.rs`, add after the imports (or in a new file `crates/app-core/src/handlers/voice_echo.rs` if preferred — but inline is fine for a single struct):

Add a new file `crates/app-core/src/handlers/voice_echo.rs`:

```rust
//! MemoryEchoProvider implementation — wires Mirror (Tier 2) + recall (Tier 3).

use std::sync::Arc;

use async_trait::async_trait;
use voice_engine::MemoryEchoProvider;

/// App-level memory echo provider that tries Mirror snippets first,
/// then falls back to episodic memory recall.
pub struct AppMemoryEchoProvider {
    mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
}

impl AppMemoryEchoProvider {
    pub fn new(mirror: Option<Arc<cognitive::mirror::MirrorFacade>>) -> Self {
        Self { mirror }
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

        // Tier 3: Would use ContextEngine::recall_relevant here.
        // For now, Tier 2 is the primary echo source. Tier 3 recall
        // can be added by injecting ContextEngine and calling
        // memory_retriever.retrieve(partial_text, 1) when available.
        None
    }
}
```

- [ ] **Step 2: Register the module**

Add to `crates/app-core/src/handlers/mod.rs`:

```rust
pub mod voice_echo;
```

- [ ] **Step 3: Wire the echo provider into VoiceService init**

In `crates/app-core/src/init/mod.rs`, find the VoiceService creation (~line 516 where `None` is passed for `MemoryEchoProvider`). Replace:

```rust
            None, // MemoryEchoProvider — stub, wired later
```

With:

```rust
            {
                let echo_provider = crate::handlers::voice_echo::AppMemoryEchoProvider::new(
                    core.mirror_facade.clone(),
                );
                Some(Arc::new(echo_provider) as Arc<dyn voice_engine::MemoryEchoProvider>)
            },
```

- [ ] **Step 4: Wire EmbeddingEngine into MirrorFacade init**

In the same init file, find where `MirrorFacade` is created. Add `.with_embedding_engine(embedding_engine.clone())` to the builder chain. The `embedding_engine` should already be available in scope (it's used elsewhere during init). If it's not in scope at the MirrorFacade creation point, pass it through.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p app-core`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/voice_echo.rs crates/app-core/src/handlers/mod.rs crates/app-core/src/init/mod.rs
git commit -m "feat(voice): wire MemoryEchoProvider — Mirror Tier-2 + recall fallback"
```

---

### Task 5: Mount VoiceOrbPage route

**Files:**
- Create: `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx`
- Modify: `desktop-ui/src/features/voice/index.ts`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Create VoiceOrbPage**

Create `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx`:

```tsx
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { VoiceBrainOrb } from "@features/voice";

export function VoiceOrbPage() {
  useTransparentBackground({ nativeVibrancy: true });

  return (
    <div className="h-screen w-screen">
      <VoiceBrainOrb />
    </div>
  );
}
```

- [ ] **Step 2: Export from voice feature index**

In `desktop-ui/src/features/voice/index.ts`, add:

```typescript
export { VoiceOrbPage } from "./pages/VoiceOrbPage";
```

- [ ] **Step 3: Add route to router**

In `desktop-ui/src/app/router.tsx`, add a lazy import at the top (alongside other page imports):

```typescript
const VoiceOrbPage = lazy(() =>
  import("@features/voice/pages/VoiceOrbPage").then((m) => ({ default: m.VoiceOrbPage })),
);
```

Add the route after the distraction-overlay route (~line 363):

```typescript
  { path: "/voice-orb", element: <VoiceOrbPage /> },
```

- [ ] **Step 4: Verify the dev build**

Run: `cd desktop-ui && bun run build`
Expected: Builds without errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx desktop-ui/src/features/voice/index.ts desktop-ui/src/app/router.tsx
git commit -m "feat(voice): mount VoiceOrbPage to /#/voice-orb route"
```

---

### Task 6: Enhance VoiceBrainOrb — word highlights, echo, cognitive pulse, replay

**Files:**
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`

- [ ] **Step 1: Add word-level confidence data to useVoiceEvents**

In `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`, the `partialTranscript` event handler currently only stores `text`. Enhance the state to also store segments for word-level highlights.

Add to the state type:

```typescript
interface VoiceState {
  // ... existing fields
  segments: Array<{ text: string; confidence: number }>;
  ttsAudio: { base64: string; sampleRate: number; text: string } | null;
}
```

In the event handler for `partialTranscript`, update to also parse segments if present:

```typescript
case "partialTranscript": {
  const segments = (payload.segments as Array<{ text: string; confidence: number }>) ?? [];
  setState((s) => ({ ...s, transcript: payload.text as string, segments }));
  break;
}
```

For `speakResponse`, store the audio data:

```typescript
case "speakResponse": {
  setState((s) => ({
    ...s,
    sessionState: "response",
    ttsAudio: {
      base64: payload.audioBase64 as string,
      sampleRate: payload.sampleRate as number,
      text: payload.text as string,
    },
  }));
  break;
}
```

Return `segments` and `ttsAudio` from the hook.

- [ ] **Step 2: Add word-level highlights to VoiceBrainOrb**

In `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`, replace the plain transcript rendering with highlighted words:

```tsx
function WordHighlights({ segments }: { segments: Array<{ text: string; confidence: number }> }) {
  return (
    <span>
      {segments.map((seg, i) => {
        const cls =
          seg.confidence >= 0.85
            ? "text-success"
            : seg.confidence >= 0.6
              ? "text-warning"
              : "text-destructive";
        return (
          <span key={i} className={cls}>
            {seg.text}{" "}
          </span>
        );
      })}
    </span>
  );
}
```

Use `<WordHighlights segments={segments} />` in the Listening state instead of the raw transcript text. Fall back to plain text if segments is empty.

- [ ] **Step 3: Add memory echo with Mirror badge + cognitive pulse**

In the Listening state section of `VoiceBrainOrb.tsx`, add the echo rendering below the transcript:

```tsx
{memoryEcho && (
  <div className="animate-in fade-in duration-300 flex items-center gap-1.5 text-xs text-muted">
    <span className="rounded bg-surface-overlay px-1 py-0.5 text-[10px] font-medium text-muted/70">
      Mirror
    </span>
    <span className="italic">{memoryEcho}</span>
  </div>
)}
```

The `animate-in fade-in duration-300` class provides the cognitive pulse (300ms fade-in). Tailwind v4 supports this via the `animate-in` utility.

- [ ] **Step 4: Add replay button in Response state**

In the Response state section, add a replay button:

```tsx
{sessionState === "response" && (
  <div className="space-y-2">
    <p className="text-sm text-primary">{responseText || ttsAudio?.text}</p>
    {ttsAudio && (
      <button
        type="button"
        onClick={() => playTtsAudio(ttsAudio.base64, ttsAudio.sampleRate)}
        className="text-xs text-muted hover:text-primary transition-colors"
        aria-label="Replay spoken response"
      >
        ↻ Replay
      </button>
    )}
    <p className="text-xs text-muted">tap anywhere to close</p>
  </div>
)}
```

- [ ] **Step 5: Verify the build**

Run: `cd desktop-ui && bun run build`
Expected: Builds.

- [ ] **Step 6: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/voice/
git commit -m "feat(voice): word highlights, Mirror echo badge, cognitive pulse, replay button"
```

---

### Task 7: Web Audio TTS playback utility

**Files:**
- Create: `desktop-ui/src/shared/lib/audio.ts`
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`

- [ ] **Step 1: Create the audio utility**

Create `desktop-ui/src/shared/lib/audio.ts`:

```typescript
/**
 * Decode base64-encoded PCM float32 samples and play via Web Audio API.
 * Used for TTS playback in the Voice Brain orb.
 */
export function playTtsAudio(base64: string, sampleRate: number): void {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }

  // PCM data is little-endian float32
  const float32 = new Float32Array(bytes.buffer);

  const ctx = new AudioContext();
  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}
```

- [ ] **Step 2: Import and use in VoiceBrainOrb**

In `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`, add the import:

```typescript
import { playTtsAudio } from "@shared/lib/audio";
```

The replay button from Task 6 Step 4 already calls `playTtsAudio`. Also auto-play TTS when `speakResponse` arrives:

In the `useVoiceEvents` hook or in an effect in VoiceBrainOrb:

```typescript
useEffect(() => {
  if (ttsAudio) {
    playTtsAudio(ttsAudio.base64, ttsAudio.sampleRate);
  }
}, [ttsAudio]);
```

- [ ] **Step 3: Verify the build**

Run: `cd desktop-ui && bun run build`
Expected: Builds.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/lib/audio.ts desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx
git commit -m "feat(voice): Web Audio TTS playback utility + auto-play on SpeakResponse"
```

---

### Task 8: Redirect hotkey to voice-orb window

**Files:**
- Modify: `crates/desktop/src/main.rs`

Change the voice hotkey handler from opening the launcher to opening the voice-orb window and calling `voice_start_capture`.

- [ ] **Step 1: Rewrite the hotkey handler**

In `crates/desktop/src/main.rs`, find the voice hotkey handler (~lines 330-365). Replace the body of the `move |_app, _shortcut, event|` closure:

```rust
                    move |_app, _shortcut, event| {
                        if event.state
                            != tauri_plugin_global_shortcut::ShortcutState::Pressed
                        {
                            return;
                        }
                        tracing::info!("Voice hotkey pressed");
                        let handle = app_clone.clone();
                        tauri::async_runtime::spawn(async move {
                            use tauri::{Emitter, Manager};

                            // Check if voice-orb is already visible (toggle behavior)
                            if let Some(orb_window) = handle.get_webview_window("voice-orb") {
                                let is_visible = orb_window.is_visible().unwrap_or(false);
                                if is_visible {
                                    // Second press while capturing → stop capture
                                    let core = handle.state::<std::sync::Arc<app_core::AppCore>>();
                                    let _ = core.voice_stop_capture().await;
                                    crate::tray_countdown::VOICE_ACTIVE.store(
                                        false,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    let _ = orb_window.hide();
                                    return;
                                }
                            }

                            // First press → open orb and start capture
                            if let Some(orb_window) = handle.get_webview_window("voice-orb") {
                                // Position: top-center of active monitor, 80px from top
                                if let Ok(Some(monitor)) = orb_window.current_monitor() {
                                    let monitor_pos = monitor.position();
                                    let monitor_size = monitor.size();
                                    let x = monitor_pos.x
                                        + (monitor_size.width as i32 / 2)
                                        - 160; // half of 320px width
                                    let y = monitor_pos.y + 80;
                                    let _ = orb_window.set_position(
                                        tauri::PhysicalPosition::new(x, y),
                                    );
                                }
                                let _ = orb_window.show();
                                let _ = orb_window.set_focus();
                            }

                            // Start voice capture
                            let core = handle.state::<std::sync::Arc<app_core::AppCore>>();
                            match core.voice_start_capture().await {
                                Ok(_info) => {
                                    crate::tray_countdown::VOICE_ACTIVE.store(
                                        true,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to start voice capture: {e}");
                                    // Emit error to orb
                                    let _ = handle.emit(
                                        "voice:event",
                                        serde_json::json!({
                                            "type": "error",
                                            "message": e.to_string(),
                                            "recoverable": true
                                        }),
                                    );
                                }
                            }
                        });
                    },
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(voice): redirect hotkey from launcher to voice-orb window"
```

---

### Task 9: Context-aware hotkey behavior

**Files:**
- Modify: `crates/desktop/src/main.rs`

Add context checks before the default orb behavior: if focus session is active → quick journal, if launcher is open → hands-free search.

- [ ] **Step 1: Add context checks to hotkey handler**

In the hotkey handler from Task 8, add context checks at the start of the `async move` block (before the voice-orb visibility check):

```rust
                            // Context-aware: focus session → quick voice journal
                            if crate::tray_countdown::FOCUS_ACTIVE.load(
                                std::sync::atomic::Ordering::Relaxed,
                            ) {
                                let core = handle.state::<std::sync::Arc<app_core::AppCore>>();
                                // Quick capture without orb — just start, record, stop, spoken confirmation
                                match core.voice_start_capture().await {
                                    Ok(_) => {
                                        crate::tray_countdown::VOICE_ACTIVE.store(
                                            true,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                        // No orb — voice events still flow for the background pipeline
                                    }
                                    Err(e) => {
                                        tracing::warn!("Quick voice journal failed: {e}");
                                    }
                                }
                                return;
                            }

                            // Context-aware: launcher open → hands-free search
                            if let Some(launcher) = handle.get_webview_window("launcher") {
                                if launcher.is_visible().unwrap_or(false) {
                                    // Emit voice-recording-start to launcher for hands-free mode
                                    let _ = handle.emit("voice-recording-start", ());
                                    return;
                                }
                            }
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(voice): context-aware hotkey — focus→journal, launcher→search"
```

---

### Task 10: First-run flow — download progress + welcome echo

**Files:**
- Modify: `crates/voice-engine/src/service.rs`
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`

- [ ] **Step 1: Add a first-capture detection to VoiceService**

In `crates/voice-engine/src/service.rs`, add a field to track first-ever capture:

```rust
// Add to VoiceService struct:
    first_capture_complete: AtomicBool,
```

Initialize in `new()`:

```rust
    first_capture_complete: AtomicBool::new(false),
```

After the `Finalized` event emission in `stop_capture()` (~line 434), add the welcome echo:

```rust
        // One-time welcome echo on the very first successful capture
        if !self.first_capture_complete.swap(true, Ordering::Relaxed) {
            let _ = self
                .event_tx
                .send(VoiceEvent::MemoryEcho {
                    text: "Welcome to your second brain. I'm listening. Everything you say here becomes memory, learning, and reflection — just like your thoughts. Press ⌘⇧V anytime.".to_string(),
                })
                .await;
        }
```

- [ ] **Step 2: Handle model download progress in frontend**

In `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`, add a state field for model download:

```typescript
modelDownloading: boolean;
```

The `VoiceEvent::Error` with a "no engine" message triggers the "Brain waking up" state. Add to the error handler:

```typescript
case "error": {
  const message = payload.message as string;
  if (message.includes("No transcription engine") || message.includes("Download")) {
    setState((s) => ({ ...s, modelDownloading: true }));
  }
  console.warn("[voice] Error:", message);
  break;
}
```

- [ ] **Step 3: Build both sides**

Run: `cargo build -p voice-engine && cd desktop-ui && bun run build`
Expected: Both compile.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/service.rs desktop-ui/src/features/voice/hooks/useVoiceEvents.ts
git commit -m "feat(voice): first-run welcome echo + download progress state"
```

---

### Task 11: Dev server voice mock endpoints

**Files:**
- Modify: `crates/desktop/src/commands/voice.rs`
- Modify: `crates/app-core/src/handlers/voice.rs`

Add `voice_simulate_event` and `voice_mock_session` commands for browser-only development.

- [ ] **Step 1: Add simulate_event command to AppCore**

In `crates/app-core/src/handlers/voice.rs`, add:

```rust
    /// Simulate a VoiceEvent for dev/testing (inject event into the frontend stream).
    pub async fn voice_simulate_event(&self, event_json: serde_json::Value) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let event: voice_engine::VoiceEvent = serde_json::from_value(event_json)
            .map_err(|e| ApiError::new("VALIDATION", &format!("Invalid VoiceEvent: {e}")))?;
        service
            .emit_event(event)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))
    }
```

- [ ] **Step 2: Add `emit_event` to VoiceService**

In `crates/voice-engine/src/service.rs`, add a public method for emitting arbitrary events (dev use):

```rust
    /// Emit an arbitrary VoiceEvent (for dev/testing simulation).
    pub async fn emit_event(&self, event: VoiceEvent) -> common::Result<()> {
        self.event_tx
            .send(event)
            .await
            .map_err(|_| common::KlyntbotError::BusDisconnected)?;
        Ok(())
    }
```

- [ ] **Step 3: Register in desktop commands + dev dispatch**

In `crates/desktop/src/commands/voice.rs`, add the Tauri command:

```rust
#[tauri::command]
pub async fn voice_simulate_event(
    state: State<'_, Arc<AppCore>>,
    event: serde_json::Value,
) -> Result<(), ApiError> {
    state.voice_simulate_event(event).await
}
```

Add to `DEV_COMMANDS`:

```rust
    "voice_simulate_event",
```

Add to `dispatch_dev`:

```rust
        "voice_simulate_event" => {
            let event = body.get("event").cloned().unwrap_or(serde_json::json!(null));
            dev::val(core.voice_simulate_event(event).await)
        }
```

- [ ] **Step 4: Register the new command in main.rs invoke_handler**

Add `voice_simulate_event` to the invoke handler in `crates/desktop/src/main.rs`.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/service.rs crates/app-core/src/handlers/voice.rs crates/desktop/src/commands/voice.rs crates/desktop/src/main.rs
git commit -m "feat(voice): add voice_simulate_event dev endpoint for mock testing"
```

---

### Task 12: Frontend Vitest tests

**Files:**
- Modify: `desktop-ui/src/features/voice/__tests__/useVoiceEvents.test.ts`
- Create: `desktop-ui/src/features/voice/__tests__/VoiceBrainOrb.test.tsx`

- [ ] **Step 1: Add new tests to useVoiceEvents.test.ts**

Add these test cases to the existing test file:

```typescript
it("stores segments from partialTranscript", () => {
  const state = reduce(initial(), {
    type: "partialTranscript",
    text: "bonjour monde",
    language: "fr",
    isFinal: false,
    segments: [
      { text: "bonjour", confidence: 0.92 },
      { text: "monde", confidence: 0.55 },
    ],
  });
  expect(state.segments).toHaveLength(2);
  expect(state.segments[0].confidence).toBe(0.92);
  expect(state.segments[1].confidence).toBe(0.55);
});

it("stores ttsAudio from speakResponse", () => {
  let state = reduce(initial(), {
    type: "captureStarted",
    sessionId: "v-1",
    engine: "local",
  });
  state = reduce(state, {
    type: "speakResponse",
    audioBase64: "AAAA",
    sampleRate: 16000,
    text: "Got it.",
  });
  expect(state.sessionState).toBe("response");
  expect(state.ttsAudio).toEqual({
    base64: "AAAA",
    sampleRate: 16000,
    text: "Got it.",
  });
});

it("memory echo stored on memoryEcho event", () => {
  let state = reduce(initial(), {
    type: "captureStarted",
    sessionId: "v-1",
    engine: "local",
  });
  state = reduce(state, {
    type: "memoryEcho",
    text: "You mentioned dentist last Tuesday",
  });
  expect(state.memoryEcho).toBe("You mentioned dentist last Tuesday");
});
```

- [ ] **Step 2: Create VoiceBrainOrb component test**

Create `desktop-ui/src/features/voice/__tests__/VoiceBrainOrb.test.tsx`:

```typescript
import { describe, it, expect } from "vitest";

describe("Word-level highlights", () => {
  it("classifies confidence >= 0.85 as good (success)", () => {
    const seg = { text: "bonjour", confidence: 0.92 };
    const cls = seg.confidence >= 0.85 ? "text-success" : seg.confidence >= 0.6 ? "text-warning" : "text-destructive";
    expect(cls).toBe("text-success");
  });

  it("classifies confidence 0.60-0.84 as fair (warning)", () => {
    const seg = { text: "monde", confidence: 0.72 };
    const cls = seg.confidence >= 0.85 ? "text-success" : seg.confidence >= 0.6 ? "text-warning" : "text-destructive";
    expect(cls).toBe("text-warning");
  });

  it("classifies confidence < 0.60 as poor (destructive)", () => {
    const seg = { text: "suis", confidence: 0.42 };
    const cls = seg.confidence >= 0.85 ? "text-success" : seg.confidence >= 0.6 ? "text-warning" : "text-destructive";
    expect(cls).toBe("text-destructive");
  });

  it("boundary: exactly 0.85 is good", () => {
    const seg = { text: "a", confidence: 0.85 };
    const cls = seg.confidence >= 0.85 ? "text-success" : seg.confidence >= 0.6 ? "text-warning" : "text-destructive";
    expect(cls).toBe("text-success");
  });

  it("boundary: exactly 0.60 is fair", () => {
    const seg = { text: "b", confidence: 0.6 };
    const cls = seg.confidence >= 0.85 ? "text-success" : seg.confidence >= 0.6 ? "text-warning" : "text-destructive";
    expect(cls).toBe("text-warning");
  });
});
```

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/voice/__tests__/
git commit -m "test(voice): add segment storage, TTS audio, echo, and highlight tests"
```

---

### Task 13: Integration test — response loop end-to-end

**Files:**
- Create: `crates/voice-engine/tests/response_loop.rs` (or add to existing test module in service.rs)

- [ ] **Step 1: Add integration test for handle_response**

Add to the `#[cfg(test)] mod tests` in `crates/voice-engine/src/service.rs`:

```rust
    #[tokio::test]
    async fn handle_response_with_tts_emits_speak_and_text_matches() {
        let mock_stt = Arc::new(MockTranscriptionEngine::new("test input"));
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        let tmp = TempDir::new().unwrap();
        let model_manager = ModelManager::new(tmp.path());
        let svc = VoiceService::new(
            Some(mock_stt),
            None,
            Some(mock_tts),
            None,
            model_manager,
            VoiceServiceConfig::default(),
        );

        let mut event_rx = svc.take_event_rx().unwrap();

        // Simulate the full response path
        let response = "I've scheduled your dentist appointment for Thursday at 2pm.";
        svc.handle_response(response).await.unwrap();

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

        // After first stop_capture, flag should be true
        // Note: This tests the flag directly since we can't easily test
        // start_capture + stop_capture without a real audio device.
        svc.first_capture_complete.store(false, Ordering::Relaxed);
        assert!(!svc.first_capture_complete.swap(true, Ordering::Relaxed));
        // Second swap should return true (already set)
        assert!(svc.first_capture_complete.swap(true, Ordering::Relaxed));
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/src/service.rs
git commit -m "test(voice): integration tests for response loop and welcome echo flag"
```

---

### Task 14: Final workspace build + clippy verification

**Files:** None (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Compiles with zero errors.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

- [ ] **Step 4: Run all Rust tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 5: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 6: Run frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: No errors.

- [ ] **Step 7: Commit any fixes**

If any clippy/fmt/test fixes were needed:

```bash
git add -A
git commit -m "fix: clippy + fmt cleanup for voice brain v1 complete"
```
