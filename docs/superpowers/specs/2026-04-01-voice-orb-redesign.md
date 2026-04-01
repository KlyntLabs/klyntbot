# Voice Orb Redesign & TTS Voice Personas

> Redesign the voice orb from a chat-like panel to a minimal ambient 3D presence, add customizable TTS voice personas, and constrain ASR to supported languages.

**Date:** 2026-04-01
**Status:** Approved

---

## 1. Voice Orb — Ambient 3D Presence

### 1.1 Window Specification

| Property | Value |
|----------|-------|
| Route | `/#/voice-orb` (replaces current component) |
| Size | 200 x 200 px (fixed) |
| Decorations | None |
| Transparent | Yes |
| Always on top | Yes |
| Skip taskbar | Yes |
| Window effect | `hudWindow` (macOS vibrancy) |
| Position | Bottom-right of active monitor, 24px margin |
| Draggable | Yes (canvas mousedown + Tauri `startDragging()`) |
| Focus | false (does not steal focus on show) |

### 1.2 Lifecycle

1. **Appear**: Global hotkey (Alt+Shift+V) shows the window at bottom-right with 150ms fade-in. Transitions immediately to "listening" state.
2. **Active**: Orb animates through states as the conversation progresses (listening -> thinking -> speaking -> idle).
3. **Auto-hide**: 3 seconds after TTS playback completes, the orb fades out (200ms) and window hides.
4. **Manual dismiss**: Hotkey toggle or Esc key hides the window immediately.
5. **Re-activate**: Same hotkey while hidden shows the orb and starts listening. Reuses warm session if within 15-minute window.

### 1.3 Rendering Approach

**2D Canvas + WebGL procedural shader.** A single `<canvas>` element fills the 200x200 window. One fullscreen quad is drawn per frame with a GLSL fragment shader that renders a procedural orb (raymarched sphere + layered simplex noise + volumetric glow). No Three.js, no scene graph — one draw call per frame.

**GPU footprint:** ~5-8 MB VRAM. No compositor contention with the main window.

### 1.4 Visual States

| State | Visual | Color Palette | Audio Reactivity |
|-------|--------|---------------|------------------|
| Idle | Gentle breathing pulse, slow internal noise swirl | Soft cyan-teal (`#4FD1C5` to `#2C7A7B`) | None |
| Listening | Particles expand outward, noise frequency increases, surface ripples | Bright cyan (`#00E5FF` to `#0097A7`) | RMS drives pulse amplitude + ripple intensity |
| Thinking | Slow inward spiral, noise contracts toward center | Warm amber-orange (`#FFB74D` to `#E65100`) | None (steady animation) |
| Speaking | Concentric wave rings expand outward, gentle glow pulse | Soft green (`#66BB6A` to `#2E7D32`) | TTS audio RMS drives wave ring intensity |

### 1.5 Shader Uniforms

| Uniform | Type | Description |
|---------|------|-------------|
| `u_time` | float | Elapsed seconds (drives base animation) |
| `u_phase` | float | 0.0=idle, 1.0=listening, 2.0=thinking, 3.0=speaking |
| `u_rms` | float | Audio RMS level (0.0-1.0, smoothed) |
| `u_resolution` | vec2 | Canvas size in pixels |
| `u_transition` | float | 0.0-1.0 lerp for smooth 300ms phase transitions |

### 1.6 Component Structure

**`VoiceOrbCanvas.tsx`** (~150 LOC):
- Creates WebGL2 context on mount
- Compiles vertex + fragment shader (inline GLSL strings)
- Reads `phase` and `audioLevel` from `useVoiceConversation` hook
- Lerps `u_phase` and `u_transition` on phase changes (300ms ease)
- RAF loop: update uniforms -> `gl.drawArrays(TRIANGLE_STRIP, 0, 4)`
- Cleanup on unmount (delete shaders, buffers, context)

**`VoiceOrbPage.tsx`** (simplified):
- Renders `VoiceOrbCanvas` fullscreen
- Applies transparent background via `useTransparentBackground`
- SSE bridge for browser dev mode (existing pattern)
- No `useWindowAutoResize` (fixed 200x200)

### 1.7 What Gets Removed

