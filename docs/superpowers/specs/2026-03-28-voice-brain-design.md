# Voice Brain: Voice as a Core Modality for Klyntbot's Second Brain

**Date:** 2026-03-28
**Status:** Approved
**Scope:** v1 — Universal voice input + language learning lens + light TTS read-back
**Timeline:** 5 weeks
**Approach:** Approach 2 ("Voice Brain")

## Vision

Voice becomes the default way the second brain hears you — offline-first, instant, deeply integrated with memory, learning, and coaching from the very first capture. Not "add voice commands"; making the agent listen, understand, speak back, and remember how you sounded, while feeding everything into the cognitive pipeline, FSRS spaced repetition, and coaching system.

**Success scenario:** User opens the launcher or taps the menu-bar mic while walking and says "Remember to schedule dentist for next week, and practice 10 French vocab words on the way." Klyntbot transcribes in real time, creates the task, spins up a quick voice practice session, gives pronunciation feedback, and logs everything to memory/FSRS. The user feels like they had a 30-second conversation with an infinitely patient tutor who knows their entire life context.

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| v1 scope | B (universal input) + language learning lens + light C (TTS) | Horizontal layer first, vertical delight on top, data flywheel for v1.5 |
| Architecture | Hybrid: native Rust audio engine + frontend UI/Web Audio TTS | Menu-bar mic must work without WebView; rich orb UI belongs in frontend |
| STT engine | Local-first (whisper-rs) with Groq fallback | Offline promise is core DNA; Groq is zero-cost insurance for first-run |
| STT model | whisper-small (multilingual, ~500MB) default, medium upgrade | Sweet spot: sub-second on M-series, strong multilingual, fast first-run |
| TTS engine | macOS AVSpeechSynthesizer for v1, pluggable trait for v1.5 | Zero download, instant, adequate for short confirmations/nudges |
| TTS playback | Web Audio API in frontend | Pixel-perfect sync with orb animation, works in browser dev mode |
| Crate placement | `crates/voice-engine/` at L1 | First-class sensory modality, reusable by channels + desktop + future MCP |
| Pipeline entry | `InboundMessage` with `kind: Voice` + `VoiceMetadata` in metadata map | Zero changes to AgentRuntime or SkillRouter; voice is richer context |
| Pronunciation | Word-level confidence from Whisper (not phoneme-level) | Effective proxy, upgradeable in v1.5 with force-alignment |

## Architecture

### Layer Placement

```
L0: common, platform-macos (+ AvSpeechEngine impl)
L1: voice-engine (TranscriptionEngine, TtsEngine, AudioCapture, VoiceService, ModelManager)
    depends on: common, platform-macos (for AvSpeech)
L5: channels (can consume TranscriptionEngine for Telegram voice notes — optional v1)
L7: app-core (holds VoiceService), desktop (thin Tauri adapter + commands)
    desktop-ui (Voice Brain orb, settings tab)
```

### New Crate: `crates/voice-engine/` (L1)

Sits alongside `config`, `bus`, `tools-core`. Zero dependencies on higher layers.

#### Core Traits

```rust
// crates/voice-engine/src/stt.rs
#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    /// Stream partial transcripts as audio arrives
    async fn transcribe_stream(&self, audio: AudioStream) -> Result<TranscriptStream>;
    /// Transcribe a complete audio file (for Telegram voice notes, etc.)
    async fn transcribe_file(&self, path: &Path, lang_hint: Option<Language>) -> Result<Transcript>;
}

// crates/voice-engine/src/tts.rs
#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> Result<AudioClip>;
    fn supports_language(&self, lang: &Language) -> bool;
    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo>;
}
```

**Opaque stream types** (implementation details, defined in voice-engine):
- `AudioStream` — `tokio::sync::mpsc::Receiver<AudioChunk>` (16kHz mono f32 chunks from cpal)
- `TranscriptStream` — `tokio::sync::mpsc::Receiver<PartialTranscript>` (partial results as they arrive)
- `TtsParams` — language, voice name, speaking rate, pitch
- `VoiceInfo` — voice identifier, display name, language, gender

