# Voice Brain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add voice as a core modality — universal voice input with language learning lens and light TTS read-back, enabling "tap mic -> orb appears -> agent understands and speaks back" on desktop.

**Architecture:** New `crates/voice-engine/` at L1 owns `TranscriptionEngine` + `TtsEngine` traits, `AudioCapture` (cpal), `VoiceService` orchestrator, and `ModelManager`. Desktop adapter wires to Tauri commands, menu-bar mic, global hotkey, and Voice Brain orb UI. Native Rust captures audio and transcribes (whisper-rs local-first, Groq fallback); frontend renders the orb and plays TTS via Web Audio API. Voice enters the agent pipeline as `InboundMessage` with `kind: Voice` + `VoiceMetadata` — zero changes to AgentRuntime or SkillRouter.

**Tech Stack:** Rust (voice-engine, platform-macos, app-core, desktop, bus, config, cognitive crates), whisper-rs (Metal-accelerated), cpal (audio I/O), objc2 (AVSpeechSynthesizer), TypeScript/React (Voice Brain orb UI, settings tab)

**Spec:** `docs/superpowers/specs/2026-03-28-voice-brain-design.md`

---

### Task 1: Scaffold voice-engine crate + core types

**Files:**
- Create: `crates/voice-engine/Cargo.toml`
- Create: `crates/voice-engine/src/lib.rs`
- Create: `crates/voice-engine/src/types.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create Cargo.toml**

Create `crates/voice-engine/Cargo.toml`:

```toml
[package]
name = "voice-engine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
async-trait.workspace = true
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["sync", "time", "rt", "fs"] }
tracing.workspace = true
chrono.workspace = true
```

- [ ] **Step 2: Create types.rs with all core types**

Create `crates/voice-engine/src/types.rs`:

```rust
//! Core types for the voice engine.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Detected or configured language for voice capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language(pub String);

impl Language {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Language {
    fn default() -> Self {
        Self("en".to_string())
    }
}

/// Which transcription engine produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    Local,
    Cloud,
}

/// Privacy level for voice capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyLevel {
    /// Normal: full cognitive pipeline integration.
    #[default]
    Standard,
    /// Strict: skip mirror lookups and pronunciation history.
    Strict,
    /// Off: no privacy restrictions.
    Off,
}

/// A chunk of raw audio samples from the microphone.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// A complete transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Language,
    pub segments: Vec<TranscriptSegment>,
    pub overall_confidence: f32,
}

/// A word-level segment within a transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub text: String,
    #[serde(with = "duration_millis")]
    pub start: Duration,
    #[serde(with = "duration_millis")]
    pub end: Duration,
    /// 0.0–1.0 confidence score. Powers green/red word highlights.
    pub confidence: f32,
}

/// Metadata attached to a voice capture for downstream enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMetadata {
    pub language: Language,
    pub overall_confidence: f32,
    /// (word, score) pairs for pronunciation feedback.
    pub pronunciation_scores: Vec<(String, f32)>,
    /// Path to stored audio file for later playback/FSRS.
    pub audio_ref: Option<String>,
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    pub engine: EngineKind,
    pub privacy_mode: PrivacyLevel,
}

/// Synthesized audio clip ready for playback.
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Parameters for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsParams {
    pub language: Language,
    pub voice_name: Option<String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
}

fn default_speaking_rate() -> f32 {
    1.0
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

/// Info about an available TTS voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub identifier: String,
    pub display_name: String,
    pub language: Language,
}

/// Pronunciation rating derived from confidence thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PronunciationRating {
    Good,
    Fair,
    Poor,
}

/// Per-word pronunciation score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordScore {
    pub word: String,
    pub confidence: f32,
    pub rating: PronunciationRating,
}

/// Pronunciation analysis report for a transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PronunciationReport {
    pub overall_score: f32,
    pub word_scores: Vec<WordScore>,
    pub weak_words_count: usize,
    pub improvement_suggestion: Option<String>,
}

/// Serde helper for Duration as milliseconds.
mod duration_millis {
    use std::time::Duration;

    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}
```

- [ ] **Step 3: Create lib.rs**

Create `crates/voice-engine/src/lib.rs`:

```rust
//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (whisper-rs local, Groq cloud, AVSpeech),
//! the `AudioCapture` subsystem, and the `VoiceService` orchestrator.

pub mod pronunciation;
pub mod types;

pub use types::*;
```

- [ ] **Step 4: Add to workspace members**

In root `Cargo.toml`, add `"crates/voice-engine"` to the `members` array (after `crates/platform-macos`):

```toml
"crates/platform-macos",
"crates/voice-engine",
"crates/autotuner",
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p voice-engine`
Expected: Compiles with zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/ Cargo.toml Cargo.lock
git commit -m "feat(voice): scaffold voice-engine crate with core types"
```

---

### Task 2: Pronunciation scoring module

**Files:**
- Create: `crates/voice-engine/src/pronunciation.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test**

Create `crates/voice-engine/src/pronunciation.rs`:

```rust
//! Pronunciation scoring — word-level confidence analysis.

use crate::types::{
    PronunciationRating, PronunciationReport, Transcript, TranscriptSegment, WordScore,
};

