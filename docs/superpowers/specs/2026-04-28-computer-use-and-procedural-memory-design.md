# Computer Use & Procedural Memory — Design

**Status:** approved (brainstorming)
**Date:** 2026-04-28
**Owner:** agent runtime / platform / cognitive
**Related work:** [`2026-04-20-seed-task-cognitive-integration.md`](../brainstorms/2026-04-20-seed-task-cognitive-integration.md), [`feature-launcher`](../../../crates/feature-launcher/), [`crates/cognitive/`](../../../crates/cognitive/)

## Goal

Enable Klynt to execute multi-step OS automation tasks issued through chat or voice — for example, *"open Chrome, go to YouTube, and play my favorite song"* — and to *learn from its own successful runs* so that subsequent invocations of similar tasks are faster, cheaper, and more reliable.

This is full-OS computer use (Claude Computer Use class), not browser-only. It is local-first: most perception is free (Accessibility tree), the cheap-and-private tier uses a local VLM, and a frontier cloud model handles only the hard cases. Every action is gated by composable safety rules, audited with before/after screenshots, and surfaced through a HUD the user can always see and interrupt.

A second, equally important goal: **procedural memory**. Successful trajectories (Intent → Stage → Action trees) are distilled into reusable workflow templates indexed by site fingerprint and goal embedding. Future tasks first query this memory; on a high-confidence match the agent can replay or use the trajectory as a planning prior, degrading gracefully when the underlying UI has changed.

## Non-goals

Per CLAUDE.md and explicit design choices made during brainstorming:

- **No structured observability infrastructure.** Klynt is a single-user local app; existing `tracing` logs and `PipelineEvent` SSE streams are sufficient. No OpenTelemetry, Prometheus, or metrics dashboards.
- **No browser-only scope.** This feature controls any application on macOS.
- **No Windows or Linux v1 implementation.** The platform abstraction crates accommodate them; only macOS impls ship in v1.
- **No CAPTCHA solving or behavioral fingerprint defeat.** Out of scope for ethical and legal reasons.
- **No distributed execution.** Single machine, single user.
- **No standing permissions.** Background automation requires explicit time-bounded sessions (`ComputerUseSession`).
- **No FSRS5 spaced repetition for procedural memory.** Workflows decay via run-count + last-used-at, not recall scoring.
- **No cross-session prompt caching of screenshots.** Privacy-by-default; cache only inside a single session.

## Background

### Decisions captured during brainstorming

| Question | Choice | Rationale |
|---|---|---|
| Task horizon for v1 | **E** — full multi-app + background automation | Pre-release, no backwards compatibility owed; comprehensive scope avoids future refactor |
| Vision strategy | **C** — hybrid AVR (AX → local-VLM → cloud-VLM) routed through `ProviderManager` | Pareto-optimal cost/latency/privacy; aligns with local-first stance |
| Safety model | **D** — risk-tier interceptor + scope locks + skill allowlist composed | Background automation works while destructive actions stay gated |
| Confirmation surface | **iii** — native `NSAlert` for tier-3 destructive, in-chat `AskUserTool` for tier-2 sensitive | NSAlert impossible to miss while watching screen; chat form preserves session context for reversible cases |
| Platform scope | **B** — macOS-only impl, but introduce `platform-input` + `platform-capture` traits now | Cheap optionality for Windows/Linux later; enables `MockInput`/`MockCapture` in CI |
| Tool integration | **C** — augmentation; launcher and `computer_use` coexist with non-overlapping scopes | Fast deterministic path for known apps + full power for screen-driven cases |
| UI surface | **E** — HUD (live awareness) + side panel (review) | Best for both real-time control and post-hoc trust |
| Cursor indicator / callouts / voice narration | All **on by default**, individually toggleable | Visceral "the agent is doing this for me" feel |
| Procedural memory | **Add as 7th pillar** (Intent → Stage → Action distilled to `WebTreeMemory`) | Self-improvement; turns repetitive workflows into one-shot replays |

### What already exists in the codebase

- **`feature-launcher`** — index-backed deterministic actions (search, execute, apply_window, pin, unpin). 20-variant `LauncherItemKind`. Backend `execute` is a stub today; real launch happens frontend-side via the returned item kind.
- **`platform-macos`** — `AXUIElement` (window framework), `NSPasteboard`, `NSWorkspace` (running apps), `CGEventSource` (idle detection), permission helpers (`AXIsProcessTrusted`, `CGPreflightScreenCaptureAccess`). **No CGEvent input injection. No screen capture code.**
- **`tools-core`** — `Tool` trait, `#[tool_actions]` proc-macro, `PermissionLevel`, `RoutingContext`, `InterceptorChain` (`crates/tools-core/src/interceptor.rs`).
- **`providers`** — Anthropic native + OpenAI-compat adapters. `ContentPart::ImageUrl` for URL-based images. `anthropic-version` defaults to `2023-06-01`. **No base64 image path. No `computer_use` tool block.**
- **`agent` runtime** — Unified ReAct loop in `crates/agent/src/execution/execute_loop.rs`. `MidLoopCompressor` truncates tool results at 50 KB and compresses at 70% context fill. `MAX_CONCURRENT_TOOLS = 10`. `AskUserTool` for blocking confirmation.
- **`voice-engine`** — Both STT and TTS wired with multiple backends (AVSpeech, cloud APIs, Qwen3 local). Voice already feeds the chat pipeline.
- **`cognitive`** — Episodic + semantic memory, FSRS5 spaced repetition, salience decay, mirror (6 signal sources), reforge (Collect → Review → Synthesize → Apply phases). LanceDB collections. **No procedural-memory or trajectory store today.**
- **`skill-system`** — Markdown + YAML frontmatter. `KlyntbotMeta.tools` per-skill allowlist. Orchestrator + skill + persona types. No precedent for runtime-generated skills, but the loader is structured to allow it.

### State of the art (April 2026, from research)