#### Key Types

```rust
pub struct Transcript {
    pub text: String,
    pub language: Language,
    pub segments: Vec<TranscriptSegment>,
    pub overall_confidence: f32,
}

pub struct TranscriptSegment {
    pub text: String,
    pub start: Duration,
    pub end: Duration,
    pub confidence: f32,  // 0.0-1.0, powers green/red word highlights
}

pub struct VoiceMetadata {
    pub language: Language,
    pub overall_confidence: f32,
    pub pronunciation_scores: Vec<(String, f32)>,  // (word, score) pairs
    pub audio_ref: Option<String>,                  // path to stored audio
    pub duration: Duration,
    pub engine: EngineKind,                         // Local | Cloud
    pub privacy_mode: PrivacyLevel,                 // Standard | Strict | Off
}

pub struct AudioClip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct PronunciationReport {
    pub overall_score: f32,
    pub word_scores: Vec<WordScore>,
    pub weak_words_count: usize,
    pub improvement_suggestion: Option<String>,
}

pub struct WordScore {
    pub word: String,
    pub confidence: f32,
    pub rating: PronunciationRating,  // Good (>=0.85) | Fair (>=0.60) | Poor
}
```

#### Implementations

**In voice-engine:**
- `WhisperLocalEngine` — wraps `whisper-rs`, Metal-accelerated, handles model loading
- `GroqWhisperEngine` — wraps existing `providers::transcription` adapter (moved here)

**In platform-macos:**
- `AvSpeechEngine` — wraps `AVSpeechSynthesizer` via objc2 bindings

#### Audio Capture

- `AudioCapture` struct wrapping `cpal` — device enumeration, mic selection, streaming chunks
- Emits `AudioChunk` events that feed into `TranscriptionEngine::transcribe_stream`
- Silence detection: 1.5s threshold (configurable 0.5-3.0s)

#### Model Management

- `ModelManager` — handles whisper-small download (async, background), model path resolution, upgrade to medium
- Downloads to `{data_dir}/models/whisper-small.bin`
- Emits progress events for UI (download bar in settings)
- First-run: Groq fallback while download happens, silent switch to local on completion

#### VoiceService (Orchestrator)

```rust
pub struct VoiceService {
    stt: Arc<dyn TranscriptionEngine>,
    tts: Arc<dyn TtsEngine>,
    capture: AudioCapture,
    model_manager: ModelManager,
    config: VoiceConfig,
}
```

AppCore holds `Option<Arc<VoiceService>>` (same pattern as `MirrorFacade`).

**State machine:**
```rust
enum VoiceSessionState {
    Capturing,          // mic active, partials flowing
    Finalizing,         // mic stopped, final whisper pass running
    WaitingForResponse, // transcript sent to agent, awaiting response
    Complete,           // agent responded, TTS played
}
```

## Message Pipeline & Event Flow

### Voice Event Stream

Events emitted by `VoiceService`, consumed by frontend orb via Tauri event channel (same pattern as `agent:content_chunk`):

```rust
#[derive(Serialize, Clone)]
#[serde(tag = "type")]
pub enum VoiceEvent {
    CaptureStarted { session_id: String, engine: EngineKind },
    AudioLevel { rms: f32 },  // ~30fps for waveform
    PartialTranscript {
        text: String,
        segments: Vec<TranscriptSegment>,
        language: Language,
        is_final: bool,
    },
    RoutingSuggestion {
        skill: String,
        confidence: f32,
        label: String,
        trigger_phrase: String,
    },
    MemoryEcho { text: String },  // contextual hint from cognitive memory/mirror
    CaptureEnded { duration: Duration },
    ProcessingInBackground,  // emitted on dismiss during capture/finalizing
    Finalized {
        transcript: Transcript,
        metadata: VoiceMetadata,
        routed_to: String,
        response_preview: String,
    },
    SpeakResponse {
        audio_base64: String,
        sample_rate: u32,
        text: String,
    },
    Error { message: String, recoverable: bool },
}
```