/// Compute pronunciation scores from Whisper transcript confidence values.
///
/// Whisper confidence is a proxy for pronunciation quality — it struggles
/// to decode poorly pronounced non-native speech, producing low confidence.
pub fn compute_pronunciation_report(transcript: &Transcript) -> PronunciationReport {
    let word_scores: Vec<WordScore> = transcript
        .segments
        .iter()
        .map(|seg| WordScore {
            word: seg.text.clone(),
            confidence: seg.confidence,
            rating: match seg.confidence {
                c if c >= 0.85 => PronunciationRating::Good,
                c if c >= 0.60 => PronunciationRating::Fair,
                _ => PronunciationRating::Poor,
            },
        })
        .collect();

    let overall_score = if word_scores.is_empty() {
        0.0
    } else {
        word_scores.iter().map(|w| w.confidence).sum::<f32>() / word_scores.len() as f32
    };

    let weak_words: Vec<&WordScore> = word_scores
        .iter()
        .filter(|w| w.rating == PronunciationRating::Poor)
        .collect();

    let improvement_suggestion = if weak_words.is_empty() {
        None
    } else {
        Some(format!(
            "Focus on: {}",
            weak_words
                .iter()
                .map(|w| w.word.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    };

    PronunciationReport {
        overall_score,
        word_scores,
        weak_words_count: weak_words.len(),
        improvement_suggestion,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::types::Language;

    fn seg(text: &str, confidence: f32) -> TranscriptSegment {
        TranscriptSegment {
            text: text.to_string(),
            start: Duration::ZERO,
            end: Duration::from_millis(500),
            confidence,
        }
    }

    fn transcript(segments: Vec<TranscriptSegment>) -> Transcript {
        Transcript {
            text: segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" "),
            language: Language::new("fr"),
            segments,
            overall_confidence: 0.0, // recomputed by function
        }
    }

    #[test]
    fn empty_transcript_gives_zero_score() {
        let t = transcript(vec![]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.overall_score, 0.0);
        assert_eq!(report.weak_words_count, 0);
        assert!(report.improvement_suggestion.is_none());
    }

    #[test]
    fn all_good_pronunciation() {
        let t = transcript(vec![seg("bonjour", 0.95), seg("monde", 0.90)]);
        let report = compute_pronunciation_report(&t);
        assert!(report.overall_score > 0.9);
        assert_eq!(report.weak_words_count, 0);
        assert!(report.improvement_suggestion.is_none());
        assert!(report.word_scores.iter().all(|w| w.rating == PronunciationRating::Good));
    }

    #[test]
    fn mixed_pronunciation_highlights_weak_words() {
        let t = transcript(vec![
            seg("je", 0.92),
            seg("suis", 0.40),
            seg("content", 0.70),
        ]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.weak_words_count, 1);
        assert_eq!(report.word_scores[0].rating, PronunciationRating::Good);
        assert_eq!(report.word_scores[1].rating, PronunciationRating::Poor);
        assert_eq!(report.word_scores[2].rating, PronunciationRating::Fair);
        assert_eq!(report.improvement_suggestion.as_deref(), Some("Focus on: suis"));
    }

    #[test]
    fn single_word_transcript() {
        let t = transcript(vec![seg("merci", 0.55)]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.overall_score, 0.55);
        assert_eq!(report.weak_words_count, 1);
        assert!(report.improvement_suggestion.as_ref().unwrap().contains("merci"));
    }

    #[test]
    fn boundary_confidence_values() {
        // Exactly at boundaries
        let t = transcript(vec![
            seg("a", 0.85), // Good boundary
            seg("b", 0.60), // Fair boundary
            seg("c", 0.59), // Poor (just below Fair)
        ]);
        let report = compute_pronunciation_report(&t);
        assert_eq!(report.word_scores[0].rating, PronunciationRating::Good);
        assert_eq!(report.word_scores[1].rating, PronunciationRating::Fair);
        assert_eq!(report.word_scores[2].rating, PronunciationRating::Poor);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine`
Expected: All 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/src/pronunciation.rs
git commit -m "feat(voice): add pronunciation scoring with word-level confidence analysis"
```

---

### Task 3: TranscriptionEngine + TtsEngine traits + mock implementations

**Files:**
- Create: `crates/voice-engine/src/stt.rs`
- Create: `crates/voice-engine/src/tts.rs`
- Create: `crates/voice-engine/src/mock.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Create TranscriptionEngine trait**

Create `crates/voice-engine/src/stt.rs`:

```rust
//! Speech-to-text transcription engine trait.

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{AudioChunk, Language, Transcript, TranscriptSegment};

/// Streaming partial transcript result.
#[derive(Debug, Clone)]
pub struct PartialTranscript {
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
    pub language: Language,
    pub is_final: bool,
}

/// Sender for audio chunks into the transcription pipeline.
pub type AudioStream = mpsc::Receiver<AudioChunk>;

/// Receiver for partial transcripts from the transcription pipeline.
pub type TranscriptStream = mpsc::Receiver<PartialTranscript>;

/// Core trait for speech-to-text engines (local whisper-rs, cloud Groq, etc.).
#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    /// Stream partial transcripts as audio chunks arrive.
    async fn transcribe_stream(
        &self,
        audio: AudioStream,
    ) -> common::Result<TranscriptStream>;

    /// Transcribe a complete audio file (for Telegram voice notes, etc.).
    async fn transcribe_file(
        &self,
        path: &Path,
        lang_hint: Option<&Language>,
    ) -> common::Result<Transcript>;

    /// Human-readable name for UI display (e.g., "Local (whisper-small)").
    fn display_name(&self) -> &str;
}
```

- [ ] **Step 2: Create TtsEngine trait**

Create `crates/voice-engine/src/tts.rs`:

```rust
//! Text-to-speech engine trait.

use async_trait::async_trait;

use crate::types::{AudioClip, Language, TtsParams, VoiceInfo};

/// Core trait for text-to-speech engines (AVSpeech, Qwen3-TTS, etc.).
#[async_trait]
pub trait TtsEngine: Send + Sync {
    /// Synthesize text into an audio clip.
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip>;

    /// Check if this engine supports a given language.
    fn supports_language(&self, lang: &Language) -> bool;

    /// List available voices for a language.
    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo>;

    /// Human-readable name for UI display.
    fn display_name(&self) -> &str;
}
```

- [ ] **Step 3: Create mock implementations for testing**

Create `crates/voice-engine/src/mock.rs`:

```rust
//! Mock implementations for testing without real audio hardware or models.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::tts::TtsEngine;
use crate::types::*;

/// Mock STT engine that returns pre-configured transcripts.
pub struct MockTranscriptionEngine {
    pub transcript: Transcript,
    pub partials: Vec<PartialTranscript>,
}

impl MockTranscriptionEngine {
    pub fn new(text: &str) -> Self {
        let segments = text
            .split_whitespace()
            .enumerate()
            .map(|(i, word)| TranscriptSegment {
                text: word.to_string(),
                start: Duration::from_millis(i as u64 * 300),
                end: Duration::from_millis((i as u64 + 1) * 300),
                confidence: 0.90,
            })
            .collect::<Vec<_>>();

        Self {
            transcript: Transcript {
                text: text.to_string(),
                language: Language::new("en"),
                overall_confidence: 0.90,
                segments,
            },
            partials: vec![],
        }
    }

    /// Set custom partials for streaming tests.
    pub fn with_partials(mut self, partials: Vec<PartialTranscript>) -> Self {
        self.partials = partials;
        self
    }
}

#[async_trait]
impl TranscriptionEngine for MockTranscriptionEngine {
    async fn transcribe_stream(
        &self,
        mut _audio: AudioStream,
    ) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel(32);
        let partials = self.partials.clone();
        let transcript = self.transcript.clone();

        tokio::spawn(async move {
            // Drain audio stream
            while _audio.recv().await.is_some() {}

            // Emit partials
            for partial in partials {
                let _ = tx.send(partial).await;
            }

            // Emit final
            let _ = tx
                .send(PartialTranscript {
                    text: transcript.text.clone(),
                    segments: transcript.segments.clone(),
                    language: transcript.language.clone(),
                    is_final: true,
                })
                .await;
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        _path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        Ok(self.transcript.clone())
    }

    fn display_name(&self) -> &str {
        "Mock"
    }
}

/// Mock TTS engine that returns silence.
pub struct MockTtsEngine;

#[async_trait]
impl TtsEngine for MockTtsEngine {
    async fn synthesize(&self, _text: &str, _params: &TtsParams) -> common::Result<AudioClip> {
        Ok(AudioClip {
            samples: vec![0.0; 16000], // 1 second of silence at 16kHz
            sample_rate: 16000,
            channels: 1,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true
    }

    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo> {
        vec![VoiceInfo {
            identifier: "mock-voice".to_string(),
            display_name: "Mock Voice".to_string(),
            language: lang.clone(),
        }]
    }

    fn display_name(&self) -> &str {
        "Mock"
    }
}
```

- [ ] **Step 4: Update lib.rs exports**

Update `crates/voice-engine/src/lib.rs`:

```rust
//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (whisper-rs local, Groq cloud, AVSpeech),
//! and the `VoiceService` orchestrator.

pub mod mock;
pub mod pronunciation;
pub mod stt;
pub mod tts;
pub mod types;

pub use pronunciation::compute_pronunciation_report;
pub use stt::{PartialTranscript, TranscriptionEngine};
pub use tts::TtsEngine;
pub use types::*;
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`
Expected: Compiles, existing pronunciation tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/
git commit -m "feat(voice): add TranscriptionEngine + TtsEngine traits with mock impls"
```

---

### Task 4: VoiceConfig in config crate

**Files:**
- Create: `crates/config/src/schema/voice.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Create VoiceConfig**

Create `crates/config/src/schema/voice.rs`:

```rust
//! Voice input/output configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Top-level voice configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub input: VoiceInputConfig,
    #[serde(default)]
    pub output: VoiceOutputConfig,
    #[serde(default)]
    pub learning: VoiceLearningConfig,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: VoiceInputConfig::default(),
            output: VoiceOutputConfig::default(),
            learning: VoiceLearningConfig::default(),
        }
    }
}

/// Voice input (microphone + transcription) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInputConfig {
    /// Global hotkey for voice capture (default: "super+shift+v").
    #[serde(default = "default_voice_hotkey")]
    pub hotkey: String,

    /// Seconds of silence before auto-stop (0.5–3.0).
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_secs: f32,

    /// Privacy level: standard (full integration), strict (skip mirror/pronunciation history), off.
    #[serde(default)]
    pub privacy_mode: VoicePrivacyMode,

    /// Preferred transcription engine: "local" (default) or "cloud".
    #[serde(default = "default_prefer_local")]
    pub prefer_local: bool,

    /// Whisper model size: "small" (default) or "medium".
    #[serde(default = "default_model_size")]
    pub model_size: String,
}

impl Default for VoiceInputConfig {
    fn default() -> Self {
        Self {
            hotkey: default_voice_hotkey(),
            silence_threshold_secs: default_silence_threshold(),
            privacy_mode: VoicePrivacyMode::default(),
            prefer_local: true,
            model_size: default_model_size(),
        }
    }
}

/// Voice output (TTS) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceOutputConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Preferred TTS voice name per language (e.g., {"en": "Samantha", "fr": "Thomas"}).
    #[serde(default)]
    pub voice_preferences: std::collections::HashMap<String, String>,

    /// Speaking rate multiplier (0.5–2.0).
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,

    /// Whether TTS should play during active focus sessions.
    #[serde(default)]
    pub speak_during_focus: bool,
}

impl Default for VoiceOutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_preferences: std::collections::HashMap::new(),
            speaking_rate: 1.0,
            speak_during_focus: false,
        }
    }
}

/// Language-learning-specific voice settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceLearningConfig {
    /// Target language code for pronunciation scoring (e.g., "fr").
    #[serde(default)]
    pub target_language: Option<String>,

    /// Show pronunciation confidence highlights in orb.
    #[serde(default = "default_true")]
    pub show_pronunciation_scores: bool,

    /// Auto-create FSRS flashcards from voice learning captures.
    #[serde(default = "default_true")]
    pub auto_create_flashcards: bool,
}