- **Anthropic computer use** is on `computer_20251124` (beta header `computer-use-2025-11-24`). Recommended model: Claude Sonnet 4.6. New `zoom` action added.
- **Action vocabulary**: `screenshot`, `left_click`, `double_click`, `triple_click`, `right_click`, `middle_click`, `type`, `key`, `mouse_move`, `scroll`, `left_click_drag`, `left_mouse_down`, `left_mouse_up`, `hold_key`, `wait`, `zoom`.
- **OSWorld-Verified leaderboard** (Apr 27 2026): Claude Mythos Preview 79.6%, Opus 4.7 Adaptive 78%, Sonnet 4.6 72.5%.
- **ScreenSpot-Pro** (UI grounding): Qwen3-VL-4B = 92.9%, Qwen3-VL-8B = 94.4%. Both MLX-optimized for Apple Silicon (3.3 GB / 16 GB unified RAM).
- **AX-tree-first** beats screenshot-only by ~10× in token cost on resolvable cases.
- **`CGDisplayCreateImage` removed in macOS 15.** Use ScreenCaptureKit. Rust bindings: `screencapturekit` and `objc2-screen-capture-kit`.
- **`enigo`** crate is the recommended Rust path for CGEvent injection in 2026.
- **Chrome DevTools Protocol's `Accessibility.getFullAXTree`** is the 2026 industry standard for browser ARIA trees (used by Playwright's `ariaSnapshot` and Browser Use).

## Architecture overview

```
                  ┌──────────────────────────┐
                  │    Chat / Voice input    │  (existing)
                  └─────────────┬────────────┘
                                ▼
                ┌─────────────────────────────────┐
                │    AgentRuntime / SkillRouter   │  (existing)
                └────────────────┬────────────────┘
                                 ▼
                  ┌────────────────────────────┐
                  │  computer-use orchestrator │   ← new skill (.md)
                  │  (in skills/computer-use/) │
                  └─────────────┬──────────────┘
                                ▼
              ┌──────────────────────────────────┐
              │  feature-computer-use crate      │   ← new
              │  ┌─────────────────────────────┐ │
              │  │  ComputerUseTool (action)   │ │
              │  └────┬────────────┬───────────┘ │
              │       ▼            ▼             │
              │  Perception     Action           │
              │  Cascade        Dispatcher       │
              └─────┬──────┬────┬───┬────────────┘
                    │      │    │   │
   ┌────────────────┘      │    │   └──────────────────┐
   ▼                       ▼    ▼                      ▼
┌────────────┐  ┌──────────────────┐  ┌──────────────────────┐
│ AX tree    │  │ ProviderManager  │  │ platform-input trait │
│ (cap trait)│  │ (vision routing) │  │ (CGEvent on macOS)   │
└────────────┘  │ ┌──────────────┐ │  └──────────────────────┘
                │ │ Local VLM    │ │
                │ │ Cloud VLM    │ │
                │ └──────────────┘ │
                └──────────────────┘
```

### Three-tier perception cascade

Every "what is on screen?" decision walks the cascade in order, exiting as soon as confidence is met for the current step:

1. **AX tree** (`platform-capture` trait) — free, structured, ~50–200 tokens. Resolves ≥60% of native macOS UI.
2. **Local VLM via `ProviderManager`** — private, ~1 s on M-series, default `qwen3-vl-8b` (configurable in `config.json`). Used when AX tree is empty (Electron, web canvases, games) or returns insufficient data.
3. **Cloud VLM via `ProviderManager`** — capable, ~3 s, default `claude-sonnet-4-6` (configurable). Reserved for ambiguous popups, multi-app coordination, or low-confidence VLM outputs.

When the active window is a Chromium-based browser, tier 1 is replaced by `feature-browser-control`, which connects to the running browser via Chrome DevTools Protocol and pulls `Accessibility.getFullAXTree`. This produces a strictly richer tree than native macOS AX gives for web content.

### Action vocabulary

`ComputerUseAction` enum in `platform-input` mirrors `computer_20251124` 1:1 to keep zero translation cost when Anthropic is the provider:

```rust
pub enum ComputerUseAction {
    Screenshot { region: Option<Rect> },
    LeftClick { x: i32, y: i32, modifiers: KeyMods },
    DoubleClick { x: i32, y: i32, modifiers: KeyMods },
    TripleClick { x: i32, y: i32, modifiers: KeyMods },
    RightClick { x: i32, y: i32 },
    MiddleClick { x: i32, y: i32 },
    Type { text: String },
    Key { keys: Vec<String> },
    MouseMove { x: i32, y: i32 },
    Scroll { x: i32, y: i32, direction: ScrollDir, amount: i32 },
    LeftClickDrag { from: Point, to: Point, hold_modifiers: KeyMods },
    LeftMouseDown { x: i32, y: i32 },
    LeftMouseUp { x: i32, y: i32 },
    HoldKey { keys: Vec<String>, duration_ms: u32 },
    Wait { duration_ms: u32 },
    Zoom { region: Rect },
}
```

The agent-facing `ComputerUseTool` exposes these via `#[tool_actions]`, plus higher-level convenience actions:

- `open_app(name)` — checks `LauncherIndex` first (NSWorkspace if hit), keyboard-driven path on miss.
- `focus_window(query)` — AX-driven, raises target window.
- `read_active_window_text()` — AX-tree text extraction (free, no screenshot).
- `find_element(query: AccessibilityQuery)` — semantic find via AX tree, returns coordinates.
- `run_shell(cmd, timeout_ms)` — bash escape hatch (gated, off by default per skill).

The skill prompt instructs the agent to prefer AX query → high-level action → raw coordinate, in that order.

### Serial execution invariant

The existing `agent` runtime allows up to `MAX_CONCURRENT_TOOLS = 10` parallel tool calls (`crates/agent/src/execution/core.rs:23`). **Computer-use actions must execute strictly sequentially** — no parallel mouse clicks, no overlapping keystrokes. This is enforced by the `ComputerUseTool` exposing `serial_only: true` on `Tool::execution_constraints()` (a new field), which the execute loop respects by acquiring a single-permit semaphore per session before dispatching any computer-use action. Concurrent calls of *other* tools (e.g. `notes`, `tasks`) are unaffected — only the computer-use tool's own dispatch is serialized.

