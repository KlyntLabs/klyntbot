# Language Learning Engine — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Whisper with Qwen3-ASR across the entire voice system, add Qwen3-TTS, add cloud API fallback, and build the pronunciation pipeline with phoneme-level + tone analysis for English and Chinese.

**Architecture:** Config-driven engine selection with local/cloud deployment toggle. Three new engine pairs (Qwen3 local + cloud) implementing existing `TtsEngine`/`TranscriptionEngine` traits. New `PronunciationAnalyzer` trait for phoneme alignment. New `feature-language-learning` crate at L4 with FSRS-5 pronunciation mastery tracking.

**Tech Stack:** Rust, `qwen3_tts` crate (MLX backend), `qwen3_asr_rs` (MLX), `pitch-detection` (YIN algorithm for Chinese tones), `reqwest` (cloud API), Tauri 2 (desktop), React/TypeScript (UI events).

---

## File Structure

### Config Layer (L1)
- Modify: `crates/config/src/schema/voice.rs` — replace `SttEngineKind::WhisperLocal` with `Qwen3`, add `EngineDeployment` enum, add deployment fields
- Create: `crates/config/src/schema/language_learning.rs` — `LanguageLearningConfig`, `FeedbackConfig`
- Modify: `crates/config/src/schema/mod.rs` — register new module

### Voice Engine Layer (L5)
- Delete: `crates/voice-engine/src/engines/whisper_local.rs` — removed entirely
- Create: `crates/voice-engine/src/engines/qwen3_tts.rs` — local Qwen3-TTS via `qwen3_tts` crate
- Create: `crates/voice-engine/src/engines/qwen3_asr.rs` — local Qwen3-ASR via `qwen3_asr_rs`
- Create: `crates/voice-engine/src/engines/cloud_tts.rs` — cloud TTS via OpenAI-compatible API
- Create: `crates/voice-engine/src/engines/cloud_asr.rs` — cloud ASR via OpenAI-compatible API
- Modify: `crates/voice-engine/src/engines/mod.rs` — register new engines, remove whisper
- Modify: `crates/voice-engine/src/lib.rs` — update re-exports
- Modify: `crates/voice-engine/Cargo.toml` — swap `whisper-rs` for `qwen3_tts`/`qwen3_asr`, add `pitch-detection`
- Modify: `crates/voice-engine/src/model_manager.rs` — remove `WhisperModelSize`, add Qwen3 model detection + auto-download
- Create: `crates/voice-engine/src/pronunciation_analyzer.rs` — `PronunciationAnalyzer` trait
- Create: `crates/voice-engine/src/phoneme_aligner.rs` — Qwen3-ForcedAligner wrapper
- Create: `crates/voice-engine/src/tone_analyzer.rs` — F0 pitch contour via YIN (Chinese only)
- Create: `crates/voice-engine/src/error_classifier.rs` — expected vs actual phoneme scoring
- Create: `crates/voice-engine/src/feedback_decider.rs` — FSRS-driven adaptive feedback level
- Modify: `crates/voice-engine/src/service.rs` — use new pronunciation pipeline
- Modify: `crates/voice-engine/src/events.rs` — add pronunciation events

### Feature Crate (L4)
- Create: `crates/feature-language-learning/Cargo.toml`
- Create: `crates/feature-language-learning/src/lib.rs` — `FeaturePackage` impl
- Create: `crates/feature-language-learning/src/types.rs` — `DetailedPronunciationReport`, `PhonemeScore`, `ToneScore`
- Create: `crates/feature-language-learning/src/practice_tool.rs` — `language_practice` tool
- Create: `crates/feature-language-learning/migrations/001_create_tables.sql`

### Orchestrator Skill
- Create: `skills/language-learning/SKILL.md` — language-tutor orchestrator

### App Core (L7)
- Modify: `crates/app-core/src/init/mod.rs` — wire new engines with config-driven selection
- Modify: `crates/app-core/Cargo.toml` — add `feature-language-learning` dep, forward `qwen3` feature

### Workspace
- Modify: `Cargo.toml` (workspace root) — add `feature-language-learning` member + dep

---

## Task 1: Config — Engine Deployment Mode + Qwen3 Variants

Update the config schema to support local/cloud deployment and the new Qwen3 engine kinds.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`

- [ ] **Step 1: Write tests for new config enums**

Add to the existing `#[cfg(test)] mod tests` in `crates/config/src/schema/voice.rs`:

```rust
    #[test]
    fn default_engine_is_qwen3() {
        let input = VoiceInputConfig::default();
        assert_eq!(input.stt_engine, SttEngineKind::Qwen3);

        let output = VoiceOutputConfig::default();
        assert_eq!(output.tts_engine, TtsEngineKind::Qwen3);
    }

    #[test]
    fn deserialize_cloud_deployment() {
        let json = r#"{"mode": "cloud", "apiUrl": "https://api.example.com/v1", "apiKey": "sk-test"}"#;
        let deployment: EngineDeployment = serde_json::from_str(json).unwrap();
        match deployment {
            EngineDeployment::Cloud { api_url, .. } => {
                assert_eq!(api_url, "https://api.example.com/v1");
            }
            _ => panic!("Expected Cloud deployment"),
        }
    }

    #[test]
    fn default_deployment_is_local() {
        let json = r#"{}"#;
        let input: VoiceInputConfig = serde_json::from_str(json).unwrap();
        assert!(matches!(input.deployment, EngineDeployment::Local));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p config -E 'test(default_engine_is_qwen3)'`

Expected: FAIL — `SttEngineKind::Qwen3` doesn't exist.

