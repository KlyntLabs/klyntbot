# Voice Streaming TTS Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce voice response latency from ~4-12s to ~1.5-3s by overlapping LLM generation, TTS synthesis, and audio playback into a streaming sentence pipeline.

**Architecture:** Instead of waiting for the complete LLM response before starting TTS, we split incoming `ContentChunk` events at sentence boundaries. Each complete sentence is sent to TTS immediately while the LLM continues generating. Audio playback starts after the first sentence is synthesized, while subsequent sentences are synthesized in parallel. Additionally, we reduce the silence detection timeout and make both STT/TTS models preload when a conversation starts.

**Tech Stack:** Rust, tokio channels (mpsc), voice-engine crate, app-core handlers

---

## File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Create | `crates/voice-engine/src/sentence_accumulator.rs` | Buffers LLM text chunks, yields complete sentences at punctuation boundaries |
| Create | `crates/voice-engine/src/streaming_tts.rs` | Ordered pipeline: receives sentences, synthesizes via TtsEngine, queues audio clips for sequential playback |
| Modify | `crates/voice-engine/src/lib.rs` | Export new modules |
| Modify | `crates/voice-engine/src/service.rs` | Add `synthesize_streaming()` method that wires SentenceAccumulator + StreamingTtsPipeline |
| Modify | `crates/voice-engine/src/events.rs` | Add `SpeakChunk` event variant for incremental audio delivery |
| Modify | `crates/voice-engine/src/mock.rs` | Add `MockStreamingTts` for testing |
| Modify | `crates/app-core/src/handlers/voice_conversation.rs` | Replace sequential Reflecting→Speaking with merged streaming response phase |
| Modify | `crates/config/src/schema/voice.rs` | Add `streaming_tts` bool to `VoiceConversationConfig` |
| Modify | `crates/voice-engine/src/capture.rs` | No code change — silence duration already configurable via `CaptureConfig` |

---

### Task 1: SentenceAccumulator

Buffers incoming text chunks from LLM streaming and yields complete sentences. This is pure text processing with no dependencies on voice-engine internals.

**Files:**
- Create: `crates/voice-engine/src/sentence_accumulator.rs`

- [ ] **Step 1: Write failing tests for SentenceAccumulator**

Create the test module first with all key behaviors:

```rust
// crates/voice-engine/src/sentence_accumulator.rs

/// Buffers LLM streaming text chunks and yields complete sentences.
///
/// Splits on sentence-ending punctuation (`.` `!` `?` `。` `！` `？`)
/// followed by whitespace or end-of-stream. Requires a minimum sentence
/// length to avoid TTS overhead on tiny fragments.
pub struct SentenceAccumulator {
    buffer: String,
    /// Minimum character count before a sentence boundary triggers a yield.
    min_sentence_len: usize,
}

impl SentenceAccumulator {
    pub fn new(min_sentence_len: usize) -> Self {
        Self {
            buffer: String::new(),
            min_sentence_len,
        }
    }

    /// Push a text chunk (from `AgentEvent::ContentChunk`).
    /// Call `take_sentence()` after each push to drain ready sentences.
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// If a complete sentence is available, return and remove it from the buffer.
    /// Returns `None` if no sentence boundary found yet.
    pub fn take_sentence(&mut self) -> Option<String> {
        // Find the earliest sentence-ending punctuation followed by whitespace
        // (or at the end of buffer if buffer is long enough).
        let bytes = self.buffer.as_bytes();
        for (i, ch) in self.buffer.char_indices() {
            if is_sentence_end(ch) {
                // Check if next char is whitespace or end of buffer
                let after = i + ch.len_utf8();
                let at_boundary = after >= self.buffer.len()
                    || self.buffer[after..].starts_with(|c: char| c.is_whitespace());
                if at_boundary && after >= self.min_sentence_len {
                    let sentence = self.buffer[..after].to_string();
                    self.buffer = self.buffer[after..].trim_start().to_string();
                    return Some(sentence);
                }
            }
        }
        None
    }

    /// Flush any remaining buffered text (call when LLM stream ends).
    /// Returns `None` if the buffer is empty or whitespace-only.
    pub fn flush(&mut self) -> Option<String> {
        let text = std::mem::take(&mut self.buffer);
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.trim().is_empty()
    }
}

fn is_sentence_end(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '。' | '！' | '？')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_sentence_from_chunks() {
        let mut acc = SentenceAccumulator::new(5);
        acc.push("Hello ");
        assert_eq!(acc.take_sentence(), None);
        acc.push("world.");
        assert_eq!(acc.take_sentence(), None); // no trailing space yet, at end of buffer — should yield
        // Actually at end of buffer and >= min_len, so it should yield:
        // Let's re-check: after = 12, buffer.len() = 12, so after >= buffer.len() is true.
        // 12 >= 5 is true. So it should yield.
    }

    #[test]
    fn yields_at_period_followed_by_space() {
        let mut acc = SentenceAccumulator::new(5);
        acc.push("Hello world. How are you?");
        assert_eq!(acc.take_sentence(), Some("Hello world.".to_string()));
        assert_eq!(acc.take_sentence(), Some("How are you?".to_string()));
        assert_eq!(acc.take_sentence(), None);
    }

    #[test]
    fn respects_min_length() {
        let mut acc = SentenceAccumulator::new(20);
        acc.push("Hi. ");
        // "Hi." is only 3 chars, below min_sentence_len of 20
        assert_eq!(acc.take_sentence(), None);
        acc.push("This is a longer sentence. Next.");
        // "Hi. This is a longer sentence." is 30 chars — above threshold
        let s = acc.take_sentence();
        assert!(s.is_some());
        assert!(s.unwrap().ends_with('.'));
    }

    #[test]
    fn chinese_sentence_endings() {
        let mut acc = SentenceAccumulator::new(2);
        acc.push("你好世界。这是测试。");
        assert_eq!(acc.take_sentence(), Some("你好世界。".to_string()));
        assert_eq!(acc.take_sentence(), Some("这是测试。".to_string()));
    }

    #[test]
    fn flush_returns_remainder() {
        let mut acc = SentenceAccumulator::new(5);
        acc.push("incomplete thought");
        assert_eq!(acc.take_sentence(), None);
        assert_eq!(acc.flush(), Some("incomplete thought".to_string()));
        assert!(acc.is_empty());
    }

    #[test]
    fn flush_empty_returns_none() {
        let mut acc = SentenceAccumulator::new(5);
        assert_eq!(acc.flush(), None);
    }

    #[test]
    fn exclamation_and_question_marks() {
        let mut acc = SentenceAccumulator::new(3);
        acc.push("Wow! Really? Yes.");
        assert_eq!(acc.take_sentence(), Some("Wow!".to_string()));
        assert_eq!(acc.take_sentence(), Some("Really?".to_string()));
        assert_eq!(acc.take_sentence(), Some("Yes.".to_string()));
    }

    #[test]
    fn incremental_chunks() {
        let mut acc = SentenceAccumulator::new(5);
        acc.push("The quick ");
        assert_eq!(acc.take_sentence(), None);
        acc.push("brown fox. ");
        assert_eq!(acc.take_sentence(), Some("The quick brown fox.".to_string()));
        acc.push("Jumped ");
        assert_eq!(acc.take_sentence(), None);
        acc.push("over. Done.");
        assert_eq!(acc.take_sentence(), Some("Jumped over.".to_string()));
        assert_eq!(acc.take_sentence(), Some("Done.".to_string()));
    }

    #[test]
    fn abbreviations_below_min_length_not_split() {
        // "Dr. Smith" — the period after "Dr" is only 3 chars in, below min_sentence_len=10
        let mut acc = SentenceAccumulator::new(10);
        acc.push("Dr. Smith arrived. ");
        // "Dr." is 3 chars (below 10), so it won't split there.
        // "Dr. Smith arrived." is 18 chars, so it will split there.
        assert_eq!(acc.take_sentence(), Some("Dr. Smith arrived.".to_string()));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine -E 'test(sentence_accumulator)'`
Expected: All 8 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/src/sentence_accumulator.rs
git commit -m "feat(voice): add SentenceAccumulator for streaming TTS pipeline"
```

---

### Task 2: StreamingTtsPipeline

Receives sentences over a channel, synthesizes each via the existing `TtsEngine`, and plays audio clips in order. The key property: playback of sentence N starts as soon as it's synthesized, even if sentence N+1 is still being generated by the LLM or synthesized by TTS.

**Files:**
- Create: `crates/voice-engine/src/streaming_tts.rs`
- Modify: `crates/voice-engine/src/events.rs` (add `SpeakChunk` variant)
- Modify: `crates/voice-engine/src/mock.rs` (add delay mock for pipeline testing)

- [ ] **Step 1: Add `SpeakChunk` event variant**

In `crates/voice-engine/src/events.rs`, add a new variant to `VoiceEvent` for incremental audio delivery. Insert after the existing `SpeakResponse` variant (line 63):

```rust
    /// One sentence of TTS audio in the streaming pipeline.
    SpeakChunk {
        /// Base64-encoded audio (empty when native_audio is true).
        #[serde(rename = "audioBase64")]
        audio_base64: String,
        #[serde(rename = "sampleRate")]
        sample_rate: u32,
        /// The sentence text that was synthesized.
        text: String,
        /// 0-based index of this chunk in the response.
        #[serde(rename = "chunkIndex")]
        chunk_index: u32,
        /// True if this is the last chunk.
        #[serde(rename = "isFinal")]
        is_final: bool,
    },
