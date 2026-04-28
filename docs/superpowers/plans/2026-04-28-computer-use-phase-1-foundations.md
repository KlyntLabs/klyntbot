# Computer Use — Phase 1 (Foundations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land two new platform-trait crates (`platform-input`, `platform-capture`), a macOS implementation module (`crates/platform-macos/src/computer_use/`), the missing `request_accessibility_for_input` permission primitive plus four Tauri commands surfacing it, and an `Info.plist` with the required usage descriptions — culminating in a macOS-gated smoke test that programmatically clicks at a screen coordinate and verifies the click landed. Zero behavior reaches the agent layer in this phase: no tool registration, no provider changes, no UI. Foundation only.

**Architecture:** Two new L0 trait crates carry platform-neutral types (`PlatformInput`, `PlatformCapture`, `ComputerUseAction`, `Frame`, `AccessibilityNode`, etc.), each with a `MockInput`/`MockCapture` impl for headless CI testing. The macOS impl in `platform-macos` uses raw `CGEvent` FFI via the existing `core-graphics 0.24` direct dependency for input injection (no new `enigo` dep), `screencapturekit 0.3` for ScreenCaptureKit single-frame capture (one new dep), and the existing raw-FFI AX pattern from `window.rs` for the accessibility-tree walker. Coordinate convention: logical points, Quartz top-left origin, virtual desktop space. The trait surface is platform-neutral; macOS quirks stay inside `platform-macos`. Permission flow gains the missing `AXIsProcessTrustedWithOptions` call so first-time users see the macOS "Klynt wants to control your computer" dialog instead of a silent failure.

**Tech Stack:** Rust (MSRV 1.93), `objc2 0.6` (existing), `objc2-app-kit 0.3` (existing — adding `NSScreen` feature), `core-graphics 0.24` (existing — using `CGEvent`/`CGEventPost` directly), `core-foundation 0.10` (existing), `screencapturekit 0.3` (NEW — single-frame capture via `SCStream`), `tokio::sync::Mutex` (for `MockInput` action recorder), existing `common::Result<T>` / `KlyntbotError`. No `enigo`, no `objc2-screen-capture-kit` (community alpha, defer).

---

## File Structure

Every file created or modified by this plan, grouped by responsibility. Each file has one focused responsibility.

### New crate: `crates/platform-input/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest; only `tokio` (for `Mutex` in `MockInput`), `thiserror` (for `PlatformError`) |
| `src/lib.rs` | `PlatformInput` trait, `ComputerUseAction` enum (16 variants matching `computer_20251124`), neutral types: `Rect`, `Point`, `KeyMods`, `MouseButton`, `ScrollDir`, `PlatformError` |
| `src/mock.rs` | `MockInput { recorded: Arc<Mutex<Vec<ComputerUseAction>>> }` impl + recorder accessor |
| `tests/mock_test.rs` | Verifies `MockInput::perform_action` records each action; verifies `release_all` does not record |

### New crate: `crates/platform-capture/`

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest; `tokio`, `thiserror`, `serde` (for `AccessibilityNode` JSON serialization) |
| `src/lib.rs` | `PlatformCapture` trait, neutral types: `Frame { width, height, scale, format, data }`, `AccessibilityNode { role, label, value, frame, children, attrs }`, `WindowInfo { id, title, bundle_id, frame, screen_id }`, `DisplayInfo { id, frame, scale, name }`, `AxScope { FullDesktop, ActiveApp, Window(WindowId) }`, `PixelFormat { Bgra, Rgba }` |
| `src/mock.rs` | `MockCapture` with fixture-loadable frames + AX trees; recorder accessor |
| `tests/mock_test.rs` | Verifies `MockCapture` returns injected fixtures |

### New module: `crates/platform-macos/src/computer_use/`

| File | Responsibility |
|---|---|
| `mod.rs` | Module exports; `cfg(target_os = "macos")` gating |
| `input.rs` | `MacInput` impl of `PlatformInput`. Uses `core-graphics 0.24` directly for `CGEvent` creation/posting. Caches one `CGEventSource` per instance. All methods callable from `spawn_blocking` workers (CGEvent is thread-safe per Apple) |
| `capture.rs` | `MacCapture` impl of `PlatformCapture`. Uses `screencapturekit 0.3` for screen capture; `NSScreen.backingScaleFactor` for DPR |
| `ax_tree.rs` | `walk_focused_app(pid, max_depth)` — raw AX FFI matching the pattern in `crates/platform-macos/src/window.rs`. Converts AppKit bottom-left coords → Quartz top-left at construction |

### Modified existing files

| File | Change |
|---|---|
| `Cargo.toml` (workspace root) | Add `crates/platform-input`, `crates/platform-capture` to `members` array |
| `crates/platform-macos/Cargo.toml` | Add `screencapturekit = "0.3"` dep; add `NSScreen` to `objc2-app-kit` features list; add `platform-input`, `platform-capture` workspace deps |
| `crates/platform-macos/src/lib.rs` | Add `#[cfg(target_os = "macos")] pub mod computer_use;` |
| `crates/desktop-shared/src/permissions.rs` | Add `request_accessibility_for_input() -> bool` calling `AXIsProcessTrustedWithOptions` |
| `crates/desktop/src/commands/permissions.rs` | Add 4 new commands wrapping the desktop-shared functions |
| `crates/desktop/src/specta_builder.rs` | Add 4 new command names to `SPECTA_COMMAND_NAMES`; add to `collect_commands![...]` |
| `crates/desktop/tauri.conf.json` | Add `bundle.macOS.infoPlist` with `NSScreenCaptureUsageDescription` and `NSAppleEventsUsageDescription` |
| `desktop-ui/src/bindings.ts` | Auto-regenerated by `bindings_are_current` test |
| `CLAUDE.md` | Update workspace crate count from 37 → 39 |

### Test files

| File | Purpose |
|---|---|
| `crates/platform-macos/tests/computer_use_smoke.rs` | macOS-only, gated by `KLYNT_E2E_COMPUTER_USE=1` env var. End-to-end smoke: instantiate `MacInput`, perform `MouseMove` to known coordinate, verify `get_cursor_position()` reports it. Skipped in default CI |

---

## Tasks

### Task 1: Workspace scaffolding for `platform-input` crate

**Files:**
- Create: `crates/platform-input/Cargo.toml`
- Create: `crates/platform-input/src/lib.rs` (initially empty placeholder)
- Modify: `Cargo.toml` (workspace root) — add member

- [ ] **Step 1: Add the new crate to the workspace members list**

Open `Cargo.toml` at the workspace root. Find the `members = [...]` array and add `crates/platform-input` in alphabetical position (before `crates/platform-macos`). Show the exact location:

```toml
[workspace]
members = [
    # ...existing members...
    "crates/notifications",
    "crates/platform-input",       # ← ADD THIS LINE
    "crates/platform-macos",
    "crates/plugin-runtime",
    # ...
]
```

- [ ] **Step 2: Create the new crate manifest**

Create `crates/platform-input/Cargo.toml`:

```toml
[package]
name = "platform-input"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { workspace = true, features = ["sync"] }
thiserror = { workspace = true }
```

- [ ] **Step 3: Create the empty lib.rs placeholder**

Create `crates/platform-input/src/lib.rs`:

```rust
//! Platform-neutral input injection trait.
//!
//! Defines `PlatformInput`, `ComputerUseAction`, and neutral coordinate types.
//! macOS impl lives in `platform-macos::computer_use::input::MacInput`.
```

- [ ] **Step 4: Verify the crate compiles**

Run: `cargo check -p platform-input`
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/platform-input/
git commit -m "feat(platform-input): scaffold new L0 trait crate"
```

---

### Task 2: Neutral coordinate and modifier types

**Files:**
- Modify: `crates/platform-input/src/lib.rs`

- [ ] **Step 1: Add the neutral types to `lib.rs`**

Append the following to `crates/platform-input/src/lib.rs`:

```rust
use serde::{Deserialize, Serialize};

/// A point in the global virtual desktop coordinate space (logical points,
/// Quartz top-left origin). On Retina displays this is logical points, not
/// physical pixels — `CGEvent` accepts these values directly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A rectangle in the global virtual desktop coordinate space (logical
/// points, Quartz top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Scroll direction. Amount is interpreted as line counts (positive = the
/// natural direction the user would expect for that axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollDir {
    Up,
    Down,
    Left,
    Right,
}

/// Modifier-key state for click/key actions. Each flag corresponds to a
/// physical modifier; combinations (e.g. `cmd | shift`) are supported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMods {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub fn_key: bool,
}
```

Also add `serde = { workspace = true, features = ["derive"] }` to the crate's `[dependencies]` in `Cargo.toml`.

- [ ] **Step 2: Verify the crate compiles**

Run: `cargo check -p platform-input`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-input/
git commit -m "feat(platform-input): add neutral coordinate + modifier types"
```

---

### Task 3: `ComputerUseAction` enum mirroring `computer_20251124`

**Files:**
- Modify: `crates/platform-input/src/lib.rs`

- [ ] **Step 1: Add the action enum**

Append to `crates/platform-input/src/lib.rs`:

```rust
/// Action vocabulary mirroring Anthropic's `computer_20251124` tool 1:1.
///
/// All coordinates are global-desktop logical points (Quartz top-left
/// origin). Each variant corresponds exactly to one Anthropic action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComputerUseAction {
    /// Take a screenshot of the full desktop or a specified region.
    Screenshot { region: Option<Rect> },

    /// Single left-button click at (x, y) with optional modifiers held.
    LeftClick { x: i32, y: i32, modifiers: KeyMods },

    /// Two left clicks within the system double-click interval.
    DoubleClick { x: i32, y: i32, modifiers: KeyMods },

    /// Three left clicks in rapid succession.
    TripleClick { x: i32, y: i32, modifiers: KeyMods },

    /// Right-button click at (x, y).
    RightClick { x: i32, y: i32 },

    /// Middle-button click at (x, y).
    MiddleClick { x: i32, y: i32 },

    /// Type a UTF-8 string. Implementations should use the system's
    /// current keyboard layout for ASCII printable characters.
    Type { text: String },

    /// Press a key combination. Each entry is a key name or modifier
    /// (e.g. `["cmd", "shift", "t"]`).
    Key { keys: Vec<String> },

    /// Move the cursor to (x, y) without clicking.
    MouseMove { x: i32, y: i32 },

    /// Scroll at (x, y) in the given direction by `amount` lines.
    Scroll { x: i32, y: i32, direction: ScrollDir, amount: i32 },

    /// Click-and-drag from `from` to `to`, optionally holding modifiers
    /// during the drag.
    LeftClickDrag { from: Point, to: Point, hold_modifiers: KeyMods },

    /// Press the left button at (x, y) without releasing. Use with
    /// `LeftMouseUp` for manual drag sequences.
    LeftMouseDown { x: i32, y: i32 },

    /// Release the left button at (x, y).
    LeftMouseUp { x: i32, y: i32 },

    /// Hold a key combination for `duration_ms`.
    HoldKey { keys: Vec<String>, duration_ms: u32 },

    /// Sleep `duration_ms` milliseconds. Used to wait for animations.
    Wait { duration_ms: u32 },

    /// Render `region` at full resolution (may be implemented as a higher-
    /// scale capture).
    Zoom { region: Rect },
}
```

- [ ] **Step 2: Verify the crate compiles**

Run: `cargo check -p platform-input`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-input/
git commit -m "feat(platform-input): add ComputerUseAction enum (16 variants)"
```

---

### Task 4: `PlatformInput` trait + `PlatformError`

**Files:**
- Modify: `crates/platform-input/src/lib.rs`

- [ ] **Step 1: Add the error type**

Append to `crates/platform-input/src/lib.rs`:

```rust
use thiserror::Error;

/// Errors produced by `PlatformInput` implementations.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid coordinates: ({x}, {y})")]
    InvalidCoordinates { x: i32, y: i32 },

    #[error("unsupported key: {0}")]
    UnsupportedKey(String),

    #[error("platform call failed: {0}")]
    PlatformCallFailed(String),

    #[error("not implemented on this platform")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, PlatformError>;
```

- [ ] **Step 2: Add the `PlatformInput` trait**

Append to `crates/platform-input/src/lib.rs`:

```rust
use async_trait::async_trait;

/// Trait implemented by per-platform input injection backends.
///
/// All methods are `async` so implementations may dispatch to a
/// `spawn_blocking` worker. CGEvent on macOS is itself thread-safe;
/// the async signature allows future Wayland-style implementations
/// that require message passing.
#[async_trait]
pub trait PlatformInput: Send + Sync {
    /// Execute a single action. Implementations must serialize the
    /// underlying OS calls so two concurrent `perform_action` calls
    /// on the same instance do not race.
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()>;

    /// Return the current cursor position in logical points (Quartz
    /// top-left origin).
    async fn get_cursor_position(&self) -> Result<Point>;

    /// Release any held mouse buttons or modifier keys. Called by the
    /// emergency-stop hotkey hook to ensure the system is left in a
    /// clean state when an in-progress action is aborted.
    async fn release_all(&self) -> Result<()>;
}
```

Add `async-trait = { workspace = true }` to the crate's `[dependencies]` in `Cargo.toml`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p platform-input`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/platform-input/
git commit -m "feat(platform-input): add PlatformInput trait + PlatformError"
```

---

### Task 5: `MockInput` implementation + tests

**Files:**
- Create: `crates/platform-input/src/mock.rs`
- Modify: `crates/platform-input/src/lib.rs` (add `pub mod mock;`)
- Create: `crates/platform-input/tests/mock_test.rs`

- [ ] **Step 1: Create the mock module**

Create `crates/platform-input/src/mock.rs`:

```rust
//! `MockInput` — records actions for testing without invoking the OS.

use crate::{ComputerUseAction, PlatformInput, Point, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test-only `PlatformInput` implementation. Records every action it
/// receives in an internal `Vec` and exposes them via `recorded()`.
#[derive(Debug, Default, Clone)]
pub struct MockInput {
    recorded: Arc<Mutex<Vec<ComputerUseAction>>>,
    cursor: Arc<Mutex<Point>>,
}

impl MockInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of recorded actions in arrival order.
    pub async fn recorded(&self) -> Vec<ComputerUseAction> {
        self.recorded.lock().await.clone()
    }

    /// Clear the recorded action log.
    pub async fn clear(&self) {
        self.recorded.lock().await.clear();
    }
}

#[async_trait]
impl PlatformInput for MockInput {
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()> {
        // Update the simulated cursor for movement actions so
        // `get_cursor_position` returns a sensible value.
        match &action {
            ComputerUseAction::MouseMove { x, y }
            | ComputerUseAction::LeftClick { x, y, .. }
            | ComputerUseAction::DoubleClick { x, y, .. }
            | ComputerUseAction::TripleClick { x, y, .. }
            | ComputerUseAction::RightClick { x, y }
            | ComputerUseAction::MiddleClick { x, y }
            | ComputerUseAction::LeftMouseDown { x, y }
            | ComputerUseAction::LeftMouseUp { x, y } => {
                let mut cursor = self.cursor.lock().await;
                cursor.x = *x as f64;
                cursor.y = *y as f64;
            }
            ComputerUseAction::LeftClickDrag { to, .. } => {
                let mut cursor = self.cursor.lock().await;
                cursor.x = to.x;
                cursor.y = to.y;
            }
            _ => {}
        }
        self.recorded.lock().await.push(action);
        Ok(())
    }

    async fn get_cursor_position(&self) -> Result<Point> {
        Ok(*self.cursor.lock().await)
    }

    async fn release_all(&self) -> Result<()> {
        // No-op for mock — does not record.
        Ok(())
    }
}
```

- [ ] **Step 2: Export the module**

Append to `crates/platform-input/src/lib.rs`:

```rust
pub mod mock;
```

- [ ] **Step 3: Write the integration test**

Create `crates/platform-input/tests/mock_test.rs`:

```rust
use platform_input::{
    mock::MockInput, ComputerUseAction, KeyMods, PlatformInput,
};

#[tokio::test]
async fn records_actions_in_arrival_order() {
    let mock = MockInput::new();
    mock.perform_action(ComputerUseAction::MouseMove { x: 100, y: 200 })
        .await
        .unwrap();
    mock.perform_action(ComputerUseAction::LeftClick {
        x: 100,
        y: 200,
        modifiers: KeyMods::default(),
    })
    .await
    .unwrap();

    let recorded = mock.recorded().await;
    assert_eq!(recorded.len(), 2);
    matches!(recorded[0], ComputerUseAction::MouseMove { x: 100, y: 200 });
    matches!(
        recorded[1],
        ComputerUseAction::LeftClick { x: 100, y: 200, .. }
    );
}

#[tokio::test]
async fn cursor_position_reflects_movement() {
    let mock = MockInput::new();
    mock.perform_action(ComputerUseAction::MouseMove { x: 50, y: 75 })
        .await
        .unwrap();
    let pos = mock.get_cursor_position().await.unwrap();
    assert_eq!(pos.x, 50.0);
    assert_eq!(pos.y, 75.0);
}

#[tokio::test]
async fn release_all_does_not_record() {
    let mock = MockInput::new();
    mock.release_all().await.unwrap();
    assert!(mock.recorded().await.is_empty());
}
```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `cargo nextest run -p platform-input`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/platform-input/
git commit -m "feat(platform-input): add MockInput with action recording + tests"
```

---

### Task 6: Workspace scaffolding for `platform-capture` crate

**Files:**
- Create: `crates/platform-capture/Cargo.toml`
- Create: `crates/platform-capture/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add the new crate to the workspace members list**

Open `Cargo.toml` at the workspace root and add `crates/platform-capture` in alphabetical order (right after `crates/platform-input`):

```toml
[workspace]
members = [
    # ...
    "crates/platform-input",
    "crates/platform-capture",     # ← ADD THIS LINE
    "crates/platform-macos",
    # ...
]
```

- [ ] **Step 2: Create the manifest**

Create `crates/platform-capture/Cargo.toml`:

```toml
[package]
name = "platform-capture"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { workspace = true, features = ["sync"] }
thiserror = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true, features = ["derive"] }
platform-input = { path = "../platform-input" }
```

- [ ] **Step 3: Create the empty lib.rs placeholder**

Create `crates/platform-capture/src/lib.rs`:

```rust
//! Platform-neutral screen-capture and accessibility-tree trait.
//!
//! Defines `PlatformCapture`, `Frame`, `AccessibilityNode`, `WindowInfo`,
//! and `DisplayInfo`. macOS impl lives in
//! `platform-macos::computer_use::capture::MacCapture`.
```

- [ ] **Step 4: Verify the crate compiles**

Run: `cargo check -p platform-capture`
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/platform-capture/
git commit -m "feat(platform-capture): scaffold new L0 trait crate"
```