- [ ] **Step 3: Update SttEngineKind and TtsEngineKind enums**

In `crates/config/src/schema/voice.rs`, replace the existing enums:

```rust
/// STT engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SttEngineKind {
    /// Qwen3-ASR local or cloud (default, replaces Whisper).
    #[default]
    Qwen3,
}

/// TTS engine selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsEngineKind {
    /// Qwen3-TTS local or cloud (default).
    #[default]
    Qwen3,
    /// macOS system TTS via AVSpeechSynthesizer.
    System,
    /// Kokoro-82M via ONNX Runtime.
    Kokoro,
}

/// Deployment mode — local model or cloud API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum EngineDeployment {
    /// Run model locally on device (MLX/Metal).
    #[default]
    Local,
    /// Call a cloud API (OpenAI-compatible endpoint).
    Cloud {
        #[serde(rename = "apiUrl")]
        api_url: String,
        #[serde(rename = "apiKey")]
        api_key: String,
    },
}

impl Default for EngineDeployment {
    fn default() -> Self {
        Self::Local
    }
}
```

- [ ] **Step 4: Add deployment field to VoiceInputConfig and VoiceOutputConfig**

Add `#[serde(default)] pub deployment: EngineDeployment` to both structs and their `Default` impls.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(default_engine_is_qwen3) | test(deserialize_cloud_deployment) | test(default_deployment_is_local)'`

Expected: All PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/config/src/schema/voice.rs
git commit -m "feat(config): replace WhisperLocal with Qwen3, add EngineDeployment local/cloud toggle"
```

---

## Task 2: Config — Language Learning Config

Add the dedicated language learning configuration.

**Files:**
- Create: `crates/config/src/schema/language_learning.rs`
- Modify: `crates/config/src/schema/mod.rs`

- [ ] **Step 1: Write test for default config**

Create `crates/config/src/schema/language_learning.rs`:

```rust
//! Language learning configuration.

use serde::{Deserialize, Serialize};

/// Feedback level for pronunciation corrections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedbackLevel {
    /// Post-turn summary card.
    #[default]
    Summary,
    /// Real-time overlay on persistent weak spots.
    Overlay,
    /// Background scoring, surface on request only.
    Silent,
}

/// Controls how aggressively pronunciation feedback is shown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackConfig {
    /// Default feedback level.
    #[serde(default)]
    pub default_level: FeedbackLevel,
    /// FSRS stability below which feedback escalates to Overlay.
    #[serde(default = "default_escalation_threshold")]
    pub escalation_threshold: f32,
    /// Minimum encounters before escalation is considered.
    #[serde(default = "default_min_encounters")]
    pub min_encounters: u32,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            default_level: FeedbackLevel::default(),
            escalation_threshold: default_escalation_threshold(),
            min_encounters: default_min_encounters(),
        }
    }
}

/// Top-level language learning configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageLearningConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_primary_languages")]
    pub primary_languages: Vec<String>,
    #[serde(default)]
    pub feedback: FeedbackConfig,
}

impl Default for LanguageLearningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            primary_languages: default_primary_languages(),
            feedback: FeedbackConfig::default(),
        }
    }
}

fn default_escalation_threshold() -> f32 {
    0.3
}

fn default_min_encounters() -> u32 {
    5
}

fn default_primary_languages() -> Vec<String> {
    vec!["en".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = LanguageLearningConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.primary_languages, vec!["en"]);
        assert_eq!(config.feedback.default_level, FeedbackLevel::Summary);
        assert!((config.feedback.escalation_threshold - 0.3).abs() < 0.01);
        assert_eq!(config.feedback.min_encounters, 5);
    }

    #[test]
    fn deserialize_with_overrides() {
        let json = r#"{"enabled": true, "primaryLanguages": ["en", "zh"], "feedback": {"defaultLevel": "overlay"}}"#;
        let config: LanguageLearningConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.primary_languages, vec!["en", "zh"]);
        assert_eq!(config.feedback.default_level, FeedbackLevel::Overlay);
    }
}
```

- [ ] **Step 2: Register module in schema/mod.rs**

In `crates/config/src/schema/mod.rs`, add:
```rust
pub mod language_learning;
pub use self::language_learning::*;
```

And add to the main `Config` struct:
```rust
    #[serde(default)]
    pub language_learning: LanguageLearningConfig,
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p config -E 'test(language_learning)'`

Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/language_learning.rs crates/config/src/schema/mod.rs
git commit -m "feat(config): add LanguageLearningConfig with feedback levels"
```

---

## Task 3: Remove Whisper — Delete Engine + Dependencies

Remove Whisper from the entire codebase. Qwen3-ASR will replace it in a later task.

**Files:**
- Delete: `crates/voice-engine/src/engines/whisper_local.rs`
- Modify: `crates/voice-engine/Cargo.toml` — remove `whisper-rs`
- Modify: `crates/voice-engine/src/engines/mod.rs` — remove whisper module
- Modify: `crates/voice-engine/src/lib.rs` — remove `WhisperLocalEngine` re-export
- Modify: `crates/voice-engine/src/model_manager.rs` — remove `WhisperModelSize`
- Modify: `crates/voice-engine/src/service.rs` — remove whisper references
- Modify: `crates/app-core/src/init/mod.rs` — remove whisper init code

- [ ] **Step 1: Delete whisper_local.rs**

```bash
rm crates/voice-engine/src/engines/whisper_local.rs
```

- [ ] **Step 2: Remove `whisper-rs` from Cargo.toml**

In `crates/voice-engine/Cargo.toml`, remove:
```toml
whisper-rs = { version = "0.16", features = ["metal"] }
```

