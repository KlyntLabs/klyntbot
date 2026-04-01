# Language Learning Engine — Design Spec

> **Status:** Approved
> **Date:** 2026-04-01
> **Goal:** Transform the voice system into an ELSA-competitive language learning platform with phoneme-level pronunciation scoring, Mandarin tone analysis, and adaptive feedback — all running locally on Apple Silicon.

---

## 1. Decisions Locked

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Pronunciation depth | Phoneme-level + Chinese tone analysis | Word-level confidence (current) is too coarse for learning |
| Learning modes | A (conversation) → B (drills) → C (exam), progressive | Ship fast with A, layer B and C incrementally |
| TTS engine | Qwen3-TTS primary, AVSpeech fallback | Rust-native (`qwen3_tts` crate, MLX backend), drops into `TtsEngine` trait |
| STT engine | Qwen3-ASR primary, Whisper fallback | Rust-native (`qwen3_asr_rs`), 52-language support, outperforms Whisper on ZH |
| Alignment | Qwen3-ForcedAligner (0.6B) | Phoneme-level timestamps for 11 languages, same model family |
| Feedback UX | Adaptive (D) — Summary → Overlay → Silent | FSRS stability drives escalation; respects conversational flow |
| Target languages | English + Chinese (Phase 1) | Both well-supported by Qwen3 ecosystem |
| CosyVoice | Deferred to Phase 4 | No Rust crate; Python sidecar breaks single-binary philosophy |

---

## 2. Model Stack

All models from the Qwen3 family. Unified model management via `ModelManager`.

| Model | Purpose | Size | Rust crate | Backend |
|-------|---------|------|-----------|---------|
| Qwen3-TTS-0.6B | Text-to-speech (10 languages, voice cloning) | ~600MB | `qwen3_tts` | MLX (Apple Silicon) |
| Qwen3-ASR-0.6B | Speech recognition (52 languages, timestamps) | ~600MB | `qwen3_asr_rs` | MLX |
| Qwen3-ForcedAligner-0.6B | Phoneme/word alignment (11 languages) | ~600MB | via `qwen3_asr_rs` | MLX |
| Whisper-small (existing) | STT fallback | ~500MB | `whisper-rs` | Metal |

**Lifecycle:**
- **Auto-download** on first app launch (background, progress bar, ~1.8GB total)
- **Lazy-load** into memory on first engine call (~2s from SSD)
- **Idle-unload** after 5 minutes of inactivity (frees ~1.8GB RAM)
- Peak memory when all active: ~2.4GB

**Model directory:**
```
~/.klyntbot/models/voice/
├── qwen3-tts-0.6b/       # safetensors + config
├── qwen3-asr-0.6b/       # safetensors + config
├── qwen3-aligner-0.6b/   # safetensors + config
└── whisper/ggml-small.bin # existing
```

---

## 3. Engine Architecture

### 3.1 New Engines (implementing existing traits)

```
voice-engine/src/engines/
├── qwen3_tts.rs      — TtsEngine impl via qwen3_tts crate (MLX)
├── qwen3_asr.rs      — TranscriptionEngine impl via qwen3_asr_rs
└── qwen3_aligner.rs  — PronunciationAnalyzer (new trait)
```

**Config-driven engine selection:**
- `voice.output.ttsEngine: "qwen3"` → Qwen3-TTS primary, AVSpeech fallback (via TtsEngineManager)
- `voice.input.sttEngine: "qwen3"` → Qwen3-ASR primary, Whisper fallback (new SttEngineManager)
- ForcedAligner always uses Qwen3 (graceful degrade to word-level if unavailable)

**Config enum additions:**
```rust
pub enum SttEngineKind {
    WhisperLocal,
    Qwen3,       // NEW
    Cloud,
}

pub enum TtsEngineKind {
    System,
    Kokoro,
    Qwen3,       // NEW
    Piper,
}
```

### 3.2 New Trait: PronunciationAnalyzer

