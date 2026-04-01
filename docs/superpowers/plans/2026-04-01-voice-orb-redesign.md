# Voice Orb Redesign & TTS Personas — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the chat-style voice orb with a minimal WebGL ambient presence, add TTS voice persona customization, and constrain ASR to en/zh/vi only.

**Architecture:** Config-driven voice personas (preset + custom) flowing through extended `TtsParams` to a branching `Qwen3TtsEngine` (`generate_with_params` vs `generate_with_instruct`). New `VoiceOrbCanvas.tsx` replaces the 388-line chat UI with a ~150-line WebGL shader component. ASR constrained via `TranscribeOptions.language` whitelist.

**Tech Stack:** Rust, TypeScript/React, WebGL2 GLSL, Tauri 2, `qwen3-tts-rs` (MLX backend), `qwen3-asr`.

---

## File Structure

### Config Layer (L1)
- Modify: `crates/config/src/schema/voice.rs` — add `VoicePersona` enum, `default_persona`/`personas` to `VoiceOutputConfig`, `allowed_languages` to `VoiceInputConfig`

### Voice Engine Layer (L5)
- Modify: `crates/voice-engine/src/types.rs` — extend `TtsParams` with `instruct` and `temperature` fields
- Modify: `crates/voice-engine/src/engines/qwen3_tts.rs` — branch on `instruct` to call `generate_with_instruct`; pass `temperature` from params
- Modify: `crates/voice-engine/src/engines/qwen3_asr.rs` — pass language hint from allowed_languages to `TranscribeOptions`
- Modify: `crates/voice-engine/src/model_manager.rs` — add `Qwen3Model::TtsInstruct` variant for 1.7B model
- Modify: `crates/voice-engine/src/service.rs` — pass language hint to ASR transcription

### App Core Layer (L7)
- Modify: `crates/app-core/src/handlers/voice_conversation.rs` — read persona from config, build extended `TtsParams`
- Modify: `crates/app-core/src/init/mod.rs` — support 1.7B model detection in TTS init

### Desktop Layer (L7)
- Modify: `crates/desktop/tauri.conf.json` — voice-orb window: 200×200, position config
- Modify: `crates/desktop/src/main.rs` — bottom-right positioning for orb window

### Frontend
- Replace: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` — strip to minimal wrapper rendering `VoiceOrbCanvas`
- Create: `desktop-ui/src/features/voice/components/VoiceOrbCanvas.tsx` — WebGL2 procedural shader orb
- Modify: `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx` — remove auto-resize, simplify

---

## Task 1: Config — Voice Personas & ASR Language Whitelist

Add the `VoicePersona` enum, extend `VoiceOutputConfig` with `default_persona` and `personas`, and add `allowed_languages` to `VoiceInputConfig`.

**Files:**
- Modify: `crates/config/src/schema/voice.rs`

- [ ] **Step 1: Write tests for new config types**

Add to the existing `#[cfg(test)] mod tests` in `crates/config/src/schema/voice.rs`:

```rust
    #[test]
    fn default_persona_is_neutral() {
        let config = VoiceOutputConfig::default();
        assert_eq!(config.default_persona, "neutral");
        assert!(!config.personas.is_empty());
        assert!(config.personas.contains_key("neutral"));
    }

    #[test]
    fn deserialize_preset_persona() {
        let json = r#"{"type": "preset", "speaker": "onyx", "speed": 0.95, "temperature": 0.8}"#;
        let persona: VoicePersona = serde_json::from_str(json).unwrap();
        match persona {
            VoicePersona::Preset { speaker, speed, .. } => {
                assert_eq!(speaker, "onyx");
                assert!((speed - 0.95).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Preset"),
        }
    }

    #[test]
    fn deserialize_custom_persona() {
        let json = r#"{"type": "custom", "description": "deep calm voice", "speed": 0.9, "temperature": 0.7}"#;
        let persona: VoicePersona = serde_json::from_str(json).unwrap();
        match persona {
            VoicePersona::Custom { description, .. } => {
                assert_eq!(description, "deep calm voice");
            }
            _ => panic!("Expected Custom"),
        }
    }

    #[test]
    fn default_allowed_languages() {
        let config = VoiceInputConfig::default();
        assert_eq!(config.allowed_languages, vec!["en", "zh", "vi"]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p config -E 'test(default_persona_is_neutral) | test(deserialize_preset_persona) | test(deserialize_custom_persona) | test(default_allowed_languages)'`

Expected: FAIL — fields don't exist yet.

- [ ] **Step 3: Add `VoicePersona` enum and update `VoiceOutputConfig`**

In `crates/config/src/schema/voice.rs`, add the enum above `VoiceOutputConfig` (after line 111):