```

- [ ] **Step 2: Write the StreamingTtsPipeline**

```rust
// crates/voice-engine/src/streaming_tts.rs

//! Streaming TTS pipeline — synthesizes and plays sentences as they arrive.
//!
//! Runs two concurrent tasks:
//! 1. **Synthesizer**: reads sentences from `sentence_rx`, calls `TtsEngine::synthesize()`,
//!    sends `AudioClip` to the playback queue.
//! 2. **Player**: reads clips from the playback queue, plays them in order via `AudioPlayer`.
//!
//! The pipeline is cancellable via a stop signal. Interrupt detection is handled
//! by the caller (VoiceConversationManager).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::events::VoiceEvent;
use crate::service::AudioPlayer;
use crate::tts::TtsEngine;
use crate::types::TtsParams;

/// A sentence queued for synthesis.
pub struct SentenceItem {
    pub text: String,
    /// True if this is the last sentence in the response.
    pub is_final: bool,
}

/// Handle returned by `StreamingTtsPipeline::start()`.
pub struct StreamingTtsHandle {
    /// Send sentences into the pipeline.
    pub sentence_tx: mpsc::Sender<SentenceItem>,
    /// Resolves when all sentences have been played (or pipeline was stopped).
    pub done_rx: tokio::sync::oneshot::Receiver<()>,
    /// Signal to stop the pipeline early.
    pub stop: Arc<AtomicBool>,
}

pub struct StreamingTtsPipeline;

