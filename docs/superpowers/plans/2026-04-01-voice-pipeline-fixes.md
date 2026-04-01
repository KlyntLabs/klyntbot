# Voice Pipeline Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all dead code (noise reduction, WebRTC VAD), fix performance bottlenecks (base64 waste, sequential TTS, blocking synthesis), and fix bugs (idle unload, legacy TtsParams, orphaned code) in the voice pipeline.

**Architecture:** 13 independent tasks organized by priority tier. Tier 1 (Tasks 1–4) wires existing dead code with minimal risk. Tier 2 (Tasks 5–8) fixes performance bottlenecks. Tier 3 (Tasks 9–13) adds features and fixes minor bugs. Each task is self-contained and can be implemented in any order within its tier.

**Tech Stack:** Rust, cpal, nnnoiseless, webrtc-vad, qwen3-asr streaming API, TypeScript/React.

---

## File Structure

### Voice Engine (L5)
- Modify: `crates/voice-engine/src/capture.rs` — Wire noise reduction + WebRTC VAD into cpal callback
- Modify: `crates/voice-engine/src/service.rs` — Persistent audio output, skip base64, playback signal, idle-awareness
- Modify: `crates/voice-engine/src/engines/qwen3_tts.rs` — Chunked streaming synthesis
- Modify: `crates/voice-engine/src/engines/qwen3_asr.rs` — Streaming ASR via feed_audio API
- Modify: `crates/voice-engine/src/events.rs` — Add `SpeakChunk` variant, slim `SpeakResponse`

### App Core (L7)
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` — Non-blocking TTS, playback signal, idle-aware unload
- Modify: `crates/app-core/src/handlers/voice.rs` — Fix legacy TtsParams
- Modify: `crates/app-core/src/init/mod.rs` — Wire 1.7B model, download progress

### Frontend
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` — Gesture controls
- Delete: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts` — Orphaned hook
- Modify: `desktop-ui/src/features/voice/index.ts` — Remove useVoiceEvents export

---

## Task 1: Wire Noise Reduction into Capture Pipeline

Wire the existing `dsp::denoise_48khz()` (dsp.rs:34–52) into the cpal callback in `capture.rs`. Insert after mono-mix and before downsample. The function operates at 48kHz which is the native capture rate.

**Files:**
- Modify: `crates/voice-engine/src/capture.rs`

- [ ] **Step 1: Add denoise call in cpal callback**

In `crates/voice-engine/src/capture.rs`, the cpal callback builds mono audio at lines 167–173, then downsamples at lines 175–183. Insert denoising between them:

```rust
                    // Mono mix
                    let mono = if channels > 1 {
                        // ... existing averaging code ...
                    } else {
                        data.to_vec()
                    };

                    // Denoise at native sample rate (before downsampling)
                    #[cfg(feature = "vad")]
                    let mono = crate::dsp::denoise_48khz(&mono);

                    // Downsample to target rate
                    let downsampled = if native_sample_rate != target_sample_rate {
                        crate::dsp::downsample_with_filter(&mono, native_sample_rate, target_sample_rate)
                    } else {
                        mono
                    };
```

Note: `denoise_48khz` takes `&[f32]` and returns `Vec<f32>`. The `#[cfg(feature = "vad")]` gate matches the function's own gate. When the feature is off, the original `mono` passes through unchanged.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p voice-engine`

Expected: Clean compile. The `vad` feature is in `default`, so `denoise_48khz` is available.

- [ ] **Step 3: Run voice-engine tests**

Run: `cargo nextest run -p voice-engine`

Expected: All pass. The denoise function has its own unit test in dsp.rs.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/capture.rs
git commit -m "feat(voice): wire RNNoise denoising into capture pipeline"
```

---

## Task 2: Replace Inline RMS with WebRTC VAD

Replace the hand-rolled RMS threshold in the cpal callback (capture.rs:143–155) with `WebrtcVadProcessor` (vad.rs:21–64) for better speech/silence discrimination. The WebRTC VAD uses a GMM-based classifier that can distinguish speech from background noise, unlike pure RMS energy.

**Files:**
- Modify: `crates/voice-engine/src/capture.rs`

- [ ] **Step 1: Add VAD to AudioCapture::start()**

In `crates/voice-engine/src/capture.rs`, the `start()` method begins at line 77. Add a VAD processor before the callback closure. The VAD needs 16kHz input, so we run it on the downsampled audio:

After the `let stop = stop_signal.clone();` line (around line 120), add:

```rust
        #[cfg(feature = "vad")]
        let vad = std::sync::Mutex::new(
            crate::vad::WebrtcVadProcessor::new(true) // aggressive mode
        );