---

### Task 7: `platform-capture` neutral types (`Frame`, `AccessibilityNode`, `WindowInfo`, `DisplayInfo`)

**Files:**
- Modify: `crates/platform-capture/src/lib.rs`

- [ ] **Step 1: Add all neutral types**

Append to `crates/platform-capture/src/lib.rs`:

```rust
use platform_input::Rect;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pixel format of a captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    /// 8 bits per channel, channels in BGRA order, no premultiplication.
    /// This is the default ScreenCaptureKit output.
    Bgra8,
    /// 8 bits per channel, channels in RGBA order.
    Rgba8,
}

/// A captured screen frame. Reports physical pixels in `width`/`height`
/// with `scale` carrying the backing scale factor (e.g. `2.0` for Retina).
/// Consumers compute logical points by dividing pixel dimensions by
/// `scale`.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

/// Identifier for a window in the active session. On macOS this is the
/// `CGWindowID`.
pub type WindowId = u32;

/// Identifier for a display. On macOS this is the `CGDirectDisplayID`.
pub type DisplayId = u32;

/// Information about a single window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub bundle_id: Option<String>,
    pub frame: Rect,
    pub screen_id: DisplayId,
    pub is_focused: bool,
    pub is_minimized: bool,
}

/// Information about a single physical display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: DisplayId,
    pub frame: Rect,
    pub scale: f64,
    pub name: String,
    pub is_primary: bool,
}

/// Scope hint passed to `get_ax_tree` to limit traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AxScope {
    /// Walk every visible window in every running app. Expensive.
    FullDesktop,
    /// Walk the AX tree of the currently focused application.
    ActiveApp,
    /// Walk the AX tree rooted at the given window.
    Window(WindowId),
}

/// A node in the platform-neutral accessibility tree. Coordinates in
/// `frame` are logical points, Quartz top-left origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityNode {
    /// AX role (e.g. `"AXButton"`, `"AXTextField"`, `"AXWindow"`).
    pub role: String,
    /// Human-readable label. Sourced from `AXTitle` if present, falling
    /// back to `AXDescription`.
    pub label: Option<String>,
    /// Current value (e.g. text-field contents). Sourced from `AXValue`.
    pub value: Option<String>,
    /// Bounding rectangle of the element in logical points (Quartz
    /// top-left origin). Empty rect if the element has no frame.
    pub frame: Rect,
    /// Direct children. May be empty.
    pub children: Vec<AccessibilityNode>,
    /// Additional attributes (e.g. `AXHelp`, `AXRoleDescription`,
    /// `aria-label` on web content). Empty if none.
    #[serde(default)]
    pub attrs: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("display not found: {0}")]
    DisplayNotFound(DisplayId),

    #[error("window not found: {0}")]
    WindowNotFound(WindowId),

    #[error("capture failed: {0}")]
    CaptureFailed(String),

    #[error("ax tree unavailable: {0}")]
    AxTreeUnavailable(String),

    #[error("not implemented on this platform")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, CaptureError>;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-capture`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-capture/
git commit -m "feat(platform-capture): add neutral types (Frame, AccessibilityNode, WindowInfo)"
```

---

### Task 8: `PlatformCapture` trait

**Files:**
- Modify: `crates/platform-capture/src/lib.rs`

- [ ] **Step 1: Add the trait**

Append to `crates/platform-capture/src/lib.rs`:

```rust
use async_trait::async_trait;

/// Trait implemented by per-platform screen-capture and accessibility-
/// tree backends.
#[async_trait]
pub trait PlatformCapture: Send + Sync {
    /// Capture a screen frame. If `region` is `None`, capture the full
    /// virtual desktop. The returned `Frame` carries physical pixels
    /// with `scale` indicating backing scale factor.
    async fn capture_screen(&self, region: Option<Rect>) -> Result<Frame>;

    /// Capture a single window by id.
    async fn capture_window(&self, window_id: WindowId) -> Result<Frame>;

    /// Enumerate active displays.
    async fn list_displays(&self) -> Result<Vec<DisplayInfo>>;

    /// Return information about the frontmost window across all apps.
    /// `None` if no window is frontmost (e.g. all apps are hidden).
    async fn get_active_window(&self) -> Result<Option<WindowInfo>>;