### Partial Transcript Timing (<300ms to routing chips)

```
Time 0ms:     Hotkey pressed → AudioCapture::start() → CaptureStarted → orb appears
Time 50ms:    First cpal audio chunk (20ms buffer @ 16kHz) → AudioLevel events
Time 200ms:   whisper-rs first partial → PartialTranscript { text: "Remember to" }
Time 400ms:   Second partial → VoiceRouter fires → RoutingSuggestion { "→ Task" }
Time 800ms:   Compound intent detected → second RoutingSuggestion { "→ French drill" }
Time ~500ms:  Memory echo prefetch fires → MemoryEcho (if relevant fact found)
Time 2s:      Silence detected → CaptureEnded → final whisper pass → Finalized
```

**VoiceRouter** runs existing `SkillRouter::keyword_scores()` on each partial (pure keyword matching, <1ms). Fires `RoutingSuggestion` when a skill crosses the 0.4 threshold.

### Two-Phase Dismiss (No Race Conditions)

The orb is a **reactive view** — `VoiceService` is the single owner of pipeline state.

**On dismiss (tap or "thanks Klyntbot"):**
1. Frontend sends `voice:dismiss` command
2. `VoiceService` transitions state:
   - If `Capturing`: stops mic → `Finalizing` → emits `ProcessingInBackground`
   - If `Finalizing` or `WaitingForResponse`: emits `ProcessingInBackground`
3. Orb closes immediately, shows transient toast: "Processing... (will speak response in background)" with "Cancel & discard" button
4. Pipeline continues in background → `InboundMessage` always created from final transcript
5. TTS response plays as transient audio-only notification (no UI needed)

### Multi-Intent Handling

When `VoiceRouter` detects 2+ high-confidence skills during partials:
- Orb shows both routing chips with a faint divider
- "Split into two turns?" pill button appears above chips
- Tap "Split" → two sequential `InboundMessage`s (default)
- Tap a single chip → only that intent is sent
- No tap in 800ms → auto-split (conservative default)

Split messages reuse existing multi-turn session behavior — no new orchestration.

### Pipeline Entry

```rust
// New variant in MessageKind (crates/bus/src/events.rs)
pub enum MessageKind {
    Text,
    Reaction,
    Voice,  // NEW
}

// VoiceService creates after finalization
let message = InboundMessage {
    channel: ChannelName::Desktop,
    sender_id: "local".into(),
    chat_id: ChatId::desktop(),
    content: transcript.text.clone(),
    media: vec![],
    kind: MessageKind::Voice,
    metadata: {
        let mut m = HashMap::new();
        m.insert("voice_metadata".into(), serde_json::to_string(&metadata)?);
        m
    },
};
```

AgentRuntime and SkillRouter see this as a normal message. Downstream consumers check `kind == Voice` and deserialize `VoiceMetadata` for enriched processing.

## Voice Brain Orb UI

### Window Spec

- **Type:** Borderless, always-on-top Tauri WebView window (same technique as distraction overlay)
- **Size:** 320x200px (compact, non-intrusive, with pin-to-edge snap option)
- **Position:** Top-center of active monitor, 80px from top
- **Style:** Rounded corners (16px), `glass-panel` backdrop blur
- **Behavior:** No title bar, no resize, click-through on transparent areas
- **Animation in:** scale 0.9->1.0 + fade, 200ms ease-out
- **Animation out:** scale 1.0->0.95 + fade, 150ms ease-in

### Three States

**State 1: Listening**
- Live waveform bar (animated by `AudioLevel.rms` at ~30fps)
- Scrolling transcript with word-level confidence highlights (green >0.85, amber 0.60-0.85, red <0.60)
- Routing chips appear as `RoutingSuggestion` events arrive (glass-panel pills with skill icon + label)
- Multi-intent pill when applicable: "Split into two turns?" with tap targets
- Memory echo line (faded, below transcript): contextual hint from cognitive memory or Mirror layer
- Cloud mode badge (small cloud icon next to mic indicator) when using Groq fallback
- Privacy mode indicator
- Hint bar: "cmd+shift+V to finish . tap to close"