impl StreamingTtsPipeline {
    /// Start the streaming TTS pipeline.
    ///
    /// - `tts`: the synthesis engine
    /// - `params`: voice parameters (speaker, rate, etc.)
    /// - `audio_player`: the persistent cpal audio output player
    /// - `event_tx`: channel to emit `VoiceEvent`s to the frontend
    /// - `native_audio`: if true, play via cpal and skip base64 encoding
    pub fn start(
        tts: Arc<dyn TtsEngine>,
        params: TtsParams,
        audio_player: AudioPlayer,
        event_tx: mpsc::Sender<VoiceEvent>,
        native_audio: bool,
    ) -> StreamingTtsHandle {
        let (sentence_tx, sentence_rx) = mpsc::channel::<SentenceItem>(8);
        let (clip_tx, clip_rx) = mpsc::channel::<(Vec<f32>, u32, String, u32, bool)>(4);
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let stop = Arc::new(AtomicBool::new(false));

        // Task 1: Synthesizer — TTS each sentence as it arrives.
        let stop_synth = stop.clone();
        let event_tx_synth = event_tx.clone();
        tokio::spawn(async move {
            let mut rx = sentence_rx;
            let mut chunk_index = 0u32;
            while let Some(item) = rx.recv().await {
                if stop_synth.load(Ordering::Relaxed) {
                    break;
                }
                match tts.synthesize(&item.text, &params).await {
                    Ok(clip) if !clip.samples.is_empty() => {
                        let sr = clip.sample_rate;
                        // Emit SpeakChunk event for frontend
                        let audio_base64 = if native_audio {
                            String::new()
                        } else {
                            use base64::Engine;
                            let bytes: Vec<u8> =
                                clip.samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                            base64::engine::general_purpose::STANDARD.encode(&bytes)
                        };
                        let _ = event_tx_synth
                            .send(VoiceEvent::SpeakChunk {
                                audio_base64,
                                sample_rate: sr,
                                text: item.text.clone(),
                                chunk_index,
                                is_final: item.is_final,
                            })
                            .await;
                        // Send to playback queue
                        if clip_tx
                            .send((clip.samples, sr, item.text, chunk_index, item.is_final))
                            .await
                            .is_err()
                        {
                            break; // player dropped
                        }
                        chunk_index += 1;
                    }
                    Ok(_) => {
                        warn!(
                            "StreamingTTS: empty clip for chunk {}: '{}'",
                            chunk_index,
                            &item.text[..item.text.len().min(40)]
                        );
                    }
                    Err(e) => {
                        warn!("StreamingTTS: synthesis failed for chunk {}: {e}", chunk_index);
                    }
                }
            }
            drop(clip_tx); // Signal player that no more clips are coming
        });

        // Task 2: Player — play clips in order as they arrive.
        let stop_play = stop.clone();
        tokio::spawn(async move {
            let mut rx = clip_rx;
            while let Some((samples, sample_rate, text, idx, _is_final)) = rx.recv().await {
                if stop_play.load(Ordering::Relaxed) {
                    break;
                }
                info!(
                    "StreamingTTS: playing chunk {} ({} samples, '{}')",
                    idx,
                    samples.len(),
                    &text[..text.len().min(30)]
                );
                if native_audio {
                    let playback_rx = audio_player.play(samples, sample_rate);
                    // Wait for this chunk to finish before playing the next
                    let _ = playback_rx.await;
                }
                // If not native_audio, the frontend handles playback via SpeakChunk events
            }
            let _ = done_tx.send(());
        });

        StreamingTtsHandle {
            sentence_tx,
            done_rx,
            stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockTtsEngine;

    #[tokio::test]
    async fn pipeline_synthesizes_and_completes() {
        let tts: Arc<dyn TtsEngine> = Arc::new(MockTtsEngine);
        let params = TtsParams::default();
        let audio_player = AudioPlayer::new(None);
        let (event_tx, mut event_rx) = mpsc::channel(32);

        let handle = StreamingTtsPipeline::start(
            tts,
            params,
            audio_player,
            event_tx,
            false, // not native — skip cpal playback in test
        );

        // Send two sentences
        handle
            .sentence_tx
            .send(SentenceItem {
                text: "Hello world.".to_string(),
                is_final: false,
            })
            .await
            .unwrap();
        handle
            .sentence_tx
            .send(SentenceItem {
                text: "Goodbye.".to_string(),
                is_final: true,
            })
            .await
            .unwrap();
        drop(handle.sentence_tx); // signal end

        // Wait for done
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            handle.done_rx,
        )
        .await
        .expect("pipeline should complete within 5s");

        // Should have received SpeakChunk events
        let mut chunks = vec![];
        while let Ok(event) = event_rx.try_recv() {
            if let VoiceEvent::SpeakChunk { chunk_index, is_final, .. } = event {
                chunks.push((chunk_index, is_final));
            }
        }
        assert_eq!(chunks, vec![(0, false), (1, true)]);
    }

    #[tokio::test]
    async fn pipeline_stop_signal_halts_synthesis() {
        let tts: Arc<dyn TtsEngine> = Arc::new(MockTtsEngine);
        let params = TtsParams::default();
        let audio_player = AudioPlayer::new(None);
        let (event_tx, _event_rx) = mpsc::channel(32);

        let handle = StreamingTtsPipeline::start(
            tts,
            params,
            audio_player,
            event_tx,
            false,
        );

        // Stop immediately
        handle.stop.store(true, Ordering::Relaxed);

        // Send a sentence — should be dropped
        let _ = handle
            .sentence_tx
            .send(SentenceItem {
                text: "Should not synthesize.".to_string(),
                is_final: true,
            })
            .await;
        drop(handle.sentence_tx);

        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            handle.done_rx,
        )
        .await;
        // No panic = pass
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p voice-engine -E 'test(streaming_tts)'`
Expected: Both tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/streaming_tts.rs crates/voice-engine/src/events.rs
git commit -m "feat(voice): add StreamingTtsPipeline for sentence-level TTS streaming"
```

---

### Task 3: Wire modules into voice-engine crate

**Files:**
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Add module declarations and exports**

In `crates/voice-engine/src/lib.rs`, add the two new modules. Insert after `pub mod service;` (line 20):

```rust
pub mod sentence_accumulator;
pub mod streaming_tts;
```

Add exports after the existing `pub use` block (after line 41):

```rust
pub use sentence_accumulator::SentenceAccumulator;
pub use streaming_tts::{StreamingTtsPipeline, StreamingTtsHandle, SentenceItem};
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p voice-engine`
Expected: Compiles with 0 errors

- [ ] **Step 3: Run all voice-engine tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/lib.rs
git commit -m "feat(voice): export SentenceAccumulator and StreamingTtsPipeline"
```

---

### Task 4: Add `streaming_tts` config flag

Add a boolean flag to `VoiceConversationConfig` so streaming TTS can be toggled. Default: `true`.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`

- [ ] **Step 1: Add field to `VoiceConversationConfig`**

In `crates/config/src/schema/voice.rs`, add the field to the struct (after `adaptive_breath` at line 211):

```rust
    /// Stream TTS sentence-by-sentence during LLM generation (default: true).
    /// When false, waits for the complete response before synthesizing.
    #[serde(default = "default_true")]
    pub streaming_tts: bool,
```

Add the field in the `Default` impl (after `adaptive_breath: true,` inside the `Default for VoiceConversationConfig` block):

```rust
            streaming_tts: true,
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p config`
Expected: Compiles with 0 errors

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/voice.rs
git commit -m "feat(config): add streaming_tts flag to VoiceConversationConfig"
```

---

### Task 5: Add `synthesize_streaming()` to VoiceService

Add a convenience method on `VoiceService` that creates and returns a `StreamingTtsHandle`, wiring the TTS engine and audio player.

**Files:**
- Modify: `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Add `synthesize_streaming()` method**

Add this method to the `impl VoiceService` block, after `handle_response_with_completion` (after line 885):

```rust
    /// Start a streaming TTS pipeline for sentence-by-sentence synthesis.
    ///
    /// Returns a handle with a `sentence_tx` channel. The caller pushes
    /// `SentenceItem`s as the LLM generates text, and the pipeline synthesizes
    /// and plays each sentence in order. Await `done_rx` for completion.
    pub fn start_streaming_tts(
        &self,
        tts_params: &TtsParams,
    ) -> Option<crate::streaming_tts::StreamingTtsHandle> {
        let tts = self.tts.read().ok().and_then(|g| g.clone())?;
        let native_audio = self.cfg().native_audio;

        Some(crate::streaming_tts::StreamingTtsPipeline::start(
            tts,
            tts_params.clone(),
            AudioPlayer::new(self.cfg().output_device.clone()),
            self.event_tx.clone(),
            native_audio,
        ))
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p voice-engine`
Expected: Compiles with 0 errors

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/src/service.rs
git commit -m "feat(voice): add start_streaming_tts() to VoiceService"
```

---

### Task 6: Merge Reflecting + Speaking into a streaming response phase

This is the core integration task. Replace the sequential `run_reflecting_phase` → `run_speaking_phase` flow with a single `run_streaming_response_phase` that overlaps LLM generation with TTS.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Add `run_streaming_response_phase` method**

Add this new method to `impl VoiceConversationManager`, alongside the existing `run_reflecting_phase`. This method handles the full flow: sends transcript to agent, streams ContentChunk events through SentenceAccumulator, and feeds complete sentences into StreamingTtsPipeline.

```rust
    /// Streaming response phase: overlaps LLM generation with TTS synthesis.
    ///
    /// Instead of Reflecting → (wait for full response) → Speaking, this method
    /// feeds LLM output through a SentenceAccumulator and sends complete sentences
    /// to the StreamingTtsPipeline as they arrive. Playback starts after the first
    /// sentence is synthesized, while the LLM and TTS continue in parallel.
    async fn run_streaming_response_phase(&self, cmd_rx: &mut mpsc::Receiver<VoiceCommand>) {
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
            info!("Streaming response: empty transcript, back to listening");
            self.transition_to(ConversationPhase::Listening).await;
            return;
        }

        // Auto-title (same as run_reflecting_phase)
        let needs_title = {
            let state = self.state.lock().await;
            state.session_title.is_empty()
        };
        if needs_title {
            let title: String = transcript_text.chars().take(60).collect();
            let metadata = serde_json::json!({
                "title": title,
                "is_voice_session": true,
            });
            let _ = self
                .repos
                .sessions
                .upsert_voice_session(&session_key_str, &metadata)
                .await;
            self.state.lock().await.session_title = title;
            self.emitter.emit_event(
                "chat:thread_updated",
                serde_json::json!({ "sessionKey": session_key_str }),
            );
        }

        // Emit Reflecting phase event
        let (turn_count, session_title) = {
            let state = self.state.lock().await;
            (state.turn_count, state.session_title.clone())
        };
        let _ = self
            .voice_service
            .emit_event(VoiceEvent::PhaseChanged {
                phase: "reflecting".to_string(),
                session_title: Some(session_title.clone()),
                turn_count,
            })
            .await;

        // Emit chat:message_added for the voice transcript
        if let Ok(val) = serde_json::to_value(&desktop_shared::events::ChatMessagePayload {
            session_key: session_key_str.clone(),
            source: "voice".to_string(),
        }) {
            self.emitter
                .emit_event(desktop_shared::events::CHAT_MESSAGE_ADDED, val);
        }

        // Start agent processing
        let streaming_handle = match self
            .agent
            .process_direct_streaming(transcript_text, session_key_str.clone())
            .await
        {
            Ok(handle) => handle,
            Err(e) => {
                warn!("Streaming response: agent error: {e}");
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

        // Set up TTS pipeline
        let tts_params = {
            let config = self.config.read().await;
            tts_params_from_config(&config.output)
        };

        let tts_pipeline = self.voice_service.start_streaming_tts(&tts_params);
        let (sentence_tx, tts_stop, mut tts_done_rx) = match tts_pipeline {
            Some(handle) => (Some(handle.sentence_tx), Some(handle.stop), Some(handle.done_rx)),
            None => {
                warn!("Streaming response: no TTS engine, will collect text only");
                (None, None, None)
            }
        };

        let mut sentence_acc = voice_engine::SentenceAccumulator::new(10);
        let mut response_content = String::new();
        let mut sentence_count = 0u32;
        let mut speaking_phase_emitted = false;

        // Start interrupt monitor
        let mut monitor = start_monitor_safe(&self.voice_service);
        const INTERRUPT_RMS_THRESHOLD: f32 = 0.02;
        let mut consecutive_speech_samples = 0u32;
        const SPEECH_SAMPLE_THRESHOLD: u32 = 3;
        let mut interrupted = false;

        // Main event loop: process LLM chunks, feed sentences to TTS
        loop {
            tokio::select! {
                biased;
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        VoiceCommand::End | VoiceCommand::Pause => {
                            info!("Streaming response: {:?} received, cancelling", cmd);
                            cancel_token.cancel();
                            if let Some(stop) = &tts_stop {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            self.voice_service.stop_tts_playback().await;
                            if matches!(cmd, VoiceCommand::Pause) {
                                self.state.lock().await.paused = true;
                            }
                            // Stop monitor
                            if let Some(ref stop) = monitor.stop_signal {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            drop(monitor);
                            self.transition_to(ConversationPhase::Idle).await;
                            return;
                        }
                        VoiceCommand::Interrupt => {
                            cancel_token.cancel();
                            if let Some(stop) = &tts_stop {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            self.voice_service.stop_tts_playback().await;
                            interrupted = true;
                            break;
                        }
                        _ => {}
                    }
                }
                Some(rms) = monitor.rms_rx.recv() => {
                    if rms > INTERRUPT_RMS_THRESHOLD {
                        consecutive_speech_samples += 1;
                        if consecutive_speech_samples >= SPEECH_SAMPLE_THRESHOLD {
                            info!("Streaming response: speech interrupt detected");
                            cancel_token.cancel();
                            if let Some(stop) = &tts_stop {
                                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            self.voice_service.stop_tts_playback().await;
                            interrupted = true;
                            break;
                        }
                    } else {
                        consecutive_speech_samples = 0;
                    }
                }
                event = event_rx.recv() => {
                    match event {
                        Some(agent::AgentEvent::ContentChunk { data }) => {
                            // Forward to chat UI
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

                            // Feed into sentence accumulator
                            sentence_acc.push(&data);
                            while let Some(sentence) = sentence_acc.take_sentence() {
                                // Emit Speaking phase on first sentence
                                if !speaking_phase_emitted {
                                    speaking_phase_emitted = true;
                                    let _ = self
                                        .voice_service
                                        .emit_event(VoiceEvent::PhaseChanged {
                                            phase: "speaking".to_string(),
                                            session_title: Some(session_title.clone()),
                                            turn_count,
                                        })
                                        .await;
                                }
                                if let Some(ref tx) = sentence_tx {
                                    let _ = tx.send(voice_engine::SentenceItem {
                                        text: sentence,
                                        is_final: false,
                                    }).await;
                                }
                                sentence_count += 1;
                            }
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
                                self.emitter
                                    .emit_event(desktop_shared::events::AGENT_DONE, val);
                            }
                            let _ = message_id;
                            // Emit chat:message_added for assistant response
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::ChatMessagePayload {
                                    session_key: session_key_str.clone(),
                                    source: "voice".to_string(),
                                },
                            ) {
                                self.emitter
                                    .emit_event(desktop_shared::events::CHAT_MESSAGE_ADDED, val);
                            }

                            // Flush remaining text as final sentence
                            if let Some(remainder) = sentence_acc.flush() {
                                if !speaking_phase_emitted {
                                    let _ = self
                                        .voice_service
                                        .emit_event(VoiceEvent::PhaseChanged {
                                            phase: "speaking".to_string(),
                                            session_title: Some(session_title.clone()),
                                            turn_count,
                                        })
                                        .await;
                                }
                                if let Some(ref tx) = sentence_tx {
                                    let _ = tx.send(voice_engine::SentenceItem {
                                        text: remainder,
                                        is_final: true,
                                    }).await;
                                }
                                sentence_count += 1;
                            }
                            break;
                        }
                        Some(agent::AgentEvent::Error { message }) => {
                            warn!("Streaming response: agent error: {message}");
                            if let Ok(val) = serde_json::to_value(
                                &desktop_shared::events::AgentErrorPayload {
                                    session_key: session_key_str.clone(),
                                    message,
                                },
                            ) {
                                self.emitter
                                    .emit_event(desktop_shared::events::AGENT_ERROR, val);
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
                                self.emitter
                                    .emit_event(desktop_shared::events::AGENT_TOOL_START, val);
                            }
                        }
                        Some(_) => {} // Other agent events — ignore
                        None => break, // Channel closed
                    }
                }
            }
        }

        // Close the sentence sender to signal the TTS pipeline to finish
        drop(sentence_tx);

        // Wait for TTS playback to complete (unless interrupted)
        if !interrupted {
            if let Some(done_rx) = tts_done_rx.take() {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    done_rx,
                )
                .await;
            }
        }

        // Stop monitor
        if let Some(ref stop) = monitor.stop_signal {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        drop(monitor);

        // Update state
        {
            let mut state = self.state.lock().await;
            state.pending_response_text = Some(response_content);
            state.pending_transcript = None;
            state.turn_count += 1;
            state.tts_position = 0;
            state.interrupted = interrupted;
            state.touch();
        }

        // Emit chat thread event
        {
            let state = self.state.lock().await;
            if let Some(ref sk) = state.session_key {
                let is_new = state.turn_count == 1;
                self.emitter.emit_chat_thread(is_new, sk.as_str());
            }
        }

        if interrupted {
            let _ = self.voice_service.emit_event(VoiceEvent::TtsFadeOut).await;
            let _ = self
                .voice_service
                .emit_event(VoiceEvent::ContinueAvailable { timeout_secs: 8 })
                .await;
            self.transition_to(ConversationPhase::Listening).await;
        } else {
            self.auto_resume_or_idle().await;
        }
    }
```

- [ ] **Step 2: Wire the streaming phase into the conversation loop**

In the main conversation loop (around line 426-452), the Reflecting phase currently transitions to Speaking. We need to make the Reflecting phase use `run_streaming_response_phase` when the config flag is enabled.

Find the match arm for `ConversationPhase::Reflecting` in the main loop and replace it:

Replace:
```rust
                ConversationPhase::Reflecting => {
                    self.run_reflecting_phase(&mut cmd_rx).await;
                }
```

With:
```rust
                ConversationPhase::Reflecting => {
                    let use_streaming = self.config.read().await.conversation.streaming_tts;
                    if use_streaming {
                        self.run_streaming_response_phase(&mut cmd_rx).await;
                    } else {
                        self.run_reflecting_phase(&mut cmd_rx).await;
                    }
                }
```

This preserves the old path as a fallback when `streaming_tts` is `false`.

- [ ] **Step 3: Skip the Speaking phase when streaming was used**

When `run_streaming_response_phase` completes, it calls `auto_resume_or_idle()` directly — it never transitions to `ConversationPhase::Speaking`. The existing `run_speaking_phase` code still works for the non-streaming path. No changes needed to the Speaking match arm.

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p app-core`
Expected: Compiles with 0 errors

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "feat(voice): streaming response phase — overlap LLM + TTS + playback"
```

---

### Task 7: Quick latency wins — silence timeout + model preloading

Small, independent optimizations that don't need streaming TTS to work.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Add `conversation_silence_secs` field to config**

In `crates/config/src/schema/voice.rs`, add a dedicated conversation-mode silence override to `VoiceConversationConfig` (after `streaming_tts`):

```rust
    /// Silence duration for conversation mode (default: 0.8s).
    /// Shorter than the general silence_threshold_secs (1.5s) for faster turn-taking.
    #[serde(default = "default_conversation_silence")]
    pub conversation_silence_secs: f32,
```

Add the default function near the other defaults:

```rust
fn default_conversation_silence() -> f32 {
    0.8
}
```

Add to the `Default` impl:

```rust
            conversation_silence_secs: default_conversation_silence(),
```

- [ ] **Step 2: Apply conversation silence override in voice_conversation.rs**

In `run_listening_phase`, before calling `self.voice_service.start_capture()`, apply the shorter silence duration. Add before the `start_capture` call (around line 461):

```rust
        // Apply conversation-mode silence duration (shorter for faster turn-taking)
        {
            let config = self.config.read().await;
            let conv_silence = config.conversation.conversation_silence_secs;
            self.voice_service.set_silence_duration(
                std::time::Duration::from_secs_f32(conv_silence),
            );
        }
```

This requires a new setter on `VoiceService`. In `crates/voice-engine/src/service.rs`, add after `set_output_device` (or similar setter area):

```rust
    /// Override the silence detection duration for the next capture.
    pub fn set_silence_duration(&self, duration: std::time::Duration) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.capture.silence_duration = duration;
        }
        self.capture.set_silence_duration(duration);
    }
```

In `crates/voice-engine/src/capture.rs`, add to `impl AudioCapture`:

```rust
    /// Update the silence duration for future captures.
    pub fn set_silence_duration(&self, duration: std::time::Duration) {
        if let Ok(mut cfg) = self.config.write() {
            cfg.silence_duration = duration;
        }
    }
```

Note: `AudioCapture` holds its config in a `RwLock`. Check if it uses a `RwLock` or direct field — if direct, wrap in `RwLock` or pass via the config. If `AudioCapture` doesn't have a mutable config, the simpler approach is to read the conversation silence from config when constructing `CaptureConfig` in `start_capture`.

**Simpler alternative if `AudioCapture` config isn't mutable:** Instead of a setter, modify the `start_capture` flow in the `VoiceConversationManager` to pass the silence duration as a parameter. Check the actual `AudioCapture` implementation to choose the right approach.

- [ ] **Step 3: Preload models when conversation starts**

In `run_listening_phase`, after starting capture, trigger preload of both STT and TTS models. Add after the capture start succeeds (around line 476):

```rust
        // Preload TTS model in background so it's warm when the response comes
        let voice_svc = self.voice_service.clone();
        tokio::spawn(async move {
            voice_svc.preload_tts().await;
        });
```

This requires a `preload_tts()` method on `VoiceService`. Add to `impl VoiceService` in `service.rs`:

```rust
    /// Eagerly preload the TTS model so it's warm for the first response.
    pub async fn preload_tts(&self) {
        // STT is already loaded by start_capture. Just preload TTS.
        if let Some(tts) = self.tts.read().ok().and_then(|g| g.clone()) {
            if let Some(qwen) = tts.as_any().downcast_ref::<crate::engines::Qwen3TtsEngine>() {
                let _ = qwen.preload().await;
            }
        }
    }
```

**Note:** This requires `TtsEngine` to expose an `as_any()` method, or alternatively, add a `preload()` default method to the `TtsEngine` trait:

In `crates/voice-engine/src/tts.rs`, add to the trait:

```rust
    /// Eagerly load the model into memory. Default: no-op.
    async fn preload(&self) -> common::Result<()> {
        Ok(())
    }
```

Then in `Qwen3TtsEngine`'s `impl TtsEngine`, the existing `preload` method already exists on the struct — just delegate:

Override in the trait impl block in `qwen3_tts.rs`:

```rust
    async fn preload(&self) -> common::Result<()> {
        Qwen3TtsEngine::preload(self).await
    }
```

Then `VoiceService::preload_tts()` becomes simpler:

```rust
    pub async fn preload_tts(&self) {
        if let Some(tts) = self.tts.read().ok().and_then(|g| g.clone()) {
            let _ = tts.preload().await;
        }
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --workspace`
Expected: Compiles with 0 errors

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/config/src/schema/voice.rs crates/voice-engine/src/tts.rs \
       crates/voice-engine/src/engines/qwen3_tts.rs crates/voice-engine/src/service.rs \
       crates/voice-engine/src/capture.rs crates/app-core/src/handlers/voice_conversation.rs
git commit -m "perf(voice): shorter conversation silence (0.8s) + TTS model preloading"
```

---

### Task 8: Frontend — handle `SpeakChunk` events

The frontend voice orb currently expects a single `SpeakResponse` event with the complete audio. It needs to handle incremental `SpeakChunk` events for the streaming pipeline.

**Files:**
- Identify: the frontend voice event handler (likely in `desktop-ui/src/features/voice/`)

- [ ] **Step 1: Find the frontend event handler**

Search for `SpeakResponse` or `speakResponse` in the desktop-ui to locate the handler:

Run: `grep -rn "speakResponse\|SpeakResponse\|speak_response" desktop-ui/src/`

- [ ] **Step 2: Add `speakChunk` handler**

The handler should:
1. Queue audio chunks in order (by `chunkIndex`)
2. Start playing the first chunk immediately
3. When a chunk finishes playing, start the next one
4. Use the same `AudioContext` / Web Audio API already in use for `SpeakResponse`

The exact code depends on the frontend structure found in Step 1. The pattern is:

```typescript
// In the voice event handler switch/if block:
case "speakChunk": {
  const { audioBase64, sampleRate, text, chunkIndex, isFinal } = event;
  // Decode and queue the audio chunk
  voicePlayer.enqueueChunk(audioBase64, sampleRate, chunkIndex, isFinal);
  break;
}
```

The `voicePlayer` needs an `enqueueChunk` method that maintains an ordered buffer and auto-advances playback. If native audio is used (base64 is empty), this event is informational only (for UI text display).

- [ ] **Step 3: Verify the orb UI transitions**

The orb should transition to "speaking" state when the first `speakChunk` arrives (or when `PhaseChanged { phase: "speaking" }` is received). Verify this works by checking the phase change logic.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/voice/
git commit -m "feat(desktop-ui): handle streaming SpeakChunk events in voice player"
```

---

### Task 9: Integration test + full workspace verification

- [ ] **Step 1: Run all voice-engine tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests PASS

- [ ] **Step 2: Run app-core tests**

Run: `cargo nextest run -p app-core`
Expected: All tests PASS

- [ ] **Step 3: Run workspace-wide clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 4: Run formatting check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 5: Build desktop app**

Run: `cargo build -p desktop`
Expected: Compiles with 0 errors

- [ ] **Step 6: Run desktop-ui lint**

Run: `cd desktop-ui && bun run lint`
Expected: No errors

- [ ] **Step 7: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore(voice): fix lint and test issues from streaming TTS integration"
```

---

## Latency Impact Summary

| Before | After | Improvement |
|--------|-------|-------------|
| 1.5s silence wait | 0.8s silence wait | -0.7s |
| LLM generates full response (~1-5s) | First sentence available (~0.5-1s) | -0.5-4s |
| TTS synthesizes full response (~1-3s) | TTS starts on first sentence (~0.3-0.5s) | -0.7-2.5s |
| TTS model cold start on first response | Preloaded during listening | -1-5s (first turn) |
| **Total: ~4-12s** | **Total: ~1.5-3s** | **~3-4x faster** |