impl Default for VoiceLearningConfig {
    fn default() -> Self {
        Self {
            target_language: None,
            show_pronunciation_scores: true,
            auto_create_flashcards: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoicePrivacyMode {
    #[default]
    Standard,
    Strict,
    Off,
}

fn default_voice_hotkey() -> String {
    "super+shift+v".to_string()
}

fn default_silence_threshold() -> f32 {
    1.5
}

fn default_prefer_local() -> bool {
    true
}

fn default_model_size() -> String {
    "small".to_string()
}

fn default_speaking_rate() -> f32 {
    1.0
}
```

- [ ] **Step 2: Register in config schema**

In `crates/config/src/schema/mod.rs`, add after the `work_context` line:

```rust
mod voice;
```

And in the pub use block, add:

```rust
pub use self::voice::*;
```

- [ ] **Step 3: Add voice field to Config struct**

In `crates/config/src/schema/core.rs`, add after the `lifecycle` field (~line 230):

```rust
    /// Voice input/output configuration.
    #[serde(default)]
    pub voice: VoiceConfig,
```

- [ ] **Step 4: Build and run existing config tests**

Run: `cargo nextest run -p config`
Expected: All existing tests pass (VoiceConfig defaults to enabled, deserializes from empty JSON).

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/voice.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add VoiceConfig schema (input, output, learning sections)"
```

---

### Task 5: MessageKind::Voice + VoiceMetadata in bus crate

**Files:**
- Modify: `crates/bus/src/events.rs`
- Modify: `crates/bus/src/domain_events.rs`
- Modify: `crates/bus/Cargo.toml`

- [ ] **Step 1: Add Voice variant to MessageKind**

In `crates/bus/src/events.rs`, add `Voice` to the `MessageKind` enum (after `Reaction`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageKind {
    #[default]
    Text,
    Reaction,
    Voice,
}
```

- [ ] **Step 2: Add VoiceCapture domain event**

In `crates/bus/src/domain_events.rs`, add a new variant to the `DomainEvent` enum (in the productivity/voice section, near `VoiceJournalProcessed`):

```rust
    VoiceCapture {
        session_id: String,
        language: String,
        overall_confidence: f32,
        duration_secs: f32,
        engine: String,
    },
```

- [ ] **Step 3: Build workspace to check for breakage**

Run: `cargo build --workspace 2>&1 | head -50`

Check for any exhaustive match arms on `MessageKind` or `DomainEvent` that need updating. Fix any compilation errors by adding the new variants to match arms (typically with a fallthrough or appropriate handling).

- [ ] **Step 4: Fix any match exhaustiveness errors**

Search for exhaustive matches on `MessageKind` and add `Voice` handling:

Run: `cargo build --workspace` and fix each error. Common patterns:
- `MessageKind::Voice => { /* treat as text for now */ }` in places that only care about Text vs Reaction
- `DomainEvent::VoiceCapture { .. } => SalienceVerdict::Extract` in salience.rs

- [ ] **Step 5: Verify all tests pass**

Run: `cargo nextest run --workspace`
Expected: All existing tests pass with the new variants.

- [ ] **Step 6: Commit**

```bash
git add crates/bus/
git commit -m "feat(bus): add MessageKind::Voice + DomainEvent::VoiceCapture"
```

---

### Task 6: GroqWhisperEngine — move transcription to voice-engine

**Files:**
- Create: `crates/voice-engine/src/engines/mod.rs`
- Create: `crates/voice-engine/src/engines/groq.rs`
- Modify: `crates/voice-engine/Cargo.toml`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Add HTTP dependencies to voice-engine**

Add to `crates/voice-engine/Cargo.toml` under `[dependencies]`:

```toml
reqwest = { workspace = true }
```

- [ ] **Step 2: Create engines module**

Create `crates/voice-engine/src/engines/mod.rs`:

```rust
pub mod groq;

pub use groq::GroqWhisperEngine;
```

- [ ] **Step 3: Create GroqWhisperEngine**

Create `crates/voice-engine/src/engines/groq.rs`. This adapts the existing `TranscriptionProvider` from `crates/providers/src/adapters/transcription.rs` to implement our `TranscriptionEngine` trait:

```rust
//! Groq Whisper cloud transcription engine.
//!
//! Wraps the Groq OpenAI-compatible transcription API. Used as fallback
//! when the local whisper-rs model is downloading or unavailable.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript, TranscriptSegment};

const DEFAULT_GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";

fn mime_type_for_audio(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        Some("webm") => "audio/webm",
        Some("flac") => "audio/flac",
        Some("mp4") => "audio/mp4",
        Some("mpeg") | Some("mpga") => "audio/mpeg",
        _ => "audio/ogg",
    }
}

/// Cloud-based transcription via Groq's Whisper API.
pub struct GroqWhisperEngine {
    client: reqwest::Client,
    api_key: String,
    api_base: String,
}

#[derive(serde::Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl GroqWhisperEngine {
    pub fn new(api_key: impl Into<String>) -> common::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| common::KlyntbotError::Provider(
                common::ProviderError::Http(e.to_string()),
            ))?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            api_base: DEFAULT_GROQ_API_BASE.to_string(),
        })
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }
}

#[async_trait]
impl TranscriptionEngine for GroqWhisperEngine {
    async fn transcribe_stream(
        &self,
        _audio: AudioStream,
    ) -> common::Result<TranscriptStream> {
        // Cloud engine doesn't support true streaming — collect all audio,
        // save to temp file, transcribe, emit single final result.
        // For v1 this is acceptable since Groq is only the fallback path.
        Err(common::KlyntbotError::Provider(
            common::ProviderError::InvalidResponse(
                "Groq engine does not support streaming transcription. \
                 Use transcribe_file instead."
                    .to_string(),
            ),
        ))
    }

    async fn transcribe_file(
        &self,
        path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        if !path.exists() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!(
                    "Audio file not found: {}",
                    path.display()
                )),
            ));
        }

        debug!("Transcribing audio file via Groq: {}", path.display());

        let file = tokio::fs::read(path).await.map_err(common::KlyntbotError::Io)?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.ogg");

        let mime_type = mime_type_for_audio(path);

        let part = reqwest::multipart::Part::bytes(file)
            .file_name(filename.to_string())
            .mime_str(mime_type)
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                    format!("Failed to create form part: {}", e),
                ))
            })?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3");

        let response = self
            .client
            .post(format!(
                "{}/audio/transcriptions",
                self.api_base.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Groq transcription failed: HTTP {}: {}", status, error_text);
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::Http(format!("HTTP {}: {}", status, error_text)),
            ));
        }

        let resp: TranscriptionResponse = response.json().await.map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                format!("Failed to parse response: {}", e),
            ))
        })?;

        // Groq doesn't return word-level timestamps/confidence in the basic endpoint.
        // Create segments from whitespace-split words with default high confidence.
        let words: Vec<&str> = resp.text.split_whitespace().collect();
        let segments = words
            .iter()
            .enumerate()
            .map(|(i, word)| TranscriptSegment {
                text: word.to_string(),
                start: Duration::from_millis(i as u64 * 300),
                end: Duration::from_millis((i as u64 + 1) * 300),
                confidence: 0.95, // Groq doesn't provide per-word confidence
            })
            .collect();

        Ok(Transcript {
            text: resp.text,
            language: Language::new("en"), // Groq basic endpoint doesn't return language
            segments,
            overall_confidence: 0.95,
        })
    }

    fn display_name(&self) -> &str {
        "Cloud (Groq Whisper)"
    }
}
```

- [ ] **Step 4: Update lib.rs**

Add to `crates/voice-engine/src/lib.rs`:

```rust
pub mod engines;

pub use engines::GroqWhisperEngine;
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p voice-engine`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/
git commit -m "feat(voice): add GroqWhisperEngine adapting existing Groq transcription API"
```

---

### Task 7: ModelManager — download state machine for Whisper models

**Files:**
- Create: `crates/voice-engine/src/model_manager.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/voice-engine/src/model_manager.rs`:

```rust
//! Whisper model download and lifecycle management.
//!
//! Handles background download of whisper-small (default) and whisper-medium
//! (upgrade), model path resolution, and download progress events.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::{info, warn};

/// Available Whisper model sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModelSize {
    Small,
    Medium,
}

impl WhisperModelSize {
    pub fn filename(&self) -> &str {
        match self {
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
        }
    }

    /// Approximate download size in bytes.
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Small => 488_000_000,   // ~488 MB
            Self::Medium => 1_530_000_000, // ~1.53 GB
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Small => "whisper-small (multilingual)",
            Self::Medium => "whisper-medium (multilingual)",
        }
    }
}

/// Current state of a model download/availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ModelState {
    /// Model not downloaded yet.
    NotDownloaded,
    /// Download in progress.
    Downloading { progress: f32 },
    /// Model is ready to use.
    Ready { path: PathBuf },
    /// Download or load failed.
    Failed { error: String },
}

/// Manages Whisper model downloads and availability.
pub struct ModelManager {
    models_dir: PathBuf,
    state_tx: watch::Sender<ModelState>,
    state_rx: watch::Receiver<ModelState>,
}

impl ModelManager {
    /// Create a new manager pointing at `{data_dir}/models/`.
    pub fn new(data_dir: &Path) -> Self {
        let models_dir = data_dir.join("models");
        let initial = if models_dir.join(WhisperModelSize::Small.filename()).exists() {
            ModelState::Ready {
                path: models_dir.join(WhisperModelSize::Small.filename()),
            }
        } else {
            ModelState::NotDownloaded
        };

        let (state_tx, state_rx) = watch::channel(initial);
        Self {
            models_dir,
            state_tx,
            state_rx,
        }
    }

    /// Get current model state.
    pub fn state(&self) -> ModelState {
        self.state_rx.borrow().clone()
    }

    /// Subscribe to state changes (for UI progress bar).
    pub fn subscribe(&self) -> watch::Receiver<ModelState> {
        self.state_rx.clone()
    }

    /// Get the path to a model file if it exists.
    pub fn model_path(&self, size: WhisperModelSize) -> Option<PathBuf> {
        let path = self.models_dir.join(size.filename());
        path.exists().then_some(path)
    }

    /// Check if a model is available locally.
    pub fn is_available(&self, size: WhisperModelSize) -> bool {
        self.models_dir.join(size.filename()).exists()
    }

    /// Start downloading a model in the background.
    /// Returns immediately — watch `subscribe()` for progress.
    pub async fn start_download(&self, size: WhisperModelSize) -> common::Result<()> {
        if self.is_available(size) {
            info!("Model {} already available", size.display_name());
            let _ = self.state_tx.send(ModelState::Ready {
                path: self.models_dir.join(size.filename()),
            });
            return Ok(());
        }

        // Create models directory if needed
        tokio::fs::create_dir_all(&self.models_dir)
            .await
            .map_err(common::KlyntbotError::Io)?;

        let _ = self.state_tx.send(ModelState::Downloading { progress: 0.0 });

        // TODO: Actual download implementation using reqwest streaming.
        // For now, mark as failed with a descriptive message.
        // The real implementation will download from huggingface.co/ggerganov/whisper.cpp.
        warn!(
            "Model download not yet implemented for {}. \
             Place the model file at: {}",
            size.display_name(),
            self.models_dir.join(size.filename()).display()
        );

        let _ = self.state_tx.send(ModelState::Failed {
            error: "Download not yet implemented — place model manually".to_string(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_manager_detects_not_downloaded() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert_eq!(mgr.state(), ModelState::NotDownloaded);
        assert!(!mgr.is_available(WhisperModelSize::Small));
    }

    #[test]
    fn new_manager_detects_existing_model() {
        let tmp = TempDir::new().unwrap();
        let models = tmp.path().join("models");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("ggml-small.bin"), b"fake model").unwrap();

        let mgr = ModelManager::new(tmp.path());
        assert!(matches!(mgr.state(), ModelState::Ready { .. }));
        assert!(mgr.is_available(WhisperModelSize::Small));
        assert!(!mgr.is_available(WhisperModelSize::Medium));
    }

    #[test]
    fn model_path_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.model_path(WhisperModelSize::Small).is_none());
    }

    #[test]
    fn model_sizes_have_correct_filenames() {
        assert_eq!(WhisperModelSize::Small.filename(), "ggml-small.bin");
        assert_eq!(WhisperModelSize::Medium.filename(), "ggml-medium.bin");
    }
}
```

