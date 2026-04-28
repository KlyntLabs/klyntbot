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