All of the following are removed from the voice orb window (they remain in the main chat window):

- Text display (transcript, response text, word highlights)
- Routing chips (skill/intent detection badges)
- Memory echo display
- Continue button, replay button
- Title bar (session info, pause/resume, new session buttons)
- `useWindowAutoResize` hook

### 1.8 Response Text

All response text goes to the **main chat window only**. The voice orb never renders text. The user hears the response via TTS and can read it in the main window. No toasts, no notifications, no speech bubbles.

---

## 2. TTS Voice Personas

### 2.1 Config Schema

Extends `VoiceOutputConfig` with two new fields:

```rust
pub struct VoiceOutputConfig {
    // ... existing fields (tts_engine, deployment, speaking_rate, speak_during_focus)
    pub default_persona: String,                      // Key into personas map
    pub personas: HashMap<String, VoicePersona>,      // Named voice configurations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoicePersona {
    Preset {
        speaker: String,       // From QWEN3_VOICES: alloy, echo, fable, onyx, nova, shimmer
        speed: f32,            // 0.5-2.0 (speaking rate multiplier)
        temperature: f32,      // 0.1-1.0 (lower = more stable/predictable)
    },
    Custom {
        description: String,   // Natural language: "deep, calm male voice with formal tone"
        speed: f32,
        temperature: f32,
    },
}
```

JSON representation in `config.json`:

```json
{
  "voice": {
    "output": {
      "defaultPersona": "professional",
      "personas": {
        "professional": {
          "type": "preset",
          "speaker": "onyx",
          "speed": 0.95,
          "temperature": 0.8
        },
        "customDeepCalm": {
          "type": "custom",
          "description": "deep, calm male voice with a formal British tone",
          "speed": 0.92,
          "temperature": 0.75
        }
      }
    }
  }
}
```

### 2.2 Built-in Presets

| Key | Speaker | Speed | Temp | Character |
|-----|---------|-------|------|-----------|
| `professional` | `onyx` | 0.95 | 0.8 | Deep, measured, formal |
| `friendly` | `nova` | 1.0 | 0.9 | Warm, conversational |
| `calm` | `shimmer` | 0.9 | 0.7 | Gentle, soothing narrator |
| `energetic` | `echo` | 1.1 | 0.95 | Upbeat, enthusiastic |
| `neutral` | `alloy` | 1.0 | 0.85 | Clean, balanced, default |
| `storyteller` | `fable` | 0.92 | 0.8 | Expressive, dramatic |

### 2.3 Custom Voice Design

- Requires the **Qwen3-TTS 1.7B CustomVoice model** (optional download)
- Uses `generate_with_instruct(text, speaker, language, description, temperature, top_k, max_codes)` instead of `generate_with_params`
- The `description` field is passed as the `instruct` parameter
- Model download triggered via Settings UI ("Download voice design model" button)
- `ModelManager` tracks the 1.7B variant separately from the 0.6B base

### 2.4 TtsParams Extension

```rust
pub struct TtsParams {
    pub language: Language,
    pub voice_name: Option<String>,
    pub speaking_rate: f32,
    pub instruct: Option<String>,       // NEW: natural language voice description
    pub temperature: Option<f32>,       // NEW: override default 0.9
}
```

### 2.5 Runtime Flow

1. `VoiceConversationManager` reads `default_persona` from `VoiceOutputConfig`
2. Looks up the `VoicePersona` entry in `personas` map
3. Maps to `TtsParams`:
   - **Preset**: `voice_name = Some(speaker)`, `speaking_rate = speed`, `temperature = Some(temp)`, `instruct = None`
   - **Custom**: `voice_name = None` (model picks), `speaking_rate = speed`, `temperature = Some(temp)`, `instruct = Some(description)`
4. `Qwen3TtsEngine::synthesize` checks: if `params.instruct.is_some()` and 1.7B model is loaded, calls `generate_with_instruct`; otherwise calls `generate_with_params`. If `instruct` is set but 1.7B is not downloaded, falls back to `generate_with_params` with `alloy` speaker and logs a warning.
5. Hot-reloadable: switching persona in Settings takes effect on the next TTS call (no restart)

### 2.6 Settings UI