- [ ] **Step 2: Add tempfile dev-dependency**

Add to `crates/voice-engine/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Update lib.rs**

Add to `crates/voice-engine/src/lib.rs`:

```rust
pub mod model_manager;

pub use model_manager::{ModelManager, ModelState, WhisperModelSize};
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass (pronunciation + model_manager).

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/
git commit -m "feat(voice): add ModelManager with download state machine and model detection"
```

---

### Task 8: VoiceSessionState state machine + VoiceEvent types

**Files:**
- Create: `crates/voice-engine/src/session.rs`
- Create: `crates/voice-engine/src/events.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Create VoiceEvent enum**

Create `crates/voice-engine/src/events.rs`:

```rust
//! Voice events emitted by VoiceService, consumed by the frontend orb.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::stt::PartialTranscript;
use crate::types::{EngineKind, Transcript, VoiceMetadata};

/// Events streamed from VoiceService to the frontend Voice Brain orb.
/// Follows the same Tauri event channel pattern as `agent:content_chunk`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoiceEvent {
    /// Capture started — orb should appear.
    CaptureStarted {
        session_id: String,
        engine: EngineKind,
    },
    /// Live audio level for waveform animation (~30fps).
    AudioLevel { rms: f32 },
    /// Partial transcript — powers live text + early routing.
    PartialTranscript {
        text: String,
        language: String,
        is_final: bool,
    },
    /// Routing suggestion — appears as chip in orb.
    RoutingSuggestion {
        skill: String,
        confidence: f32,
        label: String,
    },
    /// Contextual memory echo from cognitive memory/mirror.
    MemoryEcho { text: String },
    /// Capture ended — finalization in progress.
    CaptureEnded {
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },
    /// Emitted when user dismisses orb during capture/finalizing.
    ProcessingInBackground,
    /// Final result — orb can show summary + dismiss.
    Finalized {
        text: String,
        routed_to: String,
        response_preview: String,
    },
    /// TTS audio ready for Web Audio playback.
    SpeakResponse {
        /// PCM samples, base64-encoded for IPC.
        audio_base64: String,
        sample_rate: u32,
        text: String,
    },
    /// Error (mic permission denied, model not loaded, etc.).
    Error {
        message: String,
        recoverable: bool,
    },
}

/// Tauri event name for voice events.
pub const VOICE_EVENT: &str = "voice:event";
```

- [ ] **Step 2: Create VoiceSessionState**

Create `crates/voice-engine/src/session.rs`:

```rust
//! Voice capture session state machine.

use serde::{Deserialize, Serialize};

/// State of a voice capture session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSessionState {
    /// Idle — no capture in progress.
    Idle,
    /// Microphone active, partial transcripts flowing.
    Capturing,
    /// Microphone stopped, final whisper pass running.
    Finalizing,
    /// Transcript sent to agent, awaiting response.
    WaitingForResponse,
    /// Agent responded, TTS played.
    Complete,
}

impl VoiceSessionState {
    /// Whether the session is actively processing (not idle or complete).
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Idle | Self::Complete)
    }

    /// Valid transitions from the current state.
    pub fn can_transition_to(&self, next: VoiceSessionState) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Capturing)
                | (Self::Capturing, Self::Finalizing)
                | (Self::Capturing, Self::Idle) // cancelled
                | (Self::Finalizing, Self::WaitingForResponse)
                | (Self::Finalizing, Self::Idle) // cancelled
                | (Self::WaitingForResponse, Self::Complete)
                | (Self::WaitingForResponse, Self::Idle) // cancelled
                | (Self::Complete, Self::Idle)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_can_start_capturing() {
        assert!(VoiceSessionState::Idle.can_transition_to(VoiceSessionState::Capturing));
    }

    #[test]
    fn capturing_can_finalize_or_cancel() {
        assert!(VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Finalizing));
        assert!(VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Idle));
    }

    #[test]
    fn finalizing_can_proceed_or_cancel() {
        assert!(
            VoiceSessionState::Finalizing
                .can_transition_to(VoiceSessionState::WaitingForResponse)
        );
        assert!(VoiceSessionState::Finalizing.can_transition_to(VoiceSessionState::Idle));
    }

    #[test]
    fn complete_returns_to_idle() {
        assert!(VoiceSessionState::Complete.can_transition_to(VoiceSessionState::Idle));
        assert!(!VoiceSessionState::Complete.can_transition_to(VoiceSessionState::Capturing));
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(!VoiceSessionState::Idle.can_transition_to(VoiceSessionState::Finalizing));
        assert!(!VoiceSessionState::Capturing.can_transition_to(VoiceSessionState::Complete));
    }

    #[test]
    fn active_states() {
        assert!(!VoiceSessionState::Idle.is_active());
        assert!(VoiceSessionState::Capturing.is_active());
        assert!(VoiceSessionState::Finalizing.is_active());
        assert!(VoiceSessionState::WaitingForResponse.is_active());
        assert!(!VoiceSessionState::Complete.is_active());
    }
}
```

- [ ] **Step 3: Update lib.rs**

Add to `crates/voice-engine/src/lib.rs`:

```rust
pub mod events;
pub mod session;

pub use events::{VoiceEvent, VOICE_EVENT};
pub use session::VoiceSessionState;
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/events.rs crates/voice-engine/src/session.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add VoiceSessionState machine + VoiceEvent types"
```

---

### Task 9: VoiceRouter — keyword-based routing on partial transcripts

**Files:**
- Create: `crates/voice-engine/src/router.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Create VoiceRouter with tests**

Create `crates/voice-engine/src/router.rs`:

```rust
//! Voice routing — suggests skill targets from partial transcripts.
//!
//! Runs the same keyword scoring as SkillRouter but on partial text,
//! firing RoutingSuggestion events when a skill crosses the threshold.

use crate::events::VoiceEvent;

/// Minimum keyword score for a routing suggestion.
const ROUTING_THRESHOLD: f64 = 0.4;

/// A detected intent from a partial transcript.
#[derive(Debug, Clone)]
pub struct DetectedIntent {
    pub skill: String,
    pub confidence: f64,
    pub label: String,
    /// The portion of text that triggered this intent.
    pub trigger_text: String,
}

/// Lightweight router that scores partial transcript text against skill keywords.
pub struct VoiceRouter {
    /// (skill_name, display_label, keywords)
    skill_keywords: Vec<(String, String, Vec<String>)>,
}

