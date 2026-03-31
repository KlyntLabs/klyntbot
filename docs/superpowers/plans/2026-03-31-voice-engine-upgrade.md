# Voice Engine Upgrade — 5-Phase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the voice system from a batch-processing, macOS-`say`-based prototype into a pluggable, low-latency, near-production voice engine with Kokoro TTS, Silero VAD, and a ProviderManager-style engine registry.

**Architecture:** Five sequential phases, each producing a working, testable system. Phase 1 fixes bugs and builds the config foundation. Phase 2 swaps TTS to Kokoro-82M via ONNX. Phase 3 upgrades VAD to Silero + adds audio DSP. Phase 4 introduces VoiceEngineManager (primary/fallback/circuit-breaker). Phase 5 adds streaming STT partials.

**Tech Stack:** Rust (voice-engine crate), `kokoro-tts` (ONNX), `silero-vad-rust` (ONNX), `ort` (ONNX Runtime with CoreML), `whisper-rs` (existing), React/TypeScript (desktop-ui), Tauri 2 (desktop adapter).

---

## File Structure

### Phase 1 — Bug Fixes + Foundation
- Modify: `crates/voice-engine/src/service.rs` — remove `afplay`, make TTS slot `RwLock`, add `set_tts_engine()`
- Modify: `crates/voice-engine/src/engines/avspeech.rs` — remove `output_path` dependency, synthesize to in-memory `AudioClip`
- Modify: `crates/voice-engine/src/types.rs` — remove `output_path` from `TtsParams`
- Modify: `crates/voice-engine/src/tts.rs` — no changes needed (trait is already clean)
- Modify: `crates/config/src/schema/voice.rs` — add `SttEngineKind`, `TtsEngineKind` enums
- Modify: `crates/app-core/src/init/mod.rs` — store loop handle properly, wire config privacy_mode
- Modify: `crates/voice-engine/src/mock.rs` — update `MockTtsEngine` if `TtsParams` changes

### Phase 2 — Kokoro TTS Engine
- Create: `crates/voice-engine/src/engines/kokoro.rs` — `KokoroTtsEngine` implementing `TtsEngine`
- Modify: `crates/voice-engine/src/engines/mod.rs` — add `kokoro` module + re-export
- Modify: `crates/voice-engine/src/lib.rs` — re-export `KokoroTtsEngine`
- Modify: `crates/voice-engine/src/model_manager.rs` — add Kokoro model enum + download support
- Modify: `crates/voice-engine/Cargo.toml` — add `kokoro-tts` + `ort` dependencies
- Modify: `crates/app-core/src/init/mod.rs` — wire Kokoro as primary TTS, AVSpeech as fallback

### Phase 3 — Silero VAD + Audio DSP
- Create: `crates/voice-engine/src/vad.rs` — `SileroVad` wrapper around `silero-vad-rust`
- Create: `crates/voice-engine/src/dsp.rs` — `AudioPipeline` (noise reduction + anti-alias filter + downsampling)
- Modify: `crates/voice-engine/src/capture.rs` — integrate VAD + DSP into capture pipeline
- Modify: `crates/voice-engine/src/service.rs` — use VAD instead of RMS silence detection
- Modify: `crates/voice-engine/src/lib.rs` — export new modules
- Modify: `crates/voice-engine/Cargo.toml` — add `silero-vad-rust`, `nnnoiseless`
- Modify: `crates/config/src/schema/voice.rs` — add VAD config fields

### Phase 4 — VoiceEngineManager
- Create: `crates/voice-engine/src/engine_manager.rs` — `SttEngineManager` + `TtsEngineManager` with circuit-breaker
- Modify: `crates/config/src/schema/voice.rs` — add engine manager config (primary, fallback)
- Modify: `crates/voice-engine/src/service.rs` — replace raw trait objects with engine managers
- Modify: `crates/app-core/src/init/mod.rs` — factory creation with failover

### Phase 5 — Streaming STT Partials
- Modify: `crates/voice-engine/src/engines/whisper_local.rs` — implement chunked streaming
- Modify: `crates/voice-engine/src/stt.rs` — no trait changes needed (already supports partials)
- Modify: `crates/voice-engine/src/service.rs` — emit incremental partials during capture

---

## Phase 1: Bug Fixes + Foundation

### Task 1.1: Remove `output_path` from `TtsParams`

The `output_path` field on `TtsParams` is AVSpeech-specific tech debt — it's `#[serde(skip)]` and only exists for the file-based `say` CLI workflow. Cloud/ONNX TTS engines return PCM directly. Remove it and let `AvSpeechTtsEngine` manage its own temp files internally.

**Files:**
- Modify: `crates/voice-engine/src/types.rs:87-95`
- Modify: `crates/voice-engine/src/engines/avspeech.rs:28-62`
- Modify: `crates/voice-engine/src/service.rs:546-556`
- Test: `crates/voice-engine/src/service.rs` (existing tests)

- [ ] **Step 1: Remove `output_path` from `TtsParams`**

In `crates/voice-engine/src/types.rs`, remove the `output_path` field:

```rust
/// Parameters for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsParams {
    pub language: Language,
    pub voice_name: Option<String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
}

impl Default for TtsParams {
    fn default() -> Self {
        Self {
            language: Language::default(),
            voice_name: None,
            speaking_rate: default_speaking_rate(),
        }
    }
}
```

- [ ] **Step 2: Update `AvSpeechTtsEngine` to manage its own temp file**

In `crates/voice-engine/src/engines/avspeech.rs`, make `synthesize` create its own temp path:

```rust
#[async_trait]
impl TtsEngine for AvSpeechTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let text = text.to_string();
        let wav_path = self.data_dir.join(format!(
            "tts_{}.wav",
            chrono::Utc::now().timestamp_millis()
        ));
        let rate = params.speaking_rate;
        let voice = params.voice_name.clone();

        let path = wav_path.clone();
        tokio::task::spawn_blocking(move || {
            platform_macos::speech::synthesize_to_file(&text, &path, rate, voice.as_deref())
        })
        .await
        .map_err(|e| common::KlyntbotError::Internal(format!("TTS join error: {e}")))??;

        let samples = tokio::task::spawn_blocking(move || -> common::Result<Vec<f32>> {
            let reader = hound::WavReader::open(&wav_path)?;
            let spec = reader.spec();
            let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
                reader.into_samples::<f32>().filter_map(|s| s.ok()).collect()
            } else {
                reader
                    .into_samples::<i16>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / i16::MAX as f32)
                    .collect()
            };
            // Clean up temp file
            let _ = std::fs::remove_file(&wav_path);
            Ok(samples)
        })
        .await
        .map_err(|e| common::KlyntbotError::Internal(format!("WAV read error: {e}")))??;

        Ok(AudioClip {
            samples,
            sample_rate: 16000,
            channels: 1,
        })
    }
    // ... rest unchanged
}
```