```

- [ ] **Step 2: Replace RMS silence detection with VAD**

Replace the silence detection block (capture.rs lines 141–155) which currently does:

```rust
                    let rms = compute_rms(data);
                    // Silence detection: ...
                    if rms > silence_threshold { ... }
```

With:

```rust
                    let rms = compute_rms(data);

                    // Voice activity detection (after downsampling for 16kHz input)
                    let is_speech = {
                        #[cfg(feature = "vad")]
                        {
                            // WebRTC VAD needs 16kHz 480-sample (30ms) frames
                            if let Ok(vad) = vad.lock() {
                                // Process in 480-sample frames
                                let mut speech = false;
                                for frame in downsampled.chunks(480) {
                                    if frame.len() == 480 {
                                        if let crate::vad::VadDecision::Speech(_) = vad.process_chunk(frame) {
                                            speech = true;
                                            break;
                                        }
                                    }
                                }
                                speech
                            } else {
                                rms > silence_threshold
                            }
                        }
                        #[cfg(not(feature = "vad"))]
                        {
                            rms > silence_threshold
                        }
                    };

                    if is_speech {
                        last_voice_time = Instant::now();
                        has_heard_voice = true;
                    } else if has_heard_voice
                        && !silence_fired
                        && last_voice_time.elapsed() >= silence_duration
                    {
                        silence_fired = true;
                        let _ = silence_tx.try_send(());
                    }
```

Note: The VAD check must happen AFTER the downsampled audio is computed (since VAD needs 16kHz). This means the code ordering needs to be: mono-mix → denoise → downsample → VAD check → send audio. Move the silence detection block after the downsample step.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p voice-engine`

Expected: Clean compile.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p voice-engine`

Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/capture.rs
git commit -m "feat(voice): replace RMS silence detection with WebRTC VAD"
```

---

## Task 3: Skip Base64 Encoding in Tauri Mode

In `service.rs:handle_response()`, ~1.3MB of base64 is computed and sent via IPC even though Tauri mode skips playback. Add a flag to `VoiceServiceConfig` to skip encoding and send only text+phase.

