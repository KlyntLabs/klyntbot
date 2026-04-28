//! Platform-neutral input injection trait.
//!
//! Defines `PlatformInput`, `ComputerUseAction`, and neutral coordinate types.
//! macOS impl lives in `platform-macos::computer_use::input::MacInput`.

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