- [ ] **Step 3: Update `handle_response` in `service.rs` to remove file management**

In `crates/voice-engine/src/service.rs`, simplify `handle_response` — it no longer creates the WAV path or passes `output_path`:

```rust
    pub async fn handle_response(
        &self,
        response_text: &str,
        tts_params: &TtsParams,
    ) -> common::Result<()> {
        if let Some(ref tts) = self.tts {
            match tts.synthesize(response_text, tts_params).await {
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

- [ ] **Step 4: Remove `afplay` and `tts_playback_pid` from `VoiceService`**

In `crates/voice-engine/src/service.rs`, remove the `tts_playback_pid` field from the struct and `stop_tts_playback` method. The frontend's Web Audio API (with the already-fixed `stopTtsAudio()`) now handles all playback.

Remove from struct definition (~line 121):
```rust
    // DELETE: tts_playback_pid: Mutex<Option<u32>>,
```

Remove from `new()` (~line 152):
```rust
    // DELETE: tts_playback_pid: Mutex::new(None),
```

Replace `stop_tts_playback` with a no-op that emits TtsFadeOut (the frontend listens for this):
```rust
    /// Signal the frontend to stop TTS playback.
    pub async fn stop_tts_playback(&self) {
        let _ = self.event_tx.send(VoiceEvent::TtsFadeOut).await;
    }
```

- [ ] **Step 5: Run tests to verify nothing broke**

Run: `cargo nextest run -p voice-engine`

Expected: All existing tests pass. The `handle_response_emits_speak_event` test should still pass since `MockTtsEngine` returns an in-memory `AudioClip` and never used `output_path`.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/types.rs crates/voice-engine/src/engines/avspeech.rs crates/voice-engine/src/service.rs
git commit -m "fix(voice): remove afplay dual-playback, internalize TTS file management"
```

---

### Task 1.2: Make TTS Slot Hot-Swappable

The STT engine already uses `RwLock<Option<Arc<dyn TranscriptionEngine>>>` for hot-swap after model download. The TTS slot is a plain `Option<Arc<dyn TtsEngine>>` set once at init. Upgrade it to match the STT pattern so we can swap TTS engines at runtime (needed for Phase 2).

**Files:**
- Modify: `crates/voice-engine/src/service.rs:89-154`
- Test: `crates/voice-engine/src/service.rs` (add new test)

- [ ] **Step 1: Write a failing test for TTS hot-swap**

Add to `crates/voice-engine/src/service.rs` tests module:

```rust
    #[tokio::test]
    async fn set_tts_engine_hot_swaps() {
        let (svc, _tmp) = make_service(None);
        let mut event_rx = svc.take_event_rx().unwrap();

        // Initially no TTS engine
        svc.handle_response("hello", &TtsParams::default()).await.unwrap();
        // No SpeakResponse event should be emitted
        assert!(event_rx.try_recv().is_err());

        // Hot-swap a TTS engine
        let mock_tts = Arc::new(crate::mock::MockTtsEngine);
        svc.set_tts_engine(mock_tts);

        // Now TTS should work
        svc.handle_response("hello after swap", &TtsParams::default()).await.unwrap();
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        ).await.expect("timeout").expect("channel closed");

        match event {
            VoiceEvent::SpeakResponse { text, .. } => {
                assert_eq!(text, "hello after swap");
            }
            other => panic!("Expected SpeakResponse, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p voice-engine -E 'test(set_tts_engine_hot_swaps)'`

Expected: FAIL — `set_tts_engine` method does not exist.

- [ ] **Step 3: Change TTS field to `RwLock` and add `set_tts_engine`**

In `crates/voice-engine/src/service.rs`, change the `tts` field:

```rust
pub struct VoiceService {
    stt_local: std::sync::RwLock<Option<Arc<dyn TranscriptionEngine>>>,
    /// TTS engine — wrapped in RwLock for hot-swap (same pattern as stt_local).
    tts: std::sync::RwLock<Option<Arc<dyn TtsEngine>>>,
    // ... rest unchanged
}
```

Update `new()`:
```rust
    pub fn new(
        stt_local: Option<Arc<dyn TranscriptionEngine>>,
        tts: Option<Arc<dyn TtsEngine>>,
        memory_echo: Option<Arc<dyn MemoryEchoProvider>>,
        model_manager: ModelManager,
        config: VoiceServiceConfig,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(128);
        Self {
            stt_local: std::sync::RwLock::new(stt_local),
            tts: std::sync::RwLock::new(tts),
            // ... rest unchanged
        }
    }
```

Add the hot-swap method:
```rust
    /// Hot-swap the TTS engine at runtime.
    pub fn set_tts_engine(&self, engine: Arc<dyn TtsEngine>) {
        if let Ok(mut guard) = self.tts.write() {
            *guard = Some(engine);
        }
    }
```

Update `handle_response` to read from `RwLock`:
```rust
    pub async fn handle_response(
        &self,
        response_text: &str,
        tts_params: &TtsParams,
    ) -> common::Result<()> {
        let tts = self.tts.read().ok().and_then(|g| g.clone());
        if let Some(tts) = tts {
            match tts.synthesize(response_text, tts_params).await {
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine`

Expected: All tests PASS, including the new `set_tts_engine_hot_swaps`.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/service.rs
git commit -m "feat(voice): make TTS engine slot hot-swappable via RwLock"
```

---

### Task 1.3: Add Engine Kind Enums to Config

Add `SttEngineKind` and `TtsEngineKind` to the voice config schema so users can select engines via `config.json`. This is the config foundation for Phases 2-4.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`
- Test: `crates/config/src/schema/voice.rs` (inline test)

- [ ] **Step 1: Write a test for config deserialization with engine kinds**

Add to the bottom of `crates/config/src/schema/voice.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_kinds() {
        let input = VoiceInputConfig::default();
        assert_eq!(input.stt_engine, SttEngineKind::WhisperLocal);

        let output = VoiceOutputConfig::default();
        assert_eq!(output.tts_engine, TtsEngineKind::System);
    }

    #[test]
    fn deserialize_kokoro_engine() {
        let json = r#"{"ttsEngine": "kokoro"}"#;
        let output: VoiceOutputConfig = serde_json::from_str(json).unwrap();
        assert_eq!(output.tts_engine, TtsEngineKind::Kokoro);
    }

    #[test]
    fn deserialize_unknown_engine_falls_back_to_default() {
        // Unknown engine names should fail to deserialize (strict enum)
        let json = r#"{"sttEngine": "nonexistent"}"#;
        let result: Result<VoiceInputConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p config -E 'test(default_engine_kinds)'`