**Files:**
- Modify: `crates/voice-engine/src/service.rs`
- Modify: `crates/voice-engine/src/events.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add `native_audio` flag to VoiceServiceConfig**

In `crates/voice-engine/src/service.rs`, update `VoiceServiceConfig` (lines 123–128):

```rust
pub struct VoiceServiceConfig {
    pub capture: CaptureConfig,
    pub privacy_mode: PrivacyLevel,
    pub data_dir: PathBuf,
    /// When true, audio plays natively via cpal and base64 encoding is skipped.
    pub native_audio: bool,
}
```

Update the `Default` impl (lines 131–139) to add `native_audio: false`.

- [ ] **Step 2: Skip base64 in handle_response when native_audio is true**

In `handle_response()` (service.rs lines 659–704), replace:

```rust
                    Ok(clip) => {
                        info!(
                            "TTS: {} samples at {}Hz, playing natively + emitting SpeakResponse",
                            clip.samples.len(),
                            clip.sample_rate
                        );
                        play_audio_native(clip.samples.clone(), clip.sample_rate);
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
```

with:

```rust
                    Ok(clip) => {
                        info!(
                            "TTS: {} samples at {}Hz, emitting SpeakResponse (native_audio={})",
                            clip.samples.len(),
                            clip.sample_rate,
                            self.config.native_audio
                        );
                        if self.config.native_audio {
                            play_audio_native(clip.samples, clip.sample_rate);
                            let _ = self
                                .event_tx
                                .send(VoiceEvent::SpeakResponse {
                                    audio_base64: String::new(),
                                    sample_rate: clip.sample_rate,
                                    text: response_text.to_string(),
                                })
                                .await;
                        } else {
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
                    }
```

Note: When `native_audio` is true, we pass `clip.samples` directly (no `.clone()`) since we don't need the clip for base64 anymore.

- [ ] **Step 3: Set native_audio=true in desktop init**

In `crates/app-core/src/init/mod.rs`, update the `VoiceServiceConfig` construction (lines 711–725). Add `native_audio: true` after `data_dir`:

```rust
                let svc_config = VoiceServiceConfig {
                    capture: capture::CaptureConfig {
                        silence_threshold: 0.01,
                        silence_duration: std::time::Duration::from_secs_f32(
                            voice_config.input.silence_threshold_secs,
                        ),
                        ..Default::default()
                    },
                    privacy_mode: match voice_config.input.privacy_mode { ... },
                    data_dir: data_dir.clone(),
                    native_audio: true,
                };
```

- [ ] **Step 4: Verify compilation and run tests**

Run: `cargo check -p voice-engine -p app-core && cargo nextest run -p voice-engine`

Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/service.rs crates/app-core/src/init/mod.rs
git commit -m "perf(voice): skip base64 encoding when native audio is enabled"
```

---

## Task 4: Persistent Audio Output Stream

Replace per-call `play_audio_native()` (service.rs:50–120) with a persistent `AudioPlayer` that reuses the cpal output stream, avoids 10-50ms CoreAudio setup per call, and signals playback completion via a callback.

**Files:**
- Modify: `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Create AudioPlayer struct**

Replace `play_audio_native()` (service.rs lines 50–120) with:

```rust
/// Persistent audio output player. Reuses a single cpal stream across calls.
/// The stream is lazily opened on first play and kept alive.
pub struct AudioPlayer {
    tx: std::sync::mpsc::Sender<AudioCommand>,
    _thread: std::thread::JoinHandle<()>,
}

enum AudioCommand {
    Play {
        samples: Vec<f32>,
        sample_rate: u32,
        done_tx: tokio::sync::oneshot::Sender<()>,
    },
    Stop,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<AudioCommand>();

        let thread = std::thread::spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Play { samples, sample_rate, done_tx } => {
                        let host = cpal::default_host();
                        let Some(device) = host.default_output_device() else {
                            warn!("No default audio output device");
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
                                    data[..to_copy].copy_from_slice(&samples_ref[current..current + to_copy]);
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

                        // Wait for playback to finish
                        while !done.load(Ordering::Relaxed) {
                            // Check for stop command (non-blocking)
                            if let Ok(AudioCommand::Stop) = rx.try_recv() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                        // Stream drops here
                        let _ = done_tx.send(());
                    }
                    AudioCommand::Stop => {
                        // Already handled in the play loop
                    }
                }
            }
        });

        Self { tx, _thread: thread }
    }

    /// Play audio samples and return a future that resolves when playback completes.
    pub fn play(&self, samples: Vec<f32>, sample_rate: u32) -> tokio::sync::oneshot::Receiver<()> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let _ = self.tx.send(AudioCommand::Play { samples, sample_rate, done_tx });
        done_rx
    }

    /// Stop current playback immediately.
    pub fn stop(&self) {
        let _ = self.tx.send(AudioCommand::Stop);
    }
}
```

- [ ] **Step 2: Add AudioPlayer to VoiceService**

Add `audio_player: Arc<AudioPlayer>` field to `VoiceService`. Create it in `VoiceService::new()`. Update `handle_response()` to use `self.audio_player.play()` instead of `play_audio_native()`.

Also add a public `stop_tts_playback()` method:

```rust
    pub async fn stop_tts_playback(&self) {
        self.audio_player.stop();
    }
```

- [ ] **Step 3: Update voice_conversation.rs to use playback completion**

In `run_speaking_phase()` (voice_conversation.rs lines 849–961), replace the time-estimated playback duration with the actual completion signal:

```rust
        // Start TTS synthesis — returns playback completion future
        let playback_done = self.voice_service.handle_response_async(&tts_text, &tts_params).await?;
        let mut monitor = start_monitor_safe(&self.voice_service);

        // Wait for playback to finish OR interrupt
        tokio::select! {
            biased;
            Some(cmd) = cmd_rx.recv() => { /* handle command */ }
            _ = playback_done => {
                info!("Speaking phase: playback complete");
            }
            Some(rms) = monitor.rms_rx.recv() => {
                // interrupt detection...
            }
        }
