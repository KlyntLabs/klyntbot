# System-Wide Input Services — Design Spec

**Date:** 2026-04-19
**Status:** Draft (pending implementation plan)
**Scope:** New module `crates/platform-macos/src/event_tap.rs`, new crate `crates/feature-input-services`, app-core wiring, new desktop-ui settings page.
**Out of scope:** Snippet variables (`{{date}}`, `{{clipboard}}`), per-app snippet scoping, Karabiner-style multi-key chords, hyper-key chord remapping (e.g. `Hyper+L → Cmd+Shift+5`), MCP tool exposure of snippets, browser/Linux ports.

## Motivation

The SuperCmd vs. Klynt comparison report (2026-04-19) identified two SuperCmd capabilities Klynt lacks that meaningfully improve daily macOS productivity:

1. **Snippet expansion** — Type a short trigger like `;email`, get the expanded text instantly anywhere (Mail, Slack, browser, terminal). SuperCmd implements this via a Swift binary (`snippet-expander.swift`) using `CGEventTap` + `CGEventPost`.
2. **Hyper Key** — Remap an under-used physical key (Caps Lock by default) to a unique 4-modifier combo (Cmd+Ctrl+Opt+Shift), giving the user a collision-free namespace for global hotkeys. SuperCmd implements this via `hyper-key-monitor.swift` using `CGEventTap` at the Carbon/CGEvent level.

Both features share the same low-level primitive — a global `CGEventTap` that observes (and modifies) keystroke events system-wide — but have nothing else in common architecturally. Snippets need a database, keyword matching, and clipboard manipulation. Hyper Key needs a tiny pure state machine. They are bundled into one spec because they share a permission flow (Input Monitoring), share a primitive (event tap), and ship as one user-facing feature ("Klynt Input Services").

## Architecture Overview

Three layers, designed for clean isolation:

| Layer | Crate | Responsibility |
|-------|-------|----------------|
| L0 | `platform-macos` (extended) | `event_tap` primitive: spawn dedicated CFRunLoop thread, bridge events to tokio. Stateless transport. |
| L4 | `feature-input-services` (new) | `SnippetEngine`, `HyperKeyEngine`, `SnippetRepo`. `FeaturePackage` impl. |
| L7 | `app-core` + `desktop` + `desktop-ui` | Wire engines to event tap, expose Tauri commands, settings page UI. |

**Key invariant:** the event tap is **stateless transport**. Both engines subscribe to the same event stream and each independently decides whether to consume the event. This mirrors Karabiner-Elements' internal structure and lets each engine be unit-tested without any `CGEventTap` involvement (feed it a `Vec<RawKeyEvent>`, assert `TapAction` outputs).

Permission model: **Input Monitoring**. Lazy request — only when the user enables snippets or Hyper Key. Failure surfaces as a desktop notification + amber banner on the settings page; we never silently restore broken event taps.

## Component Designs

### 1. `platform-macos::event_tap`

**Location:** `crates/platform-macos/src/event_tap.rs` (new file), exported from `lib.rs`.

**Public API:**
```rust
pub struct EventTap {
    handle: Arc<parking_lot::Mutex<Option<TapHandle>>>,
}

#[derive(Debug, Clone, Copy)]
pub enum TapMode { Observe, Modify }

#[derive(Debug, Clone)]
pub struct EventTapConfig {
    pub mode: TapMode,
    pub event_types: Vec<TapEventType>,
}

#[derive(Debug, Clone, Copy)]
pub enum TapEventType { KeyDown, KeyUp, FlagsChanged }

#[derive(Debug, Clone)]
pub struct RawKeyEvent {
    pub keycode: u16,
    pub is_down: bool,
    pub flags: u64,                  // CGEventFlags
    pub characters: Option<String>,  // resolved via TIS lookup at event time
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone)]
pub enum TapAction {
    Pass,                       // forward unchanged
    Suppress,                   // drop the event (don't deliver to apps)
    Replace(Vec<RawKeyEvent>),  // suppress original, post these instead
}

#[derive(Debug, thiserror::Error)]
pub enum EventTapError {
    #[error("Input Monitoring permission denied")]
    PermissionDenied,
    #[error("CGEventTap creation failed (kernel/HID error)")]
    TapCreation,
    #[error("Worker thread spawn failed: {0}")]
    ThreadSpawn(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionStatus { Granted, Denied, Unknown }

impl EventTap {
    pub fn start(
        cfg: EventTapConfig,
        handler: impl Fn(RawKeyEvent) -> TapAction + Send + Sync + 'static,
    ) -> Result<Self, EventTapError>;
    pub fn stop(&self);
    pub fn is_alive(&self) -> bool;
}

pub fn check_input_monitoring() -> PermissionStatus;
pub fn request_input_monitoring();   // opens System Settings deep-link
```