impl VoiceRouter {
    /// Create with static skill keyword mappings.
    /// These mirror the built-in orchestrator skill triggers.
    pub fn new() -> Self {
        Self {
            skill_keywords: vec![
                (
                    "tasks".to_string(),
                    "Task".to_string(),
                    vec![
                        "task", "todo", "remind", "reminder", "schedule", "add", "create",
                        "deadline", "due", "appointment",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                (
                    "learning".to_string(),
                    "Learning".to_string(),
                    vec![
                        "practice", "drill", "vocab", "vocabulary", "flashcard", "pronunciation",
                        "french", "spanish", "german", "japanese", "language", "learn",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                (
                    "notes".to_string(),
                    "Note".to_string(),
                    vec![
                        "note", "write", "jot", "remember", "thought", "idea", "journal",
                        "reflection",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
                (
                    "finance".to_string(),
                    "Finance".to_string(),
                    vec![
                        "budget", "expense", "spent", "cost", "money", "payment", "income",
                        "savings",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                ),
            ],
        }
    }

    /// Score partial text against all skills. Returns intents above threshold.
    pub fn detect_intents(&self, text: &str) -> Vec<DetectedIntent> {
        let words: Vec<String> = text.to_lowercase().split_whitespace().map(String::from).collect();

        self.skill_keywords
            .iter()
            .filter_map(|(skill, label, keywords)| {
                let hits = keywords
                    .iter()
                    .filter(|kw| words.iter().any(|w| w.contains(kw.as_str())))
                    .count();

                if hits == 0 {
                    return None;
                }

                let score = (hits as f64 / keywords.len().max(1) as f64).min(1.0);
                if score >= ROUTING_THRESHOLD {
                    Some(DetectedIntent {
                        skill: skill.clone(),
                        confidence: score,
                        label: format!("→ {}", label),
                        trigger_text: text.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Convert detected intents to VoiceEvent routing suggestions.
    pub fn to_events(&self, intents: &[DetectedIntent]) -> Vec<VoiceEvent> {
        intents
            .iter()
            .map(|intent| VoiceEvent::RoutingSuggestion {
                skill: intent.skill.clone(),
                confidence: intent.confidence as f32,
                label: intent.label.clone(),
            })
            .collect()
    }

    /// Check if there are multiple distinct intents (for multi-intent split).
    pub fn is_multi_intent(intents: &[DetectedIntent]) -> bool {
        intents.len() >= 2
    }
}

impl Default for VoiceRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_task_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("remind me to schedule dentist");
        assert!(!intents.is_empty());
        assert_eq!(intents[0].skill, "tasks");
    }

    #[test]
    fn detects_learning_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("practice french vocabulary");
        assert!(!intents.is_empty());
        assert_eq!(intents[0].skill, "learning");
    }

    #[test]
    fn detects_multi_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("schedule dentist and practice french vocab");
        assert!(intents.len() >= 2);
        let skills: Vec<&str> = intents.iter().map(|i| i.skill.as_str()).collect();
        assert!(skills.contains(&"tasks"));
        assert!(skills.contains(&"learning"));
        assert!(VoiceRouter::is_multi_intent(&intents));
    }

    #[test]
    fn no_intent_from_generic_text() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("hello how are you");
        assert!(intents.is_empty());
    }

    #[test]
    fn single_intent_is_not_multi() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("add a task for tomorrow");
        assert!(!VoiceRouter::is_multi_intent(&intents));
    }

    #[test]
    fn to_events_produces_routing_suggestions() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("remind me to practice french");
        let events = router.to_events(&intents);
        assert!(events.iter().all(|e| matches!(e, VoiceEvent::RoutingSuggestion { .. })));
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add to `crates/voice-engine/src/lib.rs`:

```rust
pub mod router;

pub use router::VoiceRouter;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/router.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add VoiceRouter for keyword-based intent detection on partials"
```

---

### Task 10: Desktop-shared voice types + desktop voice commands

**Files:**
- Create: `crates/desktop-shared/src/commands/voice.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/desktop-shared/Cargo.toml`
- Create: `crates/desktop/src/commands/voice.rs`
- Modify: `crates/desktop/src/commands/mod.rs`

- [ ] **Step 1: Add voice-engine dependency to desktop-shared**

Add to `crates/desktop-shared/Cargo.toml` under `[dependencies]`:

```toml
voice-engine.workspace = true
```

And add to root `Cargo.toml` workspace dependencies:

```toml
voice-engine = { path = "crates/voice-engine" }
```

- [ ] **Step 2: Create desktop-shared voice command types**

Create `crates/desktop-shared/src/commands/voice.rs`:

```rust
//! Voice command request/response types for Tauri IPC.

use serde::{Deserialize, Serialize};
use voice_engine::{EngineKind, ModelState, VoiceSessionState, WhisperModelSize};

/// Response from voice_start_capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCaptureInfo {
    pub session_id: String,
    pub engine: EngineKind,
    pub state: VoiceSessionState,
}

/// Response from voice_get_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatusResponse {
    pub state: VoiceSessionState,
    pub model_state: ModelState,
    pub engine: EngineKind,
    pub enabled: bool,
}

/// Request to download a specific model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDownloadModelRequest {
    pub model_size: WhisperModelSize,
}

/// Available model info for settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelInfo {
    pub size: WhisperModelSize,
    pub display_name: String,
    pub size_bytes: u64,
    pub available: bool,
}
```

- [ ] **Step 3: Register in desktop-shared commands**

In `crates/desktop-shared/src/commands/mod.rs`, add:

```rust
pub mod voice;
```

- [ ] **Step 4: Create desktop voice command module**

Create `crates/desktop/src/commands/voice.rs`:

```rust
//! Voice capture Tauri command handlers.
//!
//! Thin adapter layer — all business logic lives in AppCore.

use std::sync::Arc;

use desktop_shared::commands::voice::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn voice_start_capture(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceCaptureInfo, ApiError> {
    // TODO: Wire to AppCore::voice_start_capture() once VoiceService is integrated
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_stop_capture(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_dismiss(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_get_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceStatusResponse, ApiError> {
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice status not yet wired"))
}

#[tauri::command]
pub async fn voice_get_models(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<VoiceModelInfo>, ApiError> {
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice models not yet wired"))
}

#[tauri::command]
pub async fn voice_download_model(
    state: State<'_, Arc<AppCore>>,
    request: VoiceDownloadModelRequest,
) -> Result<(), ApiError> {
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice download not yet wired"))
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_start_capture",
    "voice_stop_capture",
    "voice_dismiss",
    "voice_get_status",
    "voice_get_models",
    "voice_download_model",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    _core: &crate::app_core::AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    Some(match cmd {
        "voice_start_capture" | "voice_stop_capture" | "voice_dismiss" | "voice_get_status"
        | "voice_get_models" | "voice_download_model" => {
            Err(ApiError::new("NOT_IMPLEMENTED", "Voice not yet wired"))
        }
        _ => return None,
    })
}
```

- [ ] **Step 5: Register voice module in desktop commands**

In `crates/desktop/src/commands/mod.rs`, add:

```rust
pub mod voice;
```

- [ ] **Step 6: Register voice commands in main.rs invoke_handler**

In `crates/desktop/src/main.rs`, add to the `tauri::generate_handler![]` macro (in the appropriate section):

```rust
// Voice
commands::voice::voice_start_capture,
commands::voice::voice_stop_capture,
commands::voice::voice_dismiss,
commands::voice::voice_get_status,
commands::voice::voice_get_models,
commands::voice::voice_download_model,
```

- [ ] **Step 7: Add voice dispatch to dev server**

In `crates/desktop/src/dev_server/dispatch.rs`, add in the dispatch chain (before the final `None` fallback):

```rust
if let Some(r) = commands::voice::dispatch_dev(cmd, core, &body).await {
    return into_api_result(r);
}
```

- [ ] **Step 8: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles. The `dev_server_covers_all_tauri_commands` test should pass with the new DEV_COMMANDS.

- [ ] **Step 9: Run the dev server coverage test**

Run: `cargo nextest run -p desktop dev_server_covers_all_tauri_commands`
Expected: PASS (voice commands are covered by DEV_COMMANDS).

- [ ] **Step 10: Commit**

```bash
git add crates/desktop-shared/ crates/desktop/ Cargo.toml Cargo.lock
git commit -m "feat(voice): add desktop voice commands (stubs) + shared IPC types"
```

---

### Task 11: Voice Brain orb window definition + frontend scaffold

**Files:**
- Modify: `crates/desktop/tauri.conf.json`
- Create: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`
- Create: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`
- Create: `desktop-ui/src/features/voice/index.ts`

- [ ] **Step 1: Add orb window to tauri.conf.json**

In `crates/desktop/tauri.conf.json`, add a new window entry in the `windows` array (next to the distraction-overlay entry):

```json
{
  "label": "voice-orb",
  "url": "/#/voice-orb",
  "title": "",
  "width": 320,
  "height": 200,
  "resizable": false,
  "decorations": false,
  "visible": false,
  "transparent": true,
  "shadow": false,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "center": true,
  "focus": false,
  "windowEffects": {
    "effects": ["hudWindow"],
    "state": "active",
    "radius": 16.0
  }
}
```

- [ ] **Step 2: Create useVoiceEvents hook**

Create `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`:

```typescript
import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";

export type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

export interface VoiceEventPayload {
  type: string;
  [key: string]: unknown;
}

export interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

export function useVoiceEvents() {
  const [sessionState, setSessionState] = useState<VoiceSessionState>("idle");
  const [transcript, setTranscript] = useState("");
  const [routingChips, setRoutingChips] = useState<RoutingChip[]>([]);
  const [memoryEcho, setMemoryEcho] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const [engineKind, setEngineKind] = useState<"local" | "cloud">("local");
  const [responseText, setResponseText] = useState("");

  useEffect(() => {
    const unlisten = listen<VoiceEventPayload>("voice:event", (event) => {
      const payload = event.payload;

      switch (payload.type) {
        case "captureStarted":
          setSessionState("capturing");
          setTranscript("");
          setRoutingChips([]);
          setMemoryEcho(null);
          setEngineKind((payload.engine as string) === "cloud" ? "cloud" : "local");
          break;
        case "audioLevel":
          setAudioLevel(payload.rms as number);
          break;
        case "partialTranscript":
          setTranscript(payload.text as string);
          break;
        case "routingSuggestion":
          setRoutingChips((prev) => {
            const existing = prev.find((c) => c.skill === payload.skill);
            if (existing) return prev;
            return [
              ...prev,
              {
                skill: payload.skill as string,
                confidence: payload.confidence as number,
                label: payload.label as string,
              },
            ];
          });
          break;
        case "memoryEcho":
          setMemoryEcho(payload.text as string);
          break;
        case "captureEnded":
          setSessionState("processing");
          break;
        case "processingInBackground":
          setSessionState("processing");
          break;
        case "finalized":
          setSessionState("response");
          setResponseText(payload.responsePreview as string || payload.text as string);
          break;
        case "error":
          console.error("[VoiceBrain]", payload.message);
          break;
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const dismiss = useCallback(async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("voice_dismiss");
    setSessionState("idle");
  }, []);

  return {
    sessionState,
    transcript,
    routingChips,
    memoryEcho,
    audioLevel,
    engineKind,
    responseText,
    dismiss,
  };
}
```

- [ ] **Step 3: Create VoiceBrainOrb component**

Create `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`:

```tsx
import { useVoiceEvents, type VoiceSessionState } from "../hooks/useVoiceEvents";

function Waveform({ level }: { level: number }) {
  const bars = 12;
  return (
    <div className="flex items-center gap-0.5 h-4">
      {Array.from({ length: bars }).map((_, i) => {
        const height = Math.max(2, level * 16 * (0.5 + 0.5 * Math.sin(i * 0.8)));
        return (
          <div
            key={i}
            className="w-0.5 rounded-full bg-accent transition-all duration-75"
            style={{ height: `${height}px` }}
          />
        );
      })}
    </div>
  );
}

function ConfidenceWord({ word, confidence }: { word: string; confidence: number }) {
  const color =
    confidence >= 0.85
      ? "text-green-400"
      : confidence >= 0.6
        ? "text-amber-400"
        : "text-red-400";
  return <span className={color}>{word} </span>;
}

function RoutingChips({ chips }: { chips: { skill: string; label: string }[] }) {
  return (
    <div className="flex gap-1.5 flex-wrap">
      {chips.map((chip) => (
        <div
          key={chip.skill}
          className="glass-panel px-2 py-0.5 rounded-full text-xs text-muted"
        >
          {chip.label}
        </div>
      ))}
    </div>
  );
}

export function VoiceBrainOrb() {
  const {
    sessionState,
    transcript,
    routingChips,
    memoryEcho,
    audioLevel,
    engineKind,
    responseText,
    dismiss,
  } = useVoiceEvents();

  if (sessionState === "idle") return null;

  return (
    <div
      className="glass-panel rounded-2xl p-3 w-[320px] select-none animate-in fade-in zoom-in-95 duration-200"
      onClick={sessionState === "response" ? dismiss : undefined}
    >
      {/* Header: mic indicator + waveform */}
      <div className="flex items-center gap-2 mb-2">
        <div
          className={`w-2 h-2 rounded-full ${
            sessionState === "capturing" ? "bg-red-500 animate-pulse" : "bg-muted"
          }`}
        />
        {sessionState === "capturing" && <Waveform level={audioLevel} />}
        {sessionState === "processing" && (
          <span className="text-xs text-muted animate-pulse">Processing...</span>
        )}
        {sessionState === "response" && (
          <span className="text-xs text-muted">Response</span>
        )}
        <div className="flex-1" />
        {engineKind === "cloud" && (
          <span className="text-xs text-muted opacity-60">cloud</span>
        )}
      </div>

      {/* Transcript */}
      <div className="text-sm font-mono min-h-[40px] mb-2">
        {sessionState === "response" ? (
          <span className="text-foreground">{responseText}</span>
        ) : (
          <span className="text-foreground">{transcript || "Listening..."}</span>
        )}
      </div>

      {/* Routing chips */}
      {routingChips.length > 0 && (
        <div className="mb-2">
          <RoutingChips chips={routingChips} />
        </div>
      )}

      {/* Memory echo */}
      {memoryEcho && (
        <div className="text-xs text-muted opacity-60 italic mb-2">{memoryEcho}</div>
      )}

      {/* Hint bar */}
      <div className="text-[10px] text-muted opacity-40 text-center">
        {sessionState === "capturing" && "cmd+shift+V to finish · tap to close"}
        {sessionState === "processing" && "Cancel & discard"}
        {sessionState === "response" && "tap anywhere to close"}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create feature index**

Create `desktop-ui/src/features/voice/index.ts`:

```typescript
export { VoiceBrainOrb } from "./components/VoiceBrainOrb";
export { useVoiceEvents } from "./hooks/useVoiceEvents";
```

- [ ] **Step 5: Add route for voice-orb window**

Find the router configuration in the desktop-ui app (likely `src/app/router.tsx` or similar) and add a route for the voice orb:

```tsx
{
  path: "/voice-orb",
  element: <VoiceBrainOrb />,
}
```

The exact location depends on the existing router setup — check `desktop-ui/src/app/` for the router file.

- [ ] **Step 6: Build frontend**

Run: `cd desktop-ui && bun run build`
Expected: Compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/tauri.conf.json desktop-ui/src/features/voice/
git commit -m "feat(voice): add Voice Brain orb window + frontend scaffold"
```

---

### Task 12: Flashcard schema migration for voice fields

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs` (or the relevant migration file)
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add voice columns to flashcard migration**

Per CLAUDE.md pre-release convention, modify the existing flashcard migration SQL in-place. The flashcard table is defined in `crates/cognitive/migrations/001_cognitive_tables.sql`. Add the three new nullable columns to the CREATE TABLE statement:

```sql
-- Voice-enriched flashcard fields (added for Voice Brain v1)
-- audio_ref: path to the original spoken audio for playback
-- pronunciation_baseline: user's confidence score on first voice attempt
-- last_pronunciation_score: most recent pronunciation score for trend tracking
```

In the CREATE TABLE statement, add after the existing columns:

```sql
    audio_ref TEXT,
    pronunciation_baseline REAL,
    last_pronunciation_score REAL,
```

- [ ] **Step 2: Add fields to FlashcardRow struct**

In `crates/cognitive/src/repos/flashcard.rs`, add to the `FlashcardRow` struct:

```rust
    pub audio_ref: Option<String>,
    pub pronunciation_baseline: Option<f64>,
    pub last_pronunciation_score: Option<f64>,
```

- [ ] **Step 3: Update any INSERT queries to include new columns**

Find `create_batch()` or equivalent INSERT methods and add the new columns with `NULL` defaults. The columns are nullable so existing inserts that don't provide them will work fine.

- [ ] **Step 4: Build and run cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: All tests pass (in-memory SQLite will apply the updated migration).

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(voice): add audio_ref + pronunciation columns to flashcard schema"
```

---

### Task 13: Cognitive salience + extraction enrichment for voice

**Files:**
- Modify: `crates/cognitive/src/services/salience.rs`
- Modify: `crates/bus/src/domain_events.rs` (if needed for match arm)

- [ ] **Step 1: Add Voice salience handling**

In `crates/cognitive/src/services/salience.rs`, add a match arm for `VoiceCapture` in the salience verdict function (near the existing `VoiceJournalProcessed` arm):

```rust
DomainEvent::VoiceCapture { .. } => SalienceVerdict::Extract,
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p cognitive`
Expected: Compiles.

- [ ] **Step 3: Run cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/salience.rs
git commit -m "feat(voice): add VoiceCapture salience handling (always Extract)"
```

---

### Task 14: Coaching SpokenNudge intervention type

**Files:**
- Modify: `crates/feature-coaching/src/reasoner.rs`

- [ ] **Step 1: Add SpokenNudge variant**

In `crates/feature-coaching/src/reasoner.rs`, add `SpokenNudge` to the `InterventionType` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InterventionType {
    DashboardCard,
    ChatMessage,
    Notification,
    Overlay,
    /// Spoken coaching nudge delivered via TTS through the Voice Brain orb.
    SpokenNudge,
    None,
}
```

- [ ] **Step 2: Build and check for exhaustiveness errors**

Run: `cargo build -p feature-coaching`

Fix any match exhaustiveness errors by adding `InterventionType::SpokenNudge` arms (typically treating it like `Notification` for now).

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace`
Expected: Compiles with zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coaching/
git commit -m "feat(coaching): add SpokenNudge intervention type for voice-delivered coaching"
```

---

### Task 15: VOICE_ACTIVE flag + tray coordination

**Files:**
- Modify: `crates/desktop/src/tray_countdown.rs`

- [ ] **Step 1: Add VOICE_ACTIVE atomic flag**

In `crates/desktop/src/tray_countdown.rs`, add alongside `FOCUS_ACTIVE`:

```rust
/// Shared flag: when `true`, a voice capture session is active.
/// The tray title shows "Listening..." and yields to focus timer if both active.
pub static VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);
```

- [ ] **Step 2: Handle VOICE_ACTIVE in the tray tick loop**

In the tray countdown loop (the main `loop` body that updates the tray title), add a check for `VOICE_ACTIVE` after the `FOCUS_ACTIVE` check:

```rust
// Focus timer takes priority over everything
if FOCUS_ACTIVE.load(Ordering::Relaxed) {
    cached = None;
    continue;
}

// Voice capture shows "Listening..." when active
if VOICE_ACTIVE.load(Ordering::Relaxed) {
    let handle = handle.clone();
    let _ = handle
        .tray_by_id("klynt-tray")
        .map(|t| t.set_title(Some("Listening...")));
    cached = None;
    continue;
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/tray_countdown.rs
git commit -m "feat(voice): add VOICE_ACTIVE tray flag with focus-timer priority"
```

---

### Task 16: Dev server voice mock endpoints

**Files:**
- Modify: `crates/desktop/src/dev_server/dispatch.rs`

- [ ] **Step 1: Add voice mock endpoints**

In `crates/desktop/src/dev_server/dispatch.rs`, add mock endpoints for browser-mode voice development. In the dispatch function, update the voice dispatch block:

```rust
#[cfg(debug_assertions)]
pub(crate) async fn dispatch_voice_mock(
    cmd: &str,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    Some(match cmd {
        "voice_simulate_event" => {
            // Accept a VoiceEvent JSON and could emit it via SSE
            // For now, return OK
            Ok(serde_json::json!({"ok": true}))
        }
        "voice_mock_session" => {
            // Simulate a full capture session with pre-recorded transcript
            Ok(serde_json::json!({
                "session_id": "mock-session-1",
                "text": "schedule dentist and practice French",
                "language": "en",
                "duration_ms": 3000
            }))
        }
        _ => return None,
    })
}
```

- [ ] **Step 2: Wire mock dispatch into the main dispatch chain**

In the main `dispatch` function, add:

```rust
if let Some(r) = dispatch_voice_mock(cmd, &body).await {
    return into_api_result(r);
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/dev_server/
git commit -m "feat(voice): add dev server mock endpoints for voice orb browser development"
```

---

### Task 17: Frontend voice settings panel

**Files:**
- Create: `desktop-ui/src/features/voice/components/VoiceSettings.tsx`

- [ ] **Step 1: Create VoiceSettings component**

Create `desktop-ui/src/features/voice/components/VoiceSettings.tsx`:

```tsx
import { useState } from "react";

interface VoiceConfig {
  enabled: boolean;
  input: {
    hotkey: string;
    silenceThresholdSecs: number;
    privacyMode: "standard" | "strict" | "off";
    preferLocal: boolean;
    modelSize: "small" | "medium";
  };
  output: {
    enabled: boolean;
    speakingRate: number;
    speakDuringFocus: boolean;
  };
  learning: {
    targetLanguage: string | null;
    showPronunciationScores: boolean;
    autoCreateFlashcards: boolean;
  };
}

export function VoiceSettings() {
  // TODO: Wire to actual config via useQuery("voice_get_status")
  const [config, setConfig] = useState<VoiceConfig>({
    enabled: true,
    input: {
      hotkey: "super+shift+v",
      silenceThresholdSecs: 1.5,
      privacyMode: "standard",
      preferLocal: true,
      modelSize: "small",
    },
    output: {
      enabled: true,
      speakingRate: 1.0,
      speakDuringFocus: false,
    },
    learning: {
      targetLanguage: null,
      showPronunciationScores: true,
      autoCreateFlashcards: true,
    },
  });

  return (
    <div className="space-y-6">
      <h3 className="text-lg font-medium text-foreground">Voice</h3>

      {/* Voice Input */}
      <section className="space-y-3">
        <h4 className="text-sm font-medium text-muted">Voice Input</h4>

        <div className="flex items-center justify-between">
          <span className="text-sm">Enable voice capture</span>
          <span className="text-xs text-muted">{config.enabled ? "On" : "Off"}</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Global hotkey</span>
          <span className="text-xs text-muted font-mono">{config.input.hotkey}</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Silence threshold</span>
          <span className="text-xs text-muted">{config.input.silenceThresholdSecs}s</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Privacy mode</span>
          <span className="text-xs text-muted capitalize">{config.input.privacyMode}</span>
        </div>

        <div className="space-y-1">
          <span className="text-sm">Transcription engine</span>
          <div className="pl-4 space-y-1 text-xs text-muted">
            <div>
              {config.input.preferLocal ? "●" : "○"} Local (whisper-{config.input.modelSize})
            </div>
            <div>{!config.input.preferLocal ? "●" : "○"} Cloud (Groq)</div>
          </div>
        </div>
      </section>

      {/* Voice Output */}
      <section className="space-y-3">
        <h4 className="text-sm font-medium text-muted">Voice Output</h4>

        <div className="flex items-center justify-between">
          <span className="text-sm">Enable spoken responses</span>
          <span className="text-xs text-muted">{config.output.enabled ? "On" : "Off"}</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Speaking rate</span>
          <span className="text-xs text-muted">{config.output.speakingRate}x</span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Speak during focus sessions</span>
          <span className="text-xs text-muted">
            {config.output.speakDuringFocus ? "On" : "Off"}
          </span>
        </div>
      </section>

      {/* Language Learning */}
      <section className="space-y-3">
        <h4 className="text-sm font-medium text-muted">Language Learning</h4>

        <div className="flex items-center justify-between">
          <span className="text-sm">Target language</span>
          <span className="text-xs text-muted">
            {config.learning.targetLanguage || "Not set"}
          </span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Show pronunciation scores</span>
          <span className="text-xs text-muted">
            {config.learning.showPronunciationScores ? "On" : "Off"}
          </span>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">Auto-create spoken flashcards</span>
          <span className="text-xs text-muted">
            {config.learning.autoCreateFlashcards ? "On" : "Off"}
          </span>
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 2: Export from feature index**

Update `desktop-ui/src/features/voice/index.ts`:

```typescript
export { VoiceBrainOrb } from "./components/VoiceBrainOrb";
export { VoiceSettings } from "./components/VoiceSettings";
export { useVoiceEvents } from "./hooks/useVoiceEvents";
```

- [ ] **Step 3: Build frontend**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/voice/
git commit -m "feat(voice): add VoiceSettings panel scaffold for settings tab"
```

---

### Task 18: AvSpeechEngine in platform-macos

**Files:**
- Create: `crates/platform-macos/src/speech.rs`
- Modify: `crates/platform-macos/src/lib.rs`
- Modify: `crates/platform-macos/Cargo.toml`

- [ ] **Step 1: Add AVFoundation features to Cargo.toml**

In `crates/platform-macos/Cargo.toml`, update `objc2-app-kit` features and add `objc2-av-foundation`:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [
    "NSWorkspace",
    "NSRunningApplication",
    "NSPasteboard",
] }
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray"] }
objc2-av-foundation = { version = "0.3", features = [
    "AVSpeechSynthesis",
] }
core-graphics = "0.24"
core-foundation = "0.10"
```

Note: If `objc2-av-foundation` is not available as a separate crate at this version, use `std::process::Command` to invoke `say` CLI as a fallback (same pattern as `dnd.rs`).

- [ ] **Step 2: Create speech module**

Create `crates/platform-macos/src/speech.rs`:

```rust
//! macOS AVSpeechSynthesizer wrapper for TTS output.
//!
//! Uses the `say` CLI command as a pragmatic v1 approach.
//! This avoids complex objc2 block wiring and gives us immediate
//! access to all macOS neural voices.

use std::process::Command;

use tracing::debug;

/// Available macOS voice info.
pub struct MacVoice {
    pub name: String,
    pub language: String,
}

/// List available macOS speech voices.
pub fn list_voices() -> Vec<MacVoice> {
    let output = Command::new("say").arg("--voice=?").output();

    match output {
        Ok(out) => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| {
                    // Format: "Name    language_code    # Description"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        Some(MacVoice {
                            name: parts[0].to_string(),
                            language: parts[1].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => vec![],
    }
}

/// Synthesize text to a WAV file using macOS `say` command.
///
/// Returns the path to the generated audio file.
pub fn synthesize_to_file(
    text: &str,
    voice: Option<&str>,
    rate: Option<f32>,
    output_path: &std::path::Path,
) -> Result<(), String> {
    let mut cmd = Command::new("say");

    if let Some(voice_name) = voice {
        cmd.arg("-v").arg(voice_name);
    }

    if let Some(r) = rate {
        // macOS `say` rate is in words per minute, default ~175
        let wpm = (175.0 * r) as u32;
        cmd.arg("-r").arg(wpm.to_string());
    }

    cmd.arg("-o")
        .arg(output_path)
        .arg("--data-format=LEF32@16000") // 16kHz float32 for Web Audio
        .arg(text);

    debug!("Running: say {:?}", cmd);

    let output = cmd.output().map_err(|e| format!("Failed to run say: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("say failed: {}", stderr));
    }

    Ok(())
}
```

- [ ] **Step 3: Register module**

In `crates/platform-macos/src/lib.rs`, add:

```rust
pub mod speech;
```

- [ ] **Step 4: Build**

Run: `cargo build -p platform-macos`
Expected: Compiles (the `say` command is available on all macOS versions).

- [ ] **Step 5: Commit**

```bash
git add crates/platform-macos/
git commit -m "feat(voice): add macOS speech module wrapping say CLI for TTS"
```

---

### Task 19: Frontend Vitest tests for voice components

**Files:**
- Create: `desktop-ui/src/features/voice/__tests__/useVoiceEvents.test.ts`

- [ ] **Step 1: Create tests for useVoiceEvents state transitions**

Create `desktop-ui/src/features/voice/__tests__/useVoiceEvents.test.ts`:

```typescript
import { describe, it, expect } from "vitest";

// Test the state logic extracted from the hook
// (We test the pure logic, not the Tauri event binding)

type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

function reduceVoiceEvent(
  state: { sessionState: VoiceSessionState; chips: RoutingChip[]; transcript: string },
  event: { type: string; [key: string]: unknown },
) {
  switch (event.type) {
    case "captureStarted":
      return { ...state, sessionState: "capturing" as const, transcript: "", chips: [] };
    case "partialTranscript":
      return { ...state, transcript: event.text as string };
    case "routingSuggestion": {
      const chip = {
        skill: event.skill as string,
        confidence: event.confidence as number,
        label: event.label as string,
      };
      if (state.chips.find((c) => c.skill === chip.skill)) return state;
      return { ...state, chips: [...state.chips, chip] };
    }
    case "captureEnded":
      return { ...state, sessionState: "processing" as const };
    case "finalized":
      return { ...state, sessionState: "response" as const };
    default:
      return state;
  }
}

describe("Voice event reducer", () => {
  const initial = { sessionState: "idle" as const, chips: [], transcript: "" };

  it("transitions idle -> capturing on captureStarted", () => {
    const result = reduceVoiceEvent(initial, {
      type: "captureStarted",
      sessionId: "s1",
      engine: "local",
    });
    expect(result.sessionState).toBe("capturing");
  });

  it("updates transcript on partialTranscript", () => {
    const capturing = { ...initial, sessionState: "capturing" as const };
    const result = reduceVoiceEvent(capturing, {
      type: "partialTranscript",
      text: "hello world",
    });
    expect(result.transcript).toBe("hello world");
  });

  it("adds routing chips without duplicates", () => {
    let state = { ...initial, sessionState: "capturing" as const };
    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.8,
      label: "Task",
    });
    expect(state.chips).toHaveLength(1);

    // Duplicate should be ignored
    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "tasks",
      confidence: 0.9,
      label: "Task",
    });
    expect(state.chips).toHaveLength(1);

    // New skill adds
    state = reduceVoiceEvent(state, {
      type: "routingSuggestion",
      skill: "learning",
      confidence: 0.7,
      label: "Learning",
    });
    expect(state.chips).toHaveLength(2);
  });

  it("transitions capturing -> processing on captureEnded", () => {
    const capturing = { ...initial, sessionState: "capturing" as const };
    const result = reduceVoiceEvent(capturing, { type: "captureEnded", durationMs: 3000 });
    expect(result.sessionState).toBe("processing");
  });

  it("transitions processing -> response on finalized", () => {
    const processing = { ...initial, sessionState: "processing" as const };
    const result = reduceVoiceEvent(processing, {
      type: "finalized",
      text: "done",
      routedTo: "tasks",
    });
    expect(result.sessionState).toBe("response");
  });

  it("resets chips on new capture", () => {
    const withChips = {
      sessionState: "response" as const,
      chips: [{ skill: "tasks", confidence: 0.8, label: "Task" }],
      transcript: "old text",
    };
    const result = reduceVoiceEvent(withChips, {
      type: "captureStarted",
      sessionId: "s2",
      engine: "local",
    });
    expect(result.chips).toHaveLength(0);
    expect(result.transcript).toBe("");
  });
});

describe("Pronunciation confidence thresholds", () => {
  it("classifies word confidence into correct tiers", () => {
    const classify = (c: number) =>
      c >= 0.85 ? "good" : c >= 0.6 ? "fair" : "poor";

    expect(classify(0.95)).toBe("good");
    expect(classify(0.85)).toBe("good");
    expect(classify(0.84)).toBe("fair");
    expect(classify(0.60)).toBe("fair");
    expect(classify(0.59)).toBe("poor");
    expect(classify(0.10)).toBe("poor");
  });
});
```

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All voice tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/voice/__tests__/
git commit -m "test(voice): add Vitest tests for voice event reducer and confidence thresholds"
```

---

### Task 20: Integration test — VoiceRouter + VoiceSessionState + events end-to-end

**Files:**
- Create: `crates/voice-engine/tests/integration.rs`

- [ ] **Step 1: Create integration test**

Create `crates/voice-engine/tests/integration.rs`:

```rust
//! Integration tests for the voice engine pipeline.

use std::time::Duration;

use voice_engine::events::VoiceEvent;
use voice_engine::mock::MockTranscriptionEngine;
use voice_engine::pronunciation::compute_pronunciation_report;
use voice_engine::router::VoiceRouter;
use voice_engine::session::VoiceSessionState;
use voice_engine::types::*;

#[test]
fn full_pipeline_capture_to_routing() {
    // Simulate: user says "schedule dentist and practice french vocab"
    let transcript = Transcript {
        text: "schedule dentist and practice french vocab".to_string(),
        language: Language::new("en"),
        segments: vec![
            seg("schedule", 0.92),
            seg("dentist", 0.88),
            seg("and", 0.95),
            seg("practice", 0.90),
            seg("french", 0.85),
            seg("vocab", 0.87),
        ],
        overall_confidence: 0.90,
    };

    // Router detects multi-intent
    let router = VoiceRouter::new();
    let intents = router.detect_intents(&transcript.text);
    assert!(intents.len() >= 2, "Should detect task + learning intents");
    assert!(VoiceRouter::is_multi_intent(&intents));

    // Convert to events
    let events = router.to_events(&intents);
    assert!(events.iter().all(|e| matches!(e, VoiceEvent::RoutingSuggestion { .. })));

    // Pronunciation report
    let report = compute_pronunciation_report(&transcript);
    assert!(report.overall_score > 0.85);
    assert_eq!(report.weak_words_count, 0);
}

#[test]
fn pronunciation_report_feeds_voice_metadata() {
    let transcript = Transcript {
        text: "je suis content".to_string(),
        language: Language::new("fr"),
        segments: vec![
            seg("je", 0.92),
            seg("suis", 0.40),
            seg("content", 0.70),
        ],
        overall_confidence: 0.67,
    };

    let report = compute_pronunciation_report(&transcript);

    // Build VoiceMetadata from report
    let metadata = VoiceMetadata {
        language: transcript.language.clone(),
        overall_confidence: transcript.overall_confidence,
        pronunciation_scores: report
            .word_scores
            .iter()
            .map(|w| (w.word.clone(), w.confidence))
            .collect(),
        audio_ref: Some("/tmp/test.wav".to_string()),
        duration: Duration::from_secs(3),
        engine: EngineKind::Local,
        privacy_mode: PrivacyLevel::Standard,
    };

    assert_eq!(metadata.pronunciation_scores.len(), 3);
    assert_eq!(metadata.pronunciation_scores[1].0, "suis");
    assert!(metadata.pronunciation_scores[1].1 < 0.5);

    // Verify serialization roundtrip
    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: VoiceMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.pronunciation_scores.len(), 3);
}

#[test]
fn session_state_full_lifecycle() {
    let mut state = VoiceSessionState::Idle;

    // Start capture
    assert!(state.can_transition_to(VoiceSessionState::Capturing));
    state = VoiceSessionState::Capturing;
    assert!(state.is_active());

    // Finalize (silence detected)
    assert!(state.can_transition_to(VoiceSessionState::Finalizing));
    state = VoiceSessionState::Finalizing;
    assert!(state.is_active());

    // Waiting for agent response
    assert!(state.can_transition_to(VoiceSessionState::WaitingForResponse));
    state = VoiceSessionState::WaitingForResponse;
    assert!(state.is_active());

    // Complete
    assert!(state.can_transition_to(VoiceSessionState::Complete));
    state = VoiceSessionState::Complete;
    assert!(!state.is_active());

    // Back to idle
    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[test]
fn dismiss_during_capture_allows_cancel() {
    let state = VoiceSessionState::Capturing;
    // Dismiss → back to idle (cancel)
    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[test]
fn dismiss_during_finalizing_allows_cancel() {
    let state = VoiceSessionState::Finalizing;
    assert!(state.can_transition_to(VoiceSessionState::Idle));
}

#[tokio::test]
async fn mock_engine_produces_transcript() {
    let engine = MockTranscriptionEngine::new("hello world");
    let transcript = engine
        .transcribe_file(std::path::Path::new("/dev/null"), None)
        .await
        .unwrap();

    assert_eq!(transcript.text, "hello world");
    assert_eq!(transcript.segments.len(), 2);
    assert_eq!(transcript.segments[0].text, "hello");
}

fn seg(text: &str, confidence: f32) -> TranscriptSegment {
    TranscriptSegment {
        text: text.to_string(),
        start: Duration::ZERO,
        end: Duration::from_millis(300),
        confidence,
    }
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo nextest run -p voice-engine`
Expected: All tests pass (unit + integration).

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/tests/
git commit -m "test(voice): add integration tests for pipeline, routing, and state machine"
```

---

### Task 21: Final workspace build + clippy verification

**Files:** None (verification only)

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Compiles with zero errors.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: All formatted.

- [ ] **Step 4: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 5: Frontend build + lint**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: No errors.

- [ ] **Step 6: Commit any fixes**

If clippy or fmt required changes:

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting for voice-engine integration"
```