```rust
/// A named voice persona for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoicePersona {
    Preset {
        /// Speaker identifier from Qwen3 voices (alloy, echo, fable, onyx, nova, shimmer).
        speaker: String,
        /// Speaking rate multiplier (0.5–2.0).
        #[serde(default = "default_speaking_rate")]
        speed: f32,
        /// Generation temperature (0.1–1.0). Lower = more stable.
        #[serde(default = "default_temperature")]
        temperature: f32,
    },
    Custom {
        /// Natural language voice description (e.g., "deep, calm male voice with formal tone").
        description: String,
        /// Speaking rate multiplier (0.5–2.0).
        #[serde(default = "default_speaking_rate")]
        speed: f32,
        /// Generation temperature (0.1–1.0).
        #[serde(default = "default_temperature")]
        temperature: f32,
    },
}

fn default_temperature() -> f32 {
    0.9
}

fn default_personas() -> std::collections::HashMap<String, VoicePersona> {
    let mut m = std::collections::HashMap::new();
    m.insert("professional".into(), VoicePersona::Preset {
        speaker: "onyx".into(), speed: 0.95, temperature: 0.8,
    });
    m.insert("friendly".into(), VoicePersona::Preset {
        speaker: "nova".into(), speed: 1.0, temperature: 0.9,
    });
    m.insert("calm".into(), VoicePersona::Preset {
        speaker: "shimmer".into(), speed: 0.9, temperature: 0.7,
    });
    m.insert("energetic".into(), VoicePersona::Preset {
        speaker: "echo".into(), speed: 1.1, temperature: 0.95,
    });
    m.insert("neutral".into(), VoicePersona::Preset {
        speaker: "alloy".into(), speed: 1.0, temperature: 0.85,
    });
    m.insert("storyteller".into(), VoicePersona::Preset {
        speaker: "fable".into(), speed: 0.92, temperature: 0.8,
    });
    m
}

fn default_persona_name() -> String {
    "neutral".into()
}
```

Then update `VoiceOutputConfig` (replace lines 112–142):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceOutputConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub voice_preferences: std::collections::HashMap<String, String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
    #[serde(default)]
    pub speak_during_focus: bool,
    /// TTS engine to use.
    #[serde(default)]
    pub tts_engine: TtsEngineKind,
    /// Deployment mode: local model or cloud API.
    #[serde(default)]
    pub deployment: EngineDeployment,
    /// Active voice persona key.
    #[serde(default = "default_persona_name")]
    pub default_persona: String,
    /// Named voice persona configurations.
    #[serde(default = "default_personas")]
    pub personas: std::collections::HashMap<String, VoicePersona>,
}

impl Default for VoiceOutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            voice_preferences: std::collections::HashMap::new(),
            speaking_rate: 1.0,
            speak_during_focus: false,
            tts_engine: TtsEngineKind::default(),
            deployment: EngineDeployment::default(),
            default_persona: default_persona_name(),
            personas: default_personas(),
        }
    }
}
```

- [ ] **Step 4: Add `allowed_languages` to `VoiceInputConfig`**

In `crates/config/src/schema/voice.rs`, add a helper function near the other defaults:

```rust
fn default_allowed_languages() -> Vec<String> {
    vec!["en".into(), "zh".into(), "vi".into()]
}
```

Add the field to `VoiceInputConfig` (after line 95, before the closing `}`):

```rust
    /// Restrict ASR language detection to these languages only.
    /// Prevents mispronunciation from triggering wrong-language transcripts.
    #[serde(default = "default_allowed_languages")]
    pub allowed_languages: Vec<String>,
```

Update the `Default` impl for `VoiceInputConfig` to include the new field (add before closing `}`):

```rust
            allowed_languages: default_allowed_languages(),
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(default_persona_is_neutral) | test(deserialize_preset_persona) | test(deserialize_custom_persona) | test(default_allowed_languages)'`

Expected: All PASS.

- [ ] **Step 6: Run full config crate tests**

Run: `cargo nextest run -p config`

Expected: All tests pass, no regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/config/src/schema/voice.rs
git commit -m "feat(config): add VoicePersona enum, default personas, and ASR language whitelist"
```

---

## Task 2: TtsParams — Extend with Instruct & Temperature

Add `instruct: Option<String>` and `temperature: Option<f32>` to `TtsParams` so the TTS engine can branch between preset and custom voice synthesis.

**Files:**
- Modify: `crates/voice-engine/src/types.rs`

- [ ] **Step 1: Write tests for extended TtsParams**

Add to the existing `crates/voice-engine/src/types.rs` (inside or after existing tests, or create a new test module at the bottom):

```rust
#[cfg(test)]
mod tts_params_tests {
    use super::*;

