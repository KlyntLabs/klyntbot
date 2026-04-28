//! Platform-neutral screen-capture and accessibility-tree trait.
//!
//! Defines `PlatformCapture`, `Frame`, `AccessibilityNode`, `WindowInfo`,
//! and `DisplayInfo`. macOS impl lives in
//! `platform-macos::computer_use::capture::MacCapture`.

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
