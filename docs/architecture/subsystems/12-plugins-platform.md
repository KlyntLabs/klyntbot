# Subsystem 12 — Platform Adapters

> **Status:** 🟠 Scaffolded
> Platform Computer Use surface exists (16 `ComputerUseAction` variants, full AX walker) but is **completely unwired** — no agent tool, Tauri command, or MCP tool routes to it.
> **Status last verified:** 2026-05-20
> **Crates:** `platform-input`, `platform-capture`, `platform-macos` *(3 crates)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Two trait crates (`platform-input`, `platform-capture`) + a macOS implementation (`platform-macos`). Full `ComputerUseAction` enum (16 variants matching Anthropic's `computer_20251124` tool 1:1). `MacInput` implements 14 of 16; `MacCapture` implements 3 of 5. **No agent tool, Tauri command, or MCP tool calls any of this.** The wiring layer is missing — Computer Use is **scaffolded, not shipped**.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef trait fill:#e8eaf6,stroke:#3949ab,color:#1a237e
    classDef impl fill:#fff3e0,stroke:#f57c00,color:#e65100
    classDef gap fill:#ffcdd2,stroke:#c62828,color:#b71c1c

    PI[PlatformInput trait<br/><i>perform_action<br/>get_cursor_position<br/>release_all</i>]:::trait
    PC[PlatformCapture trait<br/><i>capture_screen<br/>capture_window<br/>list_displays<br/>get_active_window<br/>get_ax_tree</i>]:::trait
    CUA[ComputerUseAction enum<br/><i>16 variants<br/>Anthropic computer_20251124 1:1</i>]:::trait

    MI[MacInput<br/><i>CGEvent<br/>14 of 16 actions impl<br/>Screenshot + Zoom NotImpl</i>]:::impl
    MC[MacCapture<br/><i>ScreenCaptureKit<br/>3 of 5 methods impl<br/>capture_window + get_active_window NotImpl<br/>scale hardcoded 2.0</i>]:::impl
    AX[walk_focused_app<br/><i>AX walker · depth 6<br/>frames in AppKit coords (NOT Quartz)</i>]:::impl

    OTHER[Other platform-macos modules<br/><i>speech (say CLI) · DnD (shortcuts)<br/>lifecycle (idle polling only)<br/>apps · browser · pasteboard · ax</i>]:::impl

    WIRE[Agent / Tauri / MCP wiring<br/>DOES NOT EXIST]:::gap

    MI --> PI
    MC --> PC
    MC --> AX
    PI --> CUA
    WIRE -.MISSING.-> MI
    WIRE -.MISSING.-> MC
```

---

## Mental model

**Platform:** `MacInput` can move a mouse, type text, click, drag, scroll. `MacCapture` can take a screenshot of the screen. `walk_focused_app` returns an AX tree of the frontmost app. But nothing inside `agent`, `klynt-core`, or `mcp` ever calls any of this.

The clearest mental model: this subsystem is **the floor of a future feature, not a current one**. A scaffold that awaits a wiring layer.

---

## Reference

### Platform traits

#### `PlatformInput` (in `platform-input`)

```rust
#[async_trait]
pub trait PlatformInput: Send + Sync {
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()>;
    async fn get_cursor_position(&self) -> Result<Point>;
    async fn release_all(&self) -> Result<()>;
}
```

Coordinates are logical points, **Quartz top-left origin** (note this differs from AX tree below).

#### `PlatformCapture` (in `platform-capture`)

```rust
#[async_trait]
pub trait PlatformCapture: Send + Sync {
    async fn capture_screen(&self, region: Option<Rect>) -> Result<Frame>;
    async fn capture_window(&self, window_id: WindowId) -> Result<Frame>;
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>>;
    async fn get_active_window(&self) -> Result<Option<WindowInfo>>;
    async fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode>;
}

pub enum AxScope { FullDesktop, ActiveApp, Window(WindowId) }
```

#### `ComputerUseAction` — all 16 variants

Tagged `#[serde(tag = "kind", rename_all = "snake_case")]`. Maps 1:1 to Anthropic's `computer_20251124` tool surface.

| Variant | Fields | `MacInput` impl |
|---|---|---|
| `Screenshot` | `region: Option<Rect>` | ❌ `NotImplemented` |
| `LeftClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `DoubleClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `TripleClick` | `x, y: i32, modifiers: KeyMods` | ✅ |
| `RightClick` | `x, y: i32` | ✅ |
| `MiddleClick` | `x, y: i32` | ✅ |
| `Type` | `text: String` | ✅ unicode strings via `set_string_from_utf16_unchecked` |
| `Key` | `keys: Vec<String>` | ✅ static virtual-key table (a-z + function keys) |
| `MouseMove` | `x, y: i32` | ✅ |
| `Scroll` | `x, y: i32, direction: ScrollDir, amount: i32` | ✅ moves cursor first, then posts `CGScrollWheelEvent` |
| `LeftClickDrag` | `from, to: Point, hold_modifiers: KeyMods` | ✅ **16 interpolated `LeftMouseDragged` events with 8 ms sleep** |
| `LeftMouseDown` | `x, y: i32` | ✅ |
| `LeftMouseUp` | `x, y: i32` | ✅ |
| `HoldKey` | `keys: Vec<String>, duration_ms: u32` | ✅ |
| `Wait` | `duration_ms: u32` | ✅ |
| `Zoom` | `region: Rect` | ❌ `NotImplemented` |

`MacInput::release_all` posts `LeftMouseUp` + `RightMouseUp` + `OtherMouseUp` at current cursor position — used as the emergency-stop primitive.

#### `AccessibilityNode` shape

```rust
pub struct AccessibilityNode {
    pub role: String,                                // e.g. "AXButton", "AXTextField"
    pub label: Option<String>,                       // AXTitle, fallback AXDescription
    pub value: Option<String>,                       // AXValue
    pub frame: Rect,                                 // ⚠️ AppKit coords (bottom-left), NOT Quartz
    pub children: Vec<AccessibilityNode>,
    pub attrs: HashMap<String, String>,              // currently always empty from walker
}
```

**⚠️ `frame` is in AppKit coordinate space (bottom-left origin), not Quartz (top-left).** Y-flip is a Phase 4 TODO. Consumers expecting Quartz coordinates will draw bounding boxes in the wrong place. The whole `PlatformInput`/`PlatformCapture` API documents Quartz; this AX walker accidentally returns the other space.

### `platform-macos` modules

| Module | Purpose | Notes |
|---|---|---|
| `capture.rs` | `MacCapture` — `screencapturekit::SCShareableContent` + `SCScreenshotManager::capture` inside `spawn_blocking`. | Scale hardcoded `2.0` (Retina assumption). `capture_window` + `get_active_window` return `NotImplemented`. |
| `input.rs` | `MacInput` — `CGEventSource` with `HIDSystemState`. | 14 of 16 actions implemented (see table above). |
| `computer_use/ax_tree.rs` | `walk_focused_app(pid, max_depth=6)` — `AXUIElement` + recursive children. | Depth bounded at 6. Frame y-flip not done. |
| `ax.rs` | RAII wrappers for `AXUIElement` and `AXValue` (`declare_TCFType!`/`impl_TCFType!`). | Safe — no manual `CFRelease` anywhere. |
| `speech.rs` | `list_voices()` parses `say --voice=?`; `synthesize_to_file(text, voice, rate, path)` → `say -v <voice> -r <wpm> -o <path>`. | **`say` CLI, NOT `AVSpeechSynthesizer`.** objc2 wiring deferred. |
| `dnd.rs` | `is_dnd_active()` reads `defaults read com.apple.controlcenter "NSStatusItem Visible FocusModes"`; `toggle_dnd()` calls `shortcuts run "Toggle Do Not Disturb"`. | **Requires user to manually create the Shortcut.** Brittle dependency. |
| `lifecycle.rs` | `LifecycleStateMachine` (pure state machine) + `LifecycleMonitor` (tokio loop polling `CGEventSourceSecondsSinceLastEventType`). | **NSWorkspace observers stubbed** (`willSleep`/`didWake` never fire). Only idle transitions work. |
| `apps.rs` | `running_applications()` (NSWorkspace `runningApplications`, filtered `Regular` activation policy); `AppIconCache` (PlistBuddy + `sips` for icns→PNG conversion). | **`AppIconCache` deliberately avoids NSWorkspace** to prevent IconServices mmap leak. |
| `browser.rs` | Static `BROWSERS` registry — 11 entries: Chrome, Arc, Brave, Edge, Safari, Firefox, Vivaldi, Opera, Chromium, Orion, **Zen**. | AppleScript via `osascript`; sanitizes app name to prevent injection. |
| `pasteboard.rs` | `pasteboard_change_count()` + `read_pasteboard_string()` via `objc2-app-kit::NSPasteboard`. | |
| `window.rs` | Window bridge: `get_frontmost_window()`, `get_frontmost_app_name()`, `get_screen_frame()`, `set_window_frame(pid, x, y, w, h)`. | All AX read/write paths through `crate::ax::AXUIElement`. |

---

## Computer Use wiring status

**Confirmed: no wiring exists.** Searching all crates outside `platform-macos`, `platform-capture`, and `platform-input` finds only `crates/desktop-shared/src/permissions.rs` referencing these types — for permission *checking*, not invocation.

**What exists:**
- Full `ComputerUseAction` enum (16 variants) matching Anthropic's `computer_20251124` tool 1:1
- `MacInput` with 14 of 16 actions implemented
- `MacCapture` with `capture_screen` + `list_displays` + `get_ax_tree(ActiveApp)` working
- AX tree walker (`walk_focused_app`) bounded at depth 6, tested in `platform-macos/tests/computer_use_smoke.rs`

**What's missing:**
- No `ToolKit` / `FeaturePackage` wrapping `MacInput`/`MacCapture`
- No agent-loop dispatcher handling `ComputerUseAction`
- No Tauri command or MCP tool surface
- No emergency-stop hotkey hook (referenced in `release_all` docstring but not wired)
- `get_active_window` not implemented
- `capture_window` returns `NotImplemented`
- AX frame y-flip (AppKit → Quartz) deferred
- `FullDesktop` and `Window(id)` AX scopes return errors
- Retina scale hardcoded at 2.0

---

## Workflows

### What a Computer Use call *would* look like (if wired)

```
LLM emits Anthropic computer tool call:
   { tool: "computer", action: "left_click", coordinate: [800, 600] }
   ↓
(MISSING) Adapter translates Anthropic tool input → ComputerUseAction::LeftClick { x: 800, y: 600, modifiers: KeyMods::default() }
   ↓
(MISSING) FeaturePackage / ToolKit calls PlatformInput::perform_action
   ↓
MacInput::perform_action(action):
   - Match LeftClick → post_click(x, y, modifiers, click_count=1)
   - CGEvent created with HID source
   - CGEventPost(kCGHIDEventTap, event)
   ↓
Returns Ok(()) — but no audit log, no risk gate, no approval check, no screen capture
```

---

## Internals

### `LifecycleMonitor` only detects idle

The state machine handles `on_idle_reading / on_will_sleep / on_did_wake / on_grace_expired`. The monitor wraps it in a polling loop over `CGEventSourceSecondsSinceLastEventType`. **NSWorkspace `willSleep`/`didWake` observers are stubbed** — they never fire. So sleep/wake transitions aren't detected; only idle is. This means cron jobs scheduled for "after wake" don't reliably fire on resume from sleep.

### `MacInput::LeftClickDrag` interpolation

```
for step in 1..=16:
    let t = step as f64 / 16.0;
    let x = from.x + (to.x - from.x) * t;
    let y = from.y + (to.y - from.y) * t;
    post LeftMouseDragged at (x, y)
    sleep 8ms
```

16 events × 8 ms = ~128 ms total drag duration. Trade-off: too few interpolation points and the drag looks teleported (some apps drop it); too many and it's slow. 16 is empirically the sweet spot.

### Why `AppIconCache` avoids NSWorkspace

The source comment explicitly cites `IconServices mmap leak`. NSWorkspace's icon-fetch APIs (`iconForFile:`) leak file descriptors per call. PlistBuddy + `sips` shells out (slower per call) but no leak. The cache (mtime-validated, on disk) makes the slow first call irrelevant for steady-state.

### `ax::declare_TCFType!` for AX safety

AX APIs use Core Foundation reference counting (`CFRetain`/`CFRelease`). Forgetting `CFRelease` leaks; double-`CFRelease` crashes. The `declare_TCFType!`/`impl_TCFType!` macros from `core_foundation` create wrappers that integrate with Rust's RAII — `Drop` calls `CFRelease` automatically. **No manual `CFRelease` anywhere in `platform-macos`.**

---

## Dependencies & extension points

### Upstream deps

- **platform-macos:** `objc2` family, `core_foundation`, `core_graphics`, `screencapturekit`, `system_configuration`

### Adding a `ComputerUseAction` variant

⚠️ Cross-cutting. Update `platform-input::ComputerUseAction` enum, then every `PlatformInput` implementation (only `MacInput` today). Even after that, **nothing dispatches to it** until the wiring layer exists. The variant alone changes nothing.

### Implementing `PlatformInput` / `PlatformCapture` for another OS

1. Create `crates/platform-<os>/` (template after `platform-macos`).
2. Implement both traits.
3. Document any unsupported variants — return `NotImplemented` cleanly.
4. **The wiring layer still doesn't exist** — your impl won't be called.

---

## Open questions & debt

- **Computer Use is unwired.** Platform infrastructure works; no agent/Tauri/MCP surface routes to it. Spec referenced is vapor.
- **AX frame y-coordinates are in AppKit space**, not Quartz. Y-flip is a Phase 4 TODO. Significant correctness gotcha for future Computer Use.
- **NSWorkspace sleep/wake observers stubbed.** Lifecycle only detects idle.
- **Speech via `say` CLI**, not `AVSpeechSynthesizer`. CLAUDE.md and earlier docs claim AVSpeech — wrong.
- **DnD toggle requires user-created Shortcut.** Brittle.
- **`MacInput::Screenshot` + `Zoom` return `NotImplemented`**; **`MacCapture::capture_window` + `get_active_window` return `NotImplemented`**.
- **Retina scale hardcoded 2.0** — should use `NSScreen.backingScaleFactor`.
- **Browser list includes Zen** — good coverage of modern Firefox forks, but no auto-detection mechanism if user adds a new browser.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs), #5 (doc drift), #7 (anomalies) for specifics.

---

## Cross-references

- [`08-assistant-features.md`](./08-assistant-features.md) — voice-engine speech wraps `platform-macos::speech`
- [`13-desktop-frontend.md`](./13-desktop-frontend.md) — Tauri layer is the only current caller of `platform-macos` modules