    #[test]
    fn default_tts_params_has_no_instruct() {
        let params = TtsParams::default();
        assert!(params.instruct.is_none());
        assert!(params.temperature.is_none());
        assert!((params.speaking_rate - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tts_params_with_instruct() {
        let params = TtsParams {
            instruct: Some("deep calm male voice".into()),
            temperature: Some(0.75),
            ..Default::default()
        };
        assert_eq!(params.instruct.as_deref(), Some("deep calm male voice"));
        assert_eq!(params.temperature, Some(0.75));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p voice-engine -E 'test(default_tts_params_has_no_instruct) | test(tts_params_with_instruct)'`

Expected: FAIL — fields don't exist yet.

- [ ] **Step 3: Add new fields to TtsParams**

In `crates/voice-engine/src/types.rs`, update the `TtsParams` struct (currently at lines 114–121):

```rust
/// Parameters for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsParams {
    pub language: Language,
    pub voice_name: Option<String>,
    #[serde(default = "default_speaking_rate")]
    pub speaking_rate: f32,
    /// Natural language voice description for instruct-mode TTS (1.7B model).
    /// When set, the engine uses `generate_with_instruct` instead of `generate_with_params`.
    #[serde(default)]
    pub instruct: Option<String>,
    /// Override generation temperature (0.1–1.0). None uses the engine default (0.9).
    #[serde(default)]
    pub temperature: Option<f32>,
}
```

Update the `Default` impl (currently at lines 127–134):

```rust
impl Default for TtsParams {
    fn default() -> Self {
        Self {
            language: Language::default(),
            voice_name: None,
            speaking_rate: default_speaking_rate(),
            instruct: None,
            temperature: None,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine -E 'test(default_tts_params_has_no_instruct) | test(tts_params_with_instruct)'`

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/types.rs
git commit -m "feat(voice): extend TtsParams with instruct and temperature fields"
```

---

## Task 3: Qwen3TtsEngine — Instruct Branch & Temperature Passthrough

Update the engine to use `generate_with_instruct` when `params.instruct` is set and the 1.7B model is available, and pass `temperature` from params instead of hardcoding 0.9.

**Files:**
- Modify: `crates/voice-engine/src/engines/qwen3_tts.rs`

- [ ] **Step 1: Update `synthesize()` to read temperature and instruct**

In `crates/voice-engine/src/engines/qwen3_tts.rs`, update the `#[cfg(feature = "qwen3")]` block inside `synthesize()` (currently lines 134–207). Replace the block starting at line 134:

```rust
        #[cfg(feature = "qwen3")]
        {
            let voice = params.voice_name.as_deref().unwrap_or("alloy");
            let lang = map_language(&params.language);
            let temperature = params.temperature.unwrap_or(0.9) as f64;
            let instruct = params.instruct.clone();

            debug!(
                "Qwen3-TTS synthesizing '{}' ({} chars, voice={}, lang={}, instruct={})",
                &text[..text.len().min(50)],
                text.len(),
                voice,
                lang,
                instruct.is_some()
            );

            let text_owned = text.to_string();
            let voice_owned = voice.to_string();
            let state = self.state.clone();
            let model_dir = self.model_dir.clone();

            let result = tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
                let mut guard = state.lock().unwrap();
                if guard.model.is_none() {
                    info!("Lazy-loading Qwen3-TTS from {}...", model_dir.display());
                    let start = Instant::now();
                    let model = Qwen3TtsEngine::load_model(&model_dir)?;
                    info!("Qwen3-TTS loaded in {:.1}s", start.elapsed().as_secs_f32());
                    guard.model = Some(model);
                }
                guard.last_used = Instant::now();
                let model = guard.model.as_ref().ok_or("Model not loaded")?;

                let chunks = qwen3_tts_rs::api::chunking::chunk_text(&text_owned, MAX_CHUNK_CHARS);
                let mut all_samples = Vec::new();

                for (i, chunk) in chunks.iter().enumerate() {
                    debug!(
                        "Qwen3-TTS chunk {}/{}: '{}' ({} chars)",
                        i + 1,
                        chunks.len(),
                        &chunk[..chunk.len().min(40)],
                        chunk.len()
                    );

                    let (samples, _sr) = if let Some(ref desc) = instruct {
                        model
                            .generate_with_instruct(
                                chunk, &voice_owned, &lang, desc,
                                temperature, 50, 2048,
                            )
                            .map_err(|e| format!("Qwen3-TTS instruct failed: {e}"))?
                    } else {
                        model
                            .generate_with_params(
                                chunk, &voice_owned, &lang,
                                temperature, 50, 2048,
                            )
                            .map_err(|e| format!("Qwen3-TTS generation failed: {e}"))?
                    };
                    all_samples.extend(samples);
                }

                Ok(all_samples)
            })
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Qwen3-TTS task failed: {e}"
                )))
            })?;

            match result {
                Ok(samples) => {
                    debug!("Qwen3-TTS produced {} samples", samples.len());
                    Ok(AudioClip {
                        samples,
                        sample_rate: QWEN3_TTS_SAMPLE_RATE,
                        channels: 1,
                    })
                }
                Err(e) => {
                    warn!("Qwen3-TTS inference error: {e}");
                    Err(common::KlyntbotError::Provider(
                        common::ProviderError::InvalidResponse(e),
                    ))
                }
            }
        }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p voice-engine --features qwen3`

Expected: Clean compile (warnings only from pre-existing code).

- [ ] **Step 3: Run all voice-engine tests**

Run: `cargo nextest run -p voice-engine`

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/voice-engine/src/engines/qwen3_tts.rs
git commit -m "feat(voice): branch TTS on instruct param, pass temperature from TtsParams"
```

---

## Task 4: ModelManager — Add 1.7B TTS Instruct Variant

Add `Qwen3Model::TtsInstruct` for the optional 1.7B CustomVoice model download.

**Files:**
- Modify: `crates/voice-engine/src/model_manager.rs`

- [ ] **Step 1: Write tests**

Add to the existing test module in `crates/voice-engine/src/model_manager.rs`:

```rust
    #[test]
    fn tts_instruct_model_metadata() {
        assert_eq!(Qwen3Model::TtsInstruct.dir_name(), "qwen3-tts-1.7b-instruct");
        assert_eq!(
            Qwen3Model::TtsInstruct.repo_id(),
            "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice"
        );
    }

    #[test]
    fn tts_instruct_model_dir_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.qwen3_tts_instruct_model_dir().is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p voice-engine -E 'test(tts_instruct_model)'`

Expected: FAIL — variant and method don't exist.

- [ ] **Step 3: Add `TtsInstruct` variant**

In `crates/voice-engine/src/model_manager.rs`, update the `Qwen3Model` enum (currently lines 24–27):

```rust
#[derive(Debug, Clone, Copy)]
pub enum Qwen3Model {
    Tts,
    TtsInstruct,
    Asr,
}
```

Update the three methods on `Qwen3Model` (currently lines 29–49):

```rust
impl Qwen3Model {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Tts => "qwen3-tts-0.6b",
            Self::TtsInstruct => "qwen3-tts-1.7b-instruct",
            Self::Asr => "qwen3-asr-0.6b",
        }
    }

    pub fn repo_id(self) -> &'static str {
        match self {
            Self::Tts => "Qwen/Qwen3-TTS-12Hz-0.6B-Base",
            Self::TtsInstruct => "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice",
            Self::Asr => "Qwen/Qwen3-ASR-0.6B",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Tts => "Qwen3-TTS-12Hz-0.6B",
            Self::TtsInstruct => "Qwen3-TTS-12Hz-1.7B-CustomVoice",
            Self::Asr => "Qwen3-ASR-0.6B",
        }
    }
}
```

Add a public accessor to `ModelManager` (after `qwen3_asr_model_dir` at line 84):

```rust
    /// Check if Qwen3-TTS 1.7B instruct model exists.
    pub fn qwen3_tts_instruct_model_dir(&self) -> Option<PathBuf> {
        self.model_dir(Qwen3Model::TtsInstruct)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p voice-engine -E 'test(tts_instruct_model)'`

Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/voice-engine/src/model_manager.rs
git commit -m "feat(voice): add Qwen3Model::TtsInstruct for 1.7B CustomVoice model"
```

---

## Task 5: ASR Language Whitelist

Pass the `allowed_languages` from config through to `TranscribeOptions` so Qwen3-ASR only considers en/zh/vi.

**Files:**
- Modify: `crates/voice-engine/src/engines/qwen3_asr.rs`
- Modify: `crates/voice-engine/src/stt.rs` (the `TranscriptionEngine` trait — check if `transcribe_stream` needs a language hint param)
- Modify: `crates/voice-engine/src/service.rs`

- [ ] **Step 1: Check the `TranscriptionEngine` trait**

Read `crates/voice-engine/src/stt.rs` to understand the current `transcribe_stream` signature. The `language` hint needs to reach the ASR engine. Two options: (a) add a param to `transcribe_stream`, or (b) configure it on the engine at construction time. Option (b) is simpler and doesn't break the trait.

- [ ] **Step 2: Add `allowed_languages` to `Qwen3AsrEngine`**

In `crates/voice-engine/src/engines/qwen3_asr.rs`, add a field to the struct (after line 29):

```rust
pub struct Qwen3AsrEngine {
    models_dir: PathBuf,
    state: Arc<Mutex<InnerState>>,
    allowed_languages: Vec<String>,
}
```

Update the `new()` constructor (currently lines 37–49) to accept `allowed_languages`:

```rust
    pub fn new(
        models_dir: impl Into<PathBuf>,
        allowed_languages: Vec<String>,
    ) -> common::Result<Self> {
        let models_dir = models_dir.into();
        info!(
            "Qwen3-ASR engine created for cache: {} (languages: {:?})",
            models_dir.display(),
            allowed_languages
        );
        Ok(Self {
            models_dir,
            state: Arc::new(Mutex::new(InnerState {
                last_used: Instant::now(),
                model: None,
            })),
            allowed_languages,
        })
    }
```

- [ ] **Step 3: Use `allowed_languages` in transcription**

In `transcribe_stream()` (line 101), replace:

```rust
                    let opts = qwen3_asr::TranscribeOptions::default();
```

with:

```rust
                    let opts = if !allowed_languages.is_empty() {
                        qwen3_asr::TranscribeOptions::default()
                            .with_language(allowed_languages[0].clone())
                    } else {
                        qwen3_asr::TranscribeOptions::default()
                    };
```

To make `allowed_languages` available inside the `tokio::spawn` closure, clone it before the spawn (around line 67):

```rust
        let allowed_languages = self.allowed_languages.clone();
```

Apply the same pattern in `transcribe_file()` (line 168), clone `allowed_languages` before `spawn_blocking` and use it:

```rust
        let allowed_languages = self.allowed_languages.clone();
```

Then inside (line 168):

```rust
                    let opts = if !allowed_languages.is_empty() {
                        qwen3_asr::TranscribeOptions::default()
                            .with_language(allowed_languages[0].clone())
                    } else {
                        qwen3_asr::TranscribeOptions::default()
                    };
```

- [ ] **Step 4: Add language fallback in transcription results**

After the transcription result is obtained in `transcribe_stream()` (around lines 110–116), add a language whitelist check. Replace:

```rust
                    let lang = if lang.is_empty() {
                        "en".to_string()
                    } else {
                        lang
                    };
```

with:

```rust
                    let lang = if lang.is_empty()
                        || (!allowed_languages.is_empty()
                            && !allowed_languages.iter().any(|a| lang.starts_with(a)))
                    {
                        "en".to_string()
                    } else {
                        lang
                    };
```

- [ ] **Step 5: Update all callers of `Qwen3AsrEngine::new`**

Search for `Qwen3AsrEngine::new(` in `crates/app-core/src/init/mod.rs`. Currently at line 627:

```rust
match voice_engine::Qwen3AsrEngine::new(model_manager.models_dir()) {
```

Update to pass allowed_languages from config:

```rust
match voice_engine::Qwen3AsrEngine::new(
    model_manager.models_dir(),
    voice_config.input.allowed_languages.clone(),
) {
```

Also search the hot-swap path in the same file (around the download completion spawn). Update the `Qwen3AsrEngine::new` call there to also pass `allowed_languages`. Since the hot-swap closure doesn't have access to the config, clone it into the closure:

```rust
let allowed_langs = voice_config.input.allowed_languages.clone();
```

Then in the spawn body:

```rust
voice_engine::Qwen3AsrEngine::new(mm.models_dir(), allowed_langs)
```

- [ ] **Step 6: Verify compilation and run tests**

Run: `cargo check -p voice-engine && cargo check -p app-core`

Then: `cargo nextest run -p voice-engine -p app-core`

Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add crates/voice-engine/src/engines/qwen3_asr.rs crates/app-core/src/init/mod.rs crates/config/src/schema/voice.rs
git commit -m "feat(voice): constrain ASR to allowed_languages whitelist (en/zh/vi)"
```

---

## Task 6: Voice Conversation Manager — Build TtsParams from Persona

Wire the persona config to the TTS pipeline by reading the active persona and building `TtsParams` with instruct/temperature fields.

**Files:**
- Modify: `crates/app-core/src/handlers/voice_conversation.rs`

- [ ] **Step 1: Update TtsParams construction**

In `crates/app-core/src/handlers/voice_conversation.rs`, find the TtsParams construction (lines 816–823). Replace:

```rust
                let tts_params = {
                    let config = self.config.read().await;
                    voice_engine::TtsParams {
                        speaking_rate: config.output.speaking_rate,
                        voice_name: config.output.voice_preferences.get("default").cloned(),
                        ..Default::default()
                    }
                };
```

with:

```rust
                let tts_params = {
                    let config = self.config.read().await;
                    let persona = config
                        .output
                        .personas
                        .get(&config.output.default_persona);
                    match persona {
                        Some(config::schema::VoicePersona::Preset {
                            speaker,
                            speed,
                            temperature,
                        }) => voice_engine::TtsParams {
                            voice_name: Some(speaker.clone()),
                            speaking_rate: *speed,
                            temperature: Some(*temperature),
                            instruct: None,
                            ..Default::default()
                        },
                        Some(config::schema::VoicePersona::Custom {
                            description,
                            speed,
                            temperature,
                        }) => voice_engine::TtsParams {
                            voice_name: None,
                            speaking_rate: *speed,
                            temperature: Some(*temperature),
                            instruct: Some(description.clone()),
                            ..Default::default()
                        },
                        None => voice_engine::TtsParams {
                            speaking_rate: config.output.speaking_rate,
                            voice_name: config.output.voice_preferences.get("default").cloned(),
                            ..Default::default()
                        },
                    }
                };
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p app-core`

Expected: Clean compile.

- [ ] **Step 3: Run app-core tests**

Run: `cargo nextest run -p app-core`

Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/voice_conversation.rs
git commit -m "feat(voice): build TtsParams from active voice persona config"
```

---

## Task 7: Tauri Window — Resize & Reposition Orb

Change the voice-orb window from 320×200 (top-center) to 200×200 (bottom-right).

**Files:**
- Modify: `crates/desktop/tauri.conf.json`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Update tauri.conf.json**

In `crates/desktop/tauri.conf.json`, update the voice-orb window config (lines 107–127). Replace:

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

with:

```json
      {
        "label": "voice-orb",
        "url": "/#/voice-orb",
        "title": "",
        "width": 200,
        "height": 200,
        "resizable": false,
        "decorations": false,
        "visible": false,
        "transparent": true,
        "shadow": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "center": false,
        "focus": false,
        "windowEffects": {
          "effects": ["hudWindow"],
          "state": "active",
          "radius": 100.0
        }
      }
```

Key changes: 200×200, `center: false`, `radius: 100.0` (fully round).

- [ ] **Step 2: Update orb positioning in main.rs**

In `crates/desktop/src/main.rs`, find the orb positioning logic (around lines 392–407). The current code positions the orb at top-center:

```rust
let x = monitor_pos.x + (monitor_size.width as i32 / 2) - 160;
let y = monitor_pos.y + 80;
```

Replace with bottom-right positioning (24px margin):

```rust
let x = monitor_pos.x + monitor_size.width as i32 - 200 - 24;
let y = monitor_pos.y + monitor_size.height as i32 - 200 - 24;
```

Find the second occurrence of the same positioning pattern (around lines 485–499, the legacy fallback path) and apply the identical change.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p desktop`

Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/tauri.conf.json crates/desktop/src/main.rs
git commit -m "feat(voice): resize orb to 200x200, position bottom-right"
```

---

## Task 8: Frontend — WebGL Shader Orb Component

Create the `VoiceOrbCanvas.tsx` component that renders the procedural orb via a single WebGL2 fragment shader.

**Files:**
- Create: `desktop-ui/src/features/voice/components/VoiceOrbCanvas.tsx`

- [ ] **Step 1: Create the WebGL orb component**

Create `desktop-ui/src/features/voice/components/VoiceOrbCanvas.tsx`:

```tsx
import { useEffect, useRef } from "react";

import type { ConversationPhase } from "@features/voice/hooks/useVoiceConversation";

const VERTEX_SRC = `#version 300 es
in vec2 a_position;
void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
}`;

const FRAGMENT_SRC = `#version 300 es
precision highp float;

uniform float u_time;
uniform float u_phase;      // 0=idle, 1=listening, 2=thinking, 3=speaking
uniform float u_rms;         // 0.0–1.0 audio level
uniform float u_transition;  // 0.0–1.0 phase transition lerp
uniform vec2 u_resolution;

out vec4 fragColor;

// Simplex-style noise hash
vec3 hash3(vec3 p) {
  p = vec3(dot(p, vec3(127.1, 311.7, 74.7)),
           dot(p, vec3(269.5, 183.3, 246.1)),
           dot(p, vec3(113.5, 271.9, 124.6)));
  return -1.0 + 2.0 * fract(sin(p) * 43758.5453123);
}

float noise(vec3 p) {
  vec3 i = floor(p);
  vec3 f = fract(p);
  vec3 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(dot(hash3(i), f),
                     dot(hash3(i + vec3(1,0,0)), f - vec3(1,0,0)), u.x),
                 mix(dot(hash3(i + vec3(0,1,0)), f - vec3(0,1,0)),
                     dot(hash3(i + vec3(1,1,0)), f - vec3(1,1,0)), u.x), u.y),
             mix(mix(dot(hash3(i + vec3(0,0,1)), f - vec3(0,0,1)),
                     dot(hash3(i + vec3(1,0,1)), f - vec3(1,0,1)), u.x),
                 mix(dot(hash3(i + vec3(0,1,1)), f - vec3(0,1,1)),
                     dot(hash3(i + vec3(1,1,1)), f - vec3(1,1,1)), u.x), u.y), u.z);
}

float fbm(vec3 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * noise(p);
    p *= 2.0;
    a *= 0.5;
  }
  return v;
}

// Phase-based color palettes
vec3 phaseColor(float phase, float t) {
  // Idle: cyan-teal
  vec3 idle = mix(vec3(0.17, 0.48, 0.44), vec3(0.31, 0.82, 0.77), t);
  // Listening: bright cyan
  vec3 listen = mix(vec3(0.0, 0.59, 0.65), vec3(0.0, 0.90, 1.0), t);
  // Thinking: warm amber
  vec3 think = mix(vec3(0.90, 0.32, 0.0), vec3(1.0, 0.72, 0.30), t);
  // Speaking: soft green
  vec3 speak = mix(vec3(0.18, 0.49, 0.20), vec3(0.40, 0.73, 0.42), t);

  vec3 c = idle;
  c = mix(c, listen, clamp(1.0 - abs(phase - 1.0), 0.0, 1.0));
  c = mix(c, think,  clamp(1.0 - abs(phase - 2.0), 0.0, 1.0));
  c = mix(c, speak,  clamp(1.0 - abs(phase - 3.0), 0.0, 1.0));
  return c;
}

void main() {
  vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution) / min(u_resolution.x, u_resolution.y);
  float dist = length(uv);