### Tool integration with the launcher

The `launcher` and `computer_use` tools coexist with deliberately non-overlapping scopes:

- **`launcher`** — index-backed deterministic operations (search, open by app/file/URL, window-frame ops, pin/unpin, system actions). Cheap, fast, no perception loop.
- **`computer_use`** — screen-driven probabilistic operations (screenshot, click, type, key, scroll, drag, focus_window, AX query, run_shell). Expensive, slower, perception loop required.

System prompt rule (in the computer-use skill `.md`): *"Use launcher for any known app/file/URL/window operation. Only use computer_use actions when you need to interact with unknown or dynamic UI elements on screen."*

`computer_use::open_app(name)` always queries the `LauncherIndex` first and uses the fast `NSWorkspace` path when possible, only falling back to keyboard-driven launch on miss. `agent_action_log` rows are tagged with `tool_used: launcher | computer_use` so reforge can compare success rates per dispatch path.

## New crates and file structure

```
crates/
  platform-input/            ← NEW (L0 trait crate)
    src/lib.rs               PlatformInput trait, ComputerUseAction, KeyMods, Rect, Point
    src/mock.rs              MockInput for tests
  platform-capture/          ← NEW (L0 trait crate)
    src/lib.rs               PlatformCapture trait, Frame, AccessibilityNode, WindowInfo
    src/mock.rs              MockCapture for tests
  platform-macos/            ← EXTEND
    src/computer_use/
      input.rs               CGEvent injection via enigo + raw FFI for drag corner cases
      capture.rs             ScreenCaptureKit single-frame capture
      ax_tree.rs             AXUIElement tree walker → AccessibilityNode
      hotkey.rs              Emergency-stop hotkey on dedicated CFRunLoopSource thread
      cursor_overlay.rs      Transparent always-on-top window for click indicator
  feature-browser-control/   ← NEW (L4)
    src/lib.rs               BrowserController trait
    src/cdp.rs               chromiumoxide-based Chrome/Edge/Brave/Arc connection
    src/page_inspector.rs    Accessibility.getFullAXTree, DOM.getDocument
    src/safari.rs            wkrdp-based Safari path (best-effort)
  feature-computer-use/      ← NEW (L4)
    src/lib.rs               FeaturePackage impl, migrations, Config
    src/tool/
      mod.rs                 ComputerUseTool with #[tool_actions]
      actions.rs             ActionParams (per Anthropic 20251124 spec)
    src/perception/
      cascade.rs             AX → local-VLM → cloud-VLM router
      site_fingerprint.rs    URL-pattern + AX-shape + heading hash
    src/dispatcher.rs        ComputerUseAction → platform-input + cleanup hooks
    src/safety/
      tier_classifier.rs     Read-only / Reversible / Destructive
      scope_lock.rs          App allowlist + window-focus enforcement
      session.rs             Pre-approved time-bound sessions
      sensitive_patterns.rs  Window-title regex, AX-role match, default rules
    src/hud/
      mod.rs                 HUD state + AgentEvent emission
      events.rs              new AgentEvent variants for HUD updates
    src/replay/
      retriever.rs           Query web_tree_memories (3-stage)
      replayer.rs            Trajectory-as-prior execution + verification
      distiller.rs           trajectory → web_tree_memory row (calls LLM)
  cognitive/                 ← EXTEND
    src/services/reforge/
      web_tree_distillation.rs  ← NEW phase
    src/mirror/
      workflow_induction.rs     ← NEW; 7th MirrorSignalSource
desktop-ui/src/features/
  computer-use/              ← NEW
    HudWindow.tsx            Overlay (Tauri secondary window)
    SidePanel.tsx            Session timeline view (full session review)
    SettingsSection.tsx      Per-feature toggles + scope/sensitive-pattern editors
    hooks/useComputerUseSession.ts
skills/
  computer-use/              ← NEW (orchestrator skill)
    SKILL.md
    references/
      action-vocabulary.md
      safety-rules.md
      replay-protocol.md
```

Layering follows existing rules: traits at L0, feature crates at L4, cognitive extensions at L5.

### Trait surface (excerpt)

```rust
// crates/platform-input/src/lib.rs
pub trait PlatformInput: Send + Sync {
    fn perform_action(&self, action: ComputerUseAction) -> Result<()>;
    fn get_cursor_position(&self) -> Result<Point>;
    fn release_all(&self) -> Result<()>;          // emergency-stop hook
}

// crates/platform-capture/src/lib.rs
pub trait PlatformCapture: Send + Sync {
    fn capture_screen(&self, region: Option<Rect>) -> Result<Frame>;
    fn capture_window(&self, window_id: WindowId) -> Result<Frame>;
    fn list_displays(&self) -> Result<Vec<DisplayInfo>>;
    fn get_active_window(&self) -> Result<WindowInfo>;
    fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode>;
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub scale: f32,        // backing scale factor for Retina/HiDPI
    pub format: PixelFormat,
    pub data: Vec<u8>,     // raw pixels; encoders are downstream
}
```

The trait surface is deliberately platform-neutral. No `AXUIElement`, no `CGImage`, no `NSWindow`. Each platform translates its native types to these neutral structs. `Frame` returns raw pixels (encoding cost stays under caller control).

## Provider integration

`ProviderCapabilities` extension:

```rust
pub struct ProviderCapabilities {
    pub vision: bool,
    pub computer_use: bool,                       // ← NEW
    pub computer_use_version: Option<String>,     // ← NEW; e.g. "computer_20251124"
    // ... existing fields
}
```

Three perception backends, all routed through `ProviderManager` (no hardcoded model names):