```rust
#[async_trait]
pub trait PronunciationAnalyzer: Send + Sync {
    async fn align(
        &self,
        audio: &AudioClip,
        transcript: &str,
        lang: &Language,
    ) -> Result<PhonemeAlignment>;

    async fn extract_tones(
        &self,
        audio: &AudioClip,
        alignment: &PhonemeAlignment,
    ) -> Result<ToneContour>;
}
```

For free conversation: ASR-only first, then optional alignment for scoring.
For drills/exam: ASR + alignment in one call (reference text is known).

---

## 4. Pronunciation Pipeline

Shared across all 3 learning modes. Four components with clear responsibilities:

```
Audio capture
  → Qwen3-ASR transcribe (word timestamps + language detection)
  → PhonemeAligner (Qwen3-ForcedAligner, phoneme timestamps)
  → ToneContourAnalyzer (pitch-detection crate, YIN algorithm, Chinese only)
  → ErrorClassifier (expected vs actual phoneme, GOP-like scoring)
  → FeedbackLevelDecider (FSRS stability + error frequency → Level 1/2/3)
  → FSRS-5 update phoneme/tone mastery cards
```

### 4.1 PhonemeAligner

Wraps Qwen3-ForcedAligner-0.6B. Returns phoneme-level timestamps for a given audio + transcript pair.

Uses `qwen_asr::align::forced_align` when reference text is known (drills, exam). Falls back to ASR-only + optional alignment for free conversation.

### 4.2 ToneContourAnalyzer

Uses `pitch-detection` crate with YIN algorithm. Lightweight, pure Rust, sufficient for Mandarin tone 1-4 + neutral classification.

- Runs only when `language == "zh"`
- Stores raw F0 contour downsampled to ~50-100 points per syllable
- Classifies detected tone vs expected tone per syllable

### 4.3 ErrorClassifier

Maps actual pronunciation against expected. Produces a per-phoneme score.

- English: phoneme posterior comparison or edit distance + duration deviation
- Chinese: phoneme score + tone match + F0 contour similarity (DTW distance)
- Output: `PhonemeScore { expected, actual, word, confidence, timestamp_ms }`

### 4.4 FeedbackLevelDecider

Determines how aggressively to show corrections based on learner state.

```rust
pub enum FeedbackLevel {
    Summary,  // Level 1: post-turn card
    Overlay,  // Level 2: real-time red/green words + auto-expand
    Silent,   // Level 3: background scoring, on-request only
}
```

**Escalation logic:**
- Default: `Summary`
- Escalate to `Overlay` when: `review_count >= 5 && fsrs_stability < 0.3` for a phoneme
- `Silent` when: user says "stop corrections" or taps mute
- Manual override: "more feedback" / "less correction" voice commands
- Autotuner tunes thresholds over time (stability cutoff, review_count trigger)

---

## 5. Output Types

### 5.1 DetailedPronunciationReport

Replaces current word-level `PronunciationReport`.

```rust
pub struct DetailedPronunciationReport {
    pub overall_score: f32,                  // 0.0-1.0
    pub phoneme_scores: Vec<PhonemeScore>,   // per-phoneme with expected vs actual
    pub tone_scores: Vec<ToneScore>,         // Chinese: per-syllable tone accuracy
    pub fluency: FluencyMetrics,             // pace, pauses, filler words
    pub weak_phonemes: Vec<WeakPhoneme>,     // persistent errors from FSRS
    pub feedback_level: FeedbackLevel,       // adaptive: Summary/Overlay/Silent
}

pub struct PhonemeScore {
    pub expected: String,     // "/θ/"
    pub actual: String,       // "/s/"
    pub word: String,         // "three"
    pub confidence: f32,      // 0.0-1.0
    pub timestamp_ms: u64,
}

pub struct ToneScore {
    pub syllable: String,     // "ma"
    pub expected_tone: u8,    // 3
    pub detected_tone: u8,    // 2
    pub f0_contour: Vec<f32>, // pitch points for visualization
    pub correct: bool,
}

pub struct FluencyMetrics {
    pub words_per_minute: f32,
    pub pause_count: u32,
    pub filler_count: u32,          // "uh", "um", "那个"
    pub avg_pause_duration_ms: u64,
}

pub struct WeakPhoneme {
    pub phoneme: String,
    pub language: String,
    pub fsrs_stability: f32,
    pub error_count_7d: u32,
    pub suggested_drill: String,    // "Practice: three, think, through"
}
```