```

- [ ] **Step 4: Delete old play_audio_native function**

Remove the standalone `play_audio_native()` function (service.rs lines 50–120).

- [ ] **Step 5: Verify compilation and run tests**

Run: `cargo check --workspace && cargo nextest run -p voice-engine`

Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/service.rs crates/app-core/src/handlers/voice_conversation.rs
git commit -m "perf(voice): persistent AudioPlayer with playback completion signal"
```

---

## Task 5: Wire 1.7B TtsInstruct Model in Init

`Qwen3Model::TtsInstruct` is defined in ModelManager but `init/mod.rs` only checks `qwen3_tts_model_dir()` (0.6B). Custom personas calling `generate_with_instruct` on the 0.6B model won't work correctly. Detect and prefer the 1.7B model when available.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Check for 1.7B model in TTS engine selection**

In `crates/app-core/src/init/mod.rs`, the TTS engine selection is at lines 673–705. After the existing `qwen3_tts_model_dir()` check (line 677), add a preference for the 1.7B model if any custom personas are configured:

```rust
                        config::schema::EngineDeployment::Local => {
                            if let config::schema::TtsEngineKind::Qwen3 =
                                voice_config.output.tts_engine
                            {
                                // Prefer 1.7B instruct model if custom personas exist
                                let has_custom_personas = voice_config.output.personas.values().any(
                                    |p| matches!(p, config::schema::VoicePersona::Custom { .. }),
                                );
                                let model_dir = if has_custom_personas {
                                    model_manager
                                        .qwen3_tts_instruct_model_dir()
                                        .or_else(|| model_manager.qwen3_tts_model_dir())
                                } else {
                                    model_manager.qwen3_tts_model_dir()
                                };

                                if let Some(dir) = model_dir {
                                    // ... existing Qwen3TtsEngine::new(&dir) code ...
```

- [ ] **Step 2: Add 1.7B to download list when custom personas configured**

In the download + hot-swap block (lines 860–904), also download the 1.7B model when custom personas are configured:

```rust
                        let has_custom = voice_config.output.personas.values().any(
                            |p| matches!(p, config::schema::VoicePersona::Custom { .. }),
                        );
                        // ...
                        let (asr, tts) = tokio::join!(
                            mm.download_model(Qwen3Model::Asr),
                            mm.download_model(Qwen3Model::Tts),
                        );
                        // Also download instruct model if needed
                        if has_custom {
                            if let Err(e) = mm.download_model(Qwen3Model::TtsInstruct).await {
                                warn!("Qwen3-TTS 1.7B instruct download: {e}");
                            }
                        }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p app-core`

Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(voice): prefer 1.7B TtsInstruct model for custom personas"
```

---

## Task 6: Extend Idle Timeout During Active Conversations

Both ASR and TTS models unload after 300s idle. A 5+ minute pause mid-conversation causes cold-reload. Add conversation-awareness to the idle unload logic.

**Files:**
- Modify: `crates/voice-engine/src/engines/qwen3_asr.rs`
- Modify: `crates/voice-engine/src/engines/qwen3_tts.rs`
- Modify: `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Add conversation-active flag to VoiceService**

In `crates/voice-engine/src/service.rs`, add a shared flag:

```rust
    conversation_active: Arc<AtomicBool>,
```

Initialize to `false` in `new()`. Add methods:

```rust
    pub fn set_conversation_active(&self, active: bool) {
        self.conversation_active.store(active, Ordering::Relaxed);
    }

    pub fn is_conversation_active(&self) -> bool {
        self.conversation_active.load(Ordering::Relaxed)
    }
```

- [ ] **Step 2: Gate idle unload on conversation state**

Update `try_unload_idle_stt()` and check conversation state:

```rust
    pub fn try_unload_idle_stt(&self) {
        if self.is_conversation_active() {
            return; // Don't unload during active conversation
        }
        // ... existing unload logic ...
    }
```

Apply the same guard in the TTS idle unload timer callback in `init/mod.rs`.

- [ ] **Step 3: Set flag in voice_conversation.rs**