    /// Walk the accessibility tree at the given scope. Implementations
    /// should bound traversal depth to a reasonable default (the
    /// rendered tree of complex apps may have thousands of nodes).
    async fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode>;
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-capture`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-capture/
git commit -m "feat(platform-capture): add PlatformCapture trait"
```

---

### Task 9: `MockCapture` implementation + tests

**Files:**
- Create: `crates/platform-capture/src/mock.rs`
- Modify: `crates/platform-capture/src/lib.rs` (add `pub mod mock;`)
- Create: `crates/platform-capture/tests/mock_test.rs`

- [ ] **Step 1: Create the mock module**

Create `crates/platform-capture/src/mock.rs`:

```rust
//! `MockCapture` — returns fixture frames + AX trees for testing.

use crate::{
    AccessibilityNode, AxScope, CaptureError, DisplayInfo, Frame,
    PixelFormat, PlatformCapture, Result, WindowId, WindowInfo,
};
use async_trait::async_trait;
use platform_input::Rect;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test-only `PlatformCapture` implementation backed by injectable
/// fixtures. Use the setter methods to populate fixtures, then call
/// the trait methods to retrieve them.
#[derive(Debug, Default, Clone)]
pub struct MockCapture {
    frame: Arc<Mutex<Option<Frame>>>,
    ax_tree: Arc<Mutex<Option<AccessibilityNode>>>,
    displays: Arc<Mutex<Vec<DisplayInfo>>>,
    active_window: Arc<Mutex<Option<WindowInfo>>>,
}

impl MockCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_frame(&self, frame: Frame) {
        *self.frame.lock().await = Some(frame);
    }

    pub async fn set_ax_tree(&self, tree: AccessibilityNode) {
        *self.ax_tree.lock().await = Some(tree);
    }

    pub async fn set_displays(&self, displays: Vec<DisplayInfo>) {
        *self.displays.lock().await = displays;
    }

    pub async fn set_active_window(&self, window: Option<WindowInfo>) {
        *self.active_window.lock().await = window;
    }

    /// Build a 4×4 BGRA test frame with a known checkerboard pattern.
    /// Useful for tests that verify pixel-data round-trips.
    pub fn checkerboard_frame() -> Frame {
        let mut data = Vec::with_capacity(64);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v: u8 = if (x + y) % 2 == 0 { 255 } else { 0 };
                data.extend_from_slice(&[v, v, v, 255]); // BGRA
            }
        }
        Frame {
            width: 4,
            height: 4,
            scale: 1.0,
            format: PixelFormat::Bgra8,
            data,
        }
    }
}

#[async_trait]
impl PlatformCapture for MockCapture {
    async fn capture_screen(&self, _region: Option<Rect>) -> Result<Frame> {
        self.frame
            .lock()
            .await
            .clone()
            .ok_or_else(|| CaptureError::CaptureFailed("no fixture frame".into()))
    }

    async fn capture_window(&self, _window_id: WindowId) -> Result<Frame> {
        self.capture_screen(None).await
    }

    async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        Ok(self.displays.lock().await.clone())
    }

    async fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        Ok(self.active_window.lock().await.clone())
    }

    async fn get_ax_tree(&self, _scope: AxScope) -> Result<AccessibilityNode> {
        self.ax_tree
            .lock()
            .await
            .clone()
            .ok_or_else(|| CaptureError::AxTreeUnavailable("no fixture tree".into()))
    }
}
```

- [ ] **Step 2: Export the module**

Append to `crates/platform-capture/src/lib.rs`:

```rust
pub mod mock;
```

- [ ] **Step 3: Write the integration test**

Create `crates/platform-capture/tests/mock_test.rs`:

```rust
use platform_capture::{
    mock::MockCapture, AccessibilityNode, AxScope, PlatformCapture,
};
use platform_input::Rect;
use std::collections::HashMap;

#[tokio::test]
async fn returns_fixture_frame() {
    let mock = MockCapture::new();
    let frame = MockCapture::checkerboard_frame();
    mock.set_frame(frame.clone()).await;

    let captured = mock.capture_screen(None).await.unwrap();
    assert_eq!(captured.width, 4);
    assert_eq!(captured.height, 4);
    assert_eq!(captured.data, frame.data);
}

#[tokio::test]
async fn returns_fixture_ax_tree() {
    let mock = MockCapture::new();
    let tree = AccessibilityNode {
        role: "AXWindow".into(),
        label: Some("Test".into()),
        value: None,
        frame: Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 },
        children: vec![],
        attrs: HashMap::new(),
    };
    mock.set_ax_tree(tree.clone()).await;

    let got = mock.get_ax_tree(AxScope::ActiveApp).await.unwrap();
    assert_eq!(got.role, "AXWindow");
    assert_eq!(got.label.as_deref(), Some("Test"));
}

#[tokio::test]
async fn empty_capture_returns_error() {
    let mock = MockCapture::new();
    let result = mock.capture_screen(None).await;
    assert!(result.is_err());
}
```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `cargo nextest run -p platform-capture`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/platform-capture/
git commit -m "feat(platform-capture): add MockCapture with fixture support + tests"
```

---

### Task 10: Update `platform-macos` Cargo.toml + scaffold `computer_use` module

**Files:**
- Modify: `crates/platform-macos/Cargo.toml`
- Modify: `crates/platform-macos/src/lib.rs`
- Create: `crates/platform-macos/src/computer_use/mod.rs`

- [ ] **Step 1: Update `platform-macos/Cargo.toml` with new deps + features**

Open `crates/platform-macos/Cargo.toml`. Locate the `[target.'cfg(target_os = "macos")'.dependencies]` block and:

- Add `screencapturekit = "0.3"` as a new entry
- Add `NSScreen` to the `objc2-app-kit` features list
- Add `platform-input = { path = "../platform-input" }` and `platform-capture = { path = "../platform-capture" }`

Final shape:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [
    "NSWorkspace",
    "NSRunningApplication",
    "NSPasteboard",
    "NSImage",
    "NSImageRep",
    "NSBitmapImageRep",
    "NSScreen",                # NEW
] }
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray", "NSData"] }
core-graphics = "0.24"
core-foundation = "0.10"
screencapturekit = "0.3"        # NEW
platform-input = { path = "../platform-input" }    # NEW
platform-capture = { path = "../platform-capture" } # NEW
```

- [ ] **Step 2: Add the cfg-gated module declaration**

Open `crates/platform-macos/src/lib.rs` and append:

```rust
#[cfg(target_os = "macos")]
pub mod computer_use;
```

- [ ] **Step 3: Create the module skeleton**

Create `crates/platform-macos/src/computer_use/mod.rs`:

```rust
//! macOS implementations of `platform-input::PlatformInput` and
//! `platform-capture::PlatformCapture`.
//!
//! - [`input::MacInput`]: CGEvent injection via `core-graphics`.
//! - [`capture::MacCapture`]: ScreenCaptureKit single-frame capture.
//! - [`ax_tree`]: AXUIElement tree walker.

pub mod ax_tree;
pub mod capture;
pub mod input;

pub use capture::MacCapture;
pub use input::MacInput;
```

- [ ] **Step 4: Create empty stubs so the module tree compiles**

Create `crates/platform-macos/src/computer_use/input.rs`:

```rust
//! `MacInput` — CGEvent-based input injection on macOS.
```

Create `crates/platform-macos/src/computer_use/capture.rs`:

```rust
//! `MacCapture` — ScreenCaptureKit-based screen capture on macOS.
```

Create `crates/platform-macos/src/computer_use/ax_tree.rs`:

```rust
//! AX tree walker: AXUIElement → `AccessibilityNode`.
```

- [ ] **Step 5: Verify the workspace compiles**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors. (`screencapturekit` will be downloaded by Cargo.)

- [ ] **Step 6: Commit**

```bash
git add crates/platform-macos/
git commit -m "feat(platform-macos): scaffold computer_use module + new deps"
```

---

### Task 11: `MacInput` skeleton + `CGEventSource` cache

**Files:**
- Modify: `crates/platform-macos/src/computer_use/input.rs`

- [ ] **Step 1: Add the struct + constructor**

Replace the contents of `crates/platform-macos/src/computer_use/input.rs` with:

```rust
//! `MacInput` — CGEvent-based input injection on macOS.
//!
//! Uses `core-graphics 0.24` directly; no `enigo` dependency.
//!
//! Threading: CGEvent is thread-safe per Apple. Methods are async to
//! match the `PlatformInput` trait but the underlying calls are
//! synchronous and may run on any thread; callers should dispatch
//! into a `spawn_blocking` worker if they need to avoid blocking the
//! tokio reactor.

use async_trait::async_trait;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use platform_input::{
    ComputerUseAction, PlatformError, PlatformInput, Point, Result,
};

pub struct MacInput {
    /// Cached CGEventSource. Apple states a single source can be
    /// reused across all events from the same logical actor.
    source: CGEventSource,
}

impl MacInput {
    /// Construct a new `MacInput`.
    ///
    /// Returns `PlatformCallFailed` if `CGEventSourceCreate` fails
    /// (extremely rare — typically only when the process lacks the
    /// underlying Quartz framework).
    pub fn new() -> Result<Self> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|()| PlatformError::PlatformCallFailed(
                "CGEventSourceCreate failed".into(),
            ))?;
        Ok(Self { source })
    }
}

#[async_trait]
impl PlatformInput for MacInput {
    async fn perform_action(&self, _action: ComputerUseAction) -> Result<()> {
        Err(PlatformError::NotImplemented)
    }

    async fn get_cursor_position(&self) -> Result<Point> {
        Err(PlatformError::NotImplemented)
    }

    async fn release_all(&self) -> Result<()> {
        Err(PlatformError::NotImplemented)
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors. (Stub returns `NotImplemented` for now.)

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/
git commit -m "feat(platform-macos): MacInput skeleton with CGEventSource cache"
```

---

### Task 12: `MacInput::get_cursor_position` + `MouseMove` action

**Files:**
- Modify: `crates/platform-macos/src/computer_use/input.rs`

- [ ] **Step 1: Implement `get_cursor_position`**

Replace the `get_cursor_position` body with a real implementation:

```rust
async fn get_cursor_position(&self) -> Result<Point> {
    use core_graphics::event::CGEvent;
    let event = CGEvent::new(self.source.clone()).map_err(|()| {
        PlatformError::PlatformCallFailed("CGEventCreate failed".into())
    })?;
    let loc = event.location();
    Ok(Point { x: loc.x, y: loc.y })
}
```

- [ ] **Step 2: Implement `MouseMove` action dispatch**

Replace the `perform_action` body with:

```rust
async fn perform_action(&self, action: ComputerUseAction) -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;

    match action {
        ComputerUseAction::MouseMove { x, y } => {
            let event = CGEvent::new_mouse_event(
                self.source.clone(),
                CGEventType::MouseMoved,
                CGPoint { x: x as f64, y: y as f64 },
                CGMouseButton::Left,
            )
            .map_err(|()| PlatformError::PlatformCallFailed(
                "CGEventCreateMouseEvent failed".into(),
            ))?;
            event.post(CGEventTapLocation::HID);
            Ok(())
        }
        _ => Err(PlatformError::NotImplemented),
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/platform-macos/src/computer_use/input.rs
git commit -m "feat(platform-macos): MacInput supports MouseMove + get_cursor_position"
```

---

### Task 13: `MacInput` click variants (`LeftClick`, `RightClick`, `MiddleClick`, `DoubleClick`, `TripleClick`)

**Files:**
- Modify: `crates/platform-macos/src/computer_use/input.rs`

- [ ] **Step 1: Add a private click helper**

Insert at the top of the `impl MacInput` block (above `pub fn new`):

```rust
fn post_click(&self, x: i32, y: i32, button: core_graphics::event::CGMouseButton, count: i64) -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventField, CGEventTapLocation, CGEventType};
    use core_graphics::geometry::CGPoint;

    let down_type = match button {
        core_graphics::event::CGMouseButton::Left => CGEventType::LeftMouseDown,
        core_graphics::event::CGMouseButton::Right => CGEventType::RightMouseDown,
        _ => CGEventType::OtherMouseDown,
    };
    let up_type = match button {
        core_graphics::event::CGMouseButton::Left => CGEventType::LeftMouseUp,
        core_graphics::event::CGMouseButton::Right => CGEventType::RightMouseUp,
        _ => CGEventType::OtherMouseUp,
    };
    let point = CGPoint { x: x as f64, y: y as f64 };

    for _ in 0..count {
        let down = CGEvent::new_mouse_event(self.source.clone(), down_type, point, button)
            .map_err(|()| PlatformError::PlatformCallFailed("CGEventCreate down failed".into()))?;
        // CGEventField::MouseEventClickState = 1
        down.set_integer_value_field(CGEventField::MouseEventClickState, count);
        down.post(CGEventTapLocation::HID);
        let up = CGEvent::new_mouse_event(self.source.clone(), up_type, point, button)
            .map_err(|()| PlatformError::PlatformCallFailed("CGEventCreate up failed".into()))?;
        up.set_integer_value_field(CGEventField::MouseEventClickState, count);
        up.post(CGEventTapLocation::HID);
    }
    Ok(())
}
```

- [ ] **Step 2: Add click action arms to `perform_action`**

Inside the `match action` block in `perform_action`, replace the catch-all `_` arm with the click variants:

```rust
ComputerUseAction::LeftClick { x, y, .. } => {
    self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 1)
}
ComputerUseAction::DoubleClick { x, y, .. } => {
    self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 2)
}
ComputerUseAction::TripleClick { x, y, .. } => {
    self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 3)
}
ComputerUseAction::RightClick { x, y } => {
    self.post_click(x, y, core_graphics::event::CGMouseButton::Right, 1)
}
ComputerUseAction::MiddleClick { x, y } => {
    self.post_click(x, y, core_graphics::event::CGMouseButton::Center, 1)
}
_ => Err(PlatformError::NotImplemented),
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/platform-macos/src/computer_use/input.rs
git commit -m "feat(platform-macos): MacInput click variants (left/right/middle/double/triple)"
```

---

### Task 14: `MacInput` typing (`Type`, `Key`, `HoldKey`)

**Files:**
- Modify: `crates/platform-macos/src/computer_use/input.rs`

- [ ] **Step 1: Add a key-code resolver helper**

Insert at module top (below the `use` block):

```rust
/// Map a Klynt-canonical key name to a macOS virtual key code.
/// Returns `None` for unknown names; callers may fall back to
/// per-character `CGEventKeyboardSetUnicodeString`.
fn key_name_to_virtual_code(name: &str) -> Option<u16> {
    match name.to_lowercase().as_str() {
        "enter" | "return" => Some(0x24),
        "tab" => Some(0x30),
        "space" => Some(0x31),
        "delete" | "backspace" => Some(0x33),
        "escape" | "esc" => Some(0x35),
        "left" => Some(0x7B),
        "right" => Some(0x7C),
        "down" => Some(0x7D),
        "up" => Some(0x7E),
        "cmd" | "command" => Some(0x37),
        "shift" => Some(0x38),
        "alt" | "option" => Some(0x3A),
        "ctrl" | "control" => Some(0x3B),
        "f1" => Some(0x7A),
        "f2" => Some(0x78),
        "f3" => Some(0x63),
        "f4" => Some(0x76),
        "f5" => Some(0x60),
        "f6" => Some(0x61),
        // a-z map to 0x00..0x1D in macOS QWERTY layout
        c if c.len() == 1 => {
            let ch = c.chars().next()?;
            match ch {
                'a' => Some(0x00), 'b' => Some(0x0B), 'c' => Some(0x08),
                'd' => Some(0x02), 'e' => Some(0x0E), 'f' => Some(0x03),
                'g' => Some(0x05), 'h' => Some(0x04), 'i' => Some(0x22),
                'j' => Some(0x26), 'k' => Some(0x28), 'l' => Some(0x25),
                'm' => Some(0x2E), 'n' => Some(0x2D), 'o' => Some(0x1F),
                'p' => Some(0x23), 'q' => Some(0x0C), 'r' => Some(0x0F),
                's' => Some(0x01), 't' => Some(0x11), 'u' => Some(0x20),
                'v' => Some(0x09), 'w' => Some(0x0D), 'x' => Some(0x07),
                'y' => Some(0x10), 'z' => Some(0x06),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Map a `KeyMods` to CGEvent flag bits.
fn mods_to_flags(m: platform_input::KeyMods) -> core_graphics::event::CGEventFlags {
    use core_graphics::event::CGEventFlags;
    let mut f = CGEventFlags::empty();
    if m.cmd { f |= CGEventFlags::CGEventFlagCommand; }
    if m.shift { f |= CGEventFlags::CGEventFlagShift; }
    if m.alt { f |= CGEventFlags::CGEventFlagAlternate; }
    if m.ctrl { f |= CGEventFlags::CGEventFlagControl; }
    f
}
```

- [ ] **Step 2: Add `Type` action arm**

Inside `perform_action`, add this arm (above the `_` catch-all):

```rust
ComputerUseAction::Type { text } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation};
    // Per-character via CGEventKeyboardSetUnicodeString (works for
    // any Unicode without virtual-key resolution).
    for ch in text.chars() {
        let down = CGEvent::new_keyboard_event(self.source.clone(), 0, true)
            .map_err(|()| PlatformError::PlatformCallFailed(
                "CGEventCreateKeyboardEvent down failed".into(),
            ))?;
        let s = ch.to_string();
        let utf16: Vec<u16> = s.encode_utf16().collect();
        down.set_string_from_utf16_unchecked(&utf16);
        down.post(CGEventTapLocation::HID);
        let up = CGEvent::new_keyboard_event(self.source.clone(), 0, false)
            .map_err(|()| PlatformError::PlatformCallFailed(
                "CGEventCreateKeyboardEvent up failed".into(),
            ))?;
        up.set_string_from_utf16_unchecked(&utf16);
        up.post(CGEventTapLocation::HID);
    }
    Ok(())
}
```

- [ ] **Step 3: Add `Key` action arm**

Inside `perform_action`, add this arm:

```rust
ComputerUseAction::Key { keys } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventFlags};
    // Resolve modifiers first, then the final key.
    let mut flags = CGEventFlags::empty();
    let mut final_key: Option<u16> = None;
    for k in &keys {
        match k.to_lowercase().as_str() {
            "cmd" | "command" => flags |= CGEventFlags::CGEventFlagCommand,
            "shift"           => flags |= CGEventFlags::CGEventFlagShift,
            "alt" | "option"  => flags |= CGEventFlags::CGEventFlagAlternate,
            "ctrl" | "control"=> flags |= CGEventFlags::CGEventFlagControl,
            other => {
                final_key = key_name_to_virtual_code(other)
                    .or(final_key); // first non-modifier wins
            }
        }
    }
    let vk = final_key.ok_or_else(|| {
        PlatformError::UnsupportedKey(format!("no terminal key in {:?}", keys))
    })?;
    let down = CGEvent::new_keyboard_event(self.source.clone(), vk, true)
        .map_err(|()| PlatformError::PlatformCallFailed("keyboard down failed".into()))?;
    down.set_flags(flags);
    down.post(CGEventTapLocation::HID);
    let up = CGEvent::new_keyboard_event(self.source.clone(), vk, false)
        .map_err(|()| PlatformError::PlatformCallFailed("keyboard up failed".into()))?;
    up.set_flags(flags);
    up.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 4: Add `HoldKey` action arm**

Inside `perform_action`, add:

```rust
ComputerUseAction::HoldKey { keys, duration_ms } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventFlags};
    let mut flags = CGEventFlags::empty();
    let mut final_key: Option<u16> = None;
    for k in &keys {
        match k.to_lowercase().as_str() {
            "cmd" | "command" => flags |= CGEventFlags::CGEventFlagCommand,
            "shift"           => flags |= CGEventFlags::CGEventFlagShift,
            "alt" | "option"  => flags |= CGEventFlags::CGEventFlagAlternate,
            "ctrl" | "control"=> flags |= CGEventFlags::CGEventFlagControl,
            other => final_key = key_name_to_virtual_code(other).or(final_key),
        }
    }
    let vk = final_key.ok_or_else(|| {
        PlatformError::UnsupportedKey(format!("no terminal key in {:?}", keys))
    })?;
    let down = CGEvent::new_keyboard_event(self.source.clone(), vk, true)
        .map_err(|()| PlatformError::PlatformCallFailed("keyboard down failed".into()))?;
    down.set_flags(flags);
    down.post(CGEventTapLocation::HID);
    tokio::time::sleep(std::time::Duration::from_millis(duration_ms as u64)).await;
    let up = CGEvent::new_keyboard_event(self.source.clone(), vk, false)
        .map_err(|()| PlatformError::PlatformCallFailed("keyboard up failed".into()))?;
    up.set_flags(flags);
    up.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 5: Add `Wait` arm (trivial)**