| Tier | Default | Config key |
|---|---|---|
| Cloud | `claude-sonnet-4-6` (Anthropic) | `computer_use.providers.cloud` |
| Local VLM | `qwen3-vl-8b` via local Ollama / MLX server (OpenAI-compat) | `computer_use.providers.local` |
| Embedding | existing default | `computer_use.providers.embedding` |

### Anthropic adapter additions

- `ContentPart::ImageData { media_type, base64_data }` — the missing base64 image path.
- `Tool::ComputerUse(ComputerUseToolDef)` variant; emits the vendor `computer_20251124` block.
- `MessageContent::ToolResult { content: Vec<ContentPart> }` — tool results carrying images per Anthropic spec.
- Auto-bumps `anthropic-version` header to `2024-10-22+` and adds `anthropic-beta: computer-use-2025-11-24` when the tool is in the request.
- **Image-aware `MidLoopCompressor` exception**: image-bearing tool results are *not* truncated to 150-char summaries. Instead, older screenshots are downsampled (1280×800 → 640×400) before being dropped entirely, and the latest screenshot is always preserved verbatim.

### OpenAI / Gemini adapters

- OpenAI adapter gains a `computer_use_preview` translation (CUA Responses-API tool).
- Gemini adapter gains the equivalent.
- Local providers without a computer-use tool get a JSON-schema structured-output emulation: vision call returns `{ action: "...", params: {...} }` and the agent dispatches.

This keeps the action vocabulary canonical in Klynt while letting any provider participate.

## Safety, scope, and audit

### Three-gate enforcement

All in the existing `InterceptorChain` (`crates/tools-core/src/interceptor.rs`). Each gate runs *before* the action is dispatched:

1. **Skill allowlist** (`KlyntbotMeta.tools`) — gates whether `computer_use` is callable at all.
2. **Scope lock** (per-skill YAML `computer_use: { app_allowlist: [...] }`) — drops events targeting any app outside the list. Resolved at session start; orchestrators may unlock by overriding.
3. **Risk-tier classifier** — runs before every action:
    - **Read-only** (`screenshot`, `mouse_move`, AX read, `scroll`, `wait`, `focus_window`): auto-execute.
    - **Reversible** (`left_click`, `type`, `key`, `hold_key`, drag): auto-execute *unless* target window matches a "sensitive surface" pattern → blocking `AskUserTool` in chat.
    - **Destructive** (file delete shortcuts like `cmd+delete`, send-mail keyboard shortcuts, payment forms detected via AX, system shortcuts on unsaved windows): **blocking native `NSAlert`**.

### Sensitive surface patterns

Configurable in YAML and global settings:
```yaml
sensitive_surfaces:
  - bundle_id: "com.agilebits.onepassword7"
  - window_title_regex: "(?i)banking|chase|wells fargo"
  - ax_role: "AXButton"
    title_contains: "Send"
  - ax_role: "AXButton"
    title_contains: "Pay"
```
Defaults shipped for: 1Password, Mail compose, common banking sites, payment buttons. User-extensible.

### Pre-approved sessions

```sql
CREATE TABLE computer_use_sessions (
  id              TEXT PRIMARY KEY,
  granted_at      TEXT NOT NULL,
  expires_at      TEXT NOT NULL,
  app_allowlist   TEXT NOT NULL,    -- JSON array of bundle ids
  granted_actions TEXT NOT NULL,    -- JSON array of allowed action types
  granted_tiers   TEXT NOT NULL,    -- JSON array: e.g. ["read_only", "reversible"]
  source          TEXT NOT NULL,    -- "user" | "cron" | "skill"
  source_ref      TEXT
);
```

Cron- or skill-minted sessions show a HUD pill with countdown. On expiry: agent halts mid-step, posts summary notification, awaits re-grant. Standing permission is impossible by design.

### Emergency stop

A dedicated thread bound to a `CFRunLoopSource` holds a global hotkey (default `⌥⌘.`, configurable). On fire:

1. Sends a cancel signal to the running `ExecutionLoop`.
2. Calls `PlatformInput::release_all()` (releases held mouse buttons, modifier keys).
3. Drops any active `PlatformCapture` stream.
4. Writes `agent_action_log` "user_aborted" event.
5. Fires a `WorkflowInductionSignals` mirror event so reforge sees the abort.

The hotkey is registered at *session start* (not first action) so it's available before the agent begins.

### Audit log

```sql
CREATE TABLE agent_action_log (
  id                  TEXT PRIMARY KEY,
  session_id          TEXT NOT NULL,
  trajectory_id       TEXT,
  step_index          INTEGER NOT NULL,
  action_type         TEXT NOT NULL,
  params_json         TEXT NOT NULL,
  tier                TEXT NOT NULL,    -- "read_only" | "reversible" | "destructive"
  scope               TEXT NOT NULL,
  tool_used           TEXT NOT NULL,    -- "launcher" | "computer_use"
  latency_ms          INTEGER,
  outcome             TEXT NOT NULL,    -- "success" | "failure" | "aborted" | "confirmed"
  error_msg           TEXT,
  screenshot_before   TEXT,             -- relative path to JPEG
  screenshot_after    TEXT,
  ax_tree_before_hash TEXT,
  ax_tree_after_hash  TEXT,
  created_at          TEXT NOT NULL
);
```

Screenshot blobs stored as JPEG q80 in `data/screenshots/{session_id}/{action_id}-{before|after}.jpg`. Filesystem-backed (not SQLite) — keeps `data.db` lean. Reforge consumes screenshots for trajectory distillation; mirror consumes the metadata for signals.

## UI surface — HUD + side panel

### HUD (Tauri secondary window)

- 280×80 px, always-on-top, draggable.
- Position remembered per `display_id` in app settings.
- Live readout: *"Step 3/12 · Clicking Search button"*.
- Session pill: *"Session active — 4m 12s remaining · scope: Safari, Mail"*.
- Cancel button (one-click abort, equivalent to emergency-stop hotkey).
- Emergency-stop hint shown on hover.
- "View full session" link → opens side panel.
- Hides when no session is active.