**State 2: Processing**
- Pulsing dot animation replacing waveform
- Final transcript (static) with highlights
- Routing chips show progress (checkmarks as each intent is handled)
- "Cancel & discard" escape hatch
- Background processing toast if orb was dismissed

**State 3: Response**
- TTS playback waveform (pulsing in sync with spoken audio via Web Audio API)
- Agent response text synced with audio playback
- Session summary chips (e.g., "2 tasks", "8 cards", "+12%")
- "tap anywhere to close"
- Auto-dismiss after TTS completes + 2s

### Menu Bar Integration

- **Mic icon in system tray** (Tauri tray API): outline mic (idle), filled red + pulse (listening), spinner (processing)
- Tap behavior: toggles capture on/off (same as hotkey)
- **Tray title coordination:** `VOICE_ACTIVE: AtomicBool` — voice capture temporarily shows "Listening..." in tray; yields to focus timer if both active (focus timer takes priority via existing `FOCUS_ACTIVE` flag)

### Global Hotkey

`Cmd+Shift+V` via `tauri-plugin-global-shortcut`:
- **First press:** start capture, orb appears
- **Second press (while capturing):** stop capture, finalize
- **Hold 500ms:** push-to-talk mode, release stops capture

### Desktop Tauri Commands

New module: `crates/desktop/src/commands/voice.rs`

```rust
pub const DEV_COMMANDS: &[&str] = &[
    "voice_start_capture",
    "voice_stop_capture",
    "voice_dismiss",
    "voice_get_status",
    "voice_set_config",
    "voice_get_models",
    "voice_download_model",
];
```

All delegate to `AppCore` methods (thin adapter pattern, same as all other command modules).

### Settings Tab: Voice

```
Voice Input
  |- Enable voice capture          [toggle, default ON]
  |- Global hotkey                 [Cmd+Shift+V] (editable)
  |- Silence detection threshold   [1.5s] (slider, 0.5-3.0s)
  |- Privacy mode                  [Standard v] (Standard / Strict / Off)
  |- Transcription engine
      |- * Local (whisper-small)   [Downloaded check] / [Download 500MB]
      |- o Local (whisper-medium)  [Upgrade -- better for languages]
      |- o Cloud (Groq)            [Instant, requires API key]

Voice Output
  |- Enable spoken responses       [toggle, default ON]
  |- Voice                         [Samantha (English) v] per-language selector
  |- Speaking rate                  [1.0x] (slider, 0.5-2.0x)
  |- Speak during focus sessions   [toggle, default OFF]

Language Learning
  |- Target language               [French v]
  |- Show pronunciation scores     [toggle, default ON]
  |- Auto-create spoken flashcards [toggle, default ON]
```

### First-Run Flow

1. User enables voice or taps menu-bar mic for the first time
2. macOS mic permission dialog (standard system prompt)
3. If Groq API key configured: voice works immediately via cloud engine
4. Background download of whisper-small starts (progress in settings)
5. Download completes: silent switch to local, subtle "Now using local voice -- fully offline" toast
6. If no Groq key: "Downloading voice model (500MB)..." progress, then enable

No onboarding wizard. Voice just works the moment the user taps the mic.

## Language Learning Integration

### Pronunciation Scoring (Word-Level, v1)