  // Animated noise displacement
  float speed = mix(0.3, 0.8, smoothstep(0.0, 3.0, u_phase));
  float noiseScale = mix(2.0, 3.5, u_rms);
  float n = fbm(vec3(uv * noiseScale, u_time * speed));

  // Sphere radius with breathing + audio reactivity
  float baseRadius = 0.32;
  float breathe = 0.02 * sin(u_time * 1.2);
  float audioPulse = 0.08 * u_rms;
  float radius = baseRadius + breathe + audioPulse + 0.04 * n;

  // Sphere edge
  float sphere = smoothstep(radius + 0.02, radius - 0.02, dist);

  // Inner glow
  float glow = exp(-3.0 * dist) * 0.6;

  // Outer halo
  float halo = exp(-6.0 * max(dist - radius, 0.0)) * 0.3;

  // Wave rings (speaking state)
  float waves = 0.0;
  if (u_phase > 2.5) {
    float waveStrength = smoothstep(2.5, 3.0, u_phase) * u_rms;
    for (float i = 0.0; i < 3.0; i++) {
      float r = radius + 0.1 + i * 0.06 + 0.04 * sin(u_time * 3.0 + i);
      waves += waveStrength * 0.15 * exp(-40.0 * pow(dist - r, 2.0));
    }
  }