```rust
ComputerUseAction::Wait { duration_ms } => {
    tokio::time::sleep(std::time::Duration::from_millis(duration_ms as u64)).await;
    Ok(())
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/platform-macos/src/computer_use/input.rs
git commit -m "feat(platform-macos): MacInput typing/key/hold/wait actions"
```

---

### Task 15: `MacInput` scroll + drag + manual mouse-button actions

**Files:**
- Modify: `crates/platform-macos/src/computer_use/input.rs`

- [ ] **Step 1: Add `Scroll` arm**

Inside `perform_action`, add:

```rust
ComputerUseAction::Scroll { x, y, direction, amount } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, ScrollEventUnit};
    use platform_input::ScrollDir;
    // Move cursor first so the scroll lands at the right place.
    self.move_cursor(x, y)?;
    let (dy, dx) = match direction {
        ScrollDir::Up => (amount as i32, 0),
        ScrollDir::Down => (-(amount as i32), 0),
        ScrollDir::Left => (0, amount as i32),
        ScrollDir::Right => (0, -(amount as i32)),
    };
    let event = CGEvent::new_scroll_event(
        self.source.clone(),
        ScrollEventUnit::Line,
        2,
        dy,
        dx,
        0,
    )
    .map_err(|()| PlatformError::PlatformCallFailed(
        "CGEventCreateScrollWheelEvent failed".into(),
    ))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 2: Add `LeftMouseDown`/`LeftMouseUp` arms**

```rust
ComputerUseAction::LeftMouseDown { x, y } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    let event = CGEvent::new_mouse_event(
        self.source.clone(),
        CGEventType::LeftMouseDown,
        CGPoint { x: x as f64, y: y as f64 },
        CGMouseButton::Left,
    )
    .map_err(|()| PlatformError::PlatformCallFailed("LeftMouseDown failed".into()))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
ComputerUseAction::LeftMouseUp { x, y } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    let event = CGEvent::new_mouse_event(
        self.source.clone(),
        CGEventType::LeftMouseUp,
        CGPoint { x: x as f64, y: y as f64 },
        CGMouseButton::Left,
    )
    .map_err(|()| PlatformError::PlatformCallFailed("LeftMouseUp failed".into()))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 3: Add `LeftClickDrag` arm**