```rust
pub fn compute_pronunciation_scores(
    transcript: &Transcript,
    target_language: &Language,
) -> PronunciationReport {
    let word_scores: Vec<WordScore> = transcript.segments.iter().map(|seg| {
        WordScore {
            word: seg.text.clone(),
            confidence: seg.confidence,
            rating: match seg.confidence {
                c if c >= 0.85 => PronunciationRating::Good,
                c if c >= 0.60 => PronunciationRating::Fair,
                _              => PronunciationRating::Poor,
            },
        }
    }).collect();

    PronunciationReport {
        overall_score: word_scores.iter().map(|w| w.confidence).sum::<f32>()
            / word_scores.len().max(1) as f32,
        word_scores,
        weak_words_count: word_scores.iter().filter(|w| w.rating == PronunciationRating::Poor).count(),
        improvement_suggestion: if weak_words.is_empty() {
            None
        } else {
            Some(format!("Focus on: {}",
                weak_words.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(", ")))
        },
    }
}
```

Whisper confidence is a proxy for pronunciation quality — it struggles to decode poorly pronounced non-native speech, producing low confidence scores. Effective for catching mispronounced words. Phoneme-level analysis is v1.5 territory.

### FSRS Dual-Signal (Self-Rating + Pronunciation Score)

**Schema changes (minimal):**
```sql
ALTER TABLE flashcards ADD COLUMN audio_ref TEXT;
ALTER TABLE flashcards ADD COLUMN pronunciation_baseline REAL;
ALTER TABLE flashcards ADD COLUMN last_pronunciation_score REAL;
```

**Voice-enriched review session:**
1. Card shows front (word/phrase in target language)
2. TTS plays reference pronunciation via `TtsEngine`
3. User speaks the word -> `VoiceService` captures + scores
4. Score compared against `pronunciation_baseline`:
   - Improved >=10%: bump FSRS rating by 1 (e.g., Hard -> Good)
   - Declined >=15%: drop FSRS rating by 1
   - Otherwise: normal self-assessment rating
5. `last_pronunciation_score` updated for trend tracking

Over time, cards with consistently low pronunciation scores surface more often. The system naturally drills weak sounds without explicit configuration.

### Card Creation From Voice

When skill router detects learning intent from a voice capture:
- `CardGenerator` receives enriched context including `VoiceMetadata`
- Generated card includes `audio_ref` (path to original spoken audio) and `pronunciation_baseline`
- Card type: `vocabulary` (existing type, no new type needed)

## Cognitive Pipeline Integration

### Salience Scoring

Voice captures always get `Extract` salience — spoken reflection tends to be more emotionally loaded than typed text, and the pronunciation metadata enriches downstream extraction:

```rust
DomainEvent::ChatTurnCompleted { kind: MessageKind::Voice, .. } => {
    SalienceVerdict::Extract  // always extract voice, never accumulate
}
```

### Episodic Memory Extraction

Voice-specific context provided to the LLM extractor:

```rust
let extraction_context = ExtractionContext {
    content: transcript.text.clone(),
    additional_context: Some(format!(
        "This was a spoken reflection in {}. Pronunciation confidence: {:.0}%. {}",
        metadata.language,
        metadata.overall_confidence * 100.0,
        metadata.improvement_suggestion.unwrap_or_default(),
    )),
    source: MemorySource::Voice,  // NEW variant
};
```

The extractor can produce richer facts like "user is practicing French greetings" or "user's pronunciation of 'bonjour' needs work" — feeding into mirror narrative and coaching.

### Memory Echo (Proactive, Mirror-Powered)

Triggered ~500ms after first `PartialTranscript`:

```rust
async fn prefetch_memory_echo(&self, partial_text: &str) -> Option<String> {
    // Try Mirror layer first for meta-insights
    if let Some(mirror_snippet) = self.mirror_facade
        .get_recent_voice_relevant_snippet(partial_text).await {
        return Some(format!("Mirror noticed: {}", mirror_snippet));
    }
    // Fallback to standard conversation recall
    let recall = self.context_engine
        .recall_relevant(partial_text, RecallParams {
            max_results: 1,
            max_tokens: 50,
            recency_boost: true,
        })
        .await.ok()?;
    recall.first().map(|fact| fact.content_preview.clone())
}
```

Examples: "Last week your French consistency dropped on Tuesdays", "You improved 'bonjour' from 67% to 91% since last month."