In `VoiceConversationManager`, set `conversation_active(true)` when entering Listening phase and `conversation_active(false)` when returning to Idle.

- [ ] **Step 4: Verify and commit**

Run: `cargo check -p voice-engine -p app-core`

```bash
git add crates/voice-engine/src/service.rs crates/app-core/src/handlers/voice_conversation.rs crates/app-core/src/init/mod.rs
git commit -m "fix(voice): prevent model unload during active conversation"
```

---

## Task 7: Fix Legacy voice_stop_capture TtsParams

The legacy `voice_stop_capture` in `handlers/voice.rs:53` uses `TtsParams::default()`, ignoring all persona config.

**Files:**
- Modify: `crates/app-core/src/handlers/voice.rs`

- [ ] **Step 1: Read persona config in voice_stop_capture**

In `crates/app-core/src/handlers/voice.rs`, replace the `TtsParams::default()` usage (line 53):

```rust
                                // Build TtsParams from active persona (same logic as conversation manager)
                                let tts_params = {
                                    let config = voice_svc_config.read().await;
                                    let persona = config.output.personas.get(&config.output.default_persona);
                                    match persona {
                                        Some(config::schema::VoicePersona::Preset { speaker, speed, temperature }) => {
                                            voice_engine::TtsParams {
                                                voice_name: Some(speaker.clone()),
                                                speaking_rate: *speed,
                                                temperature: Some(*temperature),
                                                instruct: None,
                                                ..Default::default()
                                            }
                                        }
                                        Some(config::schema::VoicePersona::Custom { description, speed, temperature }) => {
                                            voice_engine::TtsParams {
                                                voice_name: None,
                                                speaking_rate: *speed,
                                                temperature: Some(*temperature),
                                                instruct: Some(description.clone()),
                                                ..Default::default()
                                            }
                                        }
                                        None => voice_engine::TtsParams::default(),
                                    }
                                };
                                if let Err(e) = voice_svc
                                    .handle_response(&content, &tts_params)
                                    .await
```

This requires access to voice config. The `voice_stop_capture` handler needs `Arc<RwLock<VoiceConfig>>` — check if `AppCore` already stores it. If not, store a reference to the config and pass it to the handler.

- [ ] **Step 2: Verify compilation and commit**

Run: `cargo check -p app-core`

```bash
git add crates/app-core/src/handlers/voice.rs
git commit -m "fix(voice): use persona-aware TtsParams in legacy voice handler"
```

---

## Task 8: Delete Orphaned useVoiceEvents Hook

`desktop-ui/src/features/voice/hooks/useVoiceEvents.ts` is not used by any component after the orb redesign.

**Files:**
- Delete: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`
- Modify: `desktop-ui/src/features/voice/index.ts`

- [ ] **Step 1: Verify no imports exist**

Run: `grep -r "useVoiceEvents" desktop-ui/src/ --include="*.ts" --include="*.tsx" -l`

Expected: Only `useVoiceEvents.ts` itself and `index.ts` (the barrel export).

- [ ] **Step 2: Delete the file and remove the export**

Delete `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`.

In `desktop-ui/src/features/voice/index.ts`, remove:

```typescript
export { useVoiceEvents } from "./hooks/useVoiceEvents";
```

- [ ] **Step 3: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add -u desktop-ui/src/features/voice/
git commit -m "chore(voice): delete orphaned useVoiceEvents hook"
```

---

## Task 9: Orb Gesture Controls

The redesigned orb has no UI for interrupt, pause, or new session. Add gesture-based controls: click to interrupt during speaking, double-click for new session.