  // Inward spiral (thinking state)
  float spiral = 0.0;
  if (u_phase > 1.5 && u_phase < 2.5) {
    float angle = atan(uv.y, uv.x);
    float spiralPattern = sin(angle * 3.0 - u_time * 2.0 + dist * 10.0);
    spiral = smoothstep(1.5, 2.0, u_phase) * 0.15 * spiralPattern * sphere;
  }

  // Combine
  float alpha = sphere + glow + halo + waves;
  vec3 color = phaseColor(u_phase, 0.5 + 0.5 * n);
  color += spiral;

  // Soft vignette
  alpha *= smoothstep(0.5, 0.3, dist);

  fragColor = vec4(color * alpha, alpha);
}`;

const PHASE_MAP: Record<ConversationPhase, number> = {
  idle: 0.0,
  listening: 1.0,
  reflecting: 2.0,
  speaking: 3.0,
};

interface Props {
  phase: ConversationPhase;
  audioLevel: number;
}

export function VoiceOrbCanvas({ phase, audioLevel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<WebGLProgram | null>(null);
  const locRef = useRef<Record<string, WebGLUniformLocation | null>>({});
  const animRef = useRef<number>(0);
  const startTimeRef = useRef(performance.now());
  const targetPhaseRef = useRef(0.0);
  const currentPhaseRef = useRef(0.0);
  const transitionRef = useRef(1.0);
  const smoothRmsRef = useRef(0.0);

  // Update target phase on prop change
  useEffect(() => {
    const newTarget = PHASE_MAP[phase] ?? 0.0;
    if (newTarget !== targetPhaseRef.current) {
      targetPhaseRef.current = newTarget;
      transitionRef.current = 0.0;
    }
  }, [phase]);

  // Init WebGL + animation loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const gl = canvas.getContext("webgl2", { alpha: true, premultipliedAlpha: false });
    if (!gl) {
      console.error("[VoiceOrb] WebGL2 not available");
      return;
    }
    glRef.current = gl;

    // Compile shaders
    const vs = gl.createShader(gl.VERTEX_SHADER)!;
    gl.shaderSource(vs, VERTEX_SRC);
    gl.compileShader(vs);

    const fs = gl.createShader(gl.FRAGMENT_SHADER)!;
    gl.shaderSource(fs, FRAGMENT_SRC);
    gl.compileShader(fs);

    if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
      console.error("[VoiceOrb] Fragment shader error:", gl.getShaderInfoLog(fs));
    }

    const program = gl.createProgram()!;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    programRef.current = program;

    // Fullscreen quad
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, 1,1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(program, "a_position");
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

    // Uniform locations
    gl.useProgram(program);
    locRef.current = {
      u_time: gl.getUniformLocation(program, "u_time"),
      u_phase: gl.getUniformLocation(program, "u_phase"),
      u_rms: gl.getUniformLocation(program, "u_rms"),
      u_transition: gl.getUniformLocation(program, "u_transition"),
      u_resolution: gl.getUniformLocation(program, "u_resolution"),
    };

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    startTimeRef.current = performance.now();

    const render = () => {
      const dt = 1 / 60;

      // Smooth phase transition (300ms lerp)
      if (transitionRef.current < 1.0) {
        transitionRef.current = Math.min(1.0, transitionRef.current + dt / 0.3);
      }
      currentPhaseRef.current += (targetPhaseRef.current - currentPhaseRef.current) * Math.min(1.0, dt / 0.3);

      // Smooth RMS
      smoothRmsRef.current += (audioLevel - smoothRmsRef.current) * 0.15;

      const w = canvas.clientWidth * devicePixelRatio;
      const h = canvas.clientHeight * devicePixelRatio;
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }

      gl.viewport(0, 0, w, h);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.useProgram(program);

      const t = (performance.now() - startTimeRef.current) / 1000;
      gl.uniform1f(locRef.current.u_time, t);
      gl.uniform1f(locRef.current.u_phase, currentPhaseRef.current);
      gl.uniform1f(locRef.current.u_rms, smoothRmsRef.current);
      gl.uniform1f(locRef.current.u_transition, transitionRef.current);
      gl.uniform2f(locRef.current.u_resolution, w, h);

      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      animRef.current = requestAnimationFrame(render);
    };

    animRef.current = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(animRef.current);
      gl.deleteProgram(program);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      gl.deleteBuffer(buf);
    };
  }, []);

  // Keep audioLevel ref fresh without re-running the effect
  useEffect(() => {
    // audioLevel is read inside the render loop via smoothRmsRef
    // This effect just keeps the closure's audioLevel reference current
  }, [audioLevel]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: "100%", height: "100%", display: "block", cursor: "grab" }}
    />
  );
}
```