Emitted as `VoiceEvent::MemoryEcho`. Privacy mode `Strict` skips mirror lookup.

### Coaching Integration

New intervention type:

```rust
pub enum InterventionType {
    Notification,
    ChatMessage,
    OverlayPrompt,
    SpokenNudge,  // NEW - delivered via TtsEngine through the orb
}
```

**Triggers from voice data:**
- `SignalAccumulator` listens for `DomainEvent::VoiceCapture` events
- Pattern detector identifies: pronunciation regression, practice consistency, language mixing
- Coaching decisions surface as spoken nudges: "Your 'r' sound was 92% last week but 78% today -- want a 30-second drill?"
- Delivered via orb if open, system notification + TTS if closed

### Voice Journal Integration

Extends existing `VoiceJournalProcessor`:

```rust
let journal_entry = VoiceJournalEntry {
    recorded_at: Utc::now(),
    duration_secs: metadata.duration.as_secs() as i32,
    transcript: transcript.text.clone(),
    extracted_facts: serde_json::to_string(&extracted_facts)?,
    sentiment: sentiment_from_transcript(&transcript),
    audio_ref: metadata.audio_ref.clone(),  // NEW: enables playback
};
```

## Testing Strategy

### Unit Tests (in `crates/voice-engine/`)

- `TranscriptionEngine` trait tests with mock audio data
- `PronunciationReport` computation (boundary cases: empty, all-high/low, single word)
- `VoiceRouter::check_routing()` on partial text at various lengths
- `ModelManager` state machine (pending -> downloading -> ready -> upgrading)
- `AudioCapture` device enumeration mock, chunk sizing, silence detection
- `VoiceSessionState` transitions (all valid transitions, dismiss during each state)

### Integration Tests (in `crates/voice-engine/` and `app-core/`)

- `VoiceService` end-to-end with mock engine: capture -> partials -> finalize -> `InboundMessage`
- Multi-intent detection and split: compound utterance -> two sequential messages
- Dismiss during each state: verify pipeline completes, message still produced
- Groq fallback: mock local engine failure -> transparent switch
- `VoiceMetadata` through cognitive pipeline: mock extraction receives pronunciation data
- FSRS dual-signal: pronunciation score adjusts rating correctly
- Memory echo prefetch: relevant fact -> `MemoryEcho` emitted; no fact -> no event
- Privacy mode: `Strict` skips mirror lookup, `Standard` includes it

### Frontend Tests (Vitest)

- Orb state machine (all transitions)
- `VoiceEvent` stream rendering (partials, chips, highlights)
- Word-level CSS classes (green/amber/red thresholds)
- Multi-intent pill behavior
- Memory echo rendering
- Settings panel interactions

### Manual Testing Protocol

**No audio hardware in CI.** All tests use mock `AudioCapture` with pre-recorded chunks.

**Browser dev mode** (`localhost:1420` + dev server `:3456`):
- Orb UI development via mock endpoints
- `POST /api/voice_simulate_event { event: VoiceEvent }` — simulate individual events
- `POST /api/voice_mock_session { text, language, duration_ms }` — simulate full session
- TTS playback via Web Audio API

**Desktop** (`cargo tauri dev`):
- Real mic capture, live transcription, orb display
- Global hotkey, menu-bar icon, push-to-talk
- Model download flow (Groq -> background download -> local switch)
- VOICE_ACTIVE / FOCUS_ACTIVE flag interaction

## Success Metrics