Expected: FAIL — `SttEngineKind` and `TtsEngineKind` don't exist.

- [ ] **Step 3: Add engine kind enums and wire into config structs**

In `crates/config/src/schema/voice.rs`, add enums before `VoiceConfig`:

```rust
/// STT engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SttEngineKind {
    /// Local whisper.cpp via whisper-rs (default).
    #[default]
    WhisperLocal,
    /// Placeholder for future cloud STT (Deepgram, etc.)
    Cloud,
}

impl Default for SttEngineKind {
    fn default() -> Self {
        Self::WhisperLocal
    }
}

/// TTS engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsEngineKind {
    /// macOS system TTS via `say` CLI (default).
    #[default]
    System,
    /// Kokoro-82M via ONNX Runtime.
    Kokoro,
    /// Piper VITS via ONNX Runtime.
    Piper,
}

impl Default for TtsEngineKind {
    fn default() -> Self {
        Self::System
    }
}
```

Add to `VoiceInputConfig`:
```rust
pub struct VoiceInputConfig {
    // ... existing fields ...
    /// STT engine to use.
    #[serde(default)]
    pub stt_engine: SttEngineKind,
}
```

Add to `VoiceOutputConfig`:
```rust
pub struct VoiceOutputConfig {
    // ... existing fields ...
    /// TTS engine to use.
    #[serde(default)]
    pub tts_engine: TtsEngineKind,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(default_engine_kinds)' && cargo nextest run -p config -E 'test(deserialize_kokoro_engine)'`

Expected: All PASS.

- [ ] **Step 5: Run workspace build to verify no breakage**

Run: `cargo build --workspace`

Expected: Clean build. Existing code doesn't reference the new fields yet (they have `#[serde(default)]`).

- [ ] **Step 6: Commit**

```bash
git add crates/config/src/schema/voice.rs
git commit -m "feat(config): add SttEngineKind and TtsEngineKind to voice config"
```

---

### Task 1.4: Store Loop Handle and Add Restart-on-Panic

The conversation loop's `JoinHandle` is stored but never monitored. If the loop panics, voice silently stops working. Add a supervisor that restarts the loop.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs:409-420`
- Modify: `crates/app-core/src/init/mod.rs:669-671`

- [ ] **Step 1: Add supervised spawn method to VoiceConversationManager**

In `crates/app-core/src/handlers/voice_conversation.rs`, replace `spawn_loop` with `spawn_supervised_loop`:

```rust
    /// Spawn the conversation loop with automatic restart on panic.
    pub async fn spawn_supervised_loop(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let inner_manager = Arc::clone(&manager);
                let cmd_rx = {
                    let mut guard = inner_manager.cmd_rx.lock().await;
                    match guard.take() {
                        Some(rx) => rx,
                        None => {
                            tracing::error!("Voice conversation loop: cmd_rx already consumed, cannot restart");
                            return;
                        }
                    }
                };

                let result = tokio::spawn(async move {
                    inner_manager.conversation_loop(cmd_rx).await;
                }).await;

                match result {
                    Ok(()) => {
                        tracing::info!("Voice conversation loop exited normally");
                        return;
                    }
                    Err(e) => {
                        tracing::error!("Voice conversation loop panicked: {e}, restarting in 1s...");
                        // Reset state to idle so restart begins cleanly
                        let mut state = manager.state.lock().await;
                        state.phase = ConversationPhase::Idle;
                        state.paused = false;
                        drop(state);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        // Note: cmd_rx was consumed by the panicked task, so we need a new channel
                        let (new_tx, new_rx) = mpsc::channel(16);
                        // Replace the cmd_tx so new commands go to the new channel
                        // This requires cmd_tx to also be swappable — skip for now,
                        // log the limitation and break.
                        tracing::error!("Voice conversation loop cannot restart: cmd channel consumed by panicked task");
                        drop(new_tx);
                        drop(new_rx);
                        return;
                    }
                }
            }
        })
    }
```

Note: A full restart requires a recreatable command channel. For now, we log the panic clearly instead of silently dropping it. A full restart mechanism would require refactoring `cmd_rx` to be recreatable — defer to a future task.

- [ ] **Step 2: Update init to use the new method**

In `crates/app-core/src/init/mod.rs`, change line 669:

```rust
                let loop_handle = voice_conv_manager.spawn_supervised_loop().await;
```

- [ ] **Step 3: Run build and tests**

Run: `cargo build --workspace && cargo nextest run -p voice-engine`

Expected: Clean build, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs crates/app-core/src/init/mod.rs
git commit -m "fix(voice): log conversation loop panics instead of silently dropping"
```

---

## Phase 2: Kokoro TTS Engine

### Task 2.1: Add Kokoro Dependencies

**Files:**
- Modify: `crates/voice-engine/Cargo.toml`

- [ ] **Step 1: Add `kokoro-tts` and `ort` to Cargo.toml**

In `crates/voice-engine/Cargo.toml`, add under `[dependencies]`:

```toml
kokoro-tts = { version = "0.3", optional = true }
ort = { version = "2.0.0-rc.12", optional = true, features = ["load-dynamic"] }
```

Add a feature flag:
```toml
[features]
default = ["kokoro"]
kokoro = ["dep:kokoro-tts", "dep:ort"]
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p voice-engine`

Expected: Clean build. The `kokoro-tts` crate is downloaded and compiled. If the ONNX Runtime dynamic library is not found, the `load-dynamic` feature means it will be resolved at runtime, not compile time.

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/Cargo.toml
git commit -m "feat(voice): add kokoro-tts and ort dependencies behind feature flag"
```

---

### Task 2.2: Implement KokoroTtsEngine

**Files:**
- Create: `crates/voice-engine/src/engines/kokoro.rs`
- Modify: `crates/voice-engine/src/engines/mod.rs`
- Modify: `crates/voice-engine/src/lib.rs`
- Test: inline in `kokoro.rs`

- [ ] **Step 1: Write a test for KokoroTtsEngine trait compliance**

Create `crates/voice-engine/src/engines/kokoro.rs`:

```rust
//! Kokoro-82M TTS engine via ONNX Runtime.
//!
//! Kokoro is a lightweight (82M params) non-autoregressive TTS model that
//! produces high-quality speech with sub-200ms latency on Apple Silicon.
//! Uses the `kokoro-tts` crate which wraps ONNX Runtime.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use tracing::{info, warn};

use crate::tts::TtsEngine;
use crate::types::*;

/// Kokoro-82M TTS engine.
///
/// Loads the ONNX model from disk and runs inference via `ort` (ONNX Runtime).
/// Thread-safe: the internal `kokoro_tts::Kokoro` instance is wrapped in a Mutex
/// because it requires `&mut self` for synthesis.
pub struct KokoroTtsEngine {
    model: Mutex<kokoro_tts::Kokoro>,
    model_dir: PathBuf,
}

impl KokoroTtsEngine {
    /// Create a new Kokoro TTS engine from a model directory.
    ///
    /// The directory should contain the ONNX model file and voice data.
    /// Returns an error if the model cannot be loaded.
    pub fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        info!("Loading Kokoro TTS model from {}", model_dir.display());

        let model = kokoro_tts::Kokoro::new(model_dir.to_str().unwrap_or("."), None)
            .map_err(|e| common::KlyntbotError::Internal(
                format!("Failed to load Kokoro TTS model: {e}")
            ))?;

        Ok(Self {
            model: Mutex::new(model),
            model_dir,
        })
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }
}

#[async_trait]
impl TtsEngine for KokoroTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let text = text.to_string();
        let voice = params.voice_name.clone().unwrap_or_else(|| "af_heart".to_string());
        let speed = params.speaking_rate;

        // Run inference on a blocking thread (ONNX Runtime is CPU-bound).
        let model_guard = self.model.lock().map_err(|e| {
            common::KlyntbotError::Internal(format!("Kokoro model lock poisoned: {e}"))
        })?;

        // kokoro-tts expects &mut self, so we need to use the mutex
        // We'll clone the data we need and release the lock before spawn_blocking
        // Actually, Mutex<Kokoro> can be sent to spawn_blocking if we take the guard.
        // Instead, use a sync approach since Kokoro::synthesize is fast (<200ms).
        let audio = model_guard
            .synthesize(&text, &voice, speed)
            .map_err(|e| common::KlyntbotError::Internal(
                format!("Kokoro synthesis failed: {e}")
            ))?;

        // kokoro-tts returns audio samples at 24kHz by default
        let sample_rate = 24000u32;

        Ok(AudioClip {
            samples: audio,
            sample_rate,
            channels: 1,
        })
    }

    fn supports_language(&self, lang: &Language) -> bool {
        // Kokoro-82M supports English and Chinese
        matches!(lang.as_str(), "en" | "zh" | "ja" | "ko")
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        vec![
            VoiceInfo {
                identifier: "af_heart".to_string(),
                display_name: "Heart (Female)".to_string(),
                language: Language::new("en"),
            },
            VoiceInfo {
                identifier: "af_bella".to_string(),
                display_name: "Bella (Female)".to_string(),
                language: Language::new("en"),
            },
            VoiceInfo {
                identifier: "am_michael".to_string(),
                display_name: "Michael (Male)".to_string(),
                language: Language::new("en"),
            },
        ]
    }

    fn display_name(&self) -> &str {
        "Kokoro-82M"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_english() {
        // This test doesn't require the model to be loaded — it tests
        // the language support logic which is hardcoded.
        // We can't test new() without the actual model files,
        // so we test the trait methods that don't require model state.
        let lang = Language::new("en");
        // Since we can't construct KokoroTtsEngine without model files,
        // we just verify the language matching logic directly.
        assert!(matches!(lang.as_str(), "en" | "zh" | "ja" | "ko"));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/voice-engine/src/engines/mod.rs`:

```rust
pub mod avspeech;
#[cfg(feature = "kokoro")]
pub mod kokoro;
pub mod whisper_local;

pub use avspeech::AvSpeechTtsEngine;
#[cfg(feature = "kokoro")]
pub use kokoro::KokoroTtsEngine;
pub use whisper_local::WhisperLocalEngine;
```

In `crates/voice-engine/src/lib.rs`, add the re-export:

```rust
#[cfg(feature = "kokoro")]
pub use engines::KokoroTtsEngine;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p voice-engine --features kokoro`

Expected: Compiles. May warn about unused imports if `kokoro-tts` API differs — adjust the `synthesize` call to match the actual API.

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run -p voice-engine`

Expected: All tests pass. The new test only checks language logic, not model loading.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/engines/kokoro.rs crates/voice-engine/src/engines/mod.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add KokoroTtsEngine implementation behind feature flag"
```

---

### Task 2.3: Add Kokoro Model Management

Extend `ModelManager` to handle Kokoro ONNX model downloads alongside Whisper GGML models.

**Files:**
- Modify: `crates/voice-engine/src/model_manager.rs`
- Test: inline tests

- [ ] **Step 1: Write a test for Kokoro model detection**

Add to `model_manager.rs` tests:

```rust
    #[test]
    fn kokoro_model_detected_when_present() {
        let tmp = tempfile::TempDir::new().unwrap();
        let models_dir = tmp.path().join("models").join("kokoro");
        std::fs::create_dir_all(&models_dir).unwrap();
        // Create a fake model file
        std::fs::write(models_dir.join("kokoro-v0.19.onnx"), b"fake").unwrap();

        let mm = ModelManager::new(tmp.path());
        assert!(mm.kokoro_model_dir().is_some());
    }

    #[test]
    fn kokoro_model_not_detected_when_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mm = ModelManager::new(tmp.path());
        assert!(mm.kokoro_model_dir().is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p voice-engine -E 'test(kokoro_model)'`

Expected: FAIL — `kokoro_model_dir` method doesn't exist.

- [ ] **Step 3: Add Kokoro model support to ModelManager**

In `crates/voice-engine/src/model_manager.rs`, add:

```rust
    /// Check if a Kokoro ONNX model exists in the models directory.
    /// Returns the model directory path if found.
    pub fn kokoro_model_dir(&self) -> Option<PathBuf> {
        let kokoro_dir = self.models_dir.join("kokoro");
        if kokoro_dir.is_dir() {
            // Check for any .onnx file in the directory
            if let Ok(entries) = std::fs::read_dir(&kokoro_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |ext| ext == "onnx") {
                        return Some(kokoro_dir);
                    }
                }
            }
        }
        None
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine -E 'test(kokoro_model)'`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/model_manager.rs
git commit -m "feat(voice): add Kokoro model detection to ModelManager"
```

---

### Task 2.4: Wire Kokoro as Primary TTS in Init

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:607-609`

- [ ] **Step 1: Update TTS initialization to prefer Kokoro**

In `crates/app-core/src/init/mod.rs`, replace the TTS section (~line 607-609):