**Implementation notes:**
- The constructor spawns a dedicated OS thread (not a tokio task) running `CFRunLoopRun()`. The handler is wrapped in a `Box<dyn Fn>` whose pointer is passed as the tap's `userInfo`.
- The C trampoline unboxes, calls the handler, and translates `TapAction` into `CGEventTapPostEvent` calls or returns the appropriate `CGEventRef` (or `NULL` to suppress).
- `TapAction::Replace` posts each event via `CGEventPost(kCGSessionEventTap, ...)` from inside the callback. This is safe because the callback runs on the CFRunLoop thread.
- `kCGEventTapDisabledByTimeout` and `kCGEventTapDisabledByUserInput` are caught in the C trampoline; the handler is invoked with a synthetic `RawKeyEvent { keycode: u16::MAX, ... }` whose interpretation is "the tap is dead". Worker thread logs and exits cleanly. `is_alive()` returns false thereafter.
- Permission check uses `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)`. Request uses `IOHIDRequestAccess`. If denied, `request_input_monitoring()` opens `x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent`.

**Dependencies (new in `platform-macos/Cargo.toml`):**
```toml
core-foundation = "0.10"
core-graphics = "0.24"
objc2 = "0.6"
objc2-foundation = "0.3"
objc2-core-foundation = "0.3"
```

**Tests:**
- Unit: `RawKeyEvent` conversion (extract from `CGEventRef` mock).
- Unit: `TapAction::Replace` produces correct `CGEventPost` call sequence (mock the post via a trait).
- Manual integration: example binary `examples/keystroke_logger.rs` that prints every event. Gated to macOS, requires real permission. Used as a smoke test before each release of this crate.

### 2. `feature-input-services` crate

**Location:** `crates/feature-input-services/` (new).

**Cargo.toml:**
```toml
[package]
name = "feature-input-services"
version = "0.1.0"
edition = "2021"

[dependencies]
tools-core = { path = "../tools-core" }
storage = { path = "../storage" }
common = { path = "../common" }
config = { path = "../config" }
platform-macos = { path = "../platform-macos" }
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
parking_lot.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

**Module layout:**
```
src/
  lib.rs                        # FeaturePackage impl, public re-exports
  snippet/
    mod.rs                      # public API
    types.rs                    # Snippet, SnippetTable
    detector.rs                 # TriggerDetector (sliding window matcher)
    expander.rs                 # Expander (backspace + paste sequence)
    engine.rs                   # SnippetEngine (event handler entry point)
  hyper_key/
    mod.rs
    engine.rs                   # HyperKeyEngine state machine
    config.rs                   # HyperKeyConfig (source key, tap action)
  repos/
    mod.rs
    snippet_repo.rs             # SnippetRepo (CRUD + watch broadcaster)
  permission.rs                 # Re-export + helpers around platform_macos perm checks
migrations/
  001_snippets.sql
```

#### 2a. `snippet::types`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: i64,
    pub trigger: String,         // e.g. ";email" — includes leading marker
    pub body: String,            // expansion text
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

#[derive(Debug, Clone, Default)]
pub struct SnippetTable {
    pub by_trigger: HashMap<String, Snippet>,
    pub longest_trigger_len: usize,
    pub marker: char,            // default ';'
}

impl SnippetTable {
    pub fn lookup(&self, ending_at: &str) -> Option<&Snippet> {
        // longest-match: try trigger lengths from `longest_trigger_len` down to 1
        for n in (1..=self.longest_trigger_len.min(ending_at.len())).rev() {
            let candidate = &ending_at[ending_at.len() - n..];
            if let Some(s) = self.by_trigger.get(candidate) {
                if s.enabled { return Some(s); }
            }
        }
        None
    }
}
```

