//! Streaming TTS pipeline — synthesizes sentences in order and plays them sequentially.
//!
//! The pipeline consists of two tasks:
//! 1. A **synthesizer task** that reads `SentenceItem`s and calls `TtsEngine::synthesize`.
//! 2. A **player task** that receives synthesized clips and plays them in order.
//!
//! Both tasks check a shared `stop` flag between iterations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use base64::Engine as _;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::events::VoiceEvent;
use crate::service::AudioPlayer;
use crate::tts::TtsEngine;
use crate::types::TtsParams;

/// A sentence queued for synthesis.
pub struct SentenceItem {
    pub text: String,
    pub is_final: bool,
}

/// Handle returned by [`StreamingTtsPipeline::start`].
pub struct StreamingTtsHandle {
    pub sentence_tx: mpsc::Sender<SentenceItem>,
    pub done_rx: oneshot::Receiver<()>,
    pub stop: Arc<AtomicBool>,
}

/// Internal clip sent from the synthesizer task to the player task.
struct PlaybackClip {
    samples: Vec<f32>,
    sample_rate: u32,
}

/// Streaming TTS pipeline.
///
/// Call [`StreamingTtsPipeline::start`] to launch the pipeline. Push [`SentenceItem`]s through the
/// returned `sentence_tx`. When you're done sending, drop the sender (or send an item with
/// `is_final: true`). The `done_rx` resolves once the last chunk has been played.
pub struct StreamingTtsPipeline;