```rust
ComputerUseAction::LeftClickDrag { from, to, .. } => {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    // Press at `from`.
    let down = CGEvent::new_mouse_event(
        self.source.clone(),
        CGEventType::LeftMouseDown,
        CGPoint { x: from.x, y: from.y },
        CGMouseButton::Left,
    )
    .map_err(|()| PlatformError::PlatformCallFailed("drag down failed".into()))?;
    down.post(CGEventTapLocation::HID);
    // Drag through several intermediate points (more reliable than
    // a single jump for apps that watch dragged events).
    let steps = 16;
    for i in 1..=steps {
        let t = (i as f64) / (steps as f64);
        let p = CGPoint {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
        };
        let drag = CGEvent::new_mouse_event(
            self.source.clone(),
            CGEventType::LeftMouseDragged,
            p,
            CGMouseButton::Left,
        )
        .map_err(|()| PlatformError::PlatformCallFailed("drag step failed".into()))?;
        drag.post(CGEventTapLocation::HID);
        tokio::time::sleep(std::time::Duration::from_millis(8)).await;
    }
    // Release at `to`.
    let up = CGEvent::new_mouse_event(
        self.source.clone(),
        CGEventType::LeftMouseUp,
        CGPoint { x: to.x, y: to.y },
        CGMouseButton::Left,
    )
    .map_err(|()| PlatformError::PlatformCallFailed("drag up failed".into()))?;
    up.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 4: Add a private `move_cursor` helper**

Inside `impl MacInput`:

```rust
fn move_cursor(&self, x: i32, y: i32) -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    let event = CGEvent::new_mouse_event(
        self.source.clone(),
        CGEventType::MouseMoved,
        CGPoint { x: x as f64, y: y as f64 },
        CGMouseButton::Left,
    )
    .map_err(|()| PlatformError::PlatformCallFailed("MouseMoved failed".into()))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}