#### 2b. `snippet::detector`

```rust
pub struct TriggerDetector {
    buffer: VecDeque<char>,      // last 64 chars typed
    table: tokio::sync::watch::Receiver<Arc<SnippetTable>>,
}

#[derive(Debug)]
pub struct DetectionResult {
    pub snippet: Snippet,
    pub trigger_len: usize,      // chars to delete (trigger only, NOT including boundary)
    pub boundary_char: char,     // re-emit after expansion
}

impl TriggerDetector {
    pub fn new(table_rx: tokio::sync::watch::Receiver<Arc<SnippetTable>>) -> Self;

    /// Returns Some if the just-typed char is a word boundary AND the buffer
    /// (excluding the boundary itself) ends with a known trigger.
    pub fn on_char(&mut self, c: char) -> Option<DetectionResult> {
        // Push then evaluate
        if self.buffer.len() == 64 { self.buffer.pop_front(); }
        self.buffer.push_back(c);
        if !is_word_boundary(c) { return None; }

        let table = self.table.borrow();
        // Build a string from the buffer EXCLUDING the boundary char
        let s: String = self.buffer.iter().take(self.buffer.len() - 1).collect();
        let snippet = table.lookup(&s)?.clone();
        Some(DetectionResult {
            trigger_len: snippet.trigger.chars().count(),
            boundary_char: c,
            snippet,
        })
    }

    pub fn reset(&mut self) { self.buffer.clear(); }
}

fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\'' | '\n' | '\t')
}
```

**Subtle but critical:** the marker char (`;` by default) is itself a word-boundary in `is_word_boundary` — so typing `;email` followed by `;` would also trigger. That's intentional and desirable.

#### 2c. `snippet::expander`

```rust
pub struct Expander {
    poster: Arc<dyn EventPoster + Send + Sync>,    // trait for testability
    clipboard: Arc<dyn Clipboard + Send + Sync>,
}

#[async_trait]
pub trait EventPoster {
    async fn post_backspaces(&self, n: usize);
    async fn post_paste(&self);
    async fn post_char(&self, c: char);
}

#[async_trait]
pub trait Clipboard {
    async fn read(&self) -> Option<String>;
    async fn write(&self, s: &str);
}

impl Expander {
    pub async fn expand(&self, det: DetectionResult) {
        // 1. Save current clipboard
        let prev = self.clipboard.read().await;

        // 2. Backspace the trigger AND the boundary char (we'll re-emit boundary later)
        self.poster.post_backspaces(det.trigger_len + 1).await;

        // 3. Set clipboard to body
        self.clipboard.write(&det.snippet.body).await;

        // 4. Tiny delay so apps see the new clipboard
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 5. Paste
        self.poster.post_paste().await;

        // 6. Schedule clipboard restore (after paste completes)
        let cb = self.clipboard.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if let Some(prev) = prev { cb.write(&prev).await; }
        });

        // 7. Re-emit the boundary char so the user's typing flow is preserved
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.poster.post_char(det.boundary_char).await;
    }
}
```