| Metric | Target | How Measured |
|--------|--------|-------------|
| Voice capture adoption | >40% of new captures via voice by week 4 | `InboundMessage` count where `kind: Voice` / total |
| Capture-to-routing latency | <400ms for first routing chip | Timestamp: `CaptureStarted` -> first `RoutingSuggestion` |
| Transcription accuracy | >90% word-level English, >80% target language | Sampled manual review |
| Pronunciation score correlation | Tracks with manual assessment on 20-phrase test set | Periodic human evaluation |
| FSRS dual-signal impact | Cards with pronunciation data >=5% better retention at 30 days | Retention query on cards with/without `pronunciation_baseline` |
| TTS response latency | <300ms from `Finalized` to first audio byte | Frontend performance metric |
| Session completion rate | >80% of sessions reach `Finalized` (not cancelled) | `VoiceSessionState` transition logs |
| Second-brain delight score | >=4.5/5 | Post-capture micro-survey (thumbs up/down + optional "why?") in orb Response state |

## Rollout Plan (5 Weeks)

### Week 1: Foundation
- `crates/voice-engine/` scaffolding: traits, types, `VoiceConfig`, `ModelManager`
- `WhisperLocalEngine` impl (whisper-rs + Metal)
- Move existing Groq transcription into `GroqWhisperEngine` impl
- `AudioCapture` with cpal + silence detection
- Unit tests for all of the above

### Week 2: Pipeline
- `VoiceService` orchestrator: state machine, partial transcript streaming, routing
- `VoiceEvent` emission via Tauri events
- `InboundMessage` creation with `kind: Voice` + `VoiceMetadata`
- Multi-intent detection and split
- Desktop commands (`voice.rs`) + `DEV_COMMANDS`
- Integration tests: capture -> pipeline -> message bus

### Week 3: Orb UI
- Voice Brain orb window (Tauri borderless, glass-panel, monitor-aware)
- Three-state UI: Listening, Processing, Response
- Global hotkey (Cmd+Shift+V) + menu-bar mic icon + push-to-talk
- Dev server mock endpoints for browser iteration
- Frontend tests (Vitest)

### Week 4: Learning Lens + Cognitive Integration
- Pronunciation scoring + green/red word highlights in orb
- FSRS dual-signal (pronunciation score -> rating adjustment)
- Flashcard `audio_ref` + `pronunciation_baseline` columns
- Voice-enriched review session (TTS plays reference, user speaks, score compared)
- Cognitive pipeline: salience boost + extraction context + `MemorySource::Voice`
- Memory echo prefetch + Mirror-powered proactive echo
- Coaching `SpokenNudge` intervention type
- Voice journal `audio_ref` storage

### Week 5: Polish + Dogfood
- Settings tab (Voice Input / Voice Output / Language Learning)
- First-run flow (mic permission -> Groq instant -> background download -> local switch)
- `VOICE_ACTIVE` flag + focus timer coordination
- Privacy mode enforcement across pipeline
- `AvSpeechEngine` impl in platform-macos
- **Voice Dogfood Week:** All internal users live with the orb as primary input for 7 days
- Fix frictions surfaced during dogfood (memory echo tuning, coaching timing, etc.)
- Performance tuning (partial transcript latency, TTS sync)
- Delight score micro-survey wiring

## v1.5 Roadmap (Post-Data Flywheel)

These features become additive once v1 usage data validates the voice modality:

- **Full Voice Practice Mode (Approach 3 territory):** Real-time conversational drills with turn-based back-and-forth
- **Advanced TTS:** Swap `AvSpeechEngine` for Qwen3-TTS (MLX-Audio) or NeuTTS Air (GGUF) via existing `TtsEngine` trait
- **Voice cloning:** 3-10s user recording -> agent speaks in user's voice (or native tutor clone)
- **Phoneme-level pronunciation:** Force-alignment library, red/green waveform per phoneme
- **Audio flashcard type:** Front is spoken prompt, back is audio + text, review by speaking
- **Accent drift tracking:** Aggregate pronunciation scores by phoneme over weeks, coaching surfaces weak sounds
- **Wake-word activation:** "Hey Klyntbot" without hotkey (always-listening with explicit consent)
- **Native TTS fallback:** `rodio` playback for spoken nudges when orb is closed (background coaching)
- **Cross-channel voice:** Telegram voice notes use local whisper-rs instead of Groq
- **MCP voice streaming:** External AI clients can stream voice through MCP transport