**Files:**
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`

- [ ] **Step 1: Add gesture handlers**

In `VoiceBrainOrb.tsx`, expand the destructured hook values and add click handling:

```tsx
export function VoiceBrainOrb() {
  const { phase, audioLevel, start, end, interrupt, newSession, sessionInfo } = useVoiceConversation();
  const prevPhaseRef = useRef(phase);
  const clickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ... existing effects ...

  // Click: interrupt during speaking, end during listening
  // Double-click: new session
  const onClick = () => {
    if (clickTimerRef.current) {
      clearTimeout(clickTimerRef.current);
      clickTimerRef.current = null;
      newSession();
      return;
    }
    clickTimerRef.current = setTimeout(() => {
      clickTimerRef.current = null;
      if (phase === "speaking") {
        interrupt();
      }
    }, 250);
  };

  const onMouseDown = async () => {
    unlockAudioContext();
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().startDragging();
    }
  };

  return (
    <div
      onClick={onClick}
      onMouseDown={onMouseDown}
      style={{ width: "100%", height: "100%", cursor: "grab" }}
    >
      <VoiceOrbCanvas phase={phase} audioLevel={audioLevel} />
    </div>
  );
}
```

- [ ] **Step 2: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx
git commit -m "feat(voice): add click-to-interrupt and double-click-new-session gestures"
```

---

## Task 10: Streaming ASR (Real-Time Partial Transcripts)

Replace batch-mode ASR (waits for all audio, then transcribes) with the `qwen3-asr` streaming API (`init_streaming`/`feed_audio`/`finish_streaming`). This gives real-time partial transcript updates every 2 seconds while the user speaks.

**Files:**
- Modify: `crates/voice-engine/src/engines/qwen3_asr.rs`

- [ ] **Step 1: Implement streaming in transcribe_stream**

Replace the batch accumulation in `transcribe_stream()` (qwen3_asr.rs:84–175). Instead of draining all audio then inferring, use the streaming API to process 2-second chunks:

```rust
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        let state = self.state.clone();
        let models_dir = self.models_dir.clone();
        let allowed_languages = self.allowed_languages.clone();

        tokio::spawn(async move {
            // Collect audio chunks into a buffer, feed to streaming API periodically
            let mut all_samples: Vec<f32> = Vec::with_capacity(EXPECTED_SAMPLES);

            // Load model if needed (blocking)
            let model_loaded = tokio::task::spawn_blocking({
                let state = state.clone();
                let models_dir = models_dir.clone();
                move || -> Result<(), String> {
                    let mut guard = state.lock().unwrap();
                    if guard.model.is_none() {
                        info!("Lazy-loading Qwen3-ASR from {}...", models_dir.display());
                        let start = Instant::now();
                        let model = Qwen3AsrEngine::load_model(&models_dir)?;
                        info!("Qwen3-ASR loaded in {:.1}s", start.elapsed().as_secs_f32());
                        guard.model = Some(model);
                    }
                    Ok(())
                }
            })
            .await;

            if model_loaded.is_err() || model_loaded.as_ref().unwrap().is_err() {
                warn!("Qwen3-ASR model load failed");
                return;
            }

            // Initialize streaming state
            let streaming_state = {
                let guard = state.lock().unwrap();
                let model = guard.model.as_ref().unwrap();
                let opts = qwen3_asr::StreamingOptions::default()
                    .with_chunk_size_sec(2.0);
                model.init_streaming(opts)
            };
            let mut streaming_state = streaming_state;

            // Feed audio chunks as they arrive
            while let Some(chunk) = audio.recv().await {
                all_samples.extend_from_slice(&chunk.samples);

                // Feed to streaming inference
                let samples = chunk.samples;
                let result = tokio::task::spawn_blocking({
                    let state = state.clone();
                    move || -> Result<Option<qwen3_asr::TranscribeResult>, String> {
                        let guard = state.lock().unwrap();
                        let model = guard.model.as_ref().ok_or("Model not loaded")?;
                        model.feed_audio(&mut streaming_state, &samples)
                            .map_err(|e| format!("Streaming feed failed: {e}"))
                    }
                })
                .await;

                // Note: streaming_state was moved into spawn_blocking, need to restructure
                // to keep it alive. See step 2 for the correct approach.

                if let Ok(Ok(Some(partial_result))) = &result {
                    let normalized = normalize_language(&partial_result.language);
                    let _ = tx.send(PartialTranscript {
                        text: partial_result.text.trim().to_string(),
                        segments: vec![],
                        language: Language::new(normalized),
                        is_final: false,
                    }).await;
                }
            }

            // Finalize: flush remaining audio
            let final_result = tokio::task::spawn_blocking({
                let state = state.clone();
                move || -> Result<qwen3_asr::TranscribeResult, String> {
                    let guard = state.lock().unwrap();
                    let model = guard.model.as_ref().ok_or("Model not loaded")?;
                    model.finish_streaming(&mut streaming_state)
                        .map_err(|e| format!("Streaming finish failed: {e}"))
                }
            })
            .await;

            if let Ok(Ok(result)) = final_result {
                let normalized = normalize_language(&result.language);
                let lang = if result.language.is_empty()
                    || (!allowed_languages.is_empty()
                        && !allowed_languages.iter().any(|a| a == normalized))
                {
                    "en".to_string()
                } else {
                    normalized.to_string()
                };
                let _ = tx.send(PartialTranscript {
                    text: result.text.trim().to_string(),
                    segments: vec![],
                    language: Language::new(lang),
                    is_final: true,
                }).await;
            }
        });

        Ok(rx)
    }
```

