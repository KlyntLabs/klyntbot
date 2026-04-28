//! Platform-neutral input injection trait.
//!
//! Defines `PlatformInput`, `ComputerUseAction`, and neutral coordinate types.
//! macOS impl lives in `platform-macos::computer_use::input::MacInput`.

use serde::{Deserialize, Serialize};

/// A point in the global virtual desktop coordinate space (logical points,
/// Quartz top-left origin). On Retina displays this is logical points, not
/// physical pixels — `CGEvent` accepts these values directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
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
    Scroll {
        x: i32,
        y: i32,
        direction: ScrollDir,
        amount: i32,
    },

    /// Click-and-drag from `from` to `to`, optionally holding modifiers
    /// during the drag.
    LeftClickDrag {
        from: Point,
        to: Point,
        hold_modifiers: KeyMods,
    },

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

pub mod mock;