### Cursor target indicator

- Transparent always-on-top window covering all displays.
- 60 ms expanding circle at action coordinates *before* each click.
- Optional faint trail for drags.
- Per-action-type toggle in settings.

### Action callout balloons

- 60–800 ms toasts over the active window:
    - *"typed: 'lo-fi beats' into Search field"*
    - *"clicked Send"*
- Auto-dismiss; toggleable.

### Voice narration

- Uses existing AVSpeech TTS path.
- Gated on the global `voice.tts.enabled` setting.
- Configurable verbosity: `minimal` (start/end only), `moderate` (per action), `verbose` (action + reasoning).

### Side panel

A new `desktop-ui` route opened from the HUD or chat history:

- Vertical timeline of actions with thumbnails and timestamps.
- Scrubber to jump between steps.
- Action filter (by type, by tier, by tool).
- AX-tree-diff visualization between consecutive steps.
- Link to source skill.
- "Save as workflow" button — manual procedural-memory entry from any successful session.

### Settings UI

New "Computer Use" section under `desktop-ui/src/features/settings/` with:

- Global toggles for HUD position memory, cursor indicator, callout balloons, voice narration verbosity.
- Sensitive-surface pattern editor (regex + AX-role + bundle-id).
- Default scope policies (per-skill overridable).
- Emergency-stop hotkey rebinder.
- Provider tier overrides (cloud / local model selection).
- Pre-approved session history viewer (with revoke).

## Procedural memory and website tree caching

This is the self-improvement core — the system *remembers how it did things*.

### Trajectory recorder

Extends `agent_action_log`. Every session writes:

```sql
CREATE TABLE agent_sessions (
  id              TEXT PRIMARY KEY,
  started_at      TEXT NOT NULL,
  ended_at        TEXT,
  goal_summary    TEXT,
  outcome         TEXT,                -- "success" | "failure" | "aborted"
  trajectory_id   TEXT,
  source_kind     TEXT,                -- "chat" | "voice" | "cron" | "skill"
  source_ref      TEXT
);

CREATE TABLE trajectories (
  id                   TEXT PRIMARY KEY,
  session_id           TEXT NOT NULL,
  intent_stages_json   TEXT NOT NULL,
  action_count         INTEGER NOT NULL,
  ax_tree_start_hash   TEXT,
  ax_tree_end_hash     TEXT,
  duration_ms          INTEGER,
  created_at           TEXT NOT NULL
);
```

`intent_stages_json` is the **Intent → Stage → Action** tree:

```json
{
  "intent": "play favorite song on YouTube",
  "stages": [
    {
      "name": "open browser",
      "actions": [
        { "tool": "launcher", "action": "open_app", "params": { "name": "Chrome" } }
      ]
    },
    {
      "name": "navigate to YouTube",
      "actions": [
        { "tool": "computer_use", "action": "key",  "params": { "keys": ["cmd", "l"] } },
        { "tool": "computer_use", "action": "type", "params": { "text": "youtube.com" } },
        { "tool": "computer_use", "action": "key",  "params": { "keys": ["enter"] } }
      ]
    },
    { "name": "search for song", "actions": [/* ... */] },
    { "name": "play",            "actions": [/* ... */] }
  ]
}
```

Stages emerge from the agent's own reasoning trace already captured in the ReAct loop — no separate annotation step is required.

### Distiller

Runs at session end (only on `outcome = success`). Cognitive layer + LLM:

1. Hashes the **site fingerprint** triple: `(URL pattern, AX-tree-shape hash, visible-headings hash)`. URL alone is insufficient — `gmail.com/inbox` and `gmail.com/0/#search/foo` are very different surfaces. The triple survives query-string changes but distinguishes "email inbox view" from "email composer."
2. Identifies **critical AX nodes** — elements actually clicked or typed into. Stored as semantic descriptors (role + label + relative path), not pixel coordinates.
3. Parameterizes the action template — text inputs become `{{var}}` with inferred types from context (e.g. `{{search_query}}`, `{{date}}`).
4. Embeds `goal_summary` into LanceDB.
5. Writes a row to `web_tree_memories`.

### Storage

```sql
CREATE TABLE web_tree_memories (
  id                          TEXT PRIMARY KEY,
  site_fingerprint            TEXT NOT NULL,
  domain                      TEXT NOT NULL,
  page_kind                   TEXT,
  goal_summary                TEXT NOT NULL,
  goal_embedding_lance_id     TEXT NOT NULL,
  intent_stages_template_json TEXT NOT NULL,
  critical_ax_nodes_json      TEXT NOT NULL,
  parameter_schema_json       TEXT,
  run_count                   INTEGER NOT NULL DEFAULT 1,
  success_count               INTEGER NOT NULL DEFAULT 1,
  last_used_at                TEXT,
  confidence_score            REAL,
  last_failure_at             TEXT,
  source_session_id           TEXT NOT NULL,
  created_at                  TEXT NOT NULL,
  updated_at                  TEXT NOT NULL
);

CREATE TABLE web_tree_snapshots (
  id                   TEXT PRIMARY KEY,
  web_tree_memory_id   TEXT NOT NULL,
  captured_at          TEXT NOT NULL,
  ax_tree_json         BLOB NOT NULL    -- compressed (zstd)
);
```

Plus a LanceDB collection over `goal_summary` for semantic retrieval.

### Retrieval (3-stage, fast → slow)

At task start:

1. **Site fingerprint exact match** — O(1) lookup keyed on the triple.
2. **Goal vector similarity** — LanceDB top-K against goal embeddings.
3. **AX-tree compatibility check** — does the current page contain the critical nodes the workflow expects? If not, reject the candidate even if 1+2 matched.

### Replay strategy (graceful degradation)