**Important:** The `streaming_state` is owned and mutated, so it cannot be moved into `spawn_blocking` repeatedly. The correct approach is to hold the streaming state on the dedicated blocking thread and communicate via channels. This is a significant refactor — wrap the entire streaming loop in a single `spawn_blocking` that receives audio chunks via a channel and sends partial transcripts back.

- [ ] **Step 2: Restructure with channel-based blocking thread**

The streaming API requires `&mut StreamingState` which is `!Send` (contains Metal tensors). The correct architecture is:

1. Spawn a single `spawn_blocking` thread that owns the model lock and streaming state
2. Feed audio chunks to it via a `std::sync::mpsc::channel`
3. Receive partial transcripts back via `tokio::sync::mpsc`

This is a more involved refactor. The key insight: `spawn_blocking` owns the entire streaming lifecycle.

- [ ] **Step 3: Verify compilation and run tests**

Run: `cargo check -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/engines/qwen3_asr.rs
git commit -m "feat(voice): implement streaming ASR with real-time partial transcripts"
```

---

## Task 11: Non-Blocking TTS with Monitor

In `voice_conversation.rs`, `handle_response()` is fully awaited before the monitor starts. This means the user cannot interrupt during TTS synthesis. Start the monitor BEFORE synthesis begins.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Restructure run_speaking_phase**

In `run_speaking_phase()` (lines 849–961), start the monitor before calling handle_response, and run them concurrently:

```rust
        // Start monitor FIRST so interrupts work during synthesis
        let mut monitor = start_monitor_safe(&self.voice_service);

        // Start TTS synthesis concurrently with monitor
        let voice_svc = self.voice_service.clone();
        let tts_text_owned = tts_text.clone();
        let tts_params_owned = tts_params.clone();
        let tts_future = tokio::spawn(async move {
            voice_svc.handle_response(&tts_text_owned, &tts_params_owned).await
        });

        let mut interrupted = false;
        let mut consecutive_speech_samples = 0u32;

        // Wait for TTS to complete OR interrupt
        loop {
            tokio::select! {
                biased;
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        VoiceCommand::End | VoiceCommand::Interrupt => {
                            self.voice_service.stop_tts_playback().await;
                            tts_future.abort();
                            interrupted = matches!(cmd, VoiceCommand::Interrupt);
                            break;
                        }
                        _ => {}
                    }
                }
                result = &mut tts_future => {
                    if let Ok(Err(e)) = result {
                        warn!("Speaking phase: TTS failed: {e}");
                    }
                    break;
                }
                Some(rms) = monitor.rms_rx.recv() => {
                    if rms > INTERRUPT_RMS_THRESHOLD {
                        consecutive_speech_samples += 1;
                        if consecutive_speech_samples >= SPEECH_SAMPLE_THRESHOLD {
                            info!("Speaking phase: speech interrupt during TTS");
                            self.voice_service.stop_tts_playback().await;
                            tts_future.abort();
                            interrupted = true;
                            break;
                        }
                    } else {
                        consecutive_speech_samples = 0;
                    }
                }
            }
        }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p app-core`

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "feat(voice): allow interrupt during TTS synthesis, not just playback"
```

---

## Task 12: Streaming TTS Chunks (Play While Synthesizing)

Currently all TTS chunks are synthesized sequentially before any audio plays. Stream chunks: play the first chunk immediately while synthesizing the rest.

**Files:**
- Modify: `crates/voice-engine/src/engines/qwen3_tts.rs`
- Modify: `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Add a streaming synthesize method to Qwen3TtsEngine**