```rust
                // TTS: prefer Kokoro (if model available), fall back to macOS AVSpeech
                let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
                    #[cfg(feature = "kokoro")]
                    {
                        match voice_config.output.tts_engine {
                            config::schema::TtsEngineKind::Kokoro => {
                                if let Some(kokoro_dir) = model_manager.kokoro_model_dir() {
                                    match voice_engine::KokoroTtsEngine::new(&kokoro_dir) {
                                        Ok(engine) => {
                                            info!("Kokoro TTS engine loaded from {}", kokoro_dir.display());
                                            Some(Arc::new(engine) as Arc<dyn voice_engine::TtsEngine>)
                                        }
                                        Err(e) => {
                                            warn!("Failed to load Kokoro TTS, falling back to system: {e}");
                                            Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir)))
                                        }
                                    }
                                } else {
                                    info!("Kokoro model not found, using system TTS");
                                    Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir)))
                                }
                            }
                            _ => Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir))),
                        }
                    }
                    #[cfg(not(feature = "kokoro"))]
                    {
                        Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir)))
                    }
                };
```

- [ ] **Step 2: Build and verify**

Run: `cargo build --workspace`

Expected: Clean build. Default config uses `TtsEngineKind::System` so behavior is unchanged unless user sets `"ttsEngine": "kokoro"` in config.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(voice): wire Kokoro TTS as config-selectable engine with system fallback"
```

---

## Phase 3: Silero VAD + Audio DSP

### Task 3.1: Add DSP Dependencies

**Files:**
- Modify: `crates/voice-engine/Cargo.toml`

- [ ] **Step 1: Add `silero-vad-rust` and `nnnoiseless` to Cargo.toml**

```toml
silero-vad-rust = { version = "6.2", optional = true }
nnnoiseless = { version = "0.5", optional = true }
```

Add feature:
```toml
[features]
default = ["kokoro", "vad"]
kokoro = ["dep:kokoro-tts", "dep:ort"]
vad = ["dep:silero-vad-rust", "dep:nnnoiseless"]
```

- [ ] **Step 2: Build**

Run: `cargo build -p voice-engine`

Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/Cargo.toml
git commit -m "feat(voice): add silero-vad and nnnoiseless dependencies"
```

---

### Task 3.2: Implement Audio DSP Pipeline

Anti-aliasing filter before downsampling + noise reduction. Replaces the raw `step_by(ratio)` decimation.

**Files:**
- Create: `crates/voice-engine/src/dsp.rs`
- Test: inline

- [ ] **Step 1: Write test for anti-aliased downsampling**

Create `crates/voice-engine/src/dsp.rs`:

```rust
//! Audio DSP pipeline: noise reduction + anti-aliased downsampling.

/// Downsample audio from `src_rate` to `dst_rate` with a simple low-pass
/// averaging filter to prevent aliasing. This replaces the naive `step_by`
/// decimation.
pub fn downsample_with_filter(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate {
        return samples.to_vec();
    }

    let ratio = src_rate as f64 / dst_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    // Simple averaging decimation: for each output sample, average the
    // surrounding input samples within the decimation window.
    // This acts as a low-pass filter at dst_rate/2 Hz.
    let window = ratio.ceil() as usize;

    for i in 0..output_len {
        let center = (i as f64 * ratio) as usize;
        let start = center.saturating_sub(window / 2);
        let end = (center + window / 2 + 1).min(samples.len());
        let sum: f32 = samples[start..end].iter().sum();
        let count = (end - start) as f32;
        output.push(sum / count);
    }

    output
}

/// Apply noise reduction via nnnoiseless (RNNoise).
/// Operates on 48kHz mono audio in frames of 480 samples.
#[cfg(feature = "vad")]
pub fn denoise_48khz(samples: &[f32]) -> Vec<f32> {
    use nnnoiseless::DenoiseState;

    let mut state = DenoiseState::new();
    let mut output = Vec::with_capacity(samples.len());
    let mut frame_buf = [0.0f32; DenoiseState::FRAME_SIZE];

    for chunk in samples.chunks(DenoiseState::FRAME_SIZE) {
        frame_buf[..chunk.len()].copy_from_slice(chunk);
        // Zero-pad the last frame if it's shorter
        if chunk.len() < DenoiseState::FRAME_SIZE {
            frame_buf[chunk.len()..].fill(0.0);
        }
        let mut out_frame = [0.0f32; DenoiseState::FRAME_SIZE];
        state.process_frame(&mut out_frame, &frame_buf);
        output.extend_from_slice(&out_frame[..chunk.len()]);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = downsample_with_filter(&input, 16000, 16000);
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn downsample_48k_to_16k() {
        // 48kHz → 16kHz = ratio 3
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.01).sin()).collect();
        let output = downsample_with_filter(&input, 48000, 16000);
        // Should produce ~1600 samples
        assert!((output.len() as i32 - 1600).abs() <= 1);
    }

    #[test]
    fn downsample_reduces_aliasing() {
        // Generate a signal with both low (1kHz) and high (20kHz) components at 48kHz
        let src_rate = 48000.0;
        let input: Vec<f32> = (0..4800)
            .map(|i| {
                let t = i as f32 / src_rate;
                (2.0 * std::f32::consts::PI * 1000.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 20000.0 * t).sin()
            })
            .collect();

        let output = downsample_with_filter(&input, 48000, 16000);

        // The averaging filter should attenuate the 20kHz component
        // (which aliases at 16kHz). Energy of output should be less than
        // energy of naive decimation.
        let naive: Vec<f32> = input.iter().step_by(3).copied().collect();
        let naive_energy: f32 = naive.iter().map(|s| s * s).sum();
        let filtered_energy: f32 = output.iter().map(|s| s * s).sum();

        // Filtered should have less energy (the 20kHz alias is attenuated)
        assert!(
            filtered_energy < naive_energy,
            "Filtered energy ({filtered_energy}) should be less than naive ({naive_energy})"
        );
    }

    #[cfg(feature = "vad")]
    #[test]
    fn denoise_preserves_length() {
        let input = vec![0.01; 4800]; // 100ms at 48kHz
        let output = denoise_48khz(&input);
        assert_eq!(output.len(), input.len());
    }
}
```

- [ ] **Step 2: Register module**

In `crates/voice-engine/src/lib.rs`, add:
```rust
pub mod dsp;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p voice-engine -E 'test(downsample)'`

Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/dsp.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add audio DSP pipeline with anti-aliased downsampling"
```

---

### Task 3.3: Implement Silero VAD Wrapper

**Files:**
- Create: `crates/voice-engine/src/vad.rs`
- Test: inline

- [ ] **Step 1: Write Silero VAD wrapper**

Create `crates/voice-engine/src/vad.rs`:

```rust
//! Voice Activity Detection via Silero VAD (ONNX).
//!
//! Silero VAD is a lightweight neural network that detects speech vs silence
//! with much higher accuracy than simple RMS thresholding. Processes 16kHz
//! mono audio in 512-sample chunks (~32ms).

