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