**Voice & Speech tab in Settings:**

- **Default persona** dropdown (populated from config personas)
- **Quick Presets** section: 6 preset cards, one-click select, shows speaker name + character description
- **Custom Voice Designer** section (collapsible):
  - Textarea: "Describe the voice you want..."
  - Speed slider (0.5-2.0)
  - Temperature slider (0.1-1.0)
  - "Generate & Test" button: synthesizes a test sentence with current settings, plays immediately
  - "Save as new persona" button: saves to config, hot-reloads
- **Test Voice** button at bottom: speaks a sample sentence using the currently selected persona
- **Model status**: shows "0.6B (base)" or "1.7B (custom voices)" with download button if 1.7B not present

---

## 3. ASR Language Restriction

### 3.1 Config

New field in `VoiceInputConfig`:

```rust
pub struct VoiceInputConfig {
    // ... existing fields
    pub allowed_languages: Vec<String>,   // Default: ["en", "zh", "vi"]
}
```

### 3.2 Behavior

- The `allowed_languages` list is passed as a language hint/whitelist to `TranscribeOptions` on every Qwen3-ASR transcription call
- If the model returns a language code not in the whitelist, the result is remapped to `"en"` before entering the agent pipeline
- Hot-reloadable via `HotConfig`
- Default: `["en", "zh", "vi"]` (English, Chinese, Vietnamese)

### 3.3 Rationale

Qwen3-ASR supports 52 languages with auto-detection. Without a whitelist, mispronounced words in English/Chinese/Vietnamese can trigger false-positive detection of Korean, Japanese, or other languages, producing garbled transcripts. Constraining to the three actively used languages eliminates this class of UX bugs.

---

## 4. File Change Summary

### Replace
- `desktop-ui/src/features/voice/components/VoiceBrainOrb.tsx` — 388 lines of chat UI replaced with ~30 lines (renders `VoiceOrbCanvas`)
- `desktop-ui/src/features/voice/pages/VoiceOrbPage.tsx` — simplified (no auto-resize)

### Add
- `desktop-ui/src/features/voice/components/VoiceOrbCanvas.tsx` — WebGL shader component (~150 LOC)

### Modify (Rust)
- `crates/config/src/schema/voice.rs` — add `VoicePersona` enum, `default_persona`, `personas` to `VoiceOutputConfig`; add `allowed_languages` to `VoiceInputConfig`
- `crates/voice-engine/src/types.rs` — extend `TtsParams` with `instruct: Option<String>`, `temperature: Option<f32>`
- `crates/voice-engine/src/engines/qwen3_tts.rs` — branch on `instruct` to call `generate_with_instruct` vs `generate_with_params`; pass `temperature` from params
- `crates/voice-engine/src/model_manager.rs` — add `Qwen3Model::Tts1_7B` variant for the larger model
- `crates/app-core/src/handlers/voice_conversation.rs` — read persona from config, build `TtsParams` with persona fields
- `crates/app-core/src/init/mod.rs` — support 1.7B model detection and engine initialization

### Modify (Frontend)
- `desktop-ui/src/shared/lib/audio.ts` — no changes needed (TTS playback works as-is)

### Modify (Tauri)
- `crates/desktop/tauri.conf.json` — voice-orb window: 200x200, bottom-right position
- `crates/desktop/src/main.rs` — update orb window positioning logic (bottom-right instead of top-center)

### Keep (unchanged)
- All Rust voice pipeline (VoiceService, VoiceConversationManager state machine, engine manager)
- `useVoiceConversation` hook (still drives the state machine)
- `audio.ts` (AudioContext, playback, unlock)
- Voice event system (Tauri events, SSE bridge)
- ASR engine core (Qwen3AsrEngine — only the TranscribeOptions usage changes)

---

## 5. Non-Goals

- **No pronunciation correction** — the orb does not display or correct pronunciation
- **No word-level confidence display** — removed from the orb entirely
- **No voice cloning** — Custom personas use voice instructions, not reference audio cloning
- **No streaming TTS** — synthesis is still full-text-then-play (qwen3-tts-rs has no chunk streaming API)
- **No multi-persona conversations** — one active persona at a time (per-squad override is a future enhancement)