---

## 6. Feature Crate: `feature-language-learning`

New crate at L4, implements `FeaturePackage`.

### 6.1 Tools

| Tool | Actions | Mode |
|------|---------|------|
| `language_practice` | `start_session`, `end_session`, `get_feedback` | A (free conversation) |
| `language_drill` | `generate_exercise`, `evaluate_response`, `next_drill` | B (structured drills) |
| `language_exam` | `start_mock`, `submit_response`, `get_score` | C (exam simulation) |

### 6.2 Storage Tables (via `FeatureMigration`)

```sql
CREATE TABLE phoneme_mastery (
    id TEXT PRIMARY KEY,
    phoneme TEXT NOT NULL,            -- "/θ/", "/ʃ/", "tone3"
    language TEXT NOT NULL,           -- "en", "zh"
    fsrs_stability REAL DEFAULT 0.0,
    fsrs_difficulty REAL DEFAULT 0.5,
    last_reviewed_at TEXT,
    review_count INTEGER DEFAULT 0,
    correct_count INTEGER DEFAULT 0
);
CREATE INDEX idx_phoneme_mastery_lang ON phoneme_mastery(language, phoneme);

CREATE TABLE pronunciation_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    language TEXT NOT NULL,
    overall_score REAL,
    reference_text TEXT,              -- expected (for drills/exam)
    user_text TEXT,                   -- transcribed
    phoneme_scores_json TEXT,         -- JSON array of PhonemeScore
    tone_scores_json TEXT,            -- JSON array of ToneScore (Chinese)
    mode TEXT NOT NULL                -- "conversation", "drill", "exam"
);
CREATE INDEX idx_pronunciation_logs_session ON pronunciation_logs(session_id);

CREATE TABLE exam_attempts (
    id TEXT PRIMARY KEY,
    exam_type TEXT NOT NULL,          -- "ielts_speaking", "hsk3_oral"
    timestamp TEXT NOT NULL,
    scores_json TEXT,                 -- rubric breakdown
    band_estimate REAL,
    duration_secs INTEGER
);
```

### 6.3 Config

```rust
pub struct LanguageLearningConfig {
    pub enabled: bool,
    pub primary_languages: Vec<String>,   // ["en", "zh"]
    pub feedback: FeedbackConfig,
    pub models: VoiceModelConfig,
    pub exam_rubrics: HashMap<String, ExamRubric>,
}

pub struct FeedbackConfig {
    pub default_level: FeedbackLevel,     // Summary
    pub escalation_threshold: f32,        // FSRS stability < 0.3
    pub min_encounters: u32,              // 5 before escalation
    pub aggressiveness: f32,              // 0.0-1.0, tunable by Autotuner
}
```

---

## 7. Skill & Agent Integration

### 7.1 Orchestrator Skill: `language-tutor`

Added to `skills/` alongside general, task-management, etc.

**Keyword triggers:** `practice`, `drill`, `pronunciation`, `IELTS`, `HSK`, `speaking test`, `language`, `英语`, `中文`, `练习`

**Progressive loading:** First activation injects full body (templates, rubrics, phoneme inventory for EN/ZH). Subsequent messages use deduplicated references.

### 7.2 Mode → Execution Pattern Mapping