impl StreamingTtsPipeline {
    /// Start the pipeline.
    ///
    /// # Parameters
    /// - `tts` — TTS engine to synthesize with.
    /// - `params` — synthesis parameters (voice, language, rate, …).
    /// - `audio_player` — player for native audio output.
    /// - `event_tx` — channel for emitting `VoiceEvent::SpeakChunk` events to the frontend.
    /// - `native_audio` — when `true`, audio is played back via `audio_player` and
    ///   `audio_base64` in the event is left empty; when `false`, playback is skipped and the
    ///   base64-encoded audio is carried in the `SpeakChunk` event for the frontend to handle.
    pub fn start(
        tts: Arc<dyn TtsEngine>,
        params: TtsParams,
        audio_player: AudioPlayer,
        event_tx: mpsc::Sender<VoiceEvent>,
        native_audio: bool,
    ) -> StreamingTtsHandle {
        let stop = Arc::new(AtomicBool::new(false));

        let (sentence_tx, mut sentence_rx) = mpsc::channel::<SentenceItem>(32);
        let (playback_tx, mut playback_rx) = mpsc::channel::<PlaybackClip>(8);
        let (done_tx, done_rx) = oneshot::channel::<()>();

        // ── Synthesizer task ─────────────────────────────────────────────────
        let stop_synth = stop.clone();
        let event_tx_synth = event_tx.clone();
        tokio::spawn(async move {
            let mut chunk_index: u32 = 0;

            while let Some(item) = sentence_rx.recv().await {
                if stop_synth.load(Ordering::Relaxed) {
                    break;
                }

                let clip = match tts.synthesize(&item.text, &params).await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("streaming_tts: synthesis error for {:?}: {e}", item.text);
                        continue;
                    }
                };

                let audio_base64 = if native_audio {
                    String::new()
                } else {
                    let bytes: Vec<u8> =
                        clip.samples.iter().flat_map(|s| s.to_le_bytes()).collect();
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                };

                let event = VoiceEvent::SpeakChunk {
                    audio_base64,
                    sample_rate: clip.sample_rate,
                    text: item.text.clone(),
                    chunk_index,
                    is_final: item.is_final,
                };

                // Best-effort event emission; if the receiver is gone we keep going.
                let _ = event_tx_synth.send(event).await;

                // Forward clip to player task.
                let playback_clip = PlaybackClip {
                    samples: clip.samples,
                    sample_rate: clip.sample_rate,
                };

                if playback_tx.send(playback_clip).await.is_err() {
                    // Player task has gone away — nothing more to do.
                    break;
                }

                chunk_index += 1;
            }
            // Dropping `playback_tx` signals the player task that synthesis is done.
        });

        // ── Player task ───────────────────────────────────────────────────────
        let stop_player = stop.clone();
        tokio::spawn(async move {
            while let Some(clip) = playback_rx.recv().await {
                if stop_player.load(Ordering::Relaxed) {
                    break;
                }

                if native_audio {
                    // Play the clip and await completion before moving to the next sentence.
                    let done_rx = audio_player.play(clip.samples, clip.sample_rate);
                    // Ignore the result — if the receiver is dropped, audio was stopped.
                    let _ = done_rx.await;
                }
                // If !native_audio the frontend plays audio from SpeakChunk events; skip here.
            }

            // Signal that the pipeline is fully done.
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
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use super::{SentenceItem, StreamingTtsPipeline};
    use crate::events::VoiceEvent;
    use crate::mock::MockTtsEngine;
    use crate::service::AudioPlayer;
    use crate::types::TtsParams;

    fn make_player() -> AudioPlayer {
        // Pass a bogus device name — in tests we always use native_audio: false so
        // AudioPlayer is never actually invoked, avoiding cpal initialization issues.
        AudioPlayer::new(Some("__test_no_device__".to_string()))
    }

    #[tokio::test]
    async fn pipeline_synthesizes_and_completes() {
        let tts = Arc::new(MockTtsEngine);
        let (event_tx, mut event_rx) = mpsc::channel::<VoiceEvent>(16);

        let handle = StreamingTtsPipeline::start(
            tts,
            TtsParams::default(),
            make_player(),
            event_tx,
            false, // native_audio=false: skip cpal, put audio in event
        );

        handle
            .sentence_tx
            .send(SentenceItem { text: "Hello world.".to_string(), is_final: false })
            .await
            .unwrap();

        handle
            .sentence_tx
            .send(SentenceItem { text: "Goodbye world.".to_string(), is_final: true })
            .await
            .unwrap();

        // Drop the sender so the synthesizer task can finish.
        drop(handle.sentence_tx);

        // Collect both SpeakChunk events.
        let mut chunks = Vec::new();
        while let Some(event) = event_rx.recv().await {
            if let VoiceEvent::SpeakChunk {
                text,
                chunk_index,
                is_final,
                ..
            } = event
            {
                chunks.push((text, chunk_index, is_final));
            }
            if chunks.len() == 2 {
                break;
            }
        }

        // Wait for pipeline to complete.
        handle.done_rx.await.unwrap();

        assert_eq!(chunks.len(), 2, "expected 2 SpeakChunk events");

        let (text0, idx0, final0) = &chunks[0];
        assert_eq!(idx0, &0, "first chunk index must be 0");
        assert!(!final0, "first chunk should not be final");
        assert_eq!(text0, "Hello world.");

        let (text1, idx1, final1) = &chunks[1];
        assert_eq!(idx1, &1, "second chunk index must be 1");
        assert!(final1, "second chunk should be final");
        assert_eq!(text1, "Goodbye world.");
    }

    #[tokio::test]
    async fn pipeline_stop_signal_halts_synthesis() {
        let tts = Arc::new(MockTtsEngine);
        let (event_tx, mut event_rx) = mpsc::channel::<VoiceEvent>(16);

        let handle = StreamingTtsPipeline::start(
            tts,
            TtsParams::default(),
            make_player(),
            event_tx,
            false,
        );

        // Signal stop immediately before sending any sentence.
        handle.stop.store(true, Ordering::Relaxed);

        handle
            .sentence_tx
            .send(SentenceItem { text: "Should not be synthesized.".to_string(), is_final: true })
            .await
            .unwrap();

        drop(handle.sentence_tx);

        // Pipeline should finish promptly.
        handle.done_rx.await.unwrap();

        // Drain any events — there should be none (or at most 0 SpeakChunk events).
        // (The synthesizer checks the stop flag *before* synthesizing, so no chunk
        //  should arrive. We use try_recv to avoid blocking forever.)
        let mut speak_chunk_count = 0usize;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, VoiceEvent::SpeakChunk { .. }) {
                speak_chunk_count += 1;
            }
        }

        assert_eq!(speak_chunk_count, 0, "no SpeakChunk events expected after stop");
    }
}