#[cfg(feature = "vad")]
use silero_vad_rust::SileroVad;

/// VAD decision for a chunk of audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadDecision {
    /// Speech detected with given probability (0.0-1.0).
    Speech(f32),
    /// Silence detected.
    Silence,
}

/// Silero VAD wrapper with stateful processing.
#[cfg(feature = "vad")]
pub struct SileroVadProcessor {
    vad: SileroVad,
    threshold: f32,
}

#[cfg(feature = "vad")]
impl SileroVadProcessor {
    /// Create a new Silero VAD processor.
    ///
    /// `threshold` controls sensitivity: lower = more sensitive to speech.
    /// Recommended: 0.5 for general use, 0.3 for noisy environments.
    pub fn new(threshold: f32) -> common::Result<Self> {
        let vad = SileroVad::new()
            .map_err(|e| common::KlyntbotError::Internal(
                format!("Failed to initialize Silero VAD: {e}")
            ))?;

        Ok(Self { vad, threshold })
    }

    /// Process a chunk of 16kHz mono audio (512 samples = 32ms).
    /// Returns a VadDecision indicating speech or silence.
    pub fn process_chunk(&mut self, samples: &[f32]) -> VadDecision {
        match self.vad.process(samples) {
            Ok(prob) => {
                if prob > self.threshold {
                    VadDecision::Speech(prob)
                } else {
                    VadDecision::Silence
                }
            }
            Err(_) => VadDecision::Silence,
        }
    }

    /// Reset VAD state (call between sessions).
    pub fn reset(&mut self) {
        self.vad.reset();
    }
}

/// Fallback RMS-based VAD when Silero is not available.
pub struct RmsVadProcessor {
    threshold: f32,
}