- **All 3 match + confidence > 0.8** → **direct replay** with parameter substitution. Each step is verified against the current AX tree before injection; on mismatch, drops to the next strategy mid-step.
- **Site fingerprint match only** → **trajectory as prior** — the workflow is injected into the agent's planning prompt as *"you've done this before, here's how"* — narrows search but the model still reasons.
- **Goal-similarity match only** → **soft hint** — the workflow appears in the agent's context as a "related successful trajectory."
- **No match** → run from scratch; record the new trajectory for future distillation.

The graceful-degradation property is essential: replay must fail soft. Naive replay shatters the moment a button moves; trajectory-as-prior degrades into normal agent reasoning seamlessly.

### Browser-specific richness

When the active app is a browser, `feature-browser-control` connects via Chrome DevTools Protocol's `Accessibility.getFullAXTree`. The captured tree is more semantically rich than native macOS AX for web content — `aria-label`, `role`, computed name, all available — and is the 2026 industry standard (Playwright's `ariaSnapshot`, Browser Use, Anthropic's reference loop all use this).

For Safari, the WebKit Web Inspector / RWI protocol is the closest equivalent; coverage is best-effort in v1 (lower priority than Chromium).

### Reforge integration

Nightly cycle gains a phase:

- **Collect**: every successful trajectory in the last 24 hours.
- **Review**: dedupe by site fingerprint, prune low-confidence entries (`success_rate < 0.5` over 5+ runs), merge near-duplicates.
- **Synthesize**: when a workflow stabilizes (`success_rate > 0.9`, `run_count > 3`), reforge generates a new lightweight skill `.md` in `~/.klyntbot/personas/learned-workflows/` that the skill router can activate on similar future requests.
- **Apply**: writes back to `web_tree_memories` with refined templates.

### Mirror integration

`WorkflowInductionSignals` source (`crates/cognitive/src/mirror/workflow_induction.rs`) — the 7th signal source. Emits events on:

- Trajectory completed.
- Replay attempted.
- Replay succeeded.
- Replay failed mid-step (with the failing step).

Feeds `MirrorFacade` consumers + reforge.

## Cognitive integration summary

- **Episodic memory** unchanged — sessions still write episodic memories via the existing path; computer-use sessions get a `kind: "computer_use"` tag for filtered retrieval.
- **Semantic memory** — distiller-extracted facts (e.g. "user's favorite YouTube playlist is at /playlist?list=ABC") write to the existing semantic store.
- **Spaced repetition (FSRS5)** — *not* applied to procedural memory; workflows decay via run-count + last-used-at decay rather than recall scoring.
- **Reforge** — gains the web-tree-distillation phase; existing strategy-file review cycle unchanged.
- **Mirror** — adds the `WorkflowInductionSignals` source; existing 6 sources unchanged.
- **Launcher index expansion** — reforge proposes new `LauncherItem` entries when the agent successfully opens an app via the screen-driven path that wasn't in the index; user approves via the existing settings UI.

## Testing strategy

- **Trait-level**: `MockInput` + `MockCapture` enable headless integration tests in `tests/integration/computer_use/`. Most agent-loop logic is testable in CI without macOS hardware.
- **Platform-level**: `crates/platform-macos/tests/` — gated `#[cfg(target_os = "macos")]`, requires Accessibility + Screen Recording permissions in CI. Runs on a self-hosted M-series runner.
- **Provider adapter tests**: golden-file tests of `computer_20251124` tool block emission (fixtures in `crates/providers/tests/fixtures/`).
- **Distiller tests**: synthetic trajectory → expected `web_tree_memories` row.
- **Replay tests**: snapshot AX tree from a real Chrome page, mock `MockCapture` to return it, assert correct action emission.
- **Safety tests**: every tier-3 action *must* produce a confirmation prompt — single test asserting interceptor coverage for the full action enum (compile-time exhaustiveness check for tier classifier).
- **End-to-end smoke**: scripted "open Chrome, navigate to example.com, screenshot" gated behind `KLYNT_E2E_COMPUTER_USE=1` env var (off by default).

## Implementation phases

Sequenced for fastest path to a working demo while building safely. Each phase is independently shippable.

### Phase 1 — Foundations (week 1)

- `platform-input` + `platform-capture` traits, `MockInput`/`MockCapture`.
- `MacInput` (enigo + raw FFI for drag), `MacCapture` (ScreenCaptureKit + AX-tree walker).
- Permissions UI flow extensions.
- Smoke test: Rust → click at (x, y) → success.

### Phase 2 — Tool surface and cloud path (week 2)

- `feature-computer-use` crate skeleton + `ComputerUseTool` with full action enum.
- Anthropic adapter: `ImageData`, `computer_20251124` tool block, beta header, image-aware compressor.
- Skill `.md` + system prompts.
- Smoke test: chat "screenshot" returns image + agent reasons over it.

### Phase 3 — Safety + HUD (week 3)

- Risk-tier classifier, scope locks, sensitive-surface rules.
- Pre-approved sessions table + lifecycle.
- Emergency-stop hotkey thread.
- HUD window + cursor overlay + action callouts + voice narration.
- `agent_action_log` + screenshot blob storage.
- Smoke test: tier-3 action triggers `NSAlert`, emergency stop kills mid-loop.

### Phase 4 — Hybrid perception (week 4)

- Perception cascade: AX-first router.
- Local-VLM provider integration (Qwen3-VL via OpenAI-compat MLX server).
- Cost / latency telemetry (logs only — non-goal: structured observability).
- Smoke test: AX-resolvable tasks complete with zero VLM calls.

### Phase 5 — Browser-aware perception (week 5)

- `feature-browser-control` crate, `chromiumoxide` integration, CDP `Accessibility.getFullAXTree`.
- Auto-detect Chromium-based browsers, fall back to native AX.
- Safari (best-effort `wkrdp`).
- Smoke test: youtube.com workflow uses CDP tree, not screenshot.

### Phase 6 — Procedural memory and replay (weeks 6–7)