The production `EventPoster` impl wraps `CGEventPost` calls; the production `Clipboard` impl wraps `NSPasteboard` (already used by `crates/feature-launcher`'s `ClipboardMonitor`).

#### 2d. `snippet::engine`

```rust
pub struct SnippetEngine {
    detector: parking_lot::Mutex<TriggerDetector>,
    expander: Arc<Expander>,
    expand_tx: tokio::sync::mpsc::UnboundedSender<DetectionResult>,
}

impl SnippetEngine {
    pub fn start(
        table_rx: tokio::sync::watch::Receiver<Arc<SnippetTable>>,
        expander: Arc<Expander>,
    ) -> Self {
        let detector = parking_lot::Mutex::new(TriggerDetector::new(table_rx));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DetectionResult>();
        let exp = expander.clone();
        tokio::spawn(async move {
            while let Some(det) = rx.recv().await { exp.expand(det).await; }
        });
        Self { detector, expander, expand_tx: tx }
    }

    pub fn on_event(&self, ev: &RawKeyEvent) -> TapAction {
        if !ev.is_down { return TapAction::Pass; }
        let c = match ev.characters.as_deref().and_then(|s| s.chars().next()) {
            Some(c) => c, None => return TapAction::Pass,
        };
        let mut det = self.detector.lock();
        match det.on_char(c) {
            None => TapAction::Pass,
            Some(result) => {
                let _ = self.expand_tx.send(result);
                TapAction::Suppress  // suppress the boundary char; expander re-emits it
            }
        }
    }
}
```

#### 2e. `hyper_key::engine`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperKeyConfig {
    pub enabled: bool,
    pub source_keycode: u16,                  // 0x39 = Caps Lock
    pub hyper_modifier_flags: u64,            // CGEventFlags bits OR'd
    pub tap_action: TapAction,                // Escape / Nothing / Original
    pub tap_timeout_ms: u64,                  // 300
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TapActionConfig { Escape, Nothing, Original }

pub struct HyperKeyEngine {
    cfg: parking_lot::RwLock<HyperKeyConfig>,
    state: parking_lot::Mutex<HyperState>,
}

#[derive(Debug, Clone)]
struct HyperState {
    held: bool,
    held_since: Option<Instant>,
    used_with_other_key: bool,
}

impl HyperKeyEngine {
    pub fn on_event(&self, ev: &RawKeyEvent) -> TapAction {
        let cfg = self.cfg.read();
        if !cfg.enabled { return TapAction::Pass; }
        let mut state = self.state.lock();
        // ── source key down ────────────────────────────────────────────
        if ev.keycode == cfg.source_keycode && ev.is_down {
            state.held = true;
            state.held_since = Some(Instant::now());
            state.used_with_other_key = false;
            return TapAction::Suppress;
        }
        // ── source key up ──────────────────────────────────────────────
        if ev.keycode == cfg.source_keycode && !ev.is_down {
            let was_tap = state.held_since
                .map(|t| t.elapsed() < Duration::from_millis(cfg.tap_timeout_ms))
                .unwrap_or(false)
                && !state.used_with_other_key;
            state.held = false;
            state.held_since = None;
            state.used_with_other_key = false;
            return if was_tap { tap_action_to_events(cfg.tap_action) } else { TapAction::Suppress };
        }
        // ── any other key while held ───────────────────────────────────
        if state.held {
            state.used_with_other_key = true;
            let mut new_flags = ev.flags | cfg.hyper_modifier_flags;
            return TapAction::Replace(vec![RawKeyEvent {
                flags: new_flags, ..ev.clone()
            }]);
        }
        TapAction::Pass
    }
}

fn tap_action_to_events(a: TapActionConfig) -> TapAction {
    match a {
        TapActionConfig::Escape => TapAction::Replace(vec![
            RawKeyEvent { keycode: 0x35, is_down: true,  flags: 0, characters: None, timestamp_ns: 0 },
            RawKeyEvent { keycode: 0x35, is_down: false, flags: 0, characters: None, timestamp_ns: 0 },
        ]),
        TapActionConfig::Nothing  => TapAction::Suppress,
        TapActionConfig::Original => TapAction::Pass,
    }
}
```

**Caps Lock special handling:** macOS handles Caps Lock toggling at the HID layer *before* event taps see most events. We disable this at boot via `hidutil` invocation: `hidutil property --set '{"UserKeyMapping": [{"HIDKeyboardModifierMappingSrc":0x700000039,"HIDKeyboardModifierMappingDst":0x70000006B}]}'` (remaps Caps Lock to F18, which then flows through the event tap normally). This is reversed on Hyper Key disable. Documented in the engine comments and in the install README.

#### 2f. `repos::snippet_repo`

```rust
pub struct SnippetRepo {
    pool: storage::StoragePool,
    table_tx: tokio::sync::watch::Sender<Arc<SnippetTable>>,
    table_rx: tokio::sync::watch::Receiver<Arc<SnippetTable>>,
    marker: char,
}

impl SnippetRepo {
    pub async fn new(pool: storage::StoragePool, marker: char) -> Result<Self, sqlx::Error>;

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Arc<SnippetTable>> {
        self.table_rx.clone()
    }

    pub async fn list(&self) -> Result<Vec<Snippet>, sqlx::Error>;
    pub async fn create(&self, trigger: String, body: String, description: Option<String>) -> Result<Snippet, sqlx::Error>;
    pub async fn update(&self, id: i64, ...) -> Result<Snippet, sqlx::Error>;
    pub async fn delete(&self, id: i64) -> Result<(), sqlx::Error>;
    pub async fn toggle(&self, id: i64, enabled: bool) -> Result<(), sqlx::Error>;

    async fn rebuild_table(&self) -> Result<(), sqlx::Error> {
        let snippets = self.list().await?;
        let mut by_trigger = HashMap::new();
        let mut longest = 0;
        for s in snippets {
            longest = longest.max(s.trigger.chars().count());
            by_trigger.insert(s.trigger.clone(), s);
        }
        self.table_tx.send_replace(Arc::new(SnippetTable {
            by_trigger, longest_trigger_len: longest, marker: self.marker,
        }));
        Ok(())
    }
}
```

**Migration `001_snippets.sql`:**
```sql
CREATE TABLE IF NOT EXISTS snippets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL UNIQUE,
    body TEXT NOT NULL,
    description TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snippets_trigger ON snippets(trigger);
```

#### 2g. `InputServicesFeature`

```rust
pub struct InputServicesFeature;

#[async_trait]
impl FeaturePackage for InputServicesFeature {
    fn name(&self) -> &'static str { "input_services" }
    fn config_key(&self) -> &'static str { "inputServices" }
    fn migrations(&self) -> Vec<FeatureMigration> { /* embed 001_snippets.sql */ }
    fn tools(&self) -> Vec<DynTool> { vec![] }
    async fn health(&self, ctx: &HealthContext<'_>) -> FeatureHealth { /* event_tap_alive, perm, counts */ }
}
```

### 3. App-core wiring

**Location:** `crates/app-core/src/init/input_services.rs` (new).

```rust
pub struct InputServicesHandle {
    pub event_tap: Option<EventTap>,
    pub snippet_repo: Arc<SnippetRepo>,
    pub hyper_key_engine: Arc<HyperKeyEngine>,
    pub snippet_engine: Arc<SnippetEngine>,
    pub status: Arc<RwLock<InputServicesStatus>>,
}

pub async fn init_input_services(
    pool: storage::StoragePool,
    cfg: &config::InputServicesConfig,
) -> Result<InputServicesHandle, KlyntbotError> {
    let snippet_repo = Arc::new(SnippetRepo::new(pool.clone(), cfg.snippets.marker).await?);
    let expander = Arc::new(Expander::new(
        Arc::new(CGEventPoster), Arc::new(NSPasteboardClipboard),
    ));
    let snippet_engine = Arc::new(SnippetEngine::start(snippet_repo.subscribe(), expander));
    let hyper_engine = Arc::new(HyperKeyEngine::new(cfg.hyper_key.clone()));

    if !cfg.snippets.enabled && !cfg.hyper_key.enabled {
        return Ok(InputServicesHandle { event_tap: None, ... });
    }

    let perm = check_input_monitoring();
    if perm != PermissionStatus::Granted {
        // Don't request — defer to user opening settings page
        return Ok(InputServicesHandle { event_tap: None, ... });
    }

    let snip_for_handler = snippet_engine.clone();
    let hyper_for_handler = hyper_engine.clone();
    let event_tap = EventTap::start(
        EventTapConfig {
            mode: TapMode::Modify,
            event_types: vec![TapEventType::KeyDown, TapEventType::KeyUp, TapEventType::FlagsChanged],
        },
        move |ev| {
            // Hyper key first (it might transform the event before snippet sees it)
            match hyper_for_handler.on_event(&ev) {
                TapAction::Pass => snip_for_handler.on_event(&ev),
                other => other,
            }
        },
    )?;

    Ok(InputServicesHandle { event_tap: Some(event_tap), ... })
}
```

### 4. Tauri commands

**Location:** `crates/desktop/src/commands/input_services.rs` (new).

```rust
#[tauri::command] pub async fn input_services_snippet_list(...) -> Result<Vec<Snippet>, ApiError>;
#[tauri::command] pub async fn input_services_snippet_create(...) -> Result<Snippet, ApiError>;
#[tauri::command] pub async fn input_services_snippet_update(...) -> Result<Snippet, ApiError>;
#[tauri::command] pub async fn input_services_snippet_delete(...) -> Result<(), ApiError>;
#[tauri::command] pub async fn input_services_snippet_toggle(...) -> Result<(), ApiError>;
#[tauri::command] pub async fn input_services_permission_status() -> Result<PermissionStatus, ApiError>;
#[tauri::command] pub async fn input_services_request_permission() -> Result<(), ApiError>;
#[tauri::command] pub async fn input_services_status(...) -> Result<InputServicesStatus, ApiError>;
#[tauri::command] pub async fn input_services_set_hyper_key(...) -> Result<(), ApiError>;

pub(crate) const DEV_COMMANDS: &[&str] = &[
    "input_services_snippet_list", /* ... all 9 */
];
```

`InputServicesStatus`:
```rust
#[derive(Serialize, Deserialize)]
pub struct InputServicesStatus {
    pub event_tap_alive: bool,
    pub permission: PermissionStatus,
    pub snippet_count: usize,
    pub hyper_key_enabled: bool,
    pub last_error: Option<String>,
}
```

### 5. Frontend settings page

**Location:** `desktop-ui/src/features/settings/InputServicesPage.tsx` (new).

Two tabs:
- **Snippets:** table with trigger / body preview / enabled toggle / edit / delete. "+ New" button opens a modal with `trigger` (text input), `body` (textarea, monospace), `description` (text). Conflict warning if `trigger` already exists. Save calls `input_services_snippet_create`.
- **Hyper Key:** enable toggle, source-key dropdown (Caps Lock / Right Cmd / Right Opt / Right Ctrl), tap-action dropdown (Escape / Nothing / Original). Save calls `input_services_set_hyper_key`.

Top of page: permission banner.
- Granted: green pill, no action.
- Denied / Unknown: amber banner, "Klynt needs Input Monitoring to expand snippets and remap Hyper Key. [Open System Settings]". Button calls `input_services_request_permission`. Footnote: "After granting, please restart Klynt."

Settings nav entry added to `desktop-ui/src/features/settings/SettingsNav.tsx` (or equivalent).

## Cross-Cutting Concerns

**Migrations:** new `snippets` table via `feature-input-services/migrations/001_snippets.sql`. Idempotent (`CREATE IF NOT EXISTS`).

**Backward compatibility:** entirely additive — new crate, new tables, new commands, new settings page. Zero impact on existing features when disabled.

**Performance budgets:**
- Event tap callback: ≤ 200 µs per keystroke (P95). Anything more risks input latency. Heavy work always dispatched to a tokio task.
- Snippet expansion end-to-end (boundary char → text appearing in app): ≤ 200 ms.
- `TriggerDetector::on_char`: ≤ 5 µs (in-memory hashmap lookup).

**Observability:** new `tracing` events: `input_services.tap.started`, `input_services.tap.died { reason }`, `input_services.snippet.expanded { trigger, ms }`, `input_services.hyper.toggled`, `input_services.permission.changed`. No metrics infra.

**Privacy:** the event tap sees every keystroke the user types. **We never log keystroke content** — only event types and timing. Snippet trigger names are logged at info; snippet bodies never are. This is documented in `crates/feature-input-services/PRIVACY.md` (a new file the user can audit).

**Concurrency:** the CFRunLoop callback runs on a dedicated OS thread; engines use `parking_lot::Mutex`/`RwLock` for shared state. No tokio inside the callback. Heavy work goes through `mpsc` channels.

## Sequencing

| Sub-PR | Depends on | Lands |
|--------|-----------|-------|
| PR-1 `event_tap` primitive | — | `crates/platform-macos/src/event_tap.rs`, permission helpers, example logger binary. |
| PR-2 Snippet engine + repo + UI | PR-1 | `feature-input-services` crate skeleton, `SnippetEngine`, `SnippetRepo`, migration, settings page snippets tab, all 5 snippet Tauri commands. |
| PR-3 Hyper Key engine + UI | PR-1, PR-2 | `HyperKeyEngine`, hidutil Caps Lock remap, settings page hyper-key tab. |
| PR-4 Permission flow polish + failure recovery | PR-2, PR-3 | Permission banner + deep-link, `TapDied` desktop notification, `input_services_status` command. |

PR-1 is independently useful (a future cognitive-memory feature could subscribe). PR-2 ships snippets without touching Hyper Key. PR-3 adds hyper. PR-4 makes the whole thing trustworthy in production.

Each PR independently green: `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets --all-features` zero warnings, `cd desktop-ui && bun run lint && bun run test`.

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| CGEventTap permission denied → features silently broken | Lazy request + amber banner + denial-aware desktop notification |
| Caps Lock remap leaves keyboard inert if app crashes mid-run | hidutil mapping is per-session (resets on logout); document recovery in README |
| Snippet expansion clobbers user's clipboard | Save/restore around paste, 150 ms restore delay (longer than typical paste latency) |
| Long snippets (multi-KB) cause slow paste | Document soft limit (~10KB body); v1 has no hard cap |
| Trigger collides with normal typing (e.g. user types ";note") | Marker (`;`) + word-boundary detector make collisions explicit; users learn to avoid trigger names that match prefixes of words they type |
| Two snippets share a trigger | Settings page warns before save; longest match wins, deterministic id tie-break |
| Hyper Key + non-US keyboard layout | Source keycode is configurable; document common layout variations |
| Event tap consumes a system shortcut accidentally | Hyper engine only modifies events while `held == true`; passes everything else unchanged. Snippet engine only suppresses the boundary char on a successful match. |
| Permission revoked mid-session → tap dies | `TapDied` notification with explicit message about which features are affected; if Hyper Key enabled, additionally warn about Caps Lock |

## Out of Scope (explicit)

- **Snippet variables** (`{{date}}`, `{{clipboard}}`, `{{cursor}}`, shell snippets) — defer to follow-up.
- **Per-app snippet scoping** (e.g. "only expand `;sig` in Mail") — defer.
- **Karabiner-style multi-key chords** beyond Hyper Key — explicitly not pursued.
- **Hyper Key chord remapping** (e.g. `Hyper+L → launch Slack`) — that's the launcher's existing global-hotkey system's job, not Spec 2.
- **Browser/Linux ports** — macOS only.
- **MCP exposure** of snippet CRUD or hyper-key config — possible future work, separate spec.
- **Sync** of snippets across machines — defer to follow-up (would integrate with whatever cloud sync the broader app eventually adopts).
- **Rich-text snippets** (RTF, HTML) — plain text only for v1.

## Acceptance Criteria

- Type `;email` followed by space in TextEdit. `;email ` is replaced by the configured body within 200 ms. Clipboard is restored to its prior value within 300 ms.
- Type `;email` followed by space in a different app (Slack desktop). Same behavior.
- With Hyper Key enabled (Caps Lock source, Escape on tap), tapping Caps Lock alone produces an Escape keystroke in the focused app. Holding Caps Lock + L produces `Cmd+Ctrl+Opt+Shift+L` (verified via Key Codes app).
- Revoke Input Monitoring from System Settings → Klynt shows desktop notification within 5 seconds, settings page banner turns amber.
- Re-grant Input Monitoring → settings page detects on next launch, restart prompt appears.
- Disable both snippets and Hyper Key in config → no event tap is created (verified via `is_alive() == false` and absence of `input_services.tap.started` log line).
- Add a snippet via the settings page → it expands within ~50 ms on the next trigger (verifying watch-channel hot reload).
- All 4 PRs ship independently green; main branch never broken.