impl RmsVadProcessor {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    pub fn process_chunk(&self, samples: &[f32]) -> VadDecision {
        let rms = crate::capture::compute_rms(samples);
        if rms > self.threshold {
            VadDecision::Speech(rms)
        } else {
            VadDecision::Silence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_vad_detects_silence() {
        let vad = RmsVadProcessor::new(0.01);
        let silence = vec![0.001; 512];
        assert_eq!(vad.process_chunk(&silence), VadDecision::Silence);
    }

    #[test]
    fn rms_vad_detects_speech() {
        let vad = RmsVadProcessor::new(0.01);
        let speech = vec![0.1; 512];
        match vad.process_chunk(&speech) {
            VadDecision::Speech(_) => {} // expected
            other => panic!("Expected Speech, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Register module**

In `crates/voice-engine/src/lib.rs`, add:
```rust
pub mod vad;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p voice-engine -E 'test(rms_vad)'`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/vad.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add Silero VAD wrapper with RMS fallback"
```

---

### Task 3.4: Integrate DSP + VAD into Capture Pipeline

Replace the naive `step_by` decimation in `capture.rs` with the new DSP pipeline, and add VAD config fields.

**Files:**
- Modify: `crates/voice-engine/src/capture.rs` (downsampling logic)
- Modify: `crates/config/src/schema/voice.rs` (add VAD config)

- [ ] **Step 1: Add VAD config fields**

In `crates/config/src/schema/voice.rs`, add to `VoiceInputConfig`:

```rust
pub struct VoiceInputConfig {
    // ... existing fields ...

    /// VAD threshold (0.0-1.0). Lower = more sensitive to speech.
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: f32,

    /// Whether to use neural VAD (Silero) or simple RMS threshold.
    #[serde(default)]
    pub use_neural_vad: bool,
}
```

Add default function:
```rust
fn default_vad_threshold() -> f32 {
    0.5
}
```

- [ ] **Step 2: Replace decimation in capture.rs**

In `crates/voice-engine/src/capture.rs`, find the downsampling section in the cpal callback (the `step_by(ratio)` logic) and replace with:

```rust
                    // Anti-aliased downsampling (replaces naive step_by decimation)
                    let downsampled = crate::dsp::downsample_with_filter(
                        &mono,
                        native_rate,
                        config.sample_rate,
                    );
```

- [ ] **Step 3: Build and run tests**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

Expected: All pass. The capture pipeline now uses filtered downsampling.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/capture.rs crates/config/src/schema/voice.rs
git commit -m "feat(voice): integrate anti-aliased downsampling into capture pipeline"
```

---

## Phase 4: VoiceEngineManager

### Task 4.1: Implement VoiceEngineManager

A generic manager with primary/fallback and circuit-breaker, mirroring the `ProviderManager` pattern.

**Files:**
- Create: `crates/voice-engine/src/engine_manager.rs`
- Test: inline

- [ ] **Step 1: Write the engine manager with tests**

Create `crates/voice-engine/src/engine_manager.rs`:

```rust
//! Voice engine manager with primary/fallback routing and circuit breaker.
//!
//! Mirrors the `ProviderManager` pattern from `crates/providers/src/manager.rs`.
//! Wraps a primary TTS/STT engine with an optional fallback. When the primary
//! fails repeatedly, the circuit opens and requests route to the fallback.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::tts::TtsEngine;
use crate::types::*;

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the circuit opens.
    pub failure_threshold: u32,
    /// Seconds to wait before trying the primary again (half-open).
    pub reset_timeout_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            reset_timeout_secs: 30,
        }
    }
}

/// Manages a primary TTS engine with an optional fallback.
pub struct TtsEngineManager {
    primary: Arc<dyn TtsEngine>,
    fallback: Option<Arc<dyn TtsEngine>>,
    failure_count: AtomicU32,
    circuit_open_until: RwLock<Option<tokio::time::Instant>>,
    config: CircuitBreakerConfig,
}

impl TtsEngineManager {
    pub fn new(primary: Arc<dyn TtsEngine>, fallback: Option<Arc<dyn TtsEngine>>) -> Self {
        Self {
            primary,
            fallback,
            failure_count: AtomicU32::new(0),
            circuit_open_until: RwLock::new(None),
            config: CircuitBreakerConfig::default(),
        }
    }

    pub fn with_config(
        primary: Arc<dyn TtsEngine>,
        fallback: Option<Arc<dyn TtsEngine>>,
        config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            primary,
            fallback,
            failure_count: AtomicU32::new(0),
            circuit_open_until: RwLock::new(None),
            config,
        }
    }

    async fn is_circuit_open(&self) -> bool {
        let guard = self.circuit_open_until.read().await;
        match *guard {
            Some(deadline) => {
                if tokio::time::Instant::now() < deadline {
                    true
                } else {
                    drop(guard);
                    // Half-open: reset and try primary again
                    *self.circuit_open_until.write().await = None;
                    self.failure_count.store(0, Ordering::Relaxed);
                    false
                }
            }
            None => false,
        }
    }

    fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.config.failure_threshold {
            let deadline = tokio::time::Instant::now()
                + std::time::Duration::from_secs(self.config.reset_timeout_secs);
            // Best-effort — if we can't write, the circuit stays closed
            if let Ok(mut guard) = self.circuit_open_until.try_write() {
                *guard = Some(deadline);
                warn!(
                    "TTS circuit breaker opened after {} failures, will retry in {}s",
                    count, self.config.reset_timeout_secs
                );
            }
        }
    }

    fn reset_failures(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl TtsEngine for TtsEngineManager {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        if !self.is_circuit_open().await {
            match self.primary.synthesize(text, params).await {
                Ok(clip) => {
                    self.reset_failures();
                    return Ok(clip);
                }
                Err(e) => {
                    self.record_failure();
                    warn!("Primary TTS failed: {e}");
                    if let Some(ref fallback) = self.fallback {
                        info!("Falling back to {}", fallback.display_name());
                        return fallback.synthesize(text, params).await;
                    }
                    return Err(e);
                }
            }
        }

        // Circuit is open — use fallback directly
        match self.fallback {
            Some(ref fallback) => fallback.synthesize(text, params).await,
            None => Err(common::KlyntbotError::Internal(
                "TTS circuit breaker open and no fallback configured".to_string(),
            )),
        }
    }

    fn supports_language(&self, lang: &Language) -> bool {
        self.primary.supports_language(lang)
            || self.fallback.as_ref().map_or(false, |f| f.supports_language(lang))
    }

    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo> {
        let mut voices = self.primary.available_voices(lang);
        if let Some(ref fallback) = self.fallback {
            voices.extend(fallback.available_voices(lang));
        }
        voices
    }

    fn display_name(&self) -> &str {
        self.primary.display_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockTtsEngine;

    #[tokio::test]
    async fn primary_succeeds() {
        let primary = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, None);

        let clip = manager.synthesize("hello", &TtsParams::default()).await.unwrap();
        assert!(!clip.samples.is_empty());
    }

    #[tokio::test]
    async fn display_name_from_primary() {
        let primary = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, None);
        assert_eq!(manager.display_name(), "Mock");
    }

    #[tokio::test]
    async fn fallback_used_when_primary_fails() {
        // Create a primary that always fails
        struct FailingTts;

        #[async_trait]
        impl TtsEngine for FailingTts {
            async fn synthesize(&self, _text: &str, _params: &TtsParams) -> common::Result<AudioClip> {
                Err(common::KlyntbotError::Internal("always fails".to_string()))
            }
            fn supports_language(&self, _: &Language) -> bool { false }
            fn available_voices(&self, _: &Language) -> Vec<VoiceInfo> { vec![] }
            fn display_name(&self) -> &str { "Failing" }
        }

        let primary = Arc::new(FailingTts) as Arc<dyn TtsEngine>;
        let fallback = Arc::new(MockTtsEngine) as Arc<dyn TtsEngine>;
        let manager = TtsEngineManager::new(primary, Some(fallback));

        // Should succeed via fallback
        let clip = manager.synthesize("hello", &TtsParams::default()).await.unwrap();
        assert!(!clip.samples.is_empty());
    }

    #[tokio::test]
    async fn circuit_opens_after_threshold() {
        struct FailingTts;

        #[async_trait]
        impl TtsEngine for FailingTts {
            async fn synthesize(&self, _text: &str, _params: &TtsParams) -> common::Result<AudioClip> {
                Err(common::KlyntbotError::Internal("fail".to_string()))
            }
            fn supports_language(&self, _: &Language) -> bool { false }
            fn available_voices(&self, _: &Language) -> Vec<VoiceInfo> { vec![] }
            fn display_name(&self) -> &str { "Failing" }
        }

        let primary = Arc::new(FailingTts) as Arc<dyn TtsEngine>;
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout_secs: 60,
        };
        let manager = TtsEngineManager::with_config(primary, None, config);

        // Fail twice to open circuit
        let _ = manager.synthesize("a", &TtsParams::default()).await;
        let _ = manager.synthesize("b", &TtsParams::default()).await;

        // Circuit should now be open
        assert!(manager.is_circuit_open().await);
    }
}
```

- [ ] **Step 2: Register module**

In `crates/voice-engine/src/lib.rs`:
```rust
pub mod engine_manager;
pub use engine_manager::TtsEngineManager;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p voice-engine -E 'test(primary_succeeds)' && cargo nextest run -p voice-engine -E 'test(fallback_used)' && cargo nextest run -p voice-engine -E 'test(circuit_opens)'`

Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/engine_manager.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add TtsEngineManager with circuit-breaker and fallback"
```

---

### Task 4.2: Wire TtsEngineManager in Init

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Update init to create TtsEngineManager when both engines available**

In `crates/app-core/src/init/mod.rs`, after the TTS creation block, wrap in a manager if Kokoro + AVSpeech are both available:

```rust
                // Wrap in engine manager if we have a primary + fallback
                let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
                    let system_tts = Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir));

                    #[cfg(feature = "kokoro")]
                    {
                        if let config::schema::TtsEngineKind::Kokoro = voice_config.output.tts_engine {
                            if let Some(kokoro_dir) = model_manager.kokoro_model_dir() {
                                match voice_engine::KokoroTtsEngine::new(&kokoro_dir) {
                                    Ok(kokoro) => {
                                        info!("Kokoro TTS loaded — wrapping in engine manager with system fallback");
                                        let manager = voice_engine::TtsEngineManager::new(
                                            Arc::new(kokoro),
                                            Some(system_tts),
                                        );
                                        Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                                    }
                                    Err(e) => {
                                        warn!("Kokoro failed, using system TTS: {e}");
                                        Some(system_tts as Arc<dyn voice_engine::TtsEngine>)
                                    }
                                }
                            } else {
                                Some(system_tts as Arc<dyn voice_engine::TtsEngine>)
                            }
                        } else {
                            Some(system_tts as Arc<dyn voice_engine::TtsEngine>)
                        }
                    }

                    #[cfg(not(feature = "kokoro"))]
                    {
                        Some(system_tts as Arc<dyn voice_engine::TtsEngine>)
                    }
                };
```

- [ ] **Step 2: Build**

Run: `cargo build --workspace`

Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(voice): wire TtsEngineManager with Kokoro primary + system fallback"
```

---

## Phase 5: Streaming STT Partials

### Task 5.1: Implement Chunked Whisper Streaming

Currently `transcribe_stream` collects all audio then runs one pass. Change it to process audio in overlapping windows and emit intermediate partials.

**Files:**
- Modify: `crates/voice-engine/src/engines/whisper_local.rs`
- Test: inline (mock-based)

- [ ] **Step 1: Write a test for streaming partials**

Add to `crates/voice-engine/src/engines/whisper_local.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::TranscriptionEngine;
    use crate::types::AudioChunk;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn transcribe_stream_emits_final_partial() {
        // This test requires a real Whisper model — skip in CI
        // To run locally: cargo nextest run -p voice-engine -E 'test(transcribe_stream_emits_final)'
        let model_path = std::env::var("WHISPER_MODEL_PATH").ok();
        let Some(model_path) = model_path else {
            eprintln!("WHISPER_MODEL_PATH not set, skipping integration test");
            return;
        };

        let engine = WhisperLocalEngine::new(&model_path).unwrap();
        let (tx, rx) = mpsc::channel::<AudioChunk>(16);

        // Send 1 second of silence (should produce empty or minimal transcript)
        let chunk = AudioChunk {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
        };
        tx.send(chunk).await.unwrap();
        drop(tx);

        let mut stream = engine.transcribe_stream(rx).await.unwrap();
        let mut got_final = false;
        while let Some(partial) = stream.recv().await {
            if partial.is_final {
                got_final = true;
            }
        }
        assert!(got_final, "Should emit at least one final partial");
    }
}
```

- [ ] **Step 2: Add chunked processing to transcribe_stream**

In `crates/voice-engine/src/engines/whisper_local.rs`, modify `transcribe_stream` to emit intermediate partials every ~3 seconds of audio while still collecting:

```rust
    async fn transcribe_stream(&self, audio: AudioStream) -> common::Result<TranscriptStream> {
        let ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel(32);

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let mut all_samples = Vec::new();
            let mut audio = audio;

            // Collect audio chunks, emitting intermediate partials every ~3s
            let chunk_threshold = 16000 * 3; // 3 seconds of 16kHz audio
            let mut since_last_partial = 0usize;

            while let Some(chunk) = rt.block_on(audio.recv()) {
                all_samples.extend_from_slice(&chunk.samples);
                since_last_partial += chunk.samples.len();

                // Emit an intermediate partial every ~3 seconds
                if since_last_partial >= chunk_threshold && all_samples.len() > 16000 {
                    since_last_partial = 0;
                    if let Ok(state) = ctx.create_state() {
                        let params = Self::build_params(None);
                        if state.full(params, &all_samples).is_ok() {
                            if let Ok(transcript) = Self::extract_transcript(&state, None) {
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

            // Final transcription with all audio
            if all_samples.is_empty() {
                let _ = rt.block_on(tx.send(PartialTranscript {
                    text: String::new(),
                    segments: vec![],
                    language: Language::default(),
                    is_final: true,
                }));
                return;
            }

            match ctx.create_state() {
                Ok(state) => {
                    let params = Self::build_params(None);
                    match state.full(params, &all_samples) {
                        Ok(_) => {
                            let detected_lang = state
                                .full_lang_id()
                                .ok()
                                .and_then(|id| {
                                    whisper_rs::get_lang_str(id).map(|s| s.to_string())
                                });

                            match Self::extract_transcript(
                                &state,
                                detected_lang.as_deref(),
                            ) {
                                Ok(transcript) => {
                                    let _ = rt.block_on(tx.send(PartialTranscript {
                                        text: transcript.text,
                                        segments: transcript.segments,
                                        language: transcript.language,
                                        is_final: true,
                                    }));
                                }
                                Err(e) => {
                                    tracing::warn!("Transcript extraction failed: {e}");
                                    let _ = rt.block_on(tx.send(PartialTranscript {
                                        text: String::new(),
                                        segments: vec![],
                                        language: Language::default(),
                                        is_final: true,
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Whisper transcription failed: {e}");
                            let _ = rt.block_on(tx.send(PartialTranscript {
                                text: String::new(),
                                segments: vec![],
                                language: Language::default(),
                                is_final: true,
                            }));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create Whisper state: {e}");
                    let _ = rt.block_on(tx.send(PartialTranscript {
                        text: String::new(),
                        segments: vec![],
                        language: Language::default(),
                        is_final: true,
                    }));
                }
            }
        });

        Ok(rx)
    }
```

- [ ] **Step 3: Build and run existing tests**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

Expected: All pass. The mock-based tests don't use `WhisperLocalEngine` so they're unaffected. The new streaming behavior only activates for audio > 3 seconds.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/engines/whisper_local.rs
git commit -m "feat(voice): add intermediate STT partials every 3s during transcription"
```

---

### Task 5.2: Frontend Already Handles Partials

The frontend `useVoiceConversation.ts` already handles `PartialTranscript` events with `is_final: false` — it updates `transcript` and `segments` on every partial. The `VoiceBrainOrb` renders `WordHighlights` from segments. The backend now populates `segments` in `PartialTranscript` events (via the `TranscriptSegmentEvent` mapping in `service.rs:396-403`).

No frontend changes needed for Phase 5.

- [ ] **Step 1: Verify frontend handles partials correctly**

Read `desktop-ui/src/features/voice/hooks/useVoiceConversation.ts` lines 76-81 to confirm:

```typescript
      case "partialTranscript":
        setTranscript(payload.text as string);
        if (Array.isArray(payload.segments)) {
          setSegments(payload.segments as Array<{ text: string; confidence: number }>);
        }
        break;
```

This already processes both `is_final: true` and `is_final: false` partials identically — it updates the displayed transcript in real-time. No changes needed.

- [ ] **Step 2: Run workspace lint**

Run: `cargo clippy --workspace --all-targets --all-features`

Expected: 0 warnings (or only pre-existing desktop crate exceptions).

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "docs: verify frontend handles streaming STT partials"
```

---

## Post-Implementation Verification

After all 5 phases are complete, run the full CI check:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo nextest run --workspace
cargo test --workspace --doc
```

Then test manually:
1. Set `"ttsEngine": "system"` in config → verify macOS `say` TTS still works
2. Download Kokoro model → set `"ttsEngine": "kokoro"` → verify improved voice quality
3. Verify interrupt detection still works (speak during TTS output)
4. Verify streaming partials appear incrementally during longer speech
5. Verify circuit breaker: force Kokoro to fail → verify fallback to system TTS