| Mode | ExecutionRouter | Agent behavior |
|------|----------------|----------------|
| A (Conversation) | Reactive loop | Normal voice + `PronunciationPipeline` on every turn |
| B (Drills) | Reactive loop | Agent generates exercise → user responds → `language_drill.evaluate_response` |
| C (Exam) | Direct + Squad | Examiner persona + timer → `language_exam.get_score` with rubric |

**Squad personas for Exam Mode:**
- Examiner — strict timing, official question sequences
- Native Speaker — natural conversation partner
- Grader — detailed band/score breakdown with rubric citations

### 7.3 FSRS-5 Integration

Each weak phoneme/tone creates a review card in `phoneme_mastery`.

- FSRS schedules reviews: "Practice /θ/ — say 'three, think, through'"
- Cards surface in structured drills (Mode B) as targeted exercises
- Mastery stability feeds back into `FeedbackLevelDecider`
- Extend FSRS to pronunciation atoms (stability decay based on phoneme/tone mastery)

### 7.4 RAG + Memory Integration

- `InsightForge` stores phoneme errors as semantic facts
- `NoteTreeNavigator` retrieves personal context: "You often confuse tone 3 in Health vocabulary"
- LiveContextRefresher injects corrections mid-loop (similar to MemoryPromoted)

### 7.5 Mirror Integration

- Weekly narrative: "Tone accuracy improved from 62% to 84% this week"
- Coaching interventions: "Today focus on the /r/ sound — 4 errors yesterday"
- Autotuner adjusts feedback aggressiveness based on improvement rate

---

## 8. UI Events

| Event | Payload | When |
|-------|---------|------|
| `voice:pronunciation_report` | Full `DetailedPronunciationReport` | After each scored turn |
| `voice:feedback_escalated` | `{ phoneme, from_level, to_level }` | When adaptive feedback escalates |
| `voice:tone_contour` | `{ syllable, f0_points, expected_tone }` | After Chinese speech scored |
| `voice:drill_exercise` | `{ type, prompt, reference_audio }` | When drill generates next exercise |
| `voice:exam_score` | `{ rubric, band, breakdown }` | After mock exam scored |

---

## 9. Phased Rollout

### Phase 1 (ship first): Free Conversation + Pronunciation Core
- Qwen3-TTS + Qwen3-ASR engines (implementing existing traits)
- Qwen3-ForcedAligner + ToneContourAnalyzer
- PronunciationPipeline (4 components)
- Adaptive feedback (Summary default, Overlay on persistent errors)
- FSRS pronunciation cards
- `feature-language-learning` crate with `language_practice` tool
- `language-tutor` orchestrator skill
- Auto-download models on first launch

### Phase 2: Structured Drills
- `language_drill` tool with exercise generation
- Read-aloud, listen-and-repeat, dictation, minimal pairs, shadowing
- UI drill mode + progress tracker
- RAG retrieval for personalized exercises

### Phase 3: Exam Simulation
- `language_exam` tool with mock tests
- IELTS speaking / HSK oral rubrics with band estimates
- Squad mode (Examiner + Native + Grader personas)
- Mirror weekly exam progress reports

### Phase 4 (optional): Advanced Features
- CosyVoice integration (Python sidecar) for emotion/accent control
- Voice cloning for "sound like a specific native speaker" demos
- Advanced visualization (waveform + phoneme + tone overlay)
- Multi-profile support

---

## 10. Performance & Safety

- **Total disk:** ~2.3GB for all Qwen3 models (auto-downloaded once)
- **Peak RAM:** ~2.4GB when all engines active, 0 when idle-unloaded
- **Latency:** TTS <200ms, ASR <100ms TTFT, alignment <500ms (Apple Silicon M-series)
- **Circuit breaker:** Per engine (ASR/TTS/Aligner), reuses existing pattern
- **Alignment timeout:** 3-5s max for long utterances
- **No cloud dependency:** Everything runs locally, zero data leaves the device
- **Graceful degradation:** If Qwen3 model unavailable → Whisper + word-level scoring (current behavior)
