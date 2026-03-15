//! AXUIElement wrappers for window management.
//! Only compiled on macOS (`#[cfg(target_os = "macos")]` in mod.rs).
//!
//! TODO: Implement actual AXUIElement FFI for window positioning.
//! This requires accessibility permissions and uses the macOS Accessibility API
//! (AXUIElementCreateApplication, AXUIElementCopyAttributeValue, etc.).

pub struct AXWindow {
    _element: *const std::ffi::c_void,
    window_id: u32,
}

// SAFETY: AXUIElement handles are safe to send across threads on macOS.
// The Accessibility API is thread-safe for attribute get/set operations.
unsafe impl Send for AXWindow {}
unsafe impl Sync for AXWindow {}

impl AXWindow {
    pub fn id(&self) -> u32 {
        self.window_id
    }

    pub fn set_position(&self, _x: f64, _y: f64) {
        // TODO: Use AXUIElementSetAttributeValue with kAXPositionAttribute
        // Create an AXValue from a CGPoint and set it on the element.
    }

    pub fn set_size(&self, _w: f64, _h: f64) {
        // TODO: Use AXUIElementSetAttributeValue with kAXSizeAttribute
        // Create an AXValue from a CGSize and set it on the element.
    }
}

/// Get the frontmost application's focused window as an AXWindow.
///
/// TODO: Implement using:
/// 1. NSWorkspace.shared.frontmostApplication?.processIdentifier
/// 2. AXUIElementCreateApplication(pid)
/// 3. AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute)
/// 4. CGSGetWindowID or similar to get the window ID for cycle tracking
pub fn get_frontmost_window() -> Option<AXWindow> {
    // Stub - returns None until AX FFI is implemented
    None
}

/// Get the screen frame (visible area) of the main display.
///
/// TODO: Implement using CGMainDisplayID() + CGDisplayBounds()
/// or NSScreen.main?.visibleFrame for menu-bar-aware positioning.
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    // Stub - return a reasonable default frame
    // In production this should use CGDisplayBounds or NSScreen.main.visibleFrame
    (0.0, 0.0, 1920.0, 1080.0)
}