Note: The `audioLevel` prop is read via the ref in the render loop. The render loop closure captures `audioLevel` from the outer scope — we need to use a ref pattern instead. Fix the render loop to read from a ref:

Actually, the `smoothRmsRef` approach already works: the render loop reads `audioLevel` from the component scope. However, the `render` closure is only created once (in the `useEffect` with `[]` deps). So `audioLevel` would be stale. We need a separate ref:

Add after `smoothRmsRef`:

```tsx
const audioLevelRef = useRef(0.0);
```

Replace the `audioLevel` useEffect at the bottom with:

```tsx
useEffect(() => {
  audioLevelRef.current = audioLevel;
}, [audioLevel]);
```

And in the render loop, change:

```tsx
smoothRmsRef.current += (audioLevel - smoothRmsRef.current) * 0.15;
```

to:

```tsx
smoothRmsRef.current += (audioLevelRef.current - smoothRmsRef.current) * 0.15;
```

- [ ] **Step 2: Verify it builds**

Run: `cd desktop-ui && bun run build`

Expected: Clean build.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/voice/components/VoiceOrbCanvas.tsx
git commit -m "feat(voice): add WebGL procedural shader orb component"
```

---

## Task 9: Frontend — Replace VoiceBrainOrb & Simplify VoiceOrbPage

Strip the VoiceBrainOrb down to just the WebGL canvas. Remove all text, chips, buttons. Simplify VoiceOrbPage.

**Files:**
- Modify: `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx`
- Modify: `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx`

- [ ] **Step 1: Replace VoiceBrainOrb.tsx**

Replace the entire contents of `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` with:

```tsx
import { useVoiceConversation } from "@features/voice/hooks/useVoiceConversation";
import { useEvent } from "@shared/hooks/useEvent";
import { unlockAudioContext } from "@shared/lib/audio";
import { isTauri } from "@shared/lib/utils";
import { useEffect, useRef } from "react";