- `web_tree_memories` + `web_tree_snapshots` tables, LanceDB collection.
- Trajectory recorder, distiller, retriever, replayer.
- 3-stage retrieval + graceful-degradation replay.
- Smoke test: "play favorite song on YouTube" succeeds in <1 s on second run vs ~12 s on first.

### Phase 7 — Reforge and cognitive (week 8)

- `WorkflowInductionSignals` mirror source.
- Reforge web-tree-distillation phase.
- Auto-generated learned-workflow skills.
- Side panel UI in `desktop-ui` (full session view + "save as workflow").
- Smoke test: 5 successful runs of the same task → reforge generates a stable workflow skill.

### Phase 8 — Polish and dogfood (week 9)

- Settings UI for everything (per-feature toggles, scope rules editor, sensitive-surface patterns).
- Documentation + skill cookbook.
- Real dogfood: 50+ tasks across 5+ apps.

## Open questions parked for implementation-time

These are intentionally not resolved here — they are best decided when the relevant phase begins:

- **Local VLM hosting choice**: Ollama vs MLX-LM server vs llama.cpp. All three speak OpenAI-compat. Defer until Phase 4; pick based on actual M-series benchmarks at that time.
- **Screenshot encoding**: JPEG q80 default (saves space) vs PNG (lossless). JPEG is right for audit, PNG may be needed for VLM tier — measure in Phase 2.
- **Trajectory granularity at distillation**: per-action vs per-stage. Probably per-stage (more reusable templates) but may need both for debugging skills. Defer until Phase 6.
- **Site fingerprint stability for SPA frameworks** (React/Vue with virtual DOM): the `AX-shape hash` may be too sensitive to harmless DOM churn. May need to coarsen the hash or add a heuristic-similarity score. Defer until Phase 6 dogfood.
- **Scope-lock enforcement when the agent legitimately needs to switch apps**: Mac lets users tab between apps anytime. Do we strictly refuse out-of-scope events, or warn-and-confirm? Default: refuse, with an explicit `unlock_app(bundle_id)` action that requires user confirmation.

## Out of scope (deferred)

- Cross-platform (Windows, Linux/X11, Linux/Wayland).
- Voice-only mode without UI surface (HUD must remain visible).
- Distributed multi-machine sessions.
- CAPTCHA solving or anti-fingerprint defeat.
- Cross-session prompt caching of screenshots.
- Replay across major UI redesigns (workflows are expected to invalidate; reforge cleanup is sufficient).

## Success criteria

V1 is "shipped" when:

1. The example task — *"open Chrome, go to YouTube, and play my favorite song"* — completes end-to-end via chat with HUD + cursor indicator + voice narration visible to the user.
2. The same task on a second run completes in <2 s via direct replay (vs. ~12 s perception-driven first run).
3. A tier-3 destructive action triggers a native `NSAlert` and blocks until user confirms.
4. Emergency-stop hotkey halts a running session within 100 ms of fire.
5. `agent_action_log` shows complete before/after screenshots for every action in a 20-step session.
6. After 5 successful runs of a similar task, reforge writes a new workflow skill to `~/.klyntbot/personas/learned-workflows/`.
7. Skill YAML scope-locks demonstrably prevent the agent from sending events to apps outside the allowlist.
8. Headless integration tests using `MockInput`/`MockCapture` cover ≥80% of the agent-loop control flow on non-macOS CI.

## Game Changer Extensions (v1.1 / v2)

These five extensions are explicitly **out of scope for v1** but are designed to compose cleanly with the v1 architecture so they can land as additive releases. They are listed in the order of expected user-impact.

The five split into two categories:

- **v1.1 deepenings** (items 1, 3, 4) — reuse the v1 primitives (`ComputerUseSession`, perception cascade, ScreenCaptureKit, reforge feedback). Each could ship within weeks of v1 stabilizing.
- **v2 new vectors** (items 2, 5) — introduce dependencies the v1 spec does not have (system-level LaunchAgent, signed-artifact registry). Each is a multi-month effort.

### 1. Auto-Swarm Mode via reforge (parallel sub-skills) — *v1.1*

When a task naturally decomposes into independent sub-tasks (*"summarize each unread email," "compare prices across 5 stores," "fix every TypeScript error in directory X"*), the agent spawns **parallel sub-agents**. Each sub-agent runs in its own time-bounded `ComputerUseSession` with a narrow scope lock; a coordinator gathers results.

- **Why it matters**: A 50-email triage drops from a 10-minute serial slog to a ~1-minute parallel sweep. This is the natural extension once `ComputerUseSession` is the unit of execution and procedural memory provides templates.
- **Builds on v1**: A "swarm decomposer" detects parallelizable tasks (LLM call) and spawns N copies of a session with parameter substitution. Each session keeps its own scope lock, audit log, and HUD pill (HUD shows N concurrent sessions with progress bars, side-panel timeline forks per swarm).
- **Risks**: Cross-session interference (one click in app A while app B is also being clicked). Mitigation: scope locks must be *non-overlapping* across concurrent sessions; coordinator validates this before fan-out. TTS narration and cursor overlay need queuing/muting policies for multi-session runs.

### 2. Proactive 24/7 Personal Agent — *v2*

A long-running daemon that watches signals (calendar, email arrival, system events, clipboard history, repeating user behaviors mined from `agent_action_log`) and **proactively offers** — or, with pre-approval, **executes** — actions.

- **Why it matters**: Today's agent is reactive — user types, agent acts. v2's agent *anticipates*. *"You have a 2 pm with Sara — I queued the Notion doc you used last meeting."* *"You've copied 4 different addresses in the last 5 minutes — should I add them to a spreadsheet?"* This is the difference between *tool* and *assistant*.
- **Builds on v1**: Cron + reforge + procedural memory + mirror is all the substrate. v2 adds a "proactive observer" running as a `launchctl` LaunchAgent, subscribing to system event sources (calendar via EventKit, email arrival via IMAP idle or Mail.app AX, clipboard via existing `NSPasteboard.changeCount`, app focus changes). Suggestions surface as desktop notifications; high-confidence + pre-approved patterns auto-execute under a fresh `ComputerUseSession`.
- **Risks**: Battery drain (mitigation: aggressive idle detection — observe only when the user is present at the machine; pause completely on battery below 20%). Privacy footprint of a daemon reading inbox/calendar/clipboard (mitigation: every observed signal logged + user-auditable; daemon disabled by default; explicit per-source opt-in). False positives that train the user to ignore notifications (mitigation: confidence-gated execution, single-tap "never suggest this again," weekly summary of suggestions vs. taken actions).

