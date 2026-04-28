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

    async fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        Err(CaptureError::NotImplemented)
    }

    async fn get_ax_tree(&self, _scope: AxScope) -> Result<AccessibilityNode> {
        Err(CaptureError::NotImplemented)
    }
}