```

- [ ] **Step 5: Implement `release_all`**

Replace the `release_all` body:

```rust
async fn release_all(&self) -> Result<()> {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::geometry::CGPoint;
    // Force-release all mouse buttons at current position.
    let pos = self.get_cursor_position().await?;
    for btn in [CGMouseButton::Left, CGMouseButton::Right, CGMouseButton::Center] {
        let up_type = match btn {
            CGMouseButton::Left => CGEventType::LeftMouseUp,
            CGMouseButton::Right => CGEventType::RightMouseUp,
            _ => CGEventType::OtherMouseUp,
        };
        if let Ok(event) = CGEvent::new_mouse_event(
            self.source.clone(),
            up_type,
            CGPoint { x: pos.x, y: pos.y },
            btn,
        ) {
            event.post(CGEventTapLocation::HID);
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Add `Screenshot` and `Zoom` action stubs**

These actions delegate to `MacCapture` in the next phase; for now return `NotImplemented` explicitly so callers can detect the gap:

```rust
ComputerUseAction::Screenshot { .. } | ComputerUseAction::Zoom { .. } => {
    Err(PlatformError::NotImplemented)
}
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 8: Commit**

```bash
git add crates/platform-macos/src/computer_use/input.rs
git commit -m "feat(platform-macos): MacInput scroll/drag/release_all"
```

---

### Task 16: `MacCapture` skeleton with `screencapturekit` integration

**Files:**
- Modify: `crates/platform-macos/src/computer_use/capture.rs`

- [ ] **Step 1: Add the struct and constructor**

Replace the contents of `crates/platform-macos/src/computer_use/capture.rs`:

```rust
//! `MacCapture` — ScreenCaptureKit-based screen capture on macOS.
//!
//! Uses the `screencapturekit` crate (a safe wrapper over `SCStream`).
//! Single-frame captures wrap `SCScreenshotManager.captureImage`.

use async_trait::async_trait;
use platform_capture::{
    AccessibilityNode, AxScope, CaptureError, DisplayInfo, Frame,
    PixelFormat, PlatformCapture, Result, WindowId, WindowInfo,
};
use platform_input::Rect;

pub struct MacCapture {
    /// Default display id used when `capture_screen(None)` is called
    /// without a region. Resolved at construction via `CGMainDisplayID`.
    default_display_id: u32,
}

impl MacCapture {
    pub fn new() -> Result<Self> {
        // SAFETY: CGMainDisplayID is documented thread-safe.
        extern "C" {
            fn CGMainDisplayID() -> u32;
        }
        let id = unsafe { CGMainDisplayID() };
        Ok(Self { default_display_id: id })
    }
}

#[async_trait]
impl PlatformCapture for MacCapture {
    async fn capture_screen(&self, _region: Option<Rect>) -> Result<Frame> {
        // Phase 1 stub — full impl in Task 17.
        Err(CaptureError::NotImplemented)
    }

    async fn capture_window(&self, _window_id: WindowId) -> Result<Frame> {
        Err(CaptureError::NotImplemented)
    }

    async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        Err(CaptureError::NotImplemented)
    }

    async fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        Err(CaptureError::NotImplemented)
    }

    async fn get_ax_tree(&self, _scope: AxScope) -> Result<AccessibilityNode> {
        Err(CaptureError::NotImplemented)
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/computer_use/capture.rs
git commit -m "feat(platform-macos): MacCapture skeleton + CGMainDisplayID"
```

---

### Task 17: `MacCapture::list_displays`

**Files:**
- Modify: `crates/platform-macos/src/computer_use/capture.rs`

- [ ] **Step 1: Implement `list_displays` via `CGGetActiveDisplayList`**

Replace the `list_displays` body:

```rust
async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
    use platform_input::Rect;

    extern "C" {
        fn CGGetActiveDisplayList(
            max: u32,
            display_array: *mut u32,
            display_count: *mut u32,
        ) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
        fn CGDisplayBackingScaleFactor(display: u32) -> f64;
        fn CGMainDisplayID() -> u32;
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint { x: f64, y: f64 }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGSize { width: f64, height: f64 }

    const MAX_DISPLAYS: u32 = 32;
    let mut ids = [0u32; MAX_DISPLAYS as usize];
    let mut count: u32 = 0;
    // SAFETY: pointer + length passed correctly; CGGetActiveDisplayList
    // is documented thread-safe.
    let err = unsafe { CGGetActiveDisplayList(MAX_DISPLAYS, ids.as_mut_ptr(), &mut count) };
    if err != 0 {
        return Err(CaptureError::CaptureFailed(format!(
            "CGGetActiveDisplayList failed: {}", err
        )));
    }

    let main_id = unsafe { CGMainDisplayID() };
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let id = ids[i];
        let bounds = unsafe { CGDisplayBounds(id) };
        let scale = unsafe { CGDisplayBackingScaleFactor(id) };
        out.push(DisplayInfo {
            id,
            frame: Rect {
                x: bounds.origin.x,
                y: bounds.origin.y,
                w: bounds.size.width,
                h: bounds.size.height,
            },
            scale,
            name: format!("Display {}", id),
            is_primary: id == main_id,
        });
    }
    Ok(out)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/computer_use/capture.rs
git commit -m "feat(platform-macos): MacCapture::list_displays via CGGetActiveDisplayList"
```

---

### Task 18: `MacCapture::capture_screen` via ScreenCaptureKit

**Files:**
- Modify: `crates/platform-macos/src/computer_use/capture.rs`

- [ ] **Step 1: Implement single-frame capture**

Replace the `capture_screen` body:

```rust
async fn capture_screen(&self, _region: Option<Rect>) -> Result<Frame> {
    use screencapturekit::{
        sc_content_filter::{InitParams, SCContentFilter},
        sc_shareable_content::SCShareableContent,
        sc_stream_configuration::{PixelFormat as SckPixelFormat, SCStreamConfiguration},
        sc_screenshot_manager::SCScreenshotManager,
    };
    use tokio::task;

    let display_id = self.default_display_id;
    // SCK calls block; run on a blocking worker.
    let frame = task::spawn_blocking(move || -> Result<Frame> {
        let content = SCShareableContent::current()
            .map_err(|e| CaptureError::CaptureFailed(format!("SCShareableContent: {e:?}")))?;
        let display = content
            .displays()
            .into_iter()
            .find(|d| d.display_id() == display_id)
            .ok_or(CaptureError::DisplayNotFound(display_id))?;

        let filter = SCContentFilter::new(InitParams::Display(display.clone()));
        let mut cfg = SCStreamConfiguration::default();
        cfg.set_width(display.width());
        cfg.set_height(display.height());
        cfg.set_pixel_format(SckPixelFormat::BGRA8);
        cfg.set_scales_to_fit(false);

        let cg_image = SCScreenshotManager::capture_image(&filter, &cfg)
            .map_err(|e| CaptureError::CaptureFailed(format!("captureImage: {e:?}")))?;

        let width = cg_image.width() as u32;
        let height = cg_image.height() as u32;
        let data = cg_image.bgra_bytes();

        Ok(Frame {
            width,
            height,
            scale: 2.0, // TODO: replace with NSScreen.backingScaleFactor in Phase 2
            format: PixelFormat::Bgra8,
            data,
        })
    })
    .await
    .map_err(|e| CaptureError::CaptureFailed(format!("join error: {e}")))??;

    Ok(frame)
}
```

> Note: the exact `screencapturekit 0.3` API surface for `SCScreenshotManager::capture_image` and `SCContentFilter::new` may evolve; if a method name differs, consult the crate's `examples/` directory and adapt. The important shape — `spawn_blocking` + `SCShareableContent` lookup → `SCContentFilter` → `SCStreamConfiguration` → `SCScreenshotManager` — is stable.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors. (If `screencapturekit` API methods differ, adjust per the crate's actual API; the structural pattern stays the same.)

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/computer_use/capture.rs
git commit -m "feat(platform-macos): MacCapture::capture_screen via ScreenCaptureKit"
```

---

### Task 19: AX tree walker

**Files:**
- Modify: `crates/platform-macos/src/computer_use/ax_tree.rs`

- [ ] **Step 1: Add the walker function**

Replace the contents of `crates/platform-macos/src/computer_use/ax_tree.rs`:

```rust
//! AX tree walker: AXUIElement → `AccessibilityNode`.
//!
//! Uses the same raw-FFI pattern as `crates/platform-macos/src/window.rs`.
//! Coordinates are converted from AppKit (bottom-left) to Quartz
//! (top-left) at construction.

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFRelease, CFTypeRef, TCFType},
    string::CFString,
};
use platform_capture::{AccessibilityNode, CaptureError, Result};
use platform_input::Rect;
use std::collections::HashMap;
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> *mut c_void;
    fn AXUIElementCopyAttributeValue(
        element: *mut c_void,
        attribute: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXValueGetValue(value: CFTypeRef, the_type: u32, value_ptr: *mut c_void) -> bool;
}

const K_AX_VALUE_CG_POINT_TYPE: u32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: u32 = 2;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CGPoint { x: f64, y: f64 }
#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CGSize { width: f64, height: f64 }

/// Walk the AX tree of the given app, bounded by `max_depth`.
///
/// Returns the focused window's AX root with children populated.
pub fn walk_focused_app(pid: i32, max_depth: usize) -> Result<AccessibilityNode> {
    // SAFETY: pid is a real ProcessID; AXUIElementCreateApplication
    // returns a retained AXUIElementRef on success.
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return Err(CaptureError::AxTreeUnavailable(format!(
            "AXUIElementCreateApplication returned null for pid {pid}"
        )));
    }

    let focused = copy_attribute(app, "AXFocusedWindow")?;
    let result = walk(focused, max_depth);
    unsafe {
        CFRelease(focused);
        CFRelease(app);
    }
    result
}

fn walk(element: *mut c_void, depth_remaining: usize) -> Result<AccessibilityNode> {
    let role = copy_string_attribute(element, "AXRole").unwrap_or_default();
    let label = copy_string_attribute(element, "AXTitle")
        .or_else(|_| copy_string_attribute(element, "AXDescription"))
        .ok();
    let value = copy_string_attribute(element, "AXValue").ok();

    let frame = read_frame(element).unwrap_or(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });

    let children = if depth_remaining == 0 {
        Vec::new()
    } else {
        copy_children(element)?
            .into_iter()
            .filter_map(|child| {
                let r = walk(child, depth_remaining - 1).ok();
                unsafe { CFRelease(child) };
                r
            })
            .collect()
    };

    Ok(AccessibilityNode {
        role,
        label,
        value,
        frame,
        children,
        attrs: HashMap::new(),
    })
}

fn copy_attribute(element: *mut c_void, name: &str) -> Result<*mut c_void> {
    let key = CFString::new(name);
    let mut out: CFTypeRef = std::ptr::null_mut();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef() as _, &mut out)
    };
    if err != 0 || out.is_null() {
        return Err(CaptureError::AxTreeUnavailable(format!(
            "AXUIElementCopyAttributeValue({name}) failed: {err}"
        )));
    }
    Ok(out as *mut c_void)
}

fn copy_string_attribute(element: *mut c_void, name: &str) -> Result<String> {
    let raw = copy_attribute(element, name)?;
    let cfstr = unsafe { CFString::wrap_under_create_rule(raw as _) };
    Ok(cfstr.to_string())
}

fn copy_children(element: *mut c_void) -> Result<Vec<*mut c_void>> {
    let raw = match copy_attribute(element, "AXChildren") {
        Ok(r) => r,
        // No children attribute = leaf node, return empty.
        Err(_) => return Ok(Vec::new()),
    };
    let array = unsafe { CFArray::<CFTypeRef>::wrap_under_create_rule(raw as CFArrayRef) };
    let count = array.len();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        if let Some(item) = array.get(i) {
            // Each item is a CFTypeRef; we transfer ownership to the
            // caller (caller must CFRelease).
            let raw_item = *item as *mut c_void;
            // Retain because wrap_under_create_rule already holds the
            // array's reference; the items inside the array do not
            // get extra retains.
            unsafe {
                core_foundation::base::CFRetain(raw_item as _);
            }
            out.push(raw_item);
        }
    }
    Ok(out)
}

fn read_frame(element: *mut c_void) -> Result<Rect> {
    let pos_raw = copy_attribute(element, "AXPosition")?;
    let size_raw = copy_attribute(element, "AXSize")?;
    let mut pos = CGPoint::default();
    let mut size = CGSize::default();
    let pos_ok = unsafe {
        AXValueGetValue(pos_raw as _, K_AX_VALUE_CG_POINT_TYPE, &mut pos as *mut _ as *mut _)
    };
    let size_ok = unsafe {
        AXValueGetValue(size_raw as _, K_AX_VALUE_CG_SIZE_TYPE, &mut size as *mut _ as *mut _)
    };
    unsafe {
        CFRelease(pos_raw);
        CFRelease(size_raw);
    }
    if !pos_ok || !size_ok {
        return Err(CaptureError::AxTreeUnavailable("AXValueGetValue failed".into()));
    }
    // AXPosition is in AppKit space (bottom-left origin); we currently
    // pass it through as-is. Y-flip will be applied in Phase 4 once we
    // have a screen-height lookup. For Phase 1 the smoke test only
    // verifies tree traversal, not exact coordinates.
    Ok(Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    })
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/computer_use/ax_tree.rs
git commit -m "feat(platform-macos): AX tree walker with raw FFI"
```

---

### Task 20: Wire `walk_focused_app` into `MacCapture::get_ax_tree`

**Files:**
- Modify: `crates/platform-macos/src/computer_use/capture.rs`

- [ ] **Step 1: Implement `get_ax_tree`**

Replace the `get_ax_tree` body:

```rust
async fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode> {
    use crate::computer_use::ax_tree;
    use objc2::rc::Retained;
    use objc2_app_kit::NSWorkspace;
    use tokio::task;

    let pid = match scope {
        AxScope::ActiveApp => {
            // Resolve current frontmost app pid via NSWorkspace.
            // SAFETY: NSWorkspace is documented thread-safe in read-only contexts.
            let pid: i32 = unsafe {
                let workspace: Retained<NSWorkspace> = NSWorkspace::sharedWorkspace();
                let app = workspace.frontmostApplication();
                app.map(|a| a.processIdentifier()).unwrap_or(0)
            };
            if pid == 0 {
                return Err(CaptureError::AxTreeUnavailable("no frontmost app".into()));
            }
            pid
        }
        AxScope::FullDesktop => {
            return Err(CaptureError::AxTreeUnavailable(
                "FullDesktop scope not supported in Phase 1; use ActiveApp".into(),
            ))
        }
        AxScope::Window(_id) => {
            return Err(CaptureError::AxTreeUnavailable(
                "Window scope deferred to Phase 4".into(),
            ))
        }
    };

    task::spawn_blocking(move || ax_tree::walk_focused_app(pid, 6))
        .await
        .map_err(|e| CaptureError::AxTreeUnavailable(format!("join: {e}")))?
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p platform-macos`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/platform-macos/src/computer_use/capture.rs
git commit -m "feat(platform-macos): MacCapture::get_ax_tree (ActiveApp scope)"
```

---

### Task 21: Add `request_accessibility_for_input` to `desktop-shared`

**Files:**
- Modify: `crates/desktop-shared/src/permissions.rs`

- [ ] **Step 1: Add the function**

Open `crates/desktop-shared/src/permissions.rs` and find the existing `check_accessibility` function. Append below it:

```rust
/// Request the macOS Accessibility permission with prompt.
///
/// Calls `AXIsProcessTrustedWithOptions({"AXTrustedCheckOptionPrompt": true})`,
/// which presents the system "wants to control your computer" dialog
/// the *first* time the app needs Accessibility. Returns the current
/// trust state. On subsequent calls (after user grants or denies),
/// the dialog is not re-shown — the function simply returns the trust
/// state.
///
/// Required for CGEvent input injection (see `MacInput`).
#[cfg(target_os = "macos")]
pub fn request_accessibility_for_input() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
    // SAFETY: CFTypeRef from a CFDictionary is valid for the call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as _) }
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_for_input() -> bool {
    true
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p desktop-shared`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/permissions.rs
git commit -m "feat(desktop-shared): request_accessibility_for_input via AXIsProcessTrustedWithOptions"
```

---

### Task 22: Add 4 new Tauri commands surfacing the permissions

**Files:**
- Modify: `crates/desktop/src/commands/permissions.rs`
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Add the new Tauri commands**

Open `crates/desktop/src/commands/permissions.rs`. Find the existing `permissions_check_accessibility` and `permissions_open_accessibility` functions. Append below them:

```rust
use desktop_macros::klynt_command;
use desktop_shared::permissions;

#[klynt_command]
pub async fn permissions_request_accessibility_for_input() -> bool {
    permissions::request_accessibility_for_input()
}

#[klynt_command]
pub async fn permissions_check_screen_recording() -> bool {
    permissions::check_screen_recording()
}

#[klynt_command]
pub async fn permissions_request_screen_recording() -> bool {
    permissions::request_screen_recording()
}

#[klynt_command]
pub async fn permissions_open_screen_recording() -> () {
    permissions::open_screen_recording_settings()
}
```

- [ ] **Step 2: Register the commands in specta_builder**

Open `crates/desktop/src/specta_builder.rs` and:

a) Add the four command names to `SPECTA_COMMAND_NAMES` (alphabetical order):

```rust
"permissions_check_accessibility",
"permissions_check_screen_recording",       // ← ADD
"permissions_open_accessibility",
"permissions_open_screen_recording",        // ← ADD
"permissions_request_accessibility_for_input",  // ← ADD
"permissions_request_screen_recording",     // ← ADD
```

b) Add them inside `collect_commands![...]` in `build_specta()` (alphabetical, matching above):

```rust
crate::commands::permissions::permissions_check_accessibility,
crate::commands::permissions::permissions_check_screen_recording,        // ← ADD
crate::commands::permissions::permissions_open_accessibility,
crate::commands::permissions::permissions_open_screen_recording,         // ← ADD
crate::commands::permissions::permissions_request_accessibility_for_input,  // ← ADD
crate::commands::permissions::permissions_request_screen_recording,      // ← ADD
```

- [ ] **Step 3: Run the registration_drift test to verify wiring**

Run: `cargo nextest run -p desktop -E 'test(registration_drift)'`
Expected: pass.

- [ ] **Step 4: Regenerate TypeScript bindings**

Run: `cargo nextest run -p desktop -E 'test(bindings_are_current)'`
Expected: this test fails the first time, writes the new `desktop-ui/src/bindings.ts`, then panics. Run it a second time:

Run: `cargo nextest run -p desktop -E 'test(bindings_are_current)'`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/permissions.rs crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts
git commit -m "feat(desktop): wire 4 new permission Tauri commands + regen bindings"
```

---

### Task 23: Add `Info.plist` usage description keys

**Files:**
- Modify: `crates/desktop/tauri.conf.json`

- [ ] **Step 1: Add the `bundle.macOS.infoPlist` block**

Open `crates/desktop/tauri.conf.json`. Find the existing `"bundle"` section. Inside `"bundle"`, ensure there is a `"macOS"` object, and within it add an `"infoPlist"` block:

```json
"bundle": {
    "active": true,
    "targets": "all",
    "macOS": {
        "infoPlist": {
            "NSScreenCaptureUsageDescription": "Klynt needs screen recording to capture screenshots for computer-use automation.",
            "NSAppleEventsUsageDescription": "Klynt sends Apple Events to control other applications during computer-use automation."
        }
    }
}
```

If `"macOS"` already exists with other keys, add `"infoPlist"` alongside the existing keys (do not replace them).

- [ ] **Step 2: Verify Tauri picks up the changes (manual smoke)**

Run: `cargo tauri build --debug 2>&1 | head -30`
Expected: build proceeds. (No assertion needed beyond "compiles" — `Info.plist` validation happens at packaging time.)

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/tauri.conf.json
git commit -m "feat(desktop): add Info.plist usage descriptions for screen capture + apple events"
```

---

### Task 24: Phase 1 smoke test (macOS-gated, env-var-gated)

**Files:**
- Create: `crates/platform-macos/tests/computer_use_smoke.rs`

- [ ] **Step 1: Write the smoke test**

Create `crates/platform-macos/tests/computer_use_smoke.rs`:

```rust
//! Phase 1 smoke test: programmatic mouse-move + cursor-position read.
//!
//! Gated by `KLYNT_E2E_COMPUTER_USE=1` env var so it only runs when
//! explicitly invoked. Requires the running process to have Accessibility
//! permission granted.

#![cfg(target_os = "macos")]

use platform_input::{ComputerUseAction, PlatformInput};
use platform_macos::computer_use::MacInput;

fn enabled() -> bool {
    std::env::var("KLYNT_E2E_COMPUTER_USE").as_deref() == Ok("1")
}

#[tokio::test]
async fn move_mouse_and_read_position() {
    if !enabled() {
        eprintln!("skip: set KLYNT_E2E_COMPUTER_USE=1 to run");
        return;
    }

    let input = MacInput::new().expect("MacInput::new");

    // Move to a known coordinate.
    input
        .perform_action(ComputerUseAction::MouseMove { x: 200, y: 200 })
        .await
        .expect("MouseMove");

    // Read back; allow ±2 pixel tolerance for compositor rounding.
    let pos = input.get_cursor_position().await.expect("get_cursor_position");
    assert!(
        (pos.x - 200.0).abs() <= 2.0 && (pos.y - 200.0).abs() <= 2.0,
        "expected ~(200,200), got ({}, {})", pos.x, pos.y
    );
}
```

- [ ] **Step 2: Run the test in default-skip mode**

Run: `cargo nextest run -p platform-macos -E 'test(move_mouse_and_read_position)'`
Expected: test runs, prints "skip: set KLYNT_E2E_COMPUTER_USE=1 to run", and passes (no assertion failure because the function returns early).

- [ ] **Step 3: Run the test in real-execution mode (manual, requires Accessibility permission)**

Run: `KLYNT_E2E_COMPUTER_USE=1 cargo nextest run -p platform-macos -E 'test(move_mouse_and_read_position)' --no-capture`
Expected: cursor moves to (200, 200) on the user's screen and the test passes.

If the test fails with "permission denied" or the cursor does not move, run the helper to grant Accessibility:

```bash
cargo run -p desktop -- --request-accessibility   # if you wire a CLI flag, otherwise grant manually:
# 1. Open System Settings → Privacy & Security → Accessibility
# 2. Enable the Klynt or `cargo` binary
```

- [ ] **Step 4: Commit**

```bash
git add crates/platform-macos/tests/computer_use_smoke.rs
git commit -m "test(platform-macos): Phase 1 smoke test for mouse-move + cursor-position"
```

---

### Task 25: Update CLAUDE.md and Phase 1 sign-off

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update workspace crate count**

Open `CLAUDE.md`. Find the line:

```
### Workspace (37 crates, 9 layers)
```

Change to:

```
### Workspace (39 crates, 9 layers)
```

Find the L0 description line:

```
L0: common, platform-macos — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey; macOS native APIs (pasteboard, window mgmt)
```

Change to:

```
L0: common, platform-macos, platform-input, platform-capture — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey; macOS native APIs (pasteboard, window mgmt, computer-use input + capture); platform-neutral input/capture trait crates
```

- [ ] **Step 2: Verify the doc still scans cleanly**

Run: `git diff CLAUDE.md`
Expected: only the two lines above changed.

- [ ] **Step 3: Run the full workspace test suite for Phase 1 sign-off**

Run: `cargo nextest run --workspace -E 'package(platform-input) | package(platform-capture) | package(platform-macos) | package(desktop-shared)'`
Expected: all tests pass. (The macOS smoke test prints "skip" and passes; mock tests pass; permission tests pass.)

Also run clippy:

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings. (Fix any new ones introduced by Phase 1 inline.)

- [ ] **Step 4: Commit Phase 1 sign-off**

```bash
git add CLAUDE.md
git commit -m "docs(claude.md): bump crate count for platform-input + platform-capture (Phase 1 sign-off)"
```

---

## Phase 1 acceptance criteria

Phase 1 is complete when **all** of the following are true:

1. `cargo build --workspace` succeeds.
2. `cargo nextest run --workspace` succeeds (all default tests pass; smoke test prints "skip" and passes).
3. `cargo clippy --workspace --all-targets -- -D warnings` reports zero warnings.
4. `cargo test -p desktop -E 'test(registration_drift) | test(bindings_are_current)'` passes — TS bindings reflect the four new permission commands.
5. Manually setting `KLYNT_E2E_COMPUTER_USE=1` and running the smoke test moves the user's cursor to `(200, 200)` and verifies it via `get_cursor_position`.
6. `MacInput` supports every `ComputerUseAction` variant *except* `Screenshot` and `Zoom` (those return `NotImplemented` — wired in Phase 2 once the `MacCapture::capture_screen` integration is exercised by the agent layer).
7. `MacCapture::list_displays`, `MacCapture::capture_screen`, and `MacCapture::get_ax_tree(ActiveApp)` all return real data on macOS. `FullDesktop` and `Window(_)` AX scopes return a clear "deferred to later phase" error.
8. The `Info.plist` usage description keys are present in `tauri.conf.json` so `cargo tauri build` does not warn about missing privacy strings.

## What Phase 1 deliberately does NOT include

These are wired in subsequent phase plans:

- `feature-computer-use` crate (Phase 2)
- `ComputerUseTool` registration in the agent loop (Phase 2)
- Anthropic adapter `ContentPart::ImageData` + `computer_20251124` tool block (Phase 2)
- `RoutingContext::screenshot_tx` sidecar channel (Phase 2)
- `MidLoopCompressor` image-aware exception (Phase 2)
- Risk-tier classifier, scope locks, sensitive-surface patterns (Phase 3)
- HUD window, cursor overlay, action callouts, voice narration (Phase 3)
- Emergency-stop hotkey wiring (Phase 3)
- `agent_action_log` table + screenshot blob storage (Phase 3)
- Hybrid perception cascade (Phase 4)
- Local VLM provider integration (Phase 4)
- `feature-browser-control` + CDP integration (Phase 5)
- `web_tree_memories` + procedural memory + replay (Phase 6)
- `WorkflowInductionSignals` mirror source + reforge phase (Phase 7)
- Settings UI section + side panel (Phase 8)

Each will be a separate plan file in `docs/superpowers/plans/`.