### 3. Self-Healing Replay — *v1.1*

When a direct replay fails mid-step (a button moved, was renamed, or no longer exists), the agent automatically:

1. Detects divergence via the AX-tree compatibility check.
2. Falls back to the perception cascade (VLM) to locate the *new* equivalent target — guided by the stored `critical_ax_nodes_json` semantic descriptor (role + label + relative path).
3. Patches the trajectory in place — updates the selector or parameter template.
4. Continues execution.
5. Records the divergence + fix as a `WorkflowInductionSignals` event so reforge improves the template.

- **Why it matters**: Without self-healing, every UI redesign invalidates entire swaths of the workflow library. With self-healing, workflows are *adaptive* — small UI changes (button rename, layout shift) are absorbed silently; the user only sees friction for major redesigns.
- **Builds on v1**: Every primitive exists already — graceful-degradation replay, AX-tree compatibility check, perception cascade, reforge feedback. v1.1 adds the *patch-and-continue* path: instead of falling all the way back to "run from scratch," surgically fix the broken step and resume from the patched index.
- **Risks**: Wrong target selection silently corrupts the workflow (mitigation: confidence threshold for auto-patch is high (>0.9); medium-confidence patches surface a "did I do this right?" prompt at session end before persisting the patched template into `web_tree_memories`).

### 4. Session video recording (lightweight) — *v1.1*

Each session is captured as a low-bitrate H.264 screen recording (e.g. 5 fps, 720p, ~50 KB/s) via ScreenCaptureKit's video stream API. Stored alongside per-action screenshots in `data/recordings/{session_id}.mp4`.

- **Why it matters**: Screenshots are great for static review; video shows the *between-action* state — animations, transitions, mouse trajectories, the moment a popup appeared. For debugging an agent that "clicked the wrong button," video makes the failure obvious in seconds. Also: shareable bug reports, future video-based reforge signal, and users get a visual library of what their agent has been doing.
- **Builds on v1**: ScreenCaptureKit (already wired in v1 for single-frame capture) supports video streams natively via the same `SCStream` API. v1.1 adds a `recording_handle` to each `ComputerUseSession` lifecycle and a side-panel video player. JPEG screenshots remain the canonical action evidence; video is supplementary.
- **Risks**: Storage growth (~3 MB/minute × 50 sessions/day ≈ 150 MB/day). Mitigation: rolling-window retention with user-configurable cap (default: last 7 days, ~1 GB ceiling). Privacy: recordings never leave the device; deletable per-session; not uploaded for distillation.

### 5. Community workflow marketplace — *v2*

Successful workflows in `web_tree_memories` can be **exported as signed, portable artifacts** (`.klynt-workflow` files). Users publish to a community registry; others browse, install, rate. The skill router can pull popular workflows on-demand from the registry.

- **Why it matters**: Procedural memory's value compounds with users — but only if users share. A library of "10,000 known workflows" beats any individual user's local memory by orders of magnitude on first-run latency. This is the economic flywheel that turns Klynt from *personal assistant* into *shared knowledge layer*.
- **Builds on v1**: Workflow templates are already structured JSON in `web_tree_memories` (parameterized, semantic descriptors, no pixel coordinates). v2 adds: (1) export with Ed25519 signing per publisher, (2) optional registry server (local-first preserved — registry is *opt-in*, never load-bearing), (3) trust UI showing signer + popularity + recent failure rate, (4) sandboxed install — community workflows always run with extra confirmation prompts until explicitly trusted by the user.
- **Risks**: Malicious workflows that look benign but exfiltrate data. Mitigation: every install runs first under **tier-3 confirmation for every action**, sandboxed scope (`app_allowlist` strictly enforced from the workflow's declared targets), community trust score, cryptographic signing identifying the publisher, community moderation flagging bad actors. Workflow staleness (mitigation: registry tracks last-known-working-date; UI warns when installing a workflow whose last successful run was >30 days ago).

### Sequencing recommendation

When v1 stabilizes, ship **3 (Self-Healing Replay)** first — it has zero new infrastructure and the largest leverage on v1's procedural memory. Then **1 (Auto-Swarm)** which extends `ComputerUseSession` parallelism. Then **4 (Session video)** — small footprint, big debugging win. Items **2** and **5** are v2 commitments and should be planned with their own design specs.

## Appendix: research summary

State-of-the-art findings (April 2026) that informed this design:

- **Anthropic computer use**: `computer_20251124` (beta header `computer-use-2025-11-24`); Claude Sonnet 4.6 recommended; new `zoom` action.
- **Action vocabulary**: matches our `ComputerUseAction` enum 1:1.
- **OSWorld-Verified leaderboard** (Apr 27 2026): Claude Mythos Preview 79.6%, Claude Opus 4.7 Adaptive 78%, Sonnet 4.6 72.5%.
- **ScreenSpot-Pro** UI grounding: Qwen3-VL-4B = 92.9%, Qwen3-VL-8B = 94.4% on Apple Silicon.
- **AX-tree-first** wins ~10× token cost vs screenshot-only on resolvable cases.
- **`CGDisplayCreateImage`** removed in macOS 15 → ScreenCaptureKit only.
- **`enigo`** crate is the recommended 2026 path for CGEvent injection.
- **CDP `Accessibility.getFullAXTree`** is the industry-standard browser ARIA tree source.
- **Hybrid AVR** (cheap-local-then-frontier) demonstrated by Cua VLM Router (NeurIPS 2025).
