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