- [ ] **Step 3: Update engines/mod.rs — remove whisper module**

Remove `pub mod whisper_local;` and `pub use whisper_local::WhisperLocalEngine;`.

- [ ] **Step 4: Update lib.rs — remove WhisperLocalEngine re-export**

Remove `pub use engines::{AvSpeechTtsEngine, WhisperLocalEngine};` and replace with `pub use engines::AvSpeechTtsEngine;`. Also remove `pub use model_manager::{ModelManager, ModelState, WhisperModelSize};` and replace with `pub use model_manager::{ModelManager, ModelState};`.

- [ ] **Step 5: Update model_manager.rs — remove WhisperModelSize**

Remove the `WhisperModelSize` enum, its `impl` block, and all methods that reference it (`model_path`, `is_available`, `start_download`). Keep the struct, `new()`, `state()`, `subscribe()`, and `kokoro_model_dir()`. Update `new()` to not check for Whisper model on init.

Remove all Whisper-related tests. Keep the Kokoro model tests.

- [ ] **Step 6: Update app-core/src/init/mod.rs — remove Whisper init**

Replace the STT init section (which loads `WhisperLocalEngine`) with a placeholder `None` for now (Qwen3-ASR will be wired in Task 5):

```rust
                // STT: will be replaced by Qwen3-ASR in next task
                let stt_local: Option<Arc<dyn TranscriptionEngine>> = None;
                let has_local_engine = false;
```

Remove the Whisper model download section and the idle-unload cron job for Whisper.

- [ ] **Step 7: Fix all compilation errors**

Run: `cargo check --workspace 2>&1 | grep "^error"`

Fix any remaining references to `WhisperLocalEngine`, `WhisperModelSize`, or `whisper_local` throughout the workspace. Common locations: `service.rs` (the `try_unload_idle_stt` cron), integration tests in `tests/`.

- [ ] **Step 8: Run tests**

Run: `cargo nextest run -p voice-engine`