import { VoiceOrbCanvas } from "./VoiceOrbCanvas";

const AUTO_HIDE_DELAY_MS = 3000;

export function VoiceBrainOrb() {
  const { phase, audioLevel, start, end, sessionInfo } = useVoiceConversation();
  const prevPhaseRef = useRef(phase);

  // Unlock AudioContext on mount (orb opens via global hotkey, not a click).
  useEffect(() => {
    unlockAudioContext();
  }, []);

  // Second unlock attempt after Rust-side set_focus().
  useEvent("voice:unlock-audio", unlockAudioContext);

  // Auto-start in browser dev mode.
  const startedRef = useRef(false);
  useEffect(() => {
    if (!isTauri && !startedRef.current && phase === "idle" && !sessionInfo) {
      startedRef.current = true;
      start().catch(() => {});
    }
  }, [phase, sessionInfo, start]);

  // Auto-hide: 3 seconds after speaking -> idle transition.
  useEffect(() => {
    const wasSpeaking = prevPhaseRef.current === "speaking";
    prevPhaseRef.current = phase;

    if (wasSpeaking && phase === "idle" && isTauri) {
      const timer = setTimeout(async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        getCurrentWindow().hide();
      }, AUTO_HIDE_DELAY_MS);
      return () => clearTimeout(timer);
    }
  }, [phase]);

  // Dismiss on Esc.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        end();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [end]);

  // Enable dragging in Tauri.
  const onMouseDown = async () => {
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().startDragging();
    }
  };

  return (
    <div
      onMouseDown={onMouseDown}
      style={{ width: "100%", height: "100%", cursor: "grab" }}
    >
      <VoiceOrbCanvas phase={phase} audioLevel={audioLevel} />
    </div>
  );
}
```

- [ ] **Step 2: Simplify VoiceOrbPage.tsx**

Replace the entire contents of `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx` with:

```tsx
import { VoiceBrainOrb } from "@features/voice/components/VoiceBrainOrb";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useEffect } from "react";