Add a method that returns a channel of audio chunks instead of a single AudioClip:

```rust
    /// Synthesize text in streaming mode: returns a channel that yields
    /// AudioClip chunks as they're generated.
    pub async fn synthesize_streaming(
        &self,
        text: &str,
        params: &TtsParams,
    ) -> common::Result<tokio::sync::mpsc::Receiver<AudioClip>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<AudioClip>(4);

        // ... setup code (voice, lang, temperature, instruct) ...

        let text_owned = text.to_string();
        let voice_owned = voice.to_string();
        let state = self.state.clone();
        let model_dir = self.model_dir.clone();

        tokio::task::spawn_blocking(move || {
            let mut guard = state.lock().unwrap();
            // ... model loading ...
            let model = guard.model.as_ref().unwrap();
            let chunks = qwen3_tts_rs::api::chunking::chunk_text(&text_owned, MAX_CHUNK_CHARS);

            for chunk in &chunks {
                let (samples, _sr) = /* generate_with_params or generate_with_instruct */;
                let clip = AudioClip {
                    samples,
                    sample_rate: QWEN3_TTS_SAMPLE_RATE,
                    channels: 1,
                };
                if tx.blocking_send(clip).is_err() {
                    break; // Receiver dropped (interrupted)
                }
            }
        });

        Ok(rx)
    }
```

- [ ] **Step 2: Update handle_response to stream chunks to AudioPlayer**

In `service.rs:handle_response()`, when using Qwen3 TTS, receive chunks and play each one as it arrives:

```rust
    // If TTS supports streaming
    let mut chunk_rx = tts.synthesize_streaming(response_text, tts_params).await?;
    let mut first_chunk = true;
    while let Some(clip) = chunk_rx.recv().await {
        let done_rx = self.audio_player.play(clip.samples, clip.sample_rate);
        if first_chunk {
            first_chunk = false;
            // Emit SpeakResponse with first chunk's text
            let _ = self.event_tx.send(VoiceEvent::SpeakResponse { ... }).await;
        }
    }
```

- [ ] **Step 3: Verify compilation and commit**

Run: `cargo check -p voice-engine -p app-core`

```bash
git add crates/voice-engine/src/engines/qwen3_tts.rs crates/voice-engine/src/service.rs
git commit -m "feat(voice): stream TTS chunks for instant first-word playback"
```

---

## Task 13: Download Progress Events

`ModelState::Downloading { progress }` exists but is never emitted. Wire progress reporting.

**Files:**
- Modify: `crates/voice-engine/src/model_manager.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add progress callback to download_model**

In `model_manager.rs`, update `download_model` to accept an optional progress callback:

```rust
    pub async fn download_model(
        &self,
        model: Qwen3Model,
    ) -> common::Result<PathBuf> {
        self.download_model_with_progress(model, |_, _| {}).await
    }

    pub async fn download_model_with_progress(
        &self,
        model: Qwen3Model,
        on_progress: impl Fn(u64, u64) + Send + 'static,
    ) -> common::Result<PathBuf> {
        // ... existing download logic ...
        // After each file download, call on_progress(bytes_downloaded, total_bytes)
    }
```

- [ ] **Step 2: Emit VoiceEvent::DownloadProgress from init**

In the download + hot-swap block of `init/mod.rs`, use the progress callback to emit events:

```rust
    let svc_for_progress = Arc::clone(&service);
    mm.download_model_with_progress(Qwen3Model::Asr, move |downloaded, total| {
        let _ = svc_for_progress.event_tx.try_send(VoiceEvent::DownloadProgress {
            model: "Qwen3-ASR".into(),
            downloaded,
            total,
        });
    }).await
```

- [ ] **Step 3: Add DownloadProgress variant to VoiceEvent**

In `events.rs`, add:

```rust
    DownloadProgress {
        model: String,
        downloaded: u64,
        total: u64,
    },
```

- [ ] **Step 4: Verify compilation and commit**

```bash
git add crates/voice-engine/src/model_manager.rs crates/voice-engine/src/events.rs crates/app-core/src/init/mod.rs
git commit -m "feat(voice): emit download progress events for model downloads"
```