Expected: All existing tests pass (mock-based tests don't use Whisper). Whisper-specific tests are gone.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor(voice): remove Whisper engine entirely, Qwen3-ASR will replace"
```

---

## Task 4: Add Qwen3 Dependencies

Add the Rust crates for Qwen3-TTS and Qwen3-ASR, plus pitch-detection for tone analysis.

**Files:**
- Modify: `crates/voice-engine/Cargo.toml`

- [ ] **Step 1: Add dependencies**

In `crates/voice-engine/Cargo.toml`, add:

```toml
qwen3_tts = { version = "0.1", optional = true, default-features = false, features = ["mlx"] }
pitch-detection = { version = "0.3", optional = true }
```

Note: `qwen3_asr_rs` may not be on crates.io yet. Check `cargo search qwen3_asr` — if not available, use a git dependency:
```toml
# If on crates.io:
qwen3_asr = { version = "0.1", optional = true }
# If git only:
# qwen3_asr = { git = "https://github.com/second-state/qwen3_asr_rs", optional = true }
```

Update features:
```toml
[features]
default = ["kokoro", "vad", "qwen3"]
kokoro = ["dep:kokoro-tts"]
vad = ["dep:webrtc-vad", "dep:nnnoiseless"]
qwen3 = ["dep:qwen3_tts", "dep:pitch-detection"]
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p voice-engine --features qwen3`

Expected: Clean build. If `qwen3_tts` has dependency issues (like the `ort` conflict we saw with `kokoroxide`), fall back to the `tch` backend instead of `mlx`:
```toml
qwen3_tts = { version = "0.1", optional = true, default-features = false, features = ["tch"] }
```

- [ ] **Step 3: Commit**

```bash
git add crates/voice-engine/Cargo.toml Cargo.lock
git commit -m "feat(voice): add qwen3_tts and pitch-detection dependencies"
```

---

## Task 5: Implement Qwen3-ASR Engine

Create the local Qwen3-ASR engine implementing `TranscriptionEngine`.

**Files:**
- Create: `crates/voice-engine/src/engines/qwen3_asr.rs`
- Modify: `crates/voice-engine/src/engines/mod.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Write test for Qwen3-ASR trait compliance**

Create `crates/voice-engine/src/engines/qwen3_asr.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name() {
        // Can't construct without model, test the constant
        assert_eq!(QWEN3_ASR_SAMPLE_RATE, 16_000);
    }
}
```

- [ ] **Step 2: Implement Qwen3AsrEngine**

```rust
//! Qwen3-ASR speech recognition engine.
//!
//! Replaces Whisper as the universal STT engine. Supports 52 languages
//! with built-in language detection and word-level timestamps.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript, TranscriptSegment};

const IDLE_UNLOAD_SECS: u64 = 300;
const QWEN3_ASR_SAMPLE_RATE: u32 = 16_000;

pub struct Qwen3AsrEngine {
    // Model state will depend on the actual qwen3_asr crate API.
    // Placeholder structure — adapt to the actual crate when integrating.
    model_dir: PathBuf,
    last_used: Arc<Mutex<Instant>>,
    loaded: Arc<Mutex<bool>>,
}

impl Qwen3AsrEngine {
    pub fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        if !model_dir.exists() {
            return Err(common::KlyntbotError::Config(
                common::ConfigError::NotFound(format!(
                    "Qwen3-ASR model not found: {}",
                    model_dir.display()
                )),
            ));
        }

        info!("Qwen3-ASR engine created (lazy) for: {}", model_dir.display());

        Ok(Self {
            model_dir,
            last_used: Arc::new(Mutex::new(Instant::now())),
            loaded: Arc::new(Mutex::new(false)),
        })
    }

    pub fn unload_if_idle(&self) -> bool {
        let last = *self.last_used.lock().unwrap();
        if last.elapsed().as_secs() >= IDLE_UNLOAD_SECS {
            let mut loaded = self.loaded.lock().unwrap();
            if *loaded {
                *loaded = false;
                info!("Qwen3-ASR model unloaded after idle");
                return true;
            }
        }
        false
    }
}

#[async_trait]
impl TranscriptionEngine for Qwen3AsrEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        *self.last_used.lock().unwrap() = Instant::now();
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        // TODO: Replace with actual qwen3_asr crate API when integrating.
        // For now, collect audio and emit a single final partial.
        // The actual implementation will use qwen3_asr::transcribe() or similar.
        let model_dir = self.model_dir.clone();
        let loaded = self.loaded.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();

            let mut all_samples: Vec<f32> = Vec::new();
            while let Some(chunk) = rt.block_on(audio.recv()) {
                all_samples.extend_from_slice(&chunk.samples);
            }

            if all_samples.is_empty() {
                warn!("Empty audio stream, skipping transcription");
                return;
            }

            *loaded.lock().unwrap() = true;

            debug!(
                "Qwen3-ASR transcribing {} samples ({:.1}s)",
                all_samples.len(),
                all_samples.len() as f32 / QWEN3_ASR_SAMPLE_RATE as f32
            );

            // Placeholder: actual qwen3_asr integration goes here.
            // The crate API will provide:
            // - Automatic language detection
            // - Word-level timestamps
            // - Streaming partial support
            let _ = rt.block_on(tx.send(PartialTranscript {
                text: String::new(), // Will be populated by actual ASR
                segments: vec![],
                language: Language::default(),
                is_final: true,
            }));
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        path: &std::path::Path,
        lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        *self.last_used.lock().unwrap() = Instant::now();

        if !path.exists() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!(
                    "Audio file not found: {}",
                    path.display()
                )),
            ));
        }

        // Placeholder: actual file transcription
        Ok(Transcript {
            text: String::new(),
            language: lang_hint.cloned().unwrap_or_default(),
            segments: vec![],
            overall_confidence: 0.0,
        })
    }

    fn display_name(&self) -> &str {
        "Qwen3-ASR"
    }

    fn unload_if_idle(&self) -> bool {
        Self::unload_if_idle(self)
    }
}
```

**Note:** This is a scaffold with the correct trait implementation. The actual `qwen3_asr` crate integration (model loading, inference) will be adapted when the crate API is verified — same approach as Kokoro where we adapted to the real API at build time.

- [ ] **Step 3: Register module**

In `crates/voice-engine/src/engines/mod.rs`, add:
```rust
#[cfg(feature = "qwen3")]
pub mod qwen3_asr;

#[cfg(feature = "qwen3")]
pub use qwen3_asr::Qwen3AsrEngine;
```

In `crates/voice-engine/src/lib.rs`, add:
```rust
#[cfg(feature = "qwen3")]
pub use engines::Qwen3AsrEngine;
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

Expected: Clean build, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/engines/qwen3_asr.rs crates/voice-engine/src/engines/mod.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add Qwen3-ASR engine scaffold implementing TranscriptionEngine"
```

---

## Task 6: Implement Qwen3-TTS Engine

Create the local Qwen3-TTS engine implementing `TtsEngine`.

**Files:**
- Create: `crates/voice-engine/src/engines/qwen3_tts.rs`
- Modify: `crates/voice-engine/src/engines/mod.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Implement Qwen3TtsEngine**

Create `crates/voice-engine/src/engines/qwen3_tts.rs`. Follow the exact pattern from `kokoro.rs` — lazy model loading, voice mapping, `TtsEngine` trait impl. Use `qwen3_tts` crate with MLX backend.

Key differences from Kokoro:
- Use `qwen3_tts::Qwen3Tts` (or whatever the crate's main struct is — verify at build time)
- Sample rate: 24kHz (Qwen3-TTS default)
- Voice list: map identifiers from the Qwen3-TTS voice catalog
- Model files: safetensors format in `~/.klyntbot/models/voice/qwen3-tts-0.6b/`

Include inline tests for voice resolution and language support.

- [ ] **Step 2: Register module**

Same pattern as Kokoro: add to `mod.rs` and `lib.rs` behind `#[cfg(feature = "qwen3")]`.

- [ ] **Step 3: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/engines/qwen3_tts.rs crates/voice-engine/src/engines/mod.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add Qwen3-TTS engine implementing TtsEngine"
```

---

## Task 7: Implement Cloud Engines (OpenAI-compatible API)

Create cloud engine implementations for both TTS and ASR using the OpenAI-compatible audio API.

**Files:**
- Create: `crates/voice-engine/src/engines/cloud_tts.rs`
- Create: `crates/voice-engine/src/engines/cloud_asr.rs`
- Modify: `crates/voice-engine/src/engines/mod.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Implement CloudTtsEngine**

Create `crates/voice-engine/src/engines/cloud_tts.rs`:

```rust
//! Cloud TTS engine via OpenAI-compatible audio API.
//!
//! Calls `POST /audio/speech` with `{ model, input, voice }`.
//! DashScope (Alibaba Cloud) is the reference provider.

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::tts::TtsEngine;
use crate::types::*;

pub struct CloudTtsEngine {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl CloudTtsEngine {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
            api_key,
            model: "qwen3-tts".to_string(),
        }
    }
}

#[async_trait]
impl TtsEngine for CloudTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let voice = params.voice_name.as_deref().unwrap_or("alloy");
        let url = format!("{}/audio/speech", self.api_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": self.model,
                "input": text,
                "voice": voice,
                "speed": params.speaking_rate,
                "response_format": "pcm",
            }))
            .send()
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!(
                    "Cloud TTS HTTP {status}: {body}"
                )),
            ));
        }

        let bytes = response.bytes().await.map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
        })?;

        // PCM 16-bit LE mono at 24kHz
        let samples: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                sample as f32 / i16::MAX as f32
            })
            .collect();

        debug!("Cloud TTS returned {} samples", samples.len());

        Ok(AudioClip {
            samples,
            sample_rate: 24_000,
            channels: 1,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true // Cloud API handles language routing server-side
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        // Standard OpenAI voices — actual list depends on provider
        ["alloy", "echo", "fable", "onyx", "nova", "shimmer"]
            .iter()
            .map(|v| VoiceInfo {
                identifier: v.to_string(),
                display_name: v.to_string(),
                language: Language::new("en"),
            })
            .collect()
    }

    fn display_name(&self) -> &str {
        "Cloud TTS"
    }
}
```

- [ ] **Step 2: Implement CloudAsrEngine**

Create `crates/voice-engine/src/engines/cloud_asr.rs`:

```rust
//! Cloud ASR engine via OpenAI-compatible audio API.
//!
//! Calls `POST /audio/transcriptions` with multipart form data.

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::*;

pub struct CloudAsrEngine {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl CloudAsrEngine {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
            api_key,
            model: "qwen3-asr".to_string(),
        }
    }
}

#[async_trait]
impl TranscriptionEngine for CloudAsrEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        // Collect all audio, encode as WAV, send to API
        let client = self.client.clone();
        let api_url = self.api_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();

        tokio::spawn(async move {
            let mut all_samples: Vec<f32> = Vec::new();
            while let Some(chunk) = audio.recv().await {
                all_samples.extend_from_slice(&chunk.samples);
            }

            if all_samples.is_empty() {
                warn!("Empty audio stream for cloud ASR");
                return;
            }

            // Encode as WAV bytes for the API
            let wav_bytes = encode_wav_bytes(&all_samples, 16000);
            let url = format!("{}/audio/transcriptions", api_url);

            let form = reqwest::multipart::Form::new()
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(wav_bytes)
                        .file_name("audio.wav")
                        .mime_str("audio/wav")
                        .unwrap(),
                )
                .text("model", model)
                .text("response_format", "verbose_json")
                .text("timestamp_granularities[]", "word");

            match client.post(&url).bearer_auth(&api_key).multipart(form).send().await {
                Ok(response) if response.status().is_success() => {
                    if let Ok(text) = response.text().await {
                        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text) {
                            let transcript_text = result["text"].as_str().unwrap_or("").to_string();
                            let language = result["language"]
                                .as_str()
                                .unwrap_or("en")
                                .to_string();

                            let _ = tx
                                .send(PartialTranscript {
                                    text: transcript_text,
                                    segments: vec![], // Parse word timestamps from response
                                    language: Language::new(language),
                                    is_final: true,
                                })
                                .await;
                        }
                    }
                }
                Ok(response) => {
                    warn!("Cloud ASR failed: HTTP {}", response.status());
                }
                Err(e) => {
                    warn!("Cloud ASR request failed: {e}");
                }
            }
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        let file_bytes = tokio::fs::read(path).await.map_err(common::KlyntbotError::Io)?;
        let url = format!("{}/audio/transcriptions", self.api_url);

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(path.file_name().unwrap_or_default().to_string_lossy().to_string())
                    .mime_str("audio/wav")
                    .unwrap(),
            )
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        let text = response
            .text()
            .await
            .map_err(|e| common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string())))?;

        let result: serde_json::Value = serde_json::from_str(&text)?;

        Ok(Transcript {
            text: result["text"].as_str().unwrap_or("").to_string(),
            language: Language::new(result["language"].as_str().unwrap_or("en")),
            segments: vec![],
            overall_confidence: 0.0,
        })
    }

    fn display_name(&self) -> &str {
        "Cloud ASR"
    }
}

/// Encode f32 samples as a WAV byte buffer (16-bit PCM).
fn encode_wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let data_len = (samples.len() * 2) as u32;
    let file_len = 36 + data_len;

    // WAV header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());

    for &s in samples {
        let i16_sample = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        buf.extend_from_slice(&i16_sample.to_le_bytes());
    }

    buf
}
```

- [ ] **Step 3: Register both modules**

In `engines/mod.rs`:
```rust
pub mod cloud_asr;
pub mod cloud_tts;
pub use cloud_asr::CloudAsrEngine;
pub use cloud_tts::CloudTtsEngine;
```

In `lib.rs`:
```rust
pub use engines::{CloudAsrEngine, CloudTtsEngine};
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/engines/cloud_tts.rs crates/voice-engine/src/engines/cloud_asr.rs crates/voice-engine/src/engines/mod.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add cloud TTS/ASR engines via OpenAI-compatible API"
```

---

## Task 8: Wire Engines in Init — Config-Driven Selection

Update the app-core init to select engines based on config + deployment mode.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/Cargo.toml` — forward `qwen3` feature
- Modify: `crates/voice-engine/src/model_manager.rs` — add Qwen3 model detection + auto-download

- [ ] **Step 1: Add Qwen3 model detection to ModelManager**

In `crates/voice-engine/src/model_manager.rs`, add methods:

```rust
    /// Check if Qwen3-TTS model exists.
    pub fn qwen3_tts_model_dir(&self) -> Option<PathBuf> {
        let dir = self.models_dir.join("qwen3-tts-0.6b");
        if dir.is_dir() {
            Some(dir)
        } else {
            None
        }
    }

    /// Check if Qwen3-ASR model exists.
    pub fn qwen3_asr_model_dir(&self) -> Option<PathBuf> {
        let dir = self.models_dir.join("qwen3-asr-0.6b");
        if dir.is_dir() {
            Some(dir)
        } else {
            None
        }
    }
```

- [ ] **Step 2: Update init to use config-driven engine selection**

Replace the TTS and STT init sections in `crates/app-core/src/init/mod.rs`:

```rust
                // STT: config-driven engine selection with deployment mode
                let stt_local: Option<Arc<dyn TranscriptionEngine>> = {
                    match voice_config.input.deployment {
                        config::schema::EngineDeployment::Local => {
                            #[cfg(feature = "qwen3")]
                            {
                                model_manager
                                    .qwen3_asr_model_dir()
                                    .and_then(|dir| {
                                        match voice_engine::Qwen3AsrEngine::new(&dir) {
                                            Ok(engine) => {
                                                info!("Qwen3-ASR loaded from {}", dir.display());
                                                Some(Arc::new(engine) as Arc<dyn TranscriptionEngine>)
                                            }
                                            Err(e) => {
                                                warn!("Failed to load Qwen3-ASR: {e}");
                                                None
                                            }
                                        }
                                    })
                            }
                            #[cfg(not(feature = "qwen3"))]
                            { None }
                        }
                        config::schema::EngineDeployment::Cloud { ref api_url, ref api_key } => {
                            Some(Arc::new(voice_engine::CloudAsrEngine::new(
                                api_url.clone(),
                                api_key.clone(),
                            )) as Arc<dyn TranscriptionEngine>)
                        }
                    }
                };

                // TTS: config-driven with engine manager wrapping primary + system fallback
                let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
                    let system_tts = Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir));

                    match voice_config.output.deployment {
                        config::schema::EngineDeployment::Cloud { ref api_url, ref api_key } => {
                            let cloud = Arc::new(voice_engine::CloudTtsEngine::new(
                                api_url.clone(),
                                api_key.clone(),
                            ));
                            let manager = voice_engine::TtsEngineManager::new(cloud, Some(system_tts));
                            Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                        }
                        config::schema::EngineDeployment::Local => {
                            // Try Qwen3 → Kokoro → System fallback chain
                            // (implementation depends on which models are available)
                            #[cfg(feature = "qwen3")]
                            {
                                if let config::schema::TtsEngineKind::Qwen3 = voice_config.output.tts_engine {
                                    if let Some(dir) = model_manager.qwen3_tts_model_dir() {
                                        match voice_engine::Qwen3TtsEngine::new(&dir).await {
                                            Ok(engine) => {
                                                info!("Qwen3-TTS loaded — wrapping with system fallback");
                                                let manager = voice_engine::TtsEngineManager::new(
                                                    Arc::new(engine),
                                                    Some(system_tts),
                                                );
                                                Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                                            }
                                            Err(e) => {
                                                warn!("Qwen3-TTS failed, trying Kokoro: {e}");
                                                // Fall through to Kokoro/System
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
                            #[cfg(not(feature = "qwen3"))]
                            {
                                Some(system_tts as Arc<dyn voice_engine::TtsEngine>)
                            }
                        }
                    }
                };
```

- [ ] **Step 3: Forward qwen3 feature in app-core**

In `crates/app-core/Cargo.toml`:
```toml
[features]
default = ["kokoro", "qwen3"]
kokoro = ["voice-engine/kokoro"]
qwen3 = ["voice-engine/qwen3"]
```

- [ ] **Step 4: Build workspace**

Run: `cargo check --workspace`

Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/app-core/Cargo.toml crates/voice-engine/src/model_manager.rs
git commit -m "feat(voice): wire config-driven engine selection with local/cloud deployment"
```

---

## Task 9: Pronunciation Analyzer Trait + Phoneme Aligner

Create the new `PronunciationAnalyzer` trait and the Qwen3-ForcedAligner wrapper.

**Files:**
- Create: `crates/voice-engine/src/pronunciation_analyzer.rs` — trait definition
- Create: `crates/voice-engine/src/phoneme_aligner.rs` — Qwen3-ForcedAligner impl
- Modify: `crates/voice-engine/src/lib.rs` — register modules

- [ ] **Step 1: Define PronunciationAnalyzer trait**

Create `crates/voice-engine/src/pronunciation_analyzer.rs`:

```rust
//! Pronunciation analysis trait — phoneme alignment and tone extraction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::{AudioClip, Language};

/// A single phoneme with timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignedPhoneme {
    pub phoneme: String,
    pub word: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

/// Result of phoneme alignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhonemeAlignment {
    pub phonemes: Vec<AlignedPhoneme>,
    pub language: Language,
}

/// F0 pitch contour for a syllable (Chinese tone analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyllableTone {
    pub syllable: String,
    pub expected_tone: u8,
    pub detected_tone: u8,
    pub f0_contour: Vec<f32>,
    pub correct: bool,
}

/// Tone contour analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToneContour {
    pub syllables: Vec<SyllableTone>,
}

#[async_trait]
pub trait PronunciationAnalyzer: Send + Sync {
    /// Align transcript to audio, returning phoneme-level timestamps.
    async fn align(
        &self,
        audio: &AudioClip,
        transcript: &str,
        lang: &Language,
    ) -> common::Result<PhonemeAlignment>;

    /// Extract F0 pitch contour for tone analysis (Chinese).
    async fn extract_tones(
        &self,
        audio: &AudioClip,
        alignment: &PhonemeAlignment,
    ) -> common::Result<ToneContour>;
}
```

- [ ] **Step 2: Implement PhonemeAligner**

Create `crates/voice-engine/src/phoneme_aligner.rs`:

```rust
//! Qwen3-ForcedAligner wrapper for phoneme-level alignment.

use std::path::PathBuf;

use async_trait::async_trait;
use tracing::{debug, info};

use crate::pronunciation_analyzer::*;
use crate::types::{AudioClip, Language};

pub struct Qwen3PhonemeAligner {
    model_dir: PathBuf,
}

impl Qwen3PhonemeAligner {
    pub fn new(model_dir: impl Into<PathBuf>) -> common::Result<Self> {
        let model_dir = model_dir.into();
        if !model_dir.exists() {
            return Err(common::KlyntbotError::Config(
                common::ConfigError::NotFound(format!(
                    "Qwen3-ForcedAligner model not found: {}",
                    model_dir.display()
                )),
            ));
        }
        info!("Qwen3 phoneme aligner ready: {}", model_dir.display());
        Ok(Self { model_dir })
    }
}

#[async_trait]
impl PronunciationAnalyzer for Qwen3PhonemeAligner {
    async fn align(
        &self,
        audio: &AudioClip,
        transcript: &str,
        lang: &Language,
    ) -> common::Result<PhonemeAlignment> {
        debug!(
            "Aligning {} samples against '{}' (lang={})",
            audio.samples.len(),
            &transcript[..transcript.len().min(50)],
            lang.as_str()
        );

        // TODO: Integrate qwen3_asr forced alignment API.
        // The crate provides word/character-level timestamps
        // which we map to phonemes using a pronunciation dictionary.
        Ok(PhonemeAlignment {
            phonemes: vec![],
            language: lang.clone(),
        })
    }

    async fn extract_tones(
        &self,
        audio: &AudioClip,
        alignment: &PhonemeAlignment,
    ) -> common::Result<ToneContour> {
        if alignment.language.as_str() != "zh" {
            return Ok(ToneContour {
                syllables: vec![],
            });
        }

        // TODO: Use pitch-detection crate (YIN algorithm) to extract F0 contour
        // per syllable, then classify as tone 1-4 or neutral.
        Ok(ToneContour {
            syllables: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_chinese_skips_tones() {
        let alignment = PhonemeAlignment {
            phonemes: vec![],
            language: Language::new("en"),
        };
        // English should produce empty tone contour
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path()).unwrap();
        let aligner = Qwen3PhonemeAligner::new(tmp.path()).unwrap();
        let audio = AudioClip {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
            channels: 1,
        };
        let result = rt.block_on(aligner.extract_tones(&audio, &alignment)).unwrap();
        assert!(result.syllables.is_empty());
    }
}
```

- [ ] **Step 3: Register modules**

In `crates/voice-engine/src/lib.rs`:
```rust
pub mod phoneme_aligner;
pub mod pronunciation_analyzer;
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/pronunciation_analyzer.rs crates/voice-engine/src/phoneme_aligner.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add PronunciationAnalyzer trait and Qwen3 phoneme aligner"
```

---

## Task 10: Tone Analyzer + Error Classifier + Feedback Decider

Build the remaining three pronunciation pipeline components.

**Files:**
- Create: `crates/voice-engine/src/tone_analyzer.rs`
- Create: `crates/voice-engine/src/error_classifier.rs`
- Create: `crates/voice-engine/src/feedback_decider.rs`
- Modify: `crates/voice-engine/src/lib.rs`

- [ ] **Step 1: Implement ToneContourAnalyzer**

Create `crates/voice-engine/src/tone_analyzer.rs` using the `pitch-detection` crate with YIN algorithm. Operates on Chinese audio only. Extracts F0 contour per syllable, classifies as tone 1-4.

- [ ] **Step 2: Implement ErrorClassifier**

Create `crates/voice-engine/src/error_classifier.rs`. Compares expected vs actual phonemes from alignment. Produces `PhonemeScore { expected, actual, word, confidence, timestamp_ms }`. For Chinese: combines phoneme score + tone match.

- [ ] **Step 3: Implement FeedbackLevelDecider**

Create `crates/voice-engine/src/feedback_decider.rs`. Uses FSRS stability + error frequency to determine `FeedbackLevel` (Summary/Overlay/Silent). Takes `phoneme_mastery` data as input.

- [ ] **Step 4: Register all modules in lib.rs**

- [ ] **Step 5: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 6: Commit**

```bash
git add crates/voice-engine/src/tone_analyzer.rs crates/voice-engine/src/error_classifier.rs crates/voice-engine/src/feedback_decider.rs crates/voice-engine/src/lib.rs
git commit -m "feat(voice): add tone analyzer, error classifier, and feedback decider"
```

---

## Task 11: Feature Crate — `feature-language-learning`

Create the new feature package with storage, types, and the `language_practice` tool.

**Files:**
- Create: `crates/feature-language-learning/Cargo.toml`
- Create: `crates/feature-language-learning/src/lib.rs`
- Create: `crates/feature-language-learning/src/types.rs`
- Create: `crates/feature-language-learning/src/practice_tool.rs`
- Create: `crates/feature-language-learning/migrations/001_create_tables.sql`
- Modify: `Cargo.toml` (workspace root) — add member + dependency

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "feature-language-learning"
version.workspace = true
edition.workspace = true

[dependencies]
common.workspace = true
storage.workspace = true
tools-core.workspace = true
tools-core-macros.workspace = true
voice-engine.workspace = true
config.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
chrono.workspace = true
```

- [ ] **Step 2: Create types.rs with DetailedPronunciationReport**

Define `DetailedPronunciationReport`, `PhonemeScore`, `ToneScore`, `FluencyMetrics`, `WeakPhoneme` as specified in the design doc Section 5.

- [ ] **Step 3: Create migrations SQL**

Create `crates/feature-language-learning/migrations/001_create_tables.sql` with the three tables from the design doc Section 6.2: `phoneme_mastery`, `pronunciation_logs`, `exam_attempts`.

- [ ] **Step 4: Create lib.rs with FeaturePackage impl**

Implement `FeaturePackage` following the `feature-tasks` pattern:
```rust
pub struct LanguageLearningFeature { pool: Option<storage::StoragePool> }

impl FeaturePackage for LanguageLearningFeature {
    fn name(&self) -> &str { "language-learning" }
    fn tools(&self) -> Vec<DynTool> { vec![] } // Tool added in next step
    fn migrations(&self) -> Vec<FeatureMigration> { /* 001_create_tables */ }
    fn config_key(&self) -> &str { "languageLearning" }
}
```

- [ ] **Step 5: Create practice_tool.rs**

Implement `language_practice` tool with actions: `start_session`, `end_session`, `get_feedback`. Use `#[derive(Tool)]` + `#[derive(ToolParams)]` macros.

- [ ] **Step 6: Add to workspace**

In root `Cargo.toml`, add `"crates/feature-language-learning"` to workspace members and add the path dependency.

- [ ] **Step 7: Build and test**

Run: `cargo build -p feature-language-learning`

- [ ] **Step 8: Commit**

```bash
git add crates/feature-language-learning/ Cargo.toml Cargo.lock
git commit -m "feat(learning): add feature-language-learning crate with FeaturePackage, types, and storage"
```

---

## Task 12: Orchestrator Skill — `language-tutor`

Create the language learning orchestrator skill.

**Files:**
- Create: `skills/language-learning/SKILL.md`

- [ ] **Step 1: Create SKILL.md**

Create `skills/language-learning/SKILL.md` following the Agent Skills format:

```yaml
---
name: language-learning
description: Language learning tutor — pronunciation coaching, conversation practice, and exam prep for English and Chinese
keywords:
  - practice
  - drill
  - pronunciation
  - IELTS
  - HSK
  - speaking test
  - language
  - 英语
  - 中文
  - 练习
tools:
  - language_practice
mcp_tools: []
---
```

Followed by the skill body with instructions for the agent on how to conduct pronunciation coaching, what feedback to give, and how to use the `language_practice` tool.

- [ ] **Step 2: Commit**

```bash
git add skills/language-learning/
git commit -m "feat(learning): add language-tutor orchestrator skill"
```

---

## Task 13: Pronunciation Events + Service Integration

Wire the pronunciation pipeline into the VoiceService and add new events.

**Files:**
- Modify: `crates/voice-engine/src/events.rs` — add pronunciation events
- Modify: `crates/voice-engine/src/service.rs` — integrate pipeline after transcription

- [ ] **Step 1: Add pronunciation events**

In `crates/voice-engine/src/events.rs`, add to the `VoiceEvent` enum:

```rust
    /// Detailed pronunciation report after a scored turn.
    PronunciationReport {
        overall_score: f32,
        phoneme_scores_json: String,
        tone_scores_json: String,
        feedback_level: String,
    },
    /// Adaptive feedback level escalated for a phoneme.
    FeedbackEscalated {
        phoneme: String,
        from_level: String,
        to_level: String,
    },
    /// Chinese tone contour data for visualization.
    ToneContourData {
        syllable: String,
        f0_points: Vec<f32>,
        expected_tone: u8,
        detected_tone: u8,
    },
```

- [ ] **Step 2: Wire pronunciation pipeline into service.rs**

After transcription completes in `stop_capture()`, optionally run the pronunciation pipeline if language learning is enabled. This is a hook point — the actual pipeline call is injected via dependency inversion (similar to `MemoryEchoProvider`).

- [ ] **Step 3: Build and test**

Run: `cargo build -p voice-engine && cargo nextest run -p voice-engine`

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/events.rs crates/voice-engine/src/service.rs
git commit -m "feat(voice): add pronunciation events and pipeline integration hook"
```

---

## Task 14: Model Auto-Download on First Launch

Extend ModelManager to auto-download Qwen3 models on first app launch.

**Files:**
- Modify: `crates/voice-engine/src/model_manager.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add Qwen3 model download to ModelManager**

Add `download_qwen3_tts()` and `download_qwen3_asr()` methods mirroring the existing `start_download()` pattern. Use HuggingFace URLs for model weights.

- [ ] **Step 2: Trigger auto-download in init**

In `crates/app-core/src/init/mod.rs`, after voice service creation, check if Qwen3 models exist and spawn background download if not (same pattern as current Whisper download).

- [ ] **Step 3: Build and test**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/model_manager.rs crates/app-core/src/init/mod.rs
git commit -m "feat(voice): auto-download Qwen3 models on first launch"
```

---

## Task 15: Final Verification

Run the full test suite, clippy, and manual verification.

- [ ] **Step 1: Run all tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets`

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`

- [ ] **Step 4: Verify dev server APIs still work**

```bash
curl -s -X POST -H "Content-Type: application/json" -d '{}' http://localhost:3456/api/voice_get_status
curl -s -X POST -H "Content-Type: application/json" -d '{"section":"voice"}' http://localhost:3456/api/config_get_section
```

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: Phase 1 language learning engine — final verification pass"
```