const DEV_SSE_BASE = "http://localhost:3456";

function VoiceEventBridge() {
  useEffect(() => {
    if (window.__TAURI_INTERNALS__) return;
    const source = new EventSource(`${DEV_SSE_BASE}/api/brain/events`);
    source.addEventListener("voice:event", (e: MessageEvent) => {
      try {
        const payload = JSON.parse(e.data);
        window.dispatchEvent(new CustomEvent("voice:event", { detail: payload }));
      } catch {
        // Ignore malformed SSE frames
      }
    });
    return () => source.close();
  }, []);
  return null;
}

export default function VoiceOrbPage() {
  useTransparentBackground();

  return (
    <div style={{ width: "100vw", height: "100vh", overflow: "hidden" }}>
      <VoiceEventBridge />
      <VoiceBrainOrb />
    </div>
  );
}
```

Key changes from original: removed `useWindowAutoResize` (fixed 200×200 now), fullscreen div with no padding.

- [ ] **Step 3: Verify it builds and lint passes**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean build, no new lint errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx
git commit -m "feat(voice): replace chat-style orb with minimal WebGL ambient presence"
```

---

## Task 10: Integration Wiring — End-to-End Verification

Verify everything compiles, tests pass, and the orb renders correctly.

**Files:**
- All modified files from Tasks 1–9

- [ ] **Step 1: Full workspace compilation**

Run: `cargo build --workspace`

Expected: Clean build.

- [ ] **Step 2: Clippy check**

Run: `cargo clippy --workspace --all-targets --all-features`

Expected: No new warnings.

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`

Expected: All pass.

- [ ] **Step 4: Frontend build + lint**

Run: `cd desktop-ui && bun run build && bun run lint`

Expected: Clean.

- [ ] **Step 5: Frontend tests**

Run: `cd desktop-ui && bun run test`

Expected: Pass (some voice tests may need updating if they reference removed components — update assertions to match the new minimal VoiceBrainOrb).

- [ ] **Step 6: Manual smoke test**

Run: `cargo tauri dev`

1. Press Alt+Shift+V — orb should appear bottom-right as a 200×200 glowing cyan sphere
2. Speak — orb should pulse with audio level (bright cyan)
3. Wait for response — orb should turn amber (thinking), then green (speaking)
4. After TTS finishes — orb should auto-hide after 3 seconds
5. Check main chat window — response text should appear there

- [ ] **Step 7: Verify persona config**

Add to `~/.klyntbot-dev/config.json`:

```json
{
  "voice": {
    "output": {
      "defaultPersona": "professional"
    }
  }
}
```

Trigger voice — TTS should use the `onyx` speaker at 0.95 speed.

- [ ] **Step 8: Final commit**

```bash
git add -A
git commit -m "feat(voice): complete voice orb redesign with WebGL shader and TTS personas"
```
